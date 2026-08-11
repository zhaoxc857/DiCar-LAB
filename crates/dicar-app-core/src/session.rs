use std::collections::VecDeque;

use dctp_protocol::{
    encode_frame, DeviceManifest, ErrorCode, ErrorPayload, Frame, FrameFlags, Heartbeat, Hello,
    HelloAck, ManifestAssembler, ManifestChunk, ManifestDone, MessageType, ParamCommit,
    ParamCommitAck, ParamRead, ParamState, ParamValue, ParamWrite, ParamWriteAck, ProtocolError,
    StreamDecoder, WireDecode, WireEncode, MAX_PAYLOAD_LEN,
};

use crate::{
    Clock, CommitPlan, ConnectedDevice, ConnectionPhase, CoreError, DeviceIdentity,
    DiagnosticsSnapshot, NonceSource, ParameterWorkspace, PendingWrite, Transport,
};

const ORDINARY_TIMEOUT_MS: u64 = 300;
const MANIFEST_TIMEOUT_MS: u64 = 500;
const COMMIT_TIMEOUT_MS: u64 = 3_000;
const SESSION_STALE_MS: u64 = 3_000;
const HEARTBEAT_INTERVAL_MS: u64 = 500;
const ORDINARY_ATTEMPTS: u8 = 4;
const COMMIT_ATTEMPTS: u8 = 3;
const UNSOLICITED_CAPACITY: usize = 128;
const READ_BUFFER_LEN: usize = 1_100;
const MAX_READS_PER_POLL: usize = 8;

pub struct ProtocolSession<T: Transport> {
    transport: T,
    clock: Box<dyn Clock>,
    nonce: Box<dyn NonceSource>,
    decoder: StreamDecoder,
    next_sequence: u16,
    session_id: Option<u32>,
    negotiated_max_payload: u16,
    last_valid_frame_at: u64,
    last_heartbeat_at: u64,
    phase: ConnectionPhase,
    diagnostics: DiagnosticsSnapshot,
    unsolicited: VecDeque<Frame>,
    manifest_cache: Option<(u32, DeviceManifest)>,
    connected: Option<ConnectedDevice>,
}

impl<T: Transport> ProtocolSession<T> {
    pub fn new<N, C>(transport: T, nonce: N, clock: C) -> Self
    where
        N: NonceSource,
        C: Clock,
    {
        let now = clock.now_ms();
        Self {
            transport,
            clock: Box::new(clock),
            nonce: Box::new(nonce),
            decoder: StreamDecoder::new(),
            next_sequence: 1,
            session_id: None,
            negotiated_max_payload: MAX_PAYLOAD_LEN as u16,
            last_valid_frame_at: now,
            last_heartbeat_at: now,
            phase: ConnectionPhase::Disconnected,
            diagnostics: DiagnosticsSnapshot::default(),
            unsolicited: VecDeque::with_capacity(UNSOLICITED_CAPACITY),
            manifest_cache: None,
            connected: None,
        }
    }

    pub fn connect_and_load(&mut self) -> Result<ConnectedDevice, CoreError> {
        let result = self.connect_and_load_inner();
        if result.is_err() {
            self.disconnect();
        }
        result
    }

