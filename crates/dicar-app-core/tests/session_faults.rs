use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use dctp_protocol::{
    crc32_iso_hdlc, decode_packet, encode_frame, CapabilityFlags, DeviceManifest, ErrorCode,
    ErrorPayload, Frame, FrameFlags, HelloAck, ManifestChunk, ManifestDone, MessageType,
    ParamCommit, ParamConstraints, ParamDescriptor, ParamFlags, ParamRead, ParamState, ParamType,
    ParamValue, ParamWrite, ParamWriteAck, WireDecode, WireEncode, MANIFEST_SCHEMA_VERSION,
};
use dicar_app_core::{
    Clock, ConnectionPhase, CoreError, Endpoint, FixedNonce, ProtocolSession, TestClock, Transport,
    TransportError, TransportIdentity,
};

const SESSION_ONE: u32 = 0x1111_0001;
const SESSION_TWO: u32 = 0x2222_0002;

#[derive(Clone, Copy)]
enum Fault {
    None,
    Drop(MessageType),
    CrcThenValid(MessageType),
    DeviceError(MessageType),
    WrongSession(MessageType),
    ManifestPartial,
    ManifestTailThenFull,
    DropCommitWithRefresh,
    TransportDisconnect(MessageType),
}

struct FakeState {
    writes: Vec<Frame>,
    reads: VecDeque<u8>,
    fault: Fault,
    connection_count: usize,
    manifest_name: String,
    negotiated_max_payload: u16,
    empty_read_advance_ms: u64,
    refreshes_sent: u8,
}

#[derive(Clone)]
struct FakeControl(Arc<Mutex<FakeState>>);

impl FakeControl {
    fn set_fault(&self, fault: Fault) {
        let mut state = self.0.lock().unwrap();
        state.empty_read_advance_ms = if matches!(
            fault,
            Fault::Drop(_)
                | Fault::WrongSession(_)
                | Fault::ManifestPartial
                | Fault::DropCommitWithRefresh
        ) {
            100
        } else {
            0
        };
        state.fault = fault;
    }

    fn set_manifest_name(&self, name: &str) {
        self.0.lock().unwrap().manifest_name = name.to_owned();
    }

    fn frames(&self, message_type: MessageType) -> Vec<Frame> {
        self.0
            .lock()
            .unwrap()
            .writes
            .iter()
            .filter(|frame| frame.header.message_type == message_type)
            .cloned()
            .collect()
    }

    fn queue_unsolicited(&self, frame: Frame) {
        FakeTransport::queue_response(&mut self.0.lock().unwrap(), frame, false);
    }

    fn pending_read_bytes(&self) -> usize {
        self.0.lock().unwrap().reads.len()
    }
}

struct FakeTransport {
    state: Arc<Mutex<FakeState>>,
    clock: TestClock,
    closed: bool,
}

impl FakeTransport {
    fn new(clock: TestClock) -> (Self, FakeControl) {
        let state = Arc::new(Mutex::new(FakeState {
            writes: Vec::new(),
            reads: VecDeque::new(),
            fault: Fault::None,
            connection_count: 0,
            manifest_name: "gain_a".to_owned(),
            negotiated_max_payload: 64,
            empty_read_advance_ms: 0,
            refreshes_sent: 0,
        }));
        (
            Self {
                state: Arc::clone(&state),
                clock,
                closed: false,
            },
            FakeControl(state),
        )
    }

    fn queue_response(state: &mut FakeState, frame: Frame, corrupt_crc: bool) {
        let mut bytes = encode_frame(&frame).unwrap();
        if corrupt_crc {
            let crc_byte = bytes.len() - 2;
            bytes[crc_byte] ^= 0x01;
        }
        state.reads.extend(bytes);
    }
}

