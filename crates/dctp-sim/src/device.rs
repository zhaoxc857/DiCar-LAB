use dctp_protocol::{
    canonical_parameter_crc32, CapabilityFlags, DeviceManifest, EnumOption, ErrorCode,
    ErrorPayload, Frame, FrameFlags, Heartbeat, Hello, HelloAck, ManifestChunk, ManifestDone,
    MessageType, ParamCommit, ParamCommitAck, ParamConstraints, ParamDescriptor, ParamFlags,
    ParamRead, ParamState, ParamType, ParamValue, ParamWrite, ParamWriteAck, ProtocolError,
    TelemetryBatch, TelemetryDescriptor, TelemetrySample, TelemetrySubscription, TelemetryType,
    WireDecode, WireEncode, MANIFEST_SCHEMA_VERSION, MAX_PAYLOAD_LEN, MAX_TELEMETRY_SAMPLES,
};

use crate::{
    speed_loop::{SpeedLoopInput, SpeedLoopModel, SpeedLoopSnapshot, MAX_SPEED_MPS},
    Priority, QueuedFrame, RequestCache, RequestKey,
};

pub const SESSION_EXPIRATION_MS: u64 = 3_000;
const HELLO_ACK_PAYLOAD_LEN: u16 = 46;
const MANIFEST_CHUNK_PREFIX_LEN: usize = 12;
const TELEMETRY_BATCH_PREFIX_LEN: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitFailure {
    Storage,
    Verify,
}

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub manifest: DeviceManifest,
    pub boot_count: u32,
    pub device_id: [u8; 16],
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            manifest: fixed_manifest(),
            boot_count: 1,
            device_id: *b"DCTP-SIM-DEVICE!",
        }
    }
}

#[derive(Clone, Debug)]
struct Session {
    id: u32,
    last_valid_frame_ms: u64,
    max_payload: u16,
}

#[derive(Clone, Debug)]
struct Parameter {
    descriptor: ParamDescriptor,
    value: ParamValue,
    persisted_value: Option<ParamValue>,
    revision: u32,
}

#[derive(Clone, Debug)]
struct CompletedHello {
    request: Frame,
    response: Frame,
}

#[derive(Clone, Debug)]
struct CompletedClose {
    request: Frame,
    response: Frame,
}

#[derive(Debug)]
pub struct SimDevice {
    config: SimConfig,
    session: Option<Session>,
    session_counter: u32,
    parameters: Vec<Parameter>,
    storage_generation: u32,
    commit_failure: Option<CommitFailure>,
    request_cache: RequestCache,
    completed_hello: Option<CompletedHello>,
    completed_close: Option<CompletedClose>,
    telemetry_subscription: Option<TelemetrySubscription>,
    next_telemetry_at_us: Option<u64>,
    next_telemetry_sequence: u16,
    pending_dropped_telemetry_samples: u16,
    speed_loop: SpeedLoopModel,
}

impl SimDevice {
    pub fn new(config: SimConfig) -> Self {
        let parameters = config
            .manifest
            .parameters
            .iter()
            .cloned()
            .map(|descriptor| Parameter {
                value: descriptor.default_value.clone(),
                persisted_value: (descriptor.flags.bits() & ParamFlags::PERSISTENT.bits() != 0)
                    .then(|| descriptor.default_value.clone()),
                descriptor,
                revision: 0,
            })
            .collect();
        Self {
            config,
            session: None,
            session_counter: 0,
            parameters,
            storage_generation: 0,
            commit_failure: None,
            request_cache: RequestCache::default(),
            completed_hello: None,
            completed_close: None,
            telemetry_subscription: None,
            next_telemetry_at_us: None,
            next_telemetry_sequence: 0,
            pending_dropped_telemetry_samples: 0,
            speed_loop: SpeedLoopModel::default(),
        }
    }