    fn connect_and_load_inner(&mut self) -> Result<ConnectedDevice, CoreError> {
        self.phase = ConnectionPhase::Connecting;
        self.session_id = None;
        self.negotiated_max_payload = MAX_PAYLOAD_LEN as u16;
        self.decoder.reset();
        self.unsolicited.clear();
        let hello = Hello {
            client_nonce: self.nonce.next_nonce(),
            min_version: 1,
            max_version: 1,
            max_payload: MAX_PAYLOAD_LEN as u16,
        };
        let hello_ack_frame = self.request_with_policy(
            MessageType::Hello,
            hello.encode()?,
            MessageType::HelloAck,
            0,
            ORDINARY_TIMEOUT_MS,
            ORDINARY_ATTEMPTS,
            true,
        )?;
        let hello_ack = HelloAck::decode(&hello_ack_frame.payload)?;
        if hello_ack.session_id == 0
            || hello_ack.max_payload == 0
            || usize::from(hello_ack.max_payload) > MAX_PAYLOAD_LEN
        {
            self.disconnect();
            return Err(ProtocolError::InvalidLength.into());
        }
        self.session_id = Some(hello_ack.session_id);
        self.negotiated_max_payload = hello_ack.max_payload;
        let now = self.clock.now_ms();
        self.last_valid_frame_at = now;
        self.last_heartbeat_at = now;

        self.phase = ConnectionPhase::LoadingManifest;
        let manifest = match &self.manifest_cache {
            Some((crc, manifest)) if *crc == hello_ack.manifest_crc32 => manifest.clone(),
            _ => {
                let manifest = self.load_manifest(hello_ack.manifest_crc32)?;
                self.manifest_cache = Some((hello_ack.manifest_crc32, manifest.clone()));
                manifest
            }
        };

        self.phase = ConnectionPhase::LoadingParameters;
        let mut parameter_states = Vec::new();
        parameter_states
            .try_reserve(manifest.parameters.len())
            .map_err(|_| ProtocolError::InvalidLength)?;
        for descriptor in &manifest.parameters {
            let response = self.request(
                MessageType::ParamRead,
                ParamRead {
                    param_id: descriptor.param_id,
                }
                .encode()?,
            )?;
            let state = ParamState::decode(&response.payload)?;
            if state.param_id != descriptor.param_id {
                return Err(ProtocolError::InvalidValue.into());
            }
            parameter_states.push(state);
        }

        self.phase = ConnectionPhase::Ready;
        let connected = ConnectedDevice {
            phase: self.phase,
            session_id: hello_ack.session_id,
            negotiated_max_payload: hello_ack.max_payload,
            identity: DeviceIdentity {
                device_id: hello_ack.device_id,
                boot_count: hello_ack.boot_count,
                firmware_version: [
                    hello_ack.firmware_major,
                    hello_ack.firmware_minor,
                    hello_ack.firmware_patch,
                ],
                sdk_version: [
                    hello_ack.sdk_major,
                    hello_ack.sdk_minor,
                    hello_ack.sdk_patch,
                ],
                capabilities: hello_ack.capabilities,
            },
            manifest,
            parameter_states,
            diagnostics: self.diagnostics,
        };
        self.connected = Some(connected.clone());
        Ok(connected)
    }