impl Transport for FakeTransport {
    fn identity(&self) -> TransportIdentity {
        TransportIdentity {
            endpoint: Endpoint::Simulator {
                address: SocketAddr::from(([127, 0, 0, 1], 1)),
            },
        }
    }

    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError> {
        if self.closed {
            return Err(TransportError::Disconnected);
        }
        let mut state = self.state.lock().unwrap();
        if state.reads.is_empty() {
            let now_ms = self.clock.now_ms();
            let refresh_due = matches!(state.fault, Fault::DropCommitWithRefresh)
                && matches!((state.refreshes_sent, now_ms), (0, 2_900) | (1, 5_900));
            if refresh_due {
                let session_id = if state.connection_count == 1 {
                    SESSION_ONE
                } else {
                    SESSION_TWO
                };
                let heartbeat = Frame::new(
                    MessageType::HeartbeatAck,
                    FrameFlags::RESPONSE,
                    60_000u16.wrapping_add(now_ms as u16),
                    session_id,
                    vec![0; 4],
                )
                .unwrap();
                Self::queue_response(&mut state, heartbeat, false);
                state.refreshes_sent += 1;
            }
        }
        if state.reads.is_empty() {
            let elapsed_ms = state.empty_read_advance_ms;
            drop(state);
            self.clock.advance_ms(elapsed_ms);
            return Ok(0);
        }
        let count = output.len().min(state.reads.len());
        for byte in output.iter_mut().take(count) {
            *byte = state.reads.pop_front().unwrap();
        }
        Ok(count)
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.closed {
            return Err(TransportError::Disconnected);
        }
        let request = decode_packet(&bytes[..bytes.len() - 1]).unwrap();
        let mut state = self.state.lock().unwrap();
        state.writes.push(request.clone());
        if matches!(state.fault, Fault::TransportDisconnect(message_type) if message_type == request.header.message_type)
        {
            return Err(TransportError::Disconnected);
        }
        if matches!(state.fault, Fault::Drop(message_type) if message_type == request.header.message_type)
            || matches!(state.fault, Fault::DropCommitWithRefresh if request.header.message_type == MessageType::ParamCommit)
        {
            return Ok(());
        }

        let response = response_for(&mut state, &request);
        match state.fault {
            Fault::ManifestPartial
                if request.header.message_type == MessageType::ManifestRequest =>
            {
                Self::queue_response(&mut state, response[0].clone(), false);
                state.fault = Fault::ManifestTailThenFull;
            }
            Fault::ManifestTailThenFull
                if request.header.message_type == MessageType::ManifestRequest =>
            {
                Self::queue_response(&mut state, response[1].clone(), false);
                for frame in response {
                    Self::queue_response(&mut state, frame, false);
                }
                state.fault = Fault::None;
                state.empty_read_advance_ms = 0;
            }
            Fault::CrcThenValid(message_type)
                if message_type == request.header.message_type && response.len() == 1 =>
            {
                Self::queue_response(&mut state, response[0].clone(), true);
                Self::queue_response(&mut state, response[0].clone(), false);
                state.fault = Fault::None;
            }
            Fault::DeviceError(message_type) if message_type == request.header.message_type => {
                let error = ErrorPayload {
                    original_message_type: request.header.message_type,
                    original_sequence: request.header.sequence,
                    error_code: ErrorCode::InvalidParamId,
                    context: "unknown parameter".to_owned(),
                };
                let frame = response_frame(
                    &request,
                    MessageType::Error,
                    error.encode().unwrap(),
                    request.header.session_id,
                );
                Self::queue_response(&mut state, frame, false);
                state.fault = Fault::None;
            }
            Fault::WrongSession(message_type) if message_type == request.header.message_type => {
                for frame in response {
                    let wrong = response_frame(
                        &request,
                        frame.header.message_type,
                        frame.payload,
                        request.header.session_id.wrapping_add(1),
                    );
                    Self::queue_response(&mut state, wrong, false);
                }
                state.fault = Fault::None;
            }
            _ => {
                for frame in response {
                    Self::queue_response(&mut state, frame, false);
                }
            }
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), TransportError> {
        self.closed = true;
        Ok(())
    }
}

fn response_for(state: &mut FakeState, request: &Frame) -> Vec<Frame> {
    match request.header.message_type {
        MessageType::Hello => {
            state.connection_count += 1;
            let session_id = if state.connection_count == 1 {
                SESSION_ONE
            } else {
                SESSION_TWO
            };
            let manifest = manifest(&state.manifest_name);
            let ack = HelloAck {
                session_id,
                device_id: *b"FAULT-TEST-DEV!!",
                boot_count: 7,
                firmware_major: 1,
                firmware_minor: 2,
                firmware_patch: 3,
                sdk_major: 4,
                sdk_minor: 5,
                sdk_patch: 6,
                capabilities: CapabilityFlags::PARAMETERS | CapabilityFlags::PERSISTENCE,
                manifest_crc32: manifest.manifest_crc32().unwrap(),
                max_payload: state.negotiated_max_payload,
            };
            vec![response_frame(
                request,
                MessageType::HelloAck,
                ack.encode().unwrap(),
                session_id,
            )]
        }
        MessageType::ManifestRequest => {
            let bytes = manifest(&state.manifest_name).encode_canonical().unwrap();
            let crc = crc32_iso_hdlc(&bytes);
            let chunk_len = usize::from(state.negotiated_max_payload) - 12;
            let mut frames = bytes
                .chunks(chunk_len)
                .enumerate()
                .map(|(index, data)| {
                    let chunk = ManifestChunk {
                        manifest_crc32: crc,
                        total_len: bytes.len() as u32,
                        offset: (index * chunk_len) as u32,
                        data: data.to_vec(),
                    };
                    response_frame(
                        request,
                        MessageType::ManifestChunk,
                        chunk.encode().unwrap(),
                        request.header.session_id,
                    )
                })
                .collect::<Vec<_>>();
            frames.push(response_frame(
                request,
                MessageType::ManifestDone,
                ManifestDone {
                    manifest_crc32: crc,
                    total_len: bytes.len() as u32,
                }
                .encode()
                .unwrap(),
                request.header.session_id,
            ));
            frames
        }
        MessageType::ParamRead => {
            let read = ParamRead::decode(&request.payload).unwrap();
            vec![response_frame(
                request,
                MessageType::ParamValue,
                ParamState {
                    param_id: read.param_id,
                    revision: 3,
                    value: ParamValue::U32(42),
                    persisted_value: Some(ParamValue::U32(40)),
                }
                .encode()
                .unwrap(),
                request.header.session_id,
            )]
        }
        MessageType::ParamWrite => vec![response_frame(
            request,
            MessageType::ParamWriteAck,
            ParamWriteAck {
                value: ParamValue::U32(43),
                new_revision: 4,
            }
            .encode()
            .unwrap(),
            request.header.session_id,
        )],
        MessageType::ParamCommit => vec![response_frame(
            request,
            MessageType::ParamCommitAck,
            vec![0; 8],
            request.header.session_id,
        )],
        MessageType::Heartbeat => vec![response_frame(
            request,
            MessageType::HeartbeatAck,
            request.payload.clone(),
            request.header.session_id,
        )],
        MessageType::SessionClose => vec![response_frame(
            request,
            MessageType::SessionClose,
            Vec::new(),
            request.header.session_id,
        )],
        _ => Vec::new(),
    }
}

fn response_frame(
    request: &Frame,
    message_type: MessageType,
    payload: Vec<u8>,
    session_id: u32,
) -> Frame {
    Frame::new(
        message_type,
        FrameFlags::RESPONSE,
        request.header.sequence,
        session_id,
        payload,
    )
    .unwrap()
}

fn manifest(name: &str) -> DeviceManifest {
    DeviceManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        parameters: vec![ParamDescriptor {
            param_id: 1,
            param_type: ParamType::U32,
            flags: ParamFlags::WRITABLE | ParamFlags::PERSISTENT,
            machine_name: name.to_owned(),
            display_name: name.to_owned(),
            group: "Control".to_owned(),
            unit: "unit".to_owned(),
            default_value: ParamValue::U32(42),
            constraints: ParamConstraints::Numeric {
                min: ParamValue::U32(0),
                max: ParamValue::U32(100),
                step: ParamValue::U32(1),
            },
        }],
        telemetry: Vec::new(),
    }
}