    pub fn handle(&mut self, request: Frame, now_ms: u64) -> Vec<QueuedFrame> {
        self.expire_session(now_ms);
        if request.header.message_type == MessageType::Hello {
            if let Some(response) = self.replay_completed_hello(&request, now_ms) {
                return vec![QueuedFrame {
                    priority: Priority::Safety,
                    frame: response,
                }];
            }
            return self.handle_hello(request, now_ms);
        }
        if request.header.message_type == MessageType::SessionClose {
            if let Some(response) = self.replay_completed_close(&request) {
                return vec![QueuedFrame {
                    priority: Priority::Safety,
                    frame: response,
                }];
            }
        }
        if self.validate_session(request.header.session_id).is_err() {
            return vec![self.error_response(&request, ErrorCode::InvalidSession, String::new())];
        }
        let negotiated_max_payload = self
            .session
            .as_ref()
            .map(|session| session.max_payload)
            .unwrap_or(MAX_PAYLOAD_LEN as u16);
        if request.payload.len() > usize::from(negotiated_max_payload) {
            return vec![self.error_response(&request, ErrorCode::InvalidLength, String::new())];
        }

        if let Some(session) = &mut self.session {
            session.last_valid_frame_ms = now_ms;
        }

        let key = RequestKey {
            session_id: request.header.session_id,
            message_type: request.header.message_type,
            sequence: request.header.sequence,
        };
        let reliable = request.header.flags.bits() & FrameFlags::ACK_REQUIRED.bits() != 0;
        if reliable {
            if let Some(frame) = self.request_cache.get(&key) {
                return vec![QueuedFrame {
                    priority: priority_for(&frame),
                    frame,
                }];
            }
        }

        let mut responses = self.dispatch(&request, now_ms);
        if responses
            .iter()
            .any(|response| response.frame.payload.len() > usize::from(negotiated_max_payload))
        {
            responses =
                vec![self.error_response(&request, ErrorCode::InternalError, String::new())];
        }
        if reliable
            && request.header.message_type != MessageType::SessionClose
            && responses.len() == 1
        {
            self.request_cache.insert(key, responses[0].frame.clone());
        }
        responses
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<QueuedFrame> {
        self.expire_session(now_ms);
        let Some((session_id, max_payload)) = self
            .session
            .as_ref()
            .map(|session| (session.id, session.max_payload))
        else {
            return Vec::new();
        };
        let Some(subscription) = self.telemetry_subscription.clone() else {
            return Vec::new();
        };
        let Some(next_telemetry_at_us) = self.next_telemetry_at_us else {
            return Vec::new();
        };
        let now_us = now_ms.saturating_mul(1_000);
        if now_us < next_telemetry_at_us {
            return Vec::new();
        }
        let period_us = telemetry_period_us(subscription.sample_rate_hz);
        let due_samples = now_us
            .saturating_sub(next_telemetry_at_us)
            .checked_div(period_us)
            .unwrap_or(0)
            .saturating_add(1);
        let descriptors = subscription
            .channel_ids
            .iter()
            .map(|channel_id| {
                self.config
                    .manifest
                    .telemetry
                    .iter()
                    .find(|descriptor| descriptor.channel_id == *channel_id)
                    .cloned()
            })
            .collect::<Option<Vec<_>>>();
        let Some(descriptors) = descriptors else {
            self.clear_telemetry_state();
            return Vec::new();
        };
        let sample_payload_len = 2usize.saturating_add(descriptors.len().saturating_mul(4));
        let payload_sample_capacity = usize::from(max_payload)
            .saturating_sub(TELEMETRY_BATCH_PREFIX_LEN)
            .checked_div(sample_payload_len)
            .unwrap_or(0);
        let delta_sample_capacity = if period_us <= u64::from(u16::MAX) {
            MAX_TELEMETRY_SAMPLES
        } else {
            1
        };
        let sample_capacity = payload_sample_capacity
            .min(delta_sample_capacity)
            .min(MAX_TELEMETRY_SAMPLES);
        if sample_capacity == 0 {
            self.clear_telemetry_state();
            return Vec::new();
        }
        let emitted_samples = due_samples.min(sample_capacity as u64);
        let skipped_samples = due_samples.saturating_sub(emitted_samples);
        self.note_telemetry_drop(u16::try_from(skipped_samples).unwrap_or(u16::MAX));
        let first_sample_sequence = self
            .next_telemetry_sequence
            .wrapping_add(skipped_samples as u16);
        self.next_telemetry_sequence = first_sample_sequence.wrapping_add(emitted_samples as u16);
        let base_timestamp_us =
            next_telemetry_at_us.saturating_add(skipped_samples.saturating_mul(period_us)) as u32;
        self.next_telemetry_at_us =
            Some(next_telemetry_at_us.saturating_add(due_samples.saturating_mul(period_us)));
        let dt_us = u16::try_from(period_us).unwrap_or(0);
        let speed_loop_input = self.speed_loop_input();
        let mut samples = Vec::with_capacity(emitted_samples as usize);
        for index in 0..emitted_samples {
            let sample_timestamp_us = next_telemetry_at_us.saturating_add(
                skipped_samples
                    .saturating_add(index)
                    .saturating_mul(period_us),
            );
            self.speed_loop
                .advance_to(sample_timestamp_us, speed_loop_input);
            let speed_loop_snapshot = self.speed_loop.snapshot(speed_loop_input);
            samples.push(TelemetrySample {
                dt_us: if index == 0 { 0 } else { dt_us },
                values: descriptors
                    .iter()
                    .map(|descriptor| {
                        telemetry_value(
                            descriptor,
                            sample_timestamp_us,
                            speed_loop_input,
                            speed_loop_snapshot,
                        )
                    })
                    .collect(),
            });
        }
        let batch = TelemetryBatch {
            subscription_version: subscription.subscription_version,
            first_sample_sequence,
            dropped_samples: std::mem::take(&mut self.pending_dropped_telemetry_samples),
            base_timestamp_us,
            samples,
        };
        let Ok(payload) = batch.encode() else {
            return Vec::new();
        };
        if payload.len() > usize::from(max_payload) {
            return Vec::new();
        }
        vec![QueuedFrame {
            priority: Priority::Telemetry,
            frame: response_frame(
                MessageType::TelemetryData,
                first_sample_sequence,
                session_id,
                payload,
                FrameFlags::NONE,
            ),
        }]
    }

    pub fn open_session(&mut self, client_nonce: u32, now_ms: u64) -> Result<u32, ProtocolError> {
        self.open_session_with_max_payload(client_nonce, now_ms, MAX_PAYLOAD_LEN as u16)
    }

    fn open_session_with_max_payload(
        &mut self,
        client_nonce: u32,
        now_ms: u64,
        max_payload: u16,
    ) -> Result<u32, ProtocolError> {
        let previous = self.session.as_ref().map(|session| session.id);
        let session_id = loop {
            self.session_counter = self.session_counter.wrapping_add(1);
            let candidate = client_nonce.rotate_left(13)
                ^ self.config.boot_count.rotate_left(7)
                ^ self.session_counter.wrapping_mul(0x9e37_79b9);
            if candidate != 0 && Some(candidate) != previous {
                break candidate;
            }
        };
        self.session = Some(Session {
            id: session_id,
            last_valid_frame_ms: now_ms,
            max_payload,
        });
        self.request_cache.clear();
        self.completed_hello = None;
        self.completed_close = None;
        self.clear_telemetry_state();
        self.speed_loop.reset_at(now_ms.saturating_mul(1_000));
        Ok(session_id)
    }

    /// Clears transport and Session ownership after a client disconnects while
    /// preserving the currently accepted RAM parameter values and Revisions.
    pub fn disconnect(&mut self) {
        self.clear_current_session();
        self.completed_close = None;
    }

    pub fn validate_session(&self, session_id: u32) -> Result<(), ProtocolError> {
        if self.session.as_ref().map(|session| session.id) == Some(session_id) {
            Ok(())
        } else {
            Err(ProtocolError::InvalidSession)
        }
    }

    pub fn parameter_revision(&self, param_id: u32) -> Option<u32> {
        self.parameters
            .iter()
            .find(|parameter| parameter.descriptor.param_id == param_id)
            .map(|parameter| parameter.revision)
    }

    pub const fn storage_generation(&self) -> u32 {
        self.storage_generation
    }

    pub fn set_commit_failure(&mut self, failure: Option<CommitFailure>) {
        self.commit_failure = failure;
    }

    pub const fn manifest(&self) -> &DeviceManifest {
        &self.config.manifest
    }

    /// Records a complete P2 telemetry batch evicted by the transport queue.
    /// The count is carried in the next emitted telemetry batch.
    pub fn note_telemetry_drop(&mut self, sample_count: u16) {
        self.pending_dropped_telemetry_samples = self
            .pending_dropped_telemetry_samples
            .saturating_add(sample_count);
    }

    fn expire_session(&mut self, now_ms: u64) {
        let expired = self.session.as_ref().is_some_and(|session| {
            now_ms.saturating_sub(session.last_valid_frame_ms) >= SESSION_EXPIRATION_MS
        });
        if expired {
            self.clear_current_session();
            self.completed_close = None;
        }
    }

    fn replay_completed_hello(&mut self, request: &Frame, now_ms: u64) -> Option<Frame> {
        let completed = self.completed_hello.as_ref()?;
        if completed.request != *request
            || request.header.flags.bits() & FrameFlags::ACK_REQUIRED.bits() == 0
            || self
                .validate_session(completed.response.header.session_id)
                .is_err()
        {
            return None;
        }
        if let Some(session) = &mut self.session {
            session.last_valid_frame_ms = now_ms;
        }
        Some(completed.response.clone())
    }

    fn replay_completed_close(&self, request: &Frame) -> Option<Frame> {
        let completed = self.completed_close.as_ref()?;
        if completed.request != *request
            || request.header.flags.bits() & FrameFlags::ACK_REQUIRED.bits() == 0
        {
            return None;
        }
        Some(completed.response.clone())
    }

    fn handle_hello(&mut self, request: Frame, now_ms: u64) -> Vec<QueuedFrame> {
        if request.header.session_id != 0 {
            return vec![self.error_response(&request, ErrorCode::InvalidSession, String::new())];
        }
        let hello = match Hello::decode(&request.payload) {
            Ok(hello) => hello,
            Err(_) => {
                return vec![self.error_response(&request, ErrorCode::InvalidLength, String::new())]
            }
        };
        if hello.min_version > 1 || hello.max_version < 1 {
            return vec![self.error_response(
                &request,
                ErrorCode::UnsupportedVersion,
                String::new(),
            )];
        }
        if hello.max_payload < HELLO_ACK_PAYLOAD_LEN {
            return vec![self.error_response(&request, ErrorCode::InvalidLength, String::new())];
        }
        let negotiated_max_payload = hello.max_payload.min(MAX_PAYLOAD_LEN as u16);
        let session_id = match self.open_session_with_max_payload(
            hello.client_nonce,
            now_ms,
            negotiated_max_payload,
        ) {
            Ok(session_id) => session_id,
            Err(_) => {
                return vec![self.error_response(&request, ErrorCode::InternalError, String::new())]
            }
        };
        let manifest_crc32 = match self.config.manifest.manifest_crc32() {
            Ok(crc32) => crc32,
            Err(_) => {
                return vec![self.error_response(&request, ErrorCode::InternalError, String::new())]
            }
        };
        let ack = HelloAck {
            session_id,
            device_id: self.config.device_id,
            boot_count: self.config.boot_count,
            firmware_major: 1,
            firmware_minor: 0,
            firmware_patch: 0,
            sdk_major: 1,
            sdk_minor: 0,
            sdk_patch: 0,
            capabilities: CapabilityFlags::PARAMETERS
                | CapabilityFlags::TELEMETRY
                | CapabilityFlags::PERSISTENCE,
            manifest_crc32,
            max_payload: negotiated_max_payload,
        };
        let response = response_frame(
            MessageType::HelloAck,
            request.header.sequence,
            session_id,
            ack.encode().expect("fixed HELLO_ACK is encodable"),
            FrameFlags::RESPONSE,
        );
        if request.header.flags.bits() & FrameFlags::ACK_REQUIRED.bits() != 0 {
            self.completed_hello = Some(CompletedHello {
                request,
                response: response.clone(),
            });
        }
        vec![QueuedFrame {
            priority: Priority::Safety,
            frame: response,
        }]
    }

    fn dispatch(&mut self, request: &Frame, now_ms: u64) -> Vec<QueuedFrame> {
        match request.header.message_type {
            MessageType::Heartbeat => self.handle_heartbeat(request),
            MessageType::ManifestRequest => self.handle_manifest_request(request),
            MessageType::ParamRead => self.handle_param_read(request),
            MessageType::ParamWrite => self.handle_param_write(request),
            MessageType::ParamCommit => self.handle_param_commit(request),
            MessageType::TelemetrySubscribe => self.handle_telemetry_subscribe(request, now_ms),
            MessageType::TelemetryStop => self.handle_telemetry_stop(request),
            MessageType::SessionClose => {
                if !request.payload.is_empty() {
                    return vec![self.error_response(
                        request,
                        ErrorCode::InvalidLength,
                        String::new(),
                    )];
                }
                let response = response_frame(
                    MessageType::SessionClose,
                    request.header.sequence,
                    request.header.session_id,
                    Vec::new(),
                    FrameFlags::RESPONSE,
                );
                let completed_close = (request.header.flags.bits()
                    & FrameFlags::ACK_REQUIRED.bits()
                    != 0)
                    .then(|| CompletedClose {
                        request: request.clone(),
                        response: response.clone(),
                    });
                self.clear_current_session();
                self.completed_close = completed_close;
                vec![QueuedFrame {
                    priority: Priority::Safety,
                    frame: response,
                }]
            }
            _ => vec![self.error_response(request, ErrorCode::UnknownMessage, String::new())],
        }
    }

    fn handle_heartbeat(&self, request: &Frame) -> Vec<QueuedFrame> {
        let heartbeat = match Heartbeat::decode(&request.payload) {
            Ok(heartbeat) => heartbeat,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())]
            }
        };
        vec![QueuedFrame {
            priority: Priority::Safety,
            frame: response_frame(
                MessageType::HeartbeatAck,
                request.header.sequence,
                request.header.session_id,
                heartbeat.encode().expect("decoded heartbeat is encodable"),
                FrameFlags::RESPONSE,
            ),
        }]
    }

    fn handle_manifest_request(&self, request: &Frame) -> Vec<QueuedFrame> {
        if !request.payload.is_empty() {
            return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())];
        }
        let bytes = match self.config.manifest.encode_canonical() {
            Ok(bytes) => bytes,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InternalError, String::new())]
            }
        };
        let crc32 = match self.config.manifest.manifest_crc32() {
            Ok(crc32) => crc32,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InternalError, String::new())]
            }
        };
        let chunk_data_len = self
            .session
            .as_ref()
            .map(|session| usize::from(session.max_payload))
            .unwrap_or(MAX_PAYLOAD_LEN)
            .saturating_sub(MANIFEST_CHUNK_PREFIX_LEN);
        let mut responses = Vec::new();
        for (chunk_index, data) in bytes.chunks(chunk_data_len).enumerate() {
            let chunk = ManifestChunk {
                manifest_crc32: crc32,
                total_len: bytes.len() as u32,
                offset: (chunk_index * chunk_data_len) as u32,
                data: data.to_vec(),
            };
            responses.push(QueuedFrame {
                priority: Priority::Reliable,
                frame: response_frame(
                    MessageType::ManifestChunk,
                    request.header.sequence,
                    request.header.session_id,
                    chunk.encode().expect("bounded manifest chunk is encodable"),
                    combine_flags(FrameFlags::RESPONSE, FrameFlags::MORE_FRAGMENTS),
                ),
            });
        }
        let done = ManifestDone {
            manifest_crc32: crc32,
            total_len: bytes.len() as u32,
        };
        responses.push(QueuedFrame {
            priority: Priority::Reliable,
            frame: response_frame(
                MessageType::ManifestDone,
                request.header.sequence,
                request.header.session_id,
                done.encode().expect("manifest completion is encodable"),
                FrameFlags::RESPONSE,
            ),
        });
        responses
    }

    fn handle_param_read(&self, request: &Frame) -> Vec<QueuedFrame> {
        let read = match ParamRead::decode(&request.payload) {
            Ok(read) => read,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())]
            }
        };
        let parameter = match self
            .parameters
            .iter()
            .find(|parameter| parameter.descriptor.param_id == read.param_id)
        {
            Some(parameter) => parameter,
            None => {
                return vec![self.error_response(request, ErrorCode::InvalidParamId, String::new())]
            }
        };
        let state = ParamState {
            param_id: read.param_id,
            revision: parameter.revision,
            value: parameter.value.clone(),
            persisted_value: parameter.persisted_value.clone(),
        };
        vec![QueuedFrame {
            priority: Priority::Reliable,
            frame: response_frame(
                MessageType::ParamValue,
                request.header.sequence,
                request.header.session_id,
                state.encode().expect("stored parameter state is encodable"),
                FrameFlags::RESPONSE,
            ),
        }]
    }

    fn handle_param_write(&mut self, request: &Frame) -> Vec<QueuedFrame> {
        let write = match ParamWrite::decode(&request.payload) {
            Ok(write) => write,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())]
            }
        };
        let parameter_index = match self
            .parameters
            .iter()
            .position(|parameter| parameter.descriptor.param_id == write.param_id)
        {
            Some(index) => index,
            None => {
                return vec![self.error_response(request, ErrorCode::InvalidParamId, String::new())]
            }
        };
        let parameter = &self.parameters[parameter_index];
        if parameter.descriptor.param_type != write.value.param_type() {
            return vec![self.error_response(request, ErrorCode::TypeMismatch, String::new())];
        }
        if parameter.descriptor.flags.bits() & ParamFlags::WRITABLE.bits() == 0 {
            return vec![self.error_response(request, ErrorCode::ReadOnly, String::new())];
        }
        if !value_within_constraints(&write.value, &parameter.descriptor.constraints) {
            return vec![self.error_response(request, ErrorCode::OutOfRange, String::new())];
        }
        if parameter.revision != write.expected_revision {
            let current = ParamWriteAck {
                value: parameter.value.clone(),
                new_revision: parameter.revision,
            };
            // ErrorPayload context is UTF-8, so embed the ParamWriteAck wire bytes
            // losslessly as lowercase hexadecimal with no prefix or separators.
            let context = lower_hex(
                &current
                    .encode()
                    .expect("stored parameter conflict context is encodable"),
            );
            return vec![self.error_response(request, ErrorCode::RevisionConflict, context)];
        }

        let parameter = &mut self.parameters[parameter_index];
        parameter.value = write.value;
        parameter.revision = parameter.revision.wrapping_add(1);
        let ack = ParamWriteAck {
            value: parameter.value.clone(),
            new_revision: parameter.revision,
        };
        vec![QueuedFrame {
            priority: Priority::Reliable,
            frame: response_frame(
                MessageType::ParamWriteAck,
                request.header.sequence,
                request.header.session_id,
                ack.encode().expect("accepted parameter ACK is encodable"),
                FrameFlags::RESPONSE,
            ),
        }]
    }

    fn handle_param_commit(&mut self, request: &Frame) -> Vec<QueuedFrame> {
        let commit = match ParamCommit::decode(&request.payload) {
            Ok(commit) => commit,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())]
            }
        };
        let mut values = Vec::with_capacity(commit.entries.len());
        for entry in &commit.entries {
            let Some(parameter) = self
                .parameters
                .iter()
                .find(|parameter| parameter.descriptor.param_id == entry.param_id)
            else {
                return vec![self.error_response(
                    request,
                    ErrorCode::InvalidParamId,
                    String::new(),
                )];
            };
            if parameter.descriptor.flags.bits() & ParamFlags::PERSISTENT.bits() == 0 {
                return vec![self.error_response(request, ErrorCode::ReadOnly, String::new())];
            }
            if parameter.revision != entry.revision {
                return vec![self.error_response(
                    request,
                    ErrorCode::RevisionConflict,
                    String::new(),
                )];
            }
            values.push((entry.param_id, parameter.value.clone()));
        }
        let Ok(canonical_crc32) = canonical_parameter_crc32(&values) else {
            return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())];
        };
        if commit.canonical_crc32 != canonical_crc32 {
            return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())];
        }
        if let Some(failure) = self.commit_failure {
            let error = match failure {
                CommitFailure::Storage => ErrorCode::StorageFailed,
                CommitFailure::Verify => ErrorCode::VerifyFailed,
            };
            return vec![self.error_response(request, error, String::new())];
        }
        for entry in &commit.entries {
            let parameter = self
                .parameters
                .iter_mut()
                .find(|parameter| parameter.descriptor.param_id == entry.param_id)
                .expect("validated parameter exists");
            parameter.persisted_value = Some(parameter.value.clone());
        }
        self.storage_generation = self.storage_generation.wrapping_add(1);
        let ack = ParamCommitAck {
            canonical_crc32,
            storage_generation: self.storage_generation,
        };
        vec![QueuedFrame {
            priority: Priority::Reliable,
            frame: response_frame(
                MessageType::ParamCommitAck,
                request.header.sequence,
                request.header.session_id,
                ack.encode().expect("commit acknowledgement is encodable"),
                FrameFlags::RESPONSE,
            ),
        }]
    }

    fn handle_telemetry_subscribe(&mut self, request: &Frame, now_ms: u64) -> Vec<QueuedFrame> {
        let subscription = match TelemetrySubscription::decode(&request.payload) {
            Ok(subscription) => subscription,
            Err(_) => {
                return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())]
            }
        };
        if subscription.channel_ids.iter().any(|channel_id| {
            !self
                .config
                .manifest
                .telemetry
                .iter()
                .any(|descriptor| descriptor.channel_id == *channel_id)
        }) {
            return vec![self.error_response(request, ErrorCode::InvalidParamId, String::new())];
        }
        let period_us = telemetry_period_us(subscription.sample_rate_hz);
        self.next_telemetry_at_us = Some(now_ms.saturating_mul(1_000).saturating_add(period_us));
        self.telemetry_subscription = Some(subscription);
        self.next_telemetry_sequence = 0;
        self.pending_dropped_telemetry_samples = 0;
        vec![QueuedFrame {
            priority: Priority::Reliable,
            frame: response_frame(
                MessageType::TelemetrySubscribeAck,
                request.header.sequence,
                request.header.session_id,
                Vec::new(),
                FrameFlags::RESPONSE,
            ),
        }]
    }

    fn handle_telemetry_stop(&mut self, request: &Frame) -> Vec<QueuedFrame> {
        if !request.payload.is_empty() {
            return vec![self.error_response(request, ErrorCode::InvalidLength, String::new())];
        }
        self.clear_telemetry_state();
        vec![QueuedFrame {
            priority: Priority::Reliable,
            frame: response_frame(
                MessageType::TelemetryStop,
                request.header.sequence,
                request.header.session_id,
                Vec::new(),
                FrameFlags::RESPONSE,
            ),
        }]
    }

    fn clear_current_session(&mut self) {
        self.session = None;
        self.request_cache.clear();
        self.completed_hello = None;
        self.clear_telemetry_state();
    }

    fn clear_telemetry_state(&mut self) {
        self.telemetry_subscription = None;
        self.next_telemetry_at_us = None;
        self.next_telemetry_sequence = 0;
        self.pending_dropped_telemetry_samples = 0;
    }

    fn speed_loop_input(&self) -> SpeedLoopInput {
        SpeedLoopInput {
            target_mps: self.f32_parameter("control.target_speed_mps", 0.0),
            kp: self.f32_parameter("pid.kp", 1.2),
            ki: self.f32_parameter("pid.speed.ki", 0.08),
            kd: self.f32_parameter("pid.speed.kd", 0.002),
        }
    }

    fn f32_parameter(&self, machine_name: &str, fallback: f32) -> f32 {
        self.parameters
            .iter()
            .find(|parameter| parameter.descriptor.machine_name == machine_name)
            .and_then(|parameter| match parameter.value {
                ParamValue::F32(value) if value.is_finite() => Some(value),
                _ => None,
            })
            .unwrap_or(fallback)
    }

    fn error_response(
        &self,
        request: &Frame,
        error_code: ErrorCode,
        context: String,
    ) -> QueuedFrame {
        let payload = ErrorPayload {
            original_message_type: request.header.message_type,
            original_sequence: request.header.sequence,
            error_code,
            context,
        };
        QueuedFrame {
            priority: Priority::Safety,
            frame: response_frame(
                MessageType::Error,
                request.header.sequence,
                request.header.session_id,
                payload
                    .encode()
                    .expect("bounded simulator error is encodable"),
                combine_flags(FrameFlags::ERROR, FrameFlags::RESPONSE),
            ),
        }
    }
}

