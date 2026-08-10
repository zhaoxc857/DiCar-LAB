use dctp_protocol::{
    CapabilityFlags, DeviceManifest, EnumOption, ErrorCode, ErrorPayload, Frame, FrameFlags,
    Heartbeat, Hello, HelloAck, ManifestChunk, ManifestDone, MessageType, ParamConstraints,
    ParamDescriptor, ParamFlags, ParamRead, ParamState, ParamType, ParamValue, ParamWrite,
    ParamWriteAck, ProtocolError, TelemetryBatch, TelemetryDescriptor, TelemetrySample,
    TelemetrySubscription, TelemetryType, WireDecode, WireEncode, MANIFEST_SCHEMA_VERSION,
    MAX_PAYLOAD_LEN,
};

use crate::{Priority, QueuedFrame, RequestCache, RequestKey};

pub const SESSION_EXPIRATION_MS: u64 = 3_000;
const MANIFEST_CHUNK_DATA_LEN: usize = 900;

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
}

#[derive(Clone, Debug)]
struct Parameter {
    descriptor: ParamDescriptor,
    value: ParamValue,
    revision: u32,
}

#[derive(Clone, Debug)]
struct CompletedHello {
    request: Frame,
    response: Frame,
}

#[derive(Debug)]
pub struct SimDevice {
    config: SimConfig,
    session: Option<Session>,
    session_counter: u32,
    parameters: Vec<Parameter>,
    request_cache: RequestCache,
    completed_hello: Option<CompletedHello>,
    telemetry_subscription: Option<TelemetrySubscription>,
    next_telemetry_sequence: u16,
    pending_dropped_telemetry_samples: u16,
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
                descriptor,
                revision: 0,
            })
            .collect();
        Self {
            config,
            session: None,
            session_counter: 0,
            parameters,
            request_cache: RequestCache::default(),
            completed_hello: None,
            telemetry_subscription: None,
            next_telemetry_sequence: 0,
            pending_dropped_telemetry_samples: 0,
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
        if self.validate_session(request.header.session_id).is_err() {
            return vec![self.error_response(&request, ErrorCode::InvalidSession, String::new())];
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

        let responses = self.dispatch(&request);
        if reliable && responses.len() == 1 {
            self.request_cache.insert(key, responses[0].frame.clone());
        }
        responses
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<QueuedFrame> {
        self.expire_session(now_ms);
        let Some(session) = &self.session else {
            return Vec::new();
        };
        let Some(subscription) = &self.telemetry_subscription else {
            return Vec::new();
        };
        let values = subscription
            .channel_ids
            .iter()
            .map(|channel_id| telemetry_value(*channel_id))
            .collect::<Vec<_>>();
        let first_sample_sequence = self.next_telemetry_sequence;
        self.next_telemetry_sequence = self.next_telemetry_sequence.wrapping_add(1);
        let batch = TelemetryBatch {
            subscription_version: subscription.subscription_version,
            first_sample_sequence,
            dropped_samples: std::mem::take(&mut self.pending_dropped_telemetry_samples),
            base_timestamp_us: u32::try_from(now_ms.saturating_mul(1_000)).unwrap_or(u32::MAX),
            samples: vec![TelemetrySample { dt_us: 0, values }],
        };
        vec![QueuedFrame {
            priority: Priority::Telemetry,
            frame: response_frame(
                MessageType::TelemetryData,
                first_sample_sequence,
                session.id,
                batch.encode().expect("fixed telemetry batch is encodable"),
                FrameFlags::NONE,
            ),
        }]
    }

    pub fn open_session(&mut self, client_nonce: u32, now_ms: u64) -> Result<u32, ProtocolError> {
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
        });
        self.request_cache.clear();
        self.completed_hello = None;
        self.telemetry_subscription = None;
        self.next_telemetry_sequence = 0;
        self.pending_dropped_telemetry_samples = 0;
        Ok(session_id)
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
            self.session = None;
            self.request_cache.clear();
            self.completed_hello = None;
            self.telemetry_subscription = None;
            self.pending_dropped_telemetry_samples = 0;
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
        let session_id = match self.open_session(hello.client_nonce, now_ms) {
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
            capabilities: CapabilityFlags::PARAMETERS,
            manifest_crc32,
            max_payload: hello.max_payload.min(MAX_PAYLOAD_LEN as u16),
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

    fn dispatch(&mut self, request: &Frame) -> Vec<QueuedFrame> {
        match request.header.message_type {
            MessageType::Heartbeat => self.handle_heartbeat(request),
            MessageType::ManifestRequest => self.handle_manifest_request(request),
            MessageType::ParamRead => self.handle_param_read(request),
            MessageType::ParamWrite => self.handle_param_write(request),
            MessageType::TelemetrySubscribe => self.handle_telemetry_subscribe(request),
            MessageType::SessionClose => {
                let response = response_frame(
                    MessageType::SessionClose,
                    request.header.sequence,
                    request.header.session_id,
                    Vec::new(),
                    FrameFlags::RESPONSE,
                );
                self.session = None;
                self.request_cache.clear();
                self.telemetry_subscription = None;
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
        let mut responses = Vec::new();
        for (chunk_index, data) in bytes.chunks(MANIFEST_CHUNK_DATA_LEN).enumerate() {
            let chunk = ManifestChunk {
                manifest_crc32: crc32,
                total_len: bytes.len() as u32,
                offset: (chunk_index * MANIFEST_CHUNK_DATA_LEN) as u32,
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

    fn handle_telemetry_subscribe(&mut self, request: &Frame) -> Vec<QueuedFrame> {
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
    DeviceManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        parameters: vec![
            numeric_f32(
                1, "pid.kp", "PID Kp", "Control", "", 1.0, 0.0, 1_000.0, 0.01, writable,
            ),
            numeric_u32(
                100,
                "encoder.left.ppr",
                "Left encoder PPR",
                "Encoder",
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
                "Right encoder PPR",
                "Encoder",
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
                display_name: "Quadrature multiplier".into(),
                group: "Encoder".into(),
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
                "Left encoder CPR",
                "Encoder",
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
                "Right encoder CPR",
                "Encoder",
                "count/rev",
                2_048,
                1,
                4_000_000,
                1,
                ParamFlags::NONE,
            ),
            boolean(
                105,
                "encoder.left.inverted",
                "Left encoder inverted",
                writable,
            ),
            boolean(
                106,
                "encoder.right.inverted",
                "Right encoder inverted",
                writable,
            ),
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
                "Encoder sample period",
                "Encoder",
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
                "Encoder speed LPF",
                "Encoder",
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
                "Encoder jump threshold",
                "Encoder",
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
                "Maximum credible RPM",
                "Encoder",
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
                "Missing pulse detection",
                writable,
            ),
        ],
        telemetry: vec![
            TelemetryDescriptor {
                channel_id: 200,
                telemetry_type: TelemetryType::F32,
                machine_name: "drive.speed_mps".into(),
                display_name: "Vehicle speed".into(),
                group: "Drive".into(),
                unit: "m/s".into(),
            },
            TelemetryDescriptor {
                channel_id: 201,
                telemetry_type: TelemetryType::I32,
                machine_name: "encoder.left_delta".into(),
                display_name: "Left encoder delta".into(),
                group: "Encoder".into(),
                unit: "count".into(),
            },
            TelemetryDescriptor {
                channel_id: 202,
                telemetry_type: TelemetryType::U32,
                machine_name: "encoder.left_total".into(),
                display_name: "Left encoder total".into(),
                group: "Encoder".into(),
                unit: "count".into(),
            },
            TelemetryDescriptor {
                channel_id: 203,
                telemetry_type: TelemetryType::Flags32,
                machine_name: "drive.fault_flags".into(),
                display_name: "Drive fault flags".into(),
                group: "Drive".into(),
                unit: String::new(),
            },
        ],
    }
}

fn telemetry_value(channel_id: u32) -> u32 {
    match channel_id {
        200 => 1.5f32.to_bits(),
        201 => (-4i32) as u32,
        202 => 8,
        203 => 0b101,
        _ => unreachable!("subscription IDs are checked against the fixed manifest"),
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
        group: "Encoder".into(),
        unit: String::new(),
        default_value: ParamValue::Bool(false),
        constraints: ParamConstraints::None,
    }
}