fn ready_session() -> (ProtocolSession<FakeTransport>, FakeControl, TestClock) {
    let clock = TestClock::new();
    let (transport, control) = FakeTransport::new(clock.clone());
    let mut session = ProtocolSession::new(transport, FixedNonce(11), clock.clone());
    session.connect_and_load().unwrap();
    (session, control, clock)
}

#[test]
fn crc_noise_is_counted_and_the_following_valid_frame_completes_the_request() {
    let (mut session, control, _clock) = ready_session();
    let before = session.diagnostics();
    control.set_fault(Fault::CrcThenValid(MessageType::Heartbeat));

    session
        .request(MessageType::Heartbeat, vec![0, 0, 0, 0])
        .unwrap();

    let after = session.diagnostics();
    assert_eq!(after.crc_errors, before.crc_errors + 1);
    assert_eq!(after.valid_frames, before.valid_frames + 1);
}

#[test]
fn heartbeat_waits_for_500_ms_of_inbound_idle_time() {
    let (mut session, control, clock) = ready_session();
    clock.advance_ms(400);
    control.queue_unsolicited(
        Frame::new(
            MessageType::DeviceEvent,
            FrameFlags::NONE,
            90,
            session.session_id().unwrap(),
            Vec::new(),
        )
        .unwrap(),
    );
    session.poll().unwrap();

    clock.advance_ms(499);
    session.poll().unwrap();
    assert_eq!(control.frames(MessageType::Heartbeat).len(), 0);

    clock.advance_ms(1);
    session.poll().unwrap();
    assert_eq!(control.frames(MessageType::Heartbeat).len(), 1);
}