    pub fn request(
        &mut self,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Result<Frame, CoreError> {
        if matches!(
            message_type,
            MessageType::ParamWrite | MessageType::ParamCommit
        ) {
            return Err(CoreError::UnauthorizedParameterOperation);
        }
        self.request_internal(message_type, payload)
    }

    fn request_internal(
        &mut self,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Result<Frame, CoreError> {
        self.ensure_writable_session()?;
        let expected = response_type(message_type).ok_or(ProtocolError::InvalidValue)?;
        let (timeout_ms, attempts) = request_policy(message_type);
        self.request_with_policy(
            message_type,
            payload,
            expected,
            self.session_id.ok_or(CoreError::Disconnected)?,
            timeout_ms,
            attempts,
            false,
        )
    }

    fn write_parameter(
        &mut self,
        param_id: u32,
        expected_revision: u32,
        value: ParamValue,
    ) -> Result<ParamWriteAck, CoreError> {
        let payload = ParamWrite {
            param_id,
            expected_revision,
            value,
        }
        .encode()?;
        match self.request_internal(MessageType::ParamWrite, payload) {
            Ok(response) => Ok(ParamWriteAck::decode(&response.payload)?),
            Err(CoreError::Device {
                code: ErrorCode::RevisionConflict,
                context,
                ..
            }) => Err(CoreError::RevisionConflict {
                current: decode_revision_conflict_context(&context)?,
            }),
            Err(error) => Err(error),
        }
    }

    fn commit_parameters(&mut self, commit: &ParamCommit) -> Result<ParamCommitAck, CoreError> {
        let response = self.request_internal(MessageType::ParamCommit, commit.encode()?)?;
        Ok(ParamCommitAck::decode(&response.payload)?)
    }

    pub fn execute_write(
        &mut self,
        workspace: &ParameterWorkspace,
        operation: &PendingWrite,
    ) -> Result<ParamWriteAck, CoreError> {
        workspace.validate_pending_execution(operation)?;
        self.write_parameter(
            operation.param_id,
            operation.expected_revision,
            operation.value.clone(),
        )
    }

    pub fn execute_commit(
        &mut self,
        workspace: &ParameterWorkspace,
        plan: &CommitPlan,
    ) -> Result<ParamCommitAck, CoreError> {
        workspace.validate_commit_execution(plan)?;
        self.commit_parameters(&plan.to_protocol_commit())
    }

    pub fn poll(&mut self) -> Result<(), CoreError> {
        self.ensure_writable_session()?;
        self.drain_available()?;
        self.ensure_writable_session()?;
        let now = self.clock.now_ms();
        let last_activity_at = self.last_valid_frame_at.max(self.last_heartbeat_at);
        if now.saturating_sub(last_activity_at) >= HEARTBEAT_INTERVAL_MS {
            let heartbeat = Heartbeat {
                monotonic_ms: now as u32,
            };
            self.request(MessageType::Heartbeat, heartbeat.encode()?)?;
            self.last_heartbeat_at = self.clock.now_ms();
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), CoreError> {
        let close_result = if self.session_id.is_some()
            && self.phase != ConnectionPhase::Disconnected
            && self.clock.now_ms().saturating_sub(self.last_valid_frame_at) < SESSION_STALE_MS
        {
            self.request(MessageType::SessionClose, Vec::new())
                .map(|_| ())
        } else {
            Ok(())
        };
        self.disconnect();
        let transport_result = self.transport.close().map_err(CoreError::from);
        close_result.and(transport_result)
    }

    pub const fn phase(&self) -> ConnectionPhase {
        self.phase
    }

    pub const fn session_id(&self) -> Option<u32> {
        self.session_id
    }

    pub const fn diagnostics(&self) -> DiagnosticsSnapshot {
        self.diagnostics
    }

    pub fn connected_device(&self) -> Option<&ConnectedDevice> {
        self.connected.as_ref()
    }

    pub fn pop_unsolicited(&mut self) -> Option<Frame> {
        self.unsolicited.pop_front()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    fn load_manifest(&mut self, expected_crc32: u32) -> Result<DeviceManifest, CoreError> {
        self.ensure_writable_session()?;
        let sequence = self.take_sequence();
        let session_id = self.session_id.ok_or(CoreError::Disconnected)?;
        let frame = Frame::new(
            MessageType::ManifestRequest,
            FrameFlags::ACK_REQUIRED,
            sequence,
            session_id,
            Vec::new(),
        )?;
        let encoded = encode_frame(&frame)?;
        let mut assembler = None;
        for attempt in 0..ORDINARY_ATTEMPTS {
            if attempt != 0 {
                self.diagnostics.retries += 1;
            }
            self.ensure_writable_session()?;
            self.write_transport(&encoded)?;
            let deadline = self.clock.now_ms().saturating_add(MANIFEST_TIMEOUT_MS);
            while self.clock.now_ms() < deadline {
                let mut completed = None;
                for received in self.read_once()? {
                    if completed.is_some() {
                        self.queue_unsolicited(received);
                        continue;
                    }
                    if received.header.session_id != session_id
                        || received.header.sequence != sequence
                        || !is_response(&received)
                    {
                        self.queue_unsolicited(received);
                        continue;
                    }
                    if received.header.message_type == MessageType::Error {
                        if let Some(error) = self.matching_device_error(
                            &received,
                            MessageType::ManifestRequest,
                            sequence,
                        )? {
                            completed = Some(Err(error));
                        } else {
                            self.queue_unsolicited(received);
                        }
                        continue;
                    }
                    match received.header.message_type {
                        MessageType::ManifestChunk => {
                            let chunk = ManifestChunk::decode(&received.payload)?;
                            if chunk.offset == 0 {
                                let mut restarted = ManifestAssembler::new();
                                restarted.push_chunk(chunk)?;
                                assembler = Some(restarted);
                            } else if let Some(current) = &mut assembler {
                                if current.push_chunk(chunk).is_err() {
                                    assembler = None;
                                }
                            }
                        }
                        MessageType::ManifestDone => {
                            let done = ManifestDone::decode(&received.payload)?;
                            let actual = done.manifest_crc32;
                            let Some(current) = assembler.take() else {
                                continue;
                            };
                            let bytes = match current.finish(done) {
                                Ok(bytes) => bytes,
                                Err(ProtocolError::InvalidLength) => continue,
                                Err(error) => return Err(error.into()),
                            };
                            if actual != expected_crc32 {
                                return Err(CoreError::ManifestCrcMismatch {
                                    expected: expected_crc32,
                                    actual,
                                });
                            }
                            let manifest = DeviceManifest::decode(&bytes)?;
                            let decoded_crc = manifest.manifest_crc32()?;
                            if decoded_crc != expected_crc32 {
                                return Err(CoreError::ManifestCrcMismatch {
                                    expected: expected_crc32,
                                    actual: decoded_crc,
                                });
                            }
                            completed = Some(Ok(manifest));
                        }
                        _ => self.queue_unsolicited(received),
                    }
                }
                if let Some(result) = completed {
                    return result;
                }
                self.clock.idle_until(deadline);
            }
        }
        Err(CoreError::Timeout {
            message_type: MessageType::ManifestRequest,
            attempts: ORDINARY_ATTEMPTS,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn request_with_policy(
        &mut self,
        message_type: MessageType,
        payload: Vec<u8>,
        expected_response: MessageType,
        session_id: u32,
        timeout_ms: u64,
        attempts: u8,
        hello: bool,
    ) -> Result<Frame, CoreError> {
        if payload.len() > usize::from(self.negotiated_max_payload) {
            return Err(ProtocolError::PayloadTooLarge(payload.len()).into());
        }
        let sequence = self.take_sequence();
        let frame = Frame::new(
            message_type,
            FrameFlags::ACK_REQUIRED,
            sequence,
            session_id,
            payload,
        )?;
        let encoded = encode_frame(&frame)?;
        for attempt in 0..attempts {
            if attempt != 0 {
                self.diagnostics.retries += 1;
            }
            if !hello {
                self.ensure_writable_session()?;
            }
            self.write_transport(&encoded)?;
            let deadline = self.clock.now_ms().saturating_add(timeout_ms);
            while self.clock.now_ms() < deadline {
                let mut matched = None;
                for received in self.read_once()? {
                    if matched.is_some() {
                        self.queue_unsolicited(received);
                        continue;
                    }
                    let matching_session = if hello {
                        received.header.message_type == MessageType::Error
                            || received.header.session_id != 0
                    } else {
                        received.header.session_id == session_id
                    };
                    if received.header.sequence != sequence
                        || !matching_session
                        || !is_response(&received)
                    {
                        self.queue_unsolicited(received);
                        continue;
                    }
                    if received.header.message_type == MessageType::Error {
                        if let Some(error) =
                            self.matching_device_error(&received, message_type, sequence)?
                        {
                            matched = Some(Err(error));
                        } else {
                            self.queue_unsolicited(received);
                        }
                        continue;
                    }
                    if received.header.message_type == expected_response {
                        matched = Some(Ok(received));
                        continue;
                    }
                    self.queue_unsolicited(received);
                }
                if let Some(result) = matched {
                    return result;
                }
                self.clock.idle_until(deadline);
            }
        }
        Err(CoreError::Timeout {
            message_type,
            attempts,
        })
    }

    fn read_once(&mut self) -> Result<Vec<Frame>, CoreError> {
        let mut bytes = [0u8; READ_BUFFER_LEN];
        let count = match self.transport.read(&mut bytes) {
            Ok(count) => count,
            Err(error) => {
                self.disconnect();
                return Err(error.into());
            }
        };
        let mut frames = Vec::new();
        for decoded in self.decoder.push(&bytes[..count]) {
            match decoded {
                Ok(frame) => {
                    if frame.payload.len() > usize::from(self.negotiated_max_payload)
                        && self.session_id.is_some()
                    {
                        self.diagnostics.malformed_frames += 1;
                        continue;
                    }
                    if let Some(session_id) = self.session_id {
                        if frame.header.session_id != session_id {
                            self.disconnect();
                            return Err(CoreError::Disconnected);
                        }
                    }
                    self.last_valid_frame_at = self.clock.now_ms();
                    self.diagnostics.valid_frames += 1;
                    frames.push(frame);
                }
                Err(error) => {
                    self.diagnostics.malformed_frames += 1;
                    if error == ProtocolError::CrcMismatch {
                        self.diagnostics.crc_errors += 1;
                    }
                    if error == ProtocolError::PacketTooLong {
                        self.diagnostics.decoder_overflows += 1;
                    }
                }
            }
        }
        Ok(frames)
    }

    fn drain_available(&mut self) -> Result<(), CoreError> {
        for _ in 0..MAX_READS_PER_POLL {
            let frames = self.read_once()?;
            if frames.is_empty() {
                return Ok(());
            }
            for frame in frames {
                self.queue_unsolicited(frame);
            }
        }
        Ok(())
    }

    fn matching_device_error(
        &mut self,
        frame: &Frame,
        request_type: MessageType,
        sequence: u16,
    ) -> Result<Option<CoreError>, CoreError> {
        let payload = ErrorPayload::decode(&frame.payload)?;
        if payload.original_message_type != request_type || payload.original_sequence != sequence {
            return Ok(None);
        }
        if payload.error_code == ErrorCode::InvalidSession {
            self.disconnect();
        }
        Ok(Some(CoreError::Device {
            original_message_type: payload.original_message_type,
            original_sequence: payload.original_sequence,
            code: payload.error_code,
            context: payload.context,
        }))
    }

    fn ensure_writable_session(&mut self) -> Result<(), CoreError> {
        if self.phase == ConnectionPhase::Disconnected || self.session_id.is_none() {
            return Err(CoreError::Disconnected);
        }
        if self.clock.now_ms().saturating_sub(self.last_valid_frame_at) >= SESSION_STALE_MS {
            self.disconnect();
            return Err(CoreError::Disconnected);
        }
        Ok(())
    }

    fn write_transport(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        match self.transport.write_all(bytes) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.disconnect();
                Err(error.into())
            }
        }
    }

    fn disconnect(&mut self) {
        self.phase = ConnectionPhase::Disconnected;
        self.session_id = None;
        self.connected = None;
    }

    fn take_sequence(&mut self) -> u16 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        sequence
    }

    fn queue_unsolicited(&mut self, frame: Frame) {
        if self.unsolicited.len() == UNSOLICITED_CAPACITY {
            self.unsolicited.pop_front();
            self.diagnostics.unsolicited_dropped += 1;
        }
        self.unsolicited.push_back(frame);
    }
}

pub fn decode_revision_conflict_context(context: &str) -> Result<ParamWriteAck, CoreError> {
    if context.len() % 2 != 0
        || context
            .bytes()
            .any(|value| !value.is_ascii_digit() && !(b'a'..=b'f').contains(&value))
    {
        return Err(ProtocolError::InvalidValue.into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve(context.len() / 2)
        .map_err(|_| ProtocolError::InvalidLength)?;
    for pair in context.as_bytes().chunks_exact(2) {
        let high = decode_lower_hex_nibble(pair[0]).ok_or(ProtocolError::InvalidValue)?;
        let low = decode_lower_hex_nibble(pair[1]).ok_or(ProtocolError::InvalidValue)?;
        bytes.push((high << 4) | low);
    }
    Ok(ParamWriteAck::decode(&bytes)?)
}

fn decode_lower_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn is_response(frame: &Frame) -> bool {
    frame.header.flags.bits() & FrameFlags::RESPONSE.bits() != 0
}

fn request_policy(message_type: MessageType) -> (u64, u8) {
    if message_type == MessageType::ParamCommit {
        (COMMIT_TIMEOUT_MS, COMMIT_ATTEMPTS)
    } else {
        (ORDINARY_TIMEOUT_MS, ORDINARY_ATTEMPTS)
    }
}

fn response_type(message_type: MessageType) -> Option<MessageType> {
    match message_type {
        MessageType::Heartbeat => Some(MessageType::HeartbeatAck),
        MessageType::SessionClose => Some(MessageType::SessionClose),
        MessageType::ParamRead => Some(MessageType::ParamValue),
        MessageType::ParamWrite => Some(MessageType::ParamWriteAck),
        MessageType::ParamCommit => Some(MessageType::ParamCommitAck),
        MessageType::TelemetrySubscribe => Some(MessageType::TelemetrySubscribeAck),
        MessageType::TelemetryStop => Some(MessageType::TelemetryStop),
        MessageType::PrepareFlash => Some(MessageType::PrepareFlashAck),
        _ => None,
    }
}