fn response_frame(
    message_type: MessageType,
    sequence: u16,
    session_id: u32,
    payload: Vec<u8>,
    flags: FrameFlags,
) -> Frame {
    Frame::new(message_type, flags, sequence, session_id, payload)
        .expect("simulator responses stay within protocol limits")
}

const fn combine_flags(left: FrameFlags, right: FrameFlags) -> FrameFlags {
    FrameFlags::from_bits(left.bits() | right.bits())
}

fn priority_for(frame: &Frame) -> Priority {
    match frame.header.message_type {
        MessageType::HeartbeatAck | MessageType::Error | MessageType::PrepareFlashAck => {
            Priority::Safety
        }
        MessageType::TelemetryData => Priority::Telemetry,
        MessageType::LogMessage => Priority::Log,
        _ => Priority::Reliable,
    }
}

fn value_within_constraints(value: &ParamValue, constraints: &ParamConstraints) -> bool {
    match (value, constraints) {
        (_, ParamConstraints::None) => true,
        (
            ParamValue::I32(value),
            ParamConstraints::Numeric {
                min: ParamValue::I32(min),
                max: ParamValue::I32(max),
                ..
            },
        ) => value >= min && value <= max,
        (
            ParamValue::U32(value),
            ParamConstraints::Numeric {
                min: ParamValue::U32(min),
                max: ParamValue::U32(max),
                ..
            },
        ) => value >= min && value <= max,
        (
            ParamValue::F32(value),
            ParamConstraints::Numeric {
                min: ParamValue::F32(min),
                max: ParamValue::F32(max),
                ..
            },
        ) => value.is_finite() && value >= min && value <= max,
        (ParamValue::Enum(value), ParamConstraints::Enum { options }) => {
            options.iter().any(|option| option.value == *value)
        }
        _ => false,
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn fixed_manifest() -> DeviceManifest {
    let writable = ParamFlags::WRITABLE | ParamFlags::PERSISTENT;
    let ram_dangerous = ParamFlags::WRITABLE | ParamFlags::DANGEROUS;
    DeviceManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        parameters: vec![
            numeric_f32(
                1,
                "pid.kp",
                "速度 Kp",
                "控制",
                "",
                1.2,
                0.0,
                20.0,
                0.01,
                writable,
            ),
            numeric_f32(
                2,
                "pid.speed.ki",
                "速度 Ki",
                "控制",
                "",
                0.08,
                0.0,
                5.0,
                0.001,
                writable,
            ),
            numeric_f32(
                3,
                "pid.speed.kd",
                "速度 Kd",
                "控制",
                "",
                0.002,
                0.0,
                1.0,
                0.0001,
                writable,
            ),
            numeric_f32(
                4,
                "control.target_speed_mps",
                "目标速度",
                "驱动",
                "m/s",
                0.0,
                0.0,
                8.0,
                0.05,
                ram_dangerous,
            ),
            numeric_u32(
                100,
                "encoder.left.ppr",
                "左编码器 PPR",
                "编码器",
                "pulse/rev",
                512,
                1,
                1_000_000,
                1,
                writable,
            ),
            numeric_u32(
                101,
                "encoder.right.ppr",
                "右编码器 PPR",
                "编码器",
                "pulse/rev",
                512,
                1,
                1_000_000,
                1,
                writable,
            ),
            ParamDescriptor {
                param_id: 102,
                param_type: ParamType::Enum,
                flags: writable,
                machine_name: "encoder.quadrature_multiplier".into(),
                display_name: "正交倍频".into(),
                group: "编码器".into(),
                unit: "x".into(),
                default_value: ParamValue::Enum(4),
                constraints: ParamConstraints::Enum {
                    options: vec![
                        EnumOption {
                            value: 1,
                            label: "1x".into(),
                        },
                        EnumOption {
                            value: 2,
                            label: "2x".into(),
                        },
                        EnumOption {
                            value: 4,
                            label: "4x".into(),
                        },
                    ],
                },
            },
            numeric_u32(
                103,
                "encoder.left.cpr",
                "左编码器 CPR",
                "编码器",
                "count/rev",
                2_048,
                1,
                4_000_000,
                1,
                ParamFlags::NONE,
            ),
            numeric_u32(
                104,
                "encoder.right.cpr",
                "右编码器 CPR",
                "编码器",
                "count/rev",
                2_048,
                1,
                4_000_000,
                1,
                ParamFlags::NONE,
            ),
            boolean(105, "encoder.left.inverted", "左编码器反向", writable),
            boolean(106, "encoder.right.inverted", "右编码器反向", writable),
            numeric_f32(
                107,
                "drive.wheel_diameter_mm",
                "Wheel diameter",
                "Drive",
                "mm",
                65.0,
                1.0,
                1_000.0,
                0.1,
                writable,
            ),
            numeric_f32(
                108,
                "drive.gear_ratio",
                "Gear ratio",
                "Drive",
                "ratio",
                1.0,
                0.01,
                100.0,
                0.01,
                writable,
            ),
            numeric_u32(
                109,
                "encoder.sample_period_us",
                "编码器采样周期",
                "编码器",
                "us",
                10_000,
                100,
                1_000_000,
                100,
                writable,
            ),
            numeric_f32(
                110,
                "encoder.speed_lpf_hz",
                "编码器速度低通截止频率",
                "编码器",
                "Hz",
                50.0,
                0.0,
                1_000.0,
                0.1,
                writable,
            ),
            numeric_u32(
                111,
                "encoder.jump_threshold_counts",
                "编码器跳变阈值",
                "编码器",
                "count",
                10_000,
                1,
                1_000_000,
                1,
                writable,
            ),
            numeric_f32(
                112,
                "encoder.max_credible_rpm",
                "编码器最大可信转速",
                "编码器",
                "rpm",
                10_000.0,
                1.0,
                100_000.0,
                1.0,
                writable,
            ),
            boolean(
                113,
                "encoder.missing_pulse_detection",
                "编码器丢脉冲检测",
                writable,
            ),
        ],
        telemetry: vec![
            TelemetryDescriptor {
                channel_id: 200,
                telemetry_type: TelemetryType::F32,
                machine_name: "drive.speed_mps".into(),
                display_name: "车辆速度".into(),
                group: "驱动".into(),
                unit: "m/s".into(),
            },
            TelemetryDescriptor {
                channel_id: 201,
                telemetry_type: TelemetryType::I32,
                machine_name: "encoder.left_delta".into(),
                display_name: "左编码器增量".into(),
                group: "编码器".into(),
                unit: "count".into(),
            },
            TelemetryDescriptor {
                channel_id: 202,
                telemetry_type: TelemetryType::U32,
                machine_name: "encoder.left_total".into(),
                display_name: "左编码器总数".into(),
                group: "编码器".into(),
                unit: "count".into(),
            },
            TelemetryDescriptor {
                channel_id: 203,
                telemetry_type: TelemetryType::Flags32,
                machine_name: "drive.fault_flags".into(),
                display_name: "驱动故障标志".into(),
                group: "驱动".into(),
                unit: String::new(),
            },
            TelemetryDescriptor {
                channel_id: 204,
                telemetry_type: TelemetryType::U32,
                machine_name: "encoder.right_total".into(),
                display_name: "右编码器总数".into(),
                group: "编码器".into(),
                unit: "count".into(),
            },
            TelemetryDescriptor {
                channel_id: 205,
                telemetry_type: TelemetryType::F32,
                machine_name: "drive.left_wheel_speed_mps".into(),
                display_name: "左轮速度".into(),
                group: "驱动".into(),
                unit: "m/s".into(),
            },
            TelemetryDescriptor {
                channel_id: 206,
                telemetry_type: TelemetryType::F32,
                machine_name: "drive.right_wheel_speed_mps".into(),
                display_name: "右轮速度".into(),
                group: "驱动".into(),
                unit: "m/s".into(),
            },
            TelemetryDescriptor {
                channel_id: 207,
                telemetry_type: TelemetryType::F32,
                machine_name: "drive.target_speed_mps".into(),
                display_name: "目标速度".into(),
                group: "驱动".into(),
                unit: "m/s".into(),
            },
            TelemetryDescriptor {
                channel_id: 208,
                telemetry_type: TelemetryType::F32,
                machine_name: "drive.speed_error_mps".into(),
                display_name: "速度误差".into(),
                group: "控制".into(),
                unit: "m/s".into(),
            },
            TelemetryDescriptor {
                channel_id: 209,
                telemetry_type: TelemetryType::U32,
                machine_name: "motor.left_pwm".into(),
                display_name: "左 PWM".into(),
                group: "电机".into(),
                unit: "permille".into(),
            },
            TelemetryDescriptor {
                channel_id: 210,
                telemetry_type: TelemetryType::U32,
                machine_name: "motor.right_pwm".into(),
                display_name: "右 PWM".into(),
                group: "电机".into(),
                unit: "permille".into(),
            },
            TelemetryDescriptor {
                channel_id: 211,
                telemetry_type: TelemetryType::I32,
                machine_name: "encoder.right_delta".into(),
                display_name: "右编码器增量".into(),
                group: "编码器".into(),
                unit: "count".into(),
            },
            TelemetryDescriptor {
                channel_id: 212,
                telemetry_type: TelemetryType::U32,
                machine_name: "control.loop_jitter_us".into(),
                display_name: "控制环抖动".into(),
                group: "控制".into(),
                unit: "us".into(),
            },
            TelemetryDescriptor {
                channel_id: 213,
                telemetry_type: TelemetryType::F32,
                machine_name: "power.battery_voltage".into(),
                display_name: "电池电压".into(),
                group: "电源".into(),
                unit: "V".into(),
            },
            TelemetryDescriptor {
                channel_id: 214,
                telemetry_type: TelemetryType::F32,
                machine_name: "steering.error_deg".into(),
                display_name: "转向误差".into(),
                group: "转向".into(),
                unit: "deg".into(),
            },
            TelemetryDescriptor {
                channel_id: 215,
                telemetry_type: TelemetryType::U32,
                machine_name: "system.uptime_ms".into(),
                display_name: "运行时间".into(),
                group: "系统".into(),
                unit: "ms".into(),
            },
        ],
    }
}

fn telemetry_period_us(sample_rate_hz: u16) -> u64 {
    1_000_000 / u64::from(sample_rate_hz)
}

fn telemetry_value(
    descriptor: &TelemetryDescriptor,
    timestamp_us: u64,
    input: SpeedLoopInput,
    snapshot: SpeedLoopSnapshot,
) -> u32 {
    match descriptor.machine_name.as_str() {
        "drive.speed_mps" => snapshot.speed_mps.to_bits(),
        "drive.left_wheel_speed_mps" => (snapshot.speed_mps * 0.99)
            .clamp(-MAX_SPEED_MPS, MAX_SPEED_MPS)
            .to_bits(),
        "drive.right_wheel_speed_mps" => (snapshot.speed_mps * 1.01)
            .clamp(-MAX_SPEED_MPS, MAX_SPEED_MPS)
            .to_bits(),
        "drive.target_speed_mps" => input.target_mps.to_bits(),
        "drive.speed_error_mps" => snapshot.error_mps.to_bits(),
        "motor.left_pwm" | "motor.right_pwm" => {
            (snapshot.motor_output.abs().clamp(0.0, 1.0) * 1_000.0).round() as u32
        }
        "encoder.left_delta" => (18 + ((timestamp_us / 2_000) % 5) as i32) as u32,
        "encoder.right_delta" => (-18 - ((timestamp_us / 2_000) % 5) as i32) as u32,
        "encoder.left_total" => (timestamp_us / 2_000 * 20) as u32,
        "encoder.right_total" => (timestamp_us / 2_000 * 19) as u32,
        "drive.fault_flags" => {
            if timestamp_us % 5_000_000 < 10_000 {
                1
            } else {
                0
            }
        }
        name if name.starts_with("custom.") => match descriptor.telemetry_type {
            TelemetryType::F32 => 1.5f32.to_bits(),
            TelemetryType::I32 => (-4i32) as u32,
            TelemetryType::U32 => 8,
            TelemetryType::Flags32 => 0b101,
        },
        _ => deterministic_value_for_type(descriptor.telemetry_type, timestamp_us),
    }
}

fn deterministic_value_for_type(telemetry_type: TelemetryType, timestamp_us: u64) -> u32 {
    match telemetry_type {
        TelemetryType::F32 => (1.0 + (timestamp_us % 1_000_000) as f32 / 1_000_000.0).to_bits(),
        TelemetryType::I32 => (timestamp_us / 2_000) as i32 as u32,
        TelemetryType::U32 | TelemetryType::Flags32 => (timestamp_us / 1_000) as u32,
    }
}

#[allow(clippy::too_many_arguments)]
fn numeric_f32(
    param_id: u32,
    machine_name: &str,
    display_name: &str,
    group: &str,
    unit: &str,
    default: f32,
    min: f32,
    max: f32,
    step: f32,
    flags: ParamFlags,
) -> ParamDescriptor {
    ParamDescriptor {
        param_id,
        param_type: ParamType::F32,
        flags,
        machine_name: machine_name.into(),
        display_name: display_name.into(),
        group: group.into(),
        unit: unit.into(),
        default_value: ParamValue::F32(default),
        constraints: ParamConstraints::Numeric {
            min: ParamValue::F32(min),
            max: ParamValue::F32(max),
            step: ParamValue::F32(step),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn numeric_u32(
    param_id: u32,
    machine_name: &str,
    display_name: &str,
    group: &str,
    unit: &str,
    default: u32,
    min: u32,
    max: u32,
    step: u32,
    flags: ParamFlags,
) -> ParamDescriptor {
    ParamDescriptor {
        param_id,
        param_type: ParamType::U32,
        flags,
        machine_name: machine_name.into(),
        display_name: display_name.into(),
        group: group.into(),
        unit: unit.into(),
        default_value: ParamValue::U32(default),
        constraints: ParamConstraints::Numeric {
            min: ParamValue::U32(min),
            max: ParamValue::U32(max),
            step: ParamValue::U32(step),
        },
    }
}

fn boolean(
    param_id: u32,
    machine_name: &str,
    display_name: &str,
    flags: ParamFlags,
) -> ParamDescriptor {
    ParamDescriptor {
        param_id,
        param_type: ParamType::Bool,
        flags,
        machine_name: machine_name.into(),
        display_name: display_name.into(),
        group: "编码器".into(),
        unit: String::new(),
        default_value: ParamValue::Bool(false),
        constraints: ParamConstraints::None,
    }
}