#[test]
fn unsolicited_queue_drops_oldest_frames_at_its_fixed_capacity() {
    let (mut session, control, _clock) = ready_session();
    let session_id = session.session_id().unwrap();
    for sequence in 0..130 {
        control.queue_unsolicited(
            Frame::new(
                MessageType::DeviceEvent,
                FrameFlags::NONE,
                sequence,
                session_id,
                Vec::new(),
            )
            .unwrap(),
        );
    }

    session.poll().unwrap();

    let mut sequences = Vec::new();
    while let Some(frame) = session.pop_unsolicited() {
        sequences.push(frame.header.sequence);
    }
    assert_eq!(sequences.len(), 128);
    assert_eq!(sequences[0], 2);
    assert_eq!(session.diagnostics().unsolicited_dropped, 2);
}

#[test]
fn poll_has_a_fixed_read_budget_under_continuous_unsolicited_traffic() {
    let (mut session, control, _clock) = ready_session();
    let session_id = session.session_id().unwrap();
    for sequence in 0..1_000 {
        control.queue_unsolicited(
            Frame::new(
                MessageType::DeviceEvent,
                FrameFlags::NONE,
                sequence,
                session_id,
                Vec::new(),
            )
            .unwrap(),
        );
    }

    session.poll().unwrap();

    assert!(control.pending_read_bytes() > 0);
}

#[test]
fn ordinary_request_uses_four_sends_and_one_sequence() {
    let (mut session, control, _clock) = ready_session();
    control.set_fault(Fault::Drop(MessageType::ParamRead));
    let before = control.frames(MessageType::ParamRead).len();

    let error = session
        .request(
            MessageType::ParamRead,
            ParamRead { param_id: 999 }.encode().unwrap(),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        CoreError::Timeout {
            message_type: MessageType::ParamRead,
            attempts: 4
        }
    ));
    let attempts = &control.frames(MessageType::ParamRead)[before..];
    assert_eq!(attempts.len(), 4);
    assert!(attempts
        .iter()
        .all(|frame| frame.header.sequence == attempts[0].header.sequence));
}

#[test]
fn manifest_request_uses_four_sends_and_one_sequence() {
    let clock = TestClock::new();
    let (transport, control) = FakeTransport::new(clock.clone());
    control.set_fault(Fault::Drop(MessageType::ManifestRequest));
    let mut session = ProtocolSession::new(transport, FixedNonce(12), clock);

    let error = session.connect_and_load().unwrap_err();

    assert!(matches!(
        error,
        CoreError::Timeout {
            message_type: MessageType::ManifestRequest,
            attempts: 4
        }
    ));
    let attempts = control.frames(MessageType::ManifestRequest);
    assert_eq!(attempts.len(), 4);
    assert!(attempts
        .iter()
        .all(|frame| frame.header.sequence == attempts[0].header.sequence));
    let writes_after_timeout = control.0.lock().unwrap().writes.len();
    assert_eq!(session.phase(), ConnectionPhase::Disconnected);
    assert!(matches!(
        session.request(MessageType::Heartbeat, vec![0, 0, 0, 0]),
        Err(CoreError::Disconnected)
    ));
    assert_eq!(control.0.lock().unwrap().writes.len(), writes_after_timeout);
}

#[test]
fn manifest_retry_ignores_a_delayed_tail_until_the_replayed_first_chunk() {
    let clock = TestClock::new();
    let (transport, control) = FakeTransport::new(clock.clone());
    control.set_fault(Fault::ManifestPartial);
    let mut session = ProtocolSession::new(transport, FixedNonce(13), clock);

    let connected = session.connect_and_load().unwrap();

    assert_eq!(connected.phase, ConnectionPhase::Ready);
    assert_eq!(connected.manifest.parameters[0].machine_name, "gain_a");
    assert_eq!(control.frames(MessageType::ManifestRequest).len(), 2);
}

#[test]
fn commit_request_uses_three_sends_and_one_sequence() {
    let (mut session, control, _clock) = ready_session();
    control.set_fault(Fault::DropCommitWithRefresh);

    let error = session
        .request(
            MessageType::ParamCommit,
            ParamCommit {
                entries: Vec::new(),
                canonical_crc32: 0,
            }
            .encode()
            .unwrap(),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        CoreError::Timeout {
            message_type: MessageType::ParamCommit,
            attempts: 3
        }
    ));
    let attempts = control.frames(MessageType::ParamCommit);
    assert_eq!(attempts.len(), 3);
    assert!(attempts
        .iter()
        .all(|frame| frame.header.sequence == attempts[0].header.sequence));
}

#[test]
fn silent_commit_stops_before_the_retry_at_the_stale_deadline() {
    let (mut session, control, _clock) = ready_session();
    control.set_fault(Fault::Drop(MessageType::ParamCommit));

    let error = session
        .request(
            MessageType::ParamCommit,
            ParamCommit {
                entries: Vec::new(),
                canonical_crc32: 0,
            }
            .encode()
            .unwrap(),
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::Disconnected));
    assert_eq!(session.phase(), ConnectionPhase::Disconnected);
    assert_eq!(control.frames(MessageType::ParamCommit).len(), 1);
}

#[test]
fn device_error_frame_is_returned_as_typed_core_error() {
    let (mut session, control, _clock) = ready_session();
    control.set_fault(Fault::DeviceError(MessageType::ParamRead));

    let error = session
        .request(
            MessageType::ParamRead,
            ParamRead { param_id: 999 }.encode().unwrap(),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        CoreError::Device {
            original_message_type: MessageType::ParamRead,
            code: ErrorCode::InvalidParamId,
            ref context,
            ..
        } if context == "unknown parameter"
    ));
}

#[test]
fn transport_disconnect_transitions_session_and_prevents_later_writes() {
    let (mut session, control, _clock) = ready_session();
    control.set_fault(Fault::TransportDisconnect(MessageType::Heartbeat));

    let error = session
        .request(MessageType::Heartbeat, vec![0, 0, 0, 0])
        .unwrap_err();
    let writes_after_disconnect = control.0.lock().unwrap().writes.len();

    assert!(matches!(
        error,
        CoreError::Transport(TransportError::Disconnected)
    ));
    assert_eq!(session.phase(), ConnectionPhase::Disconnected);
    assert!(matches!(
        session.request(MessageType::Heartbeat, vec![0, 0, 0, 0]),
        Err(CoreError::Disconnected)
    ));
    assert_eq!(
        control.0.lock().unwrap().writes.len(),
        writes_after_disconnect
    );
}

#[test]
fn session_is_writable_at_2999_ms_but_not_at_3000_ms() {
    let (mut live_session, live_control, live_clock) = ready_session();
    live_clock.advance_ms(2_999);
    live_session
        .request(MessageType::Heartbeat, vec![0, 0, 0, 0])
        .unwrap();
    assert_eq!(live_control.frames(MessageType::Heartbeat).len(), 1);

    let (mut stale_session, stale_control, stale_clock) = ready_session();
    stale_clock.advance_ms(3_000);
    let writes_before = stale_control.0.lock().unwrap().writes.len();
    assert!(matches!(
        stale_session.request(MessageType::Heartbeat, vec![0, 0, 0, 0]),
        Err(CoreError::Disconnected)
    ));
    assert_eq!(stale_session.phase(), ConnectionPhase::Disconnected);
    assert_eq!(stale_control.0.lock().unwrap().writes.len(), writes_before);
}

#[test]
fn wrong_session_response_disconnects_and_prevents_later_writes() {
    let (mut session, control, _clock) = ready_session();
    control.set_fault(Fault::WrongSession(MessageType::Heartbeat));

    let _ = session.request(MessageType::Heartbeat, vec![0, 0, 0, 0]);
    let writes_after_fault = control.0.lock().unwrap().writes.len();

    assert_eq!(session.phase(), ConnectionPhase::Disconnected);
    assert!(matches!(
        session.request(MessageType::Heartbeat, vec![0, 0, 0, 0]),
        Err(CoreError::Disconnected)
    ));
    assert_eq!(control.0.lock().unwrap().writes.len(), writes_after_fault);
}

#[test]
fn reconnect_uses_new_session_reloads_parameters_and_replaces_changed_manifest() {
    let (mut session, control, _clock) = ready_session();
    let first_session_id = session.session_id().unwrap();
    session
        .request(
            MessageType::ParamWrite,
            ParamWrite {
                param_id: 1,
                expected_revision: 3,
                value: ParamValue::U32(43),
            }
            .encode()
            .unwrap(),
        )
        .unwrap();
    control.set_manifest_name("gain_b");

    let reconnected = session.connect_and_load().unwrap();

    assert_ne!(reconnected.session_id, first_session_id);
    assert_eq!(reconnected.manifest.parameters[0].machine_name, "gain_b");
    assert_eq!(control.frames(MessageType::ParamRead).len(), 2);
    assert_eq!(control.frames(MessageType::ParamWrite).len(), 1);
    assert!(control
        .frames(MessageType::ParamRead)
        .last()
        .is_some_and(|frame| frame.header.session_id == reconnected.session_id));
}
