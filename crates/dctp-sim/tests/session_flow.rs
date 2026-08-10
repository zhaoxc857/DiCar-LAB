use std::cell::Cell;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use dctp_protocol::{
    encode_frame, EnumOption, ErrorCode, ErrorPayload, Frame, FrameFlags, Heartbeat, Hello,
    HelloAck, MessageType, ParamConstraints, ParamFlags, ParamState, ParamType, ParamValue,
    ParamWrite, ParamWriteAck, StreamDecoder, TelemetryType, WireDecode, WireEncode,
};
use dctp_sim::{
    Direction, FaultAction, FaultInjector, FaultRule, RequestCache, RequestKey, SimConfig,
    SimDevice,
};

fn request(message_type: MessageType, sequence: u16, session_id: u32, payload: Vec<u8>) -> Frame {
    Frame::new(
        message_type,
        FrameFlags::ACK_REQUIRED,
        sequence,
        session_id,
        payload,
    )
    .unwrap()
}

fn only_response(responses: Vec<dctp_sim::QueuedFrame>) -> Frame {
    assert_eq!(responses.len(), 1);
    responses.into_iter().next().unwrap().frame
}

fn error_payload(response: Frame) -> ErrorPayload {
    assert_eq!(response.header.message_type, MessageType::Error);
    assert_eq!(
        response.header.flags,
        FrameFlags::from_bits(FrameFlags::ERROR.bits() | FrameFlags::RESPONSE.bits())
    );
    ErrorPayload::decode(&response.payload).unwrap()
}

fn decode_lower_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    assert!(value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}

#[test]
fn new_hello_produces_a_nonzero_new_session_and_invalidates_the_previous_one() {
    let mut device = SimDevice::new(SimConfig::default());
    let first = device.open_session(11, 0).unwrap();
    let second = device.open_session(22, 10).unwrap();

    assert_ne!(first, 0);
    assert_ne!(second, 0);
    assert_ne!(first, second);
    assert!(device.validate_session(first).is_err());
    assert!(device.validate_session(second).is_ok());
}

#[test]
fn duplicate_wire_hello_replays_the_first_ack_without_reopening_the_session() {
    let mut device = SimDevice::new(SimConfig::default());
    let hello = Hello {
        client_nonce: 0x1234_5678,
        min_version: 1,
        max_version: 1,
        max_payload: 1_024,
    };
    let request = request(MessageType::Hello, 41, 0, hello.encode().unwrap());

    let first = device.handle(request.clone(), 1);
    let second = device.handle(request, 2);
    let first_ack = HelloAck::decode(&only_response(first.clone()).payload).unwrap();

    assert_eq!(second, first);
    assert!(device.validate_session(first_ack.session_id).is_ok());
}

#[test]
fn same_sequence_hello_with_a_different_nonce_opens_a_new_session() {
    let mut device = SimDevice::new(SimConfig::default());
    let first_request = request(
        MessageType::Hello,
        41,
        0,
        Hello {
            client_nonce: 0x1111_1111,
            min_version: 1,
            max_version: 1,
            max_payload: 1_024,
        }
        .encode()
        .unwrap(),
    );
    let second_request = request(
        MessageType::Hello,
        41,
        0,
        Hello {
            client_nonce: 0x2222_2222,
            min_version: 1,
            max_version: 1,
            max_payload: 1_024,
        }
        .encode()
        .unwrap(),
    );

    let first = HelloAck::decode(&only_response(device.handle(first_request, 1)).payload).unwrap();
    let second =
        HelloAck::decode(&only_response(device.handle(second_request, 2)).payload).unwrap();

    assert_ne!(second.session_id, first.session_id);
    assert!(device.validate_session(first.session_id).is_err());
    assert!(device.validate_session(second.session_id).is_ok());
}

#[test]
fn wrong_session_returns_invalid_session_without_mutating_the_parameter() {
    let mut device = SimDevice::new(SimConfig::default());
    let session = device.open_session(11, 0).unwrap();
    let write = ParamWrite {
        param_id: 1,
        expected_revision: 0,
        value: ParamValue::F32(2.0),
    };

    let response = only_response(device.handle(
        request(
            MessageType::ParamWrite,
            5,
            session.wrapping_add(1),
            write.encode().unwrap(),
        ),
        1,
    ));
    let error = error_payload(response);

    assert_eq!(error.error_code, ErrorCode::InvalidSession);
    assert_eq!(error.original_message_type, MessageType::ParamWrite);
    assert_eq!(error.original_sequence, 5);
    assert_eq!(device.parameter_revision(1), Some(0));
}

#[test]
fn session_expires_at_3000_ms_but_not_2999_ms() {
    let mut before_boundary = SimDevice::new(SimConfig::default());
    let before_session = before_boundary.open_session(11, 0).unwrap();
    assert!(before_boundary.tick(2_999).is_empty());
    assert!(before_boundary.validate_session(before_session).is_ok());

    let mut at_boundary = SimDevice::new(SimConfig::default());
    let boundary_session = at_boundary.open_session(11, 0).unwrap();
    let response = only_response(
        at_boundary.handle(
            request(
                MessageType::Heartbeat,
                1,
                boundary_session,
                Heartbeat {
                    monotonic_ms: 3_000,
                }
                .encode()
                .unwrap(),
            ),
            3_000,
        ),
    );
    assert_eq!(
        error_payload(response).error_code,
        ErrorCode::InvalidSession
    );
    assert!(at_boundary.validate_session(boundary_session).is_err());
}

#[test]
fn expired_session_is_rejected_and_ram_value_remains_accepted() {
    let mut device = SimDevice::new(SimConfig::default());
    let expired_session = device.open_session(11, 0).unwrap();
    let write = ParamWrite {
        param_id: 1,
        expected_revision: 0,
        value: ParamValue::F32(2.0),
    };
    let response = only_response(device.handle(
        request(
            MessageType::ParamWrite,
            5,
            expired_session,
            write.encode().unwrap(),
        ),
        1,
    ));
    assert_eq!(response.header.message_type, MessageType::ParamWriteAck);

    device.tick(3_001);
    let expired = only_response(
        device.handle(
            request(
                MessageType::Heartbeat,
                6,
                expired_session,
                Heartbeat {
                    monotonic_ms: 3_001,
                }
                .encode()
                .unwrap(),
            ),
            3_001,
        ),
    );
    assert_eq!(error_payload(expired).error_code, ErrorCode::InvalidSession);

    let new_session = device.open_session(22, 3_002).unwrap();
    let read = only_response(device.handle(
        request(
            MessageType::ParamRead,
            7,
            new_session,
            1u32.to_le_bytes().to_vec(),
        ),
        3_003,
    ));
    let state = ParamState::decode(&read.payload).unwrap();
    assert_eq!(state.value, ParamValue::F32(2.0));
    assert_eq!(state.revision, 1);
}

#[test]
fn duplicate_parameter_write_returns_cached_ack_and_mutates_revision_once() {
    let mut device = SimDevice::new(SimConfig::default());
    let session = device.open_session(11, 0).unwrap();
    let payload = ParamWrite {
        param_id: 1,
        expected_revision: 0,
        value: ParamValue::F32(2.0),
    }
    .encode()
    .unwrap();
    let write = request(MessageType::ParamWrite, 5, session, payload);

    let first = device.handle(write.clone(), 1);
    let second = device.handle(write, 2);

    assert_eq!(first, second);
    assert_eq!(device.parameter_revision(1), Some(1));
}

#[test]
fn revision_conflict_returns_current_value_and_revision_as_lowercase_ack_hex() {
    let mut device = SimDevice::new(SimConfig::default());
    let session = device.open_session(11, 0).unwrap();
    let accepted = ParamWrite {
        param_id: 1,
        expected_revision: 0,
        value: ParamValue::F32(2.0),
    };
    device.handle(
        request(
            MessageType::ParamWrite,
            5,
            session,
            accepted.encode().unwrap(),
        ),
        1,
    );
    let stale = ParamWrite {
        param_id: 1,
        expected_revision: 0,
        value: ParamValue::F32(3.0),
    };

    let response = only_response(device.handle(
        request(MessageType::ParamWrite, 6, session, stale.encode().unwrap()),
        2,
    ));
    let error = error_payload(response);

    assert_eq!(error.error_code, ErrorCode::RevisionConflict);
    assert!(error.context.len() <= 64);
    let current = ParamWriteAck::decode(&decode_lower_hex(&error.context)).unwrap();
    assert_eq!(current.value, ParamValue::F32(2.0));
    assert_eq!(current.new_revision, 1);
    assert_eq!(device.parameter_revision(1), Some(1));
}

#[test]
fn parameter_writes_reject_unknown_type_range_and_read_only_violations() {
    let mut device = SimDevice::new(SimConfig::default());
    let session = device.open_session(11, 0).unwrap();
    let cases = [
        (
            1,
            ParamWrite {
                param_id: 999_999,
                expected_revision: 0,
                value: ParamValue::F32(1.0),
            },
            ErrorCode::InvalidParamId,
        ),
        (
            2,
            ParamWrite {
                param_id: 1,
                expected_revision: 0,
                value: ParamValue::U32(2),
            },
            ErrorCode::TypeMismatch,
        ),
        (
            3,
            ParamWrite {
                param_id: 1,
                expected_revision: 0,
                value: ParamValue::F32(1_001.0),
            },
            ErrorCode::OutOfRange,
        ),
        (
            4,
            ParamWrite {
                param_id: 104,
                expected_revision: 0,
                value: ParamValue::U32(100),
            },
            ErrorCode::ReadOnly,
        ),
    ];

    for (sequence, write, expected) in cases {
        let response = only_response(device.handle(
            request(
                MessageType::ParamWrite,
                sequence,
                session,
                write.encode().unwrap(),
            ),
            u64::from(sequence),
        ));
        assert_eq!(error_payload(response).error_code, expected);
    }
    assert_eq!(device.parameter_revision(1), Some(0));
}

#[test]
fn default_manifest_has_stable_pid_and_complete_encoder_calibration_parameters() {
    let config = SimConfig::default();
    let numeric_f32 = |min, max, step| ParamConstraints::Numeric {
        min: ParamValue::F32(min),
        max: ParamValue::F32(max),
        step: ParamValue::F32(step),
    };
    let numeric_u32 = |min, max, step| ParamConstraints::Numeric {
        min: ParamValue::U32(min),
        max: ParamValue::U32(max),
        step: ParamValue::U32(step),
    };
    let expected = vec![
        (
            1,
            "pid.kp",
            ParamType::F32,
            true,
            numeric_f32(0.0, 1_000.0, 0.01),
        ),
        (
            100,
            "encoder.left.ppr",
            ParamType::U32,
            true,
            numeric_u32(1, 1_000_000, 1),
        ),
        (
            101,
            "encoder.right.ppr",
            ParamType::U32,
            true,
            numeric_u32(1, 1_000_000, 1),
        ),
        (
            102,
            "encoder.quadrature_multiplier",
            ParamType::Enum,
            true,
            ParamConstraints::Enum {
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
        ),
        (
            103,
            "encoder.left.cpr",
            ParamType::U32,
            false,
            numeric_u32(1, 4_000_000, 1),
        ),
        (
            104,
            "encoder.right.cpr",
            ParamType::U32,
            false,
            numeric_u32(1, 4_000_000, 1),
        ),
        (
            105,
            "encoder.left.inverted",
            ParamType::Bool,
            true,
            ParamConstraints::None,
        ),
        (
            106,
            "encoder.right.inverted",
            ParamType::Bool,
            true,
            ParamConstraints::None,
        ),
        (
            107,
            "drive.wheel_diameter_mm",
            ParamType::F32,
            true,
            numeric_f32(1.0, 1_000.0, 0.1),
        ),
        (
            108,
            "drive.gear_ratio",
            ParamType::F32,
            true,
            numeric_f32(0.01, 100.0, 0.01),
        ),
        (
            109,
            "encoder.sample_period_us",
            ParamType::U32,
            true,
            numeric_u32(100, 1_000_000, 100),
        ),
        (
            110,
            "encoder.speed_lpf_hz",
            ParamType::F32,
            true,
            numeric_f32(0.0, 1_000.0, 0.1),
        ),
        (
            111,
            "encoder.jump_threshold_counts",
            ParamType::U32,
            true,
            numeric_u32(1, 1_000_000, 1),
        ),
        (
            112,
            "encoder.max_credible_rpm",
            ParamType::F32,
            true,
            numeric_f32(1.0, 100_000.0, 1.0),
        ),
        (
            113,
            "encoder.missing_pulse_detection",
            ParamType::Bool,
            true,
            ParamConstraints::None,
        ),
    ];

    assert_eq!(config.manifest.parameters.len(), expected.len());
    for (id, name, param_type, writable, constraints) in expected {
        let descriptor = config
            .manifest
            .parameters
            .iter()
            .find(|descriptor| descriptor.param_id == id)
            .unwrap();
        assert_eq!(descriptor.machine_name, name);
        assert_eq!(descriptor.param_type, param_type);
        assert_eq!(
            descriptor.flags.bits() & ParamFlags::WRITABLE.bits() != 0,
            writable
        );
        assert_eq!(descriptor.constraints, constraints);
    }
}

#[test]
fn default_manifest_exposes_named_dynamic_drive_channels_with_utf8_labels() {
    let manifest = SimConfig::default().manifest;
    let required_names = [
        "drive.target_speed_mps",
        "encoder.left_delta",
        "encoder.right_delta",
        "encoder.left_total",
        "encoder.right_total",
        "drive.left_wheel_speed_mps",
        "drive.right_wheel_speed_mps",
        "drive.speed_mps",
        "drive.speed_error_mps",
        "motor.left_pwm",
        "motor.right_pwm",
        "drive.fault_flags",
        "control.loop_jitter_us",
        "power.battery_voltage",
        "steering.error_deg",
    ];

    assert!(manifest.telemetry.len() >= 16);
    for machine_name in required_names {
        assert!(
            manifest
                .telemetry
                .iter()
                .any(|descriptor| descriptor.machine_name == machine_name),
            "missing {machine_name}"
        );
    }
    assert!(manifest.telemetry.iter().any(
        |descriptor| descriptor.display_name.contains('车') || descriptor.group.contains('驱')
    ));
}

#[test]
fn default_manifest_keeps_the_existing_telemetry_channel_contracts() {
    let manifest = SimConfig::default().manifest;
    let existing = [
        (200, "drive.speed_mps", TelemetryType::F32),
        (201, "encoder.left_delta", TelemetryType::I32),
        (202, "encoder.left_total", TelemetryType::U32),
        (203, "drive.fault_flags", TelemetryType::Flags32),
    ];

    for (channel_id, machine_name, expected_type) in existing {
        let descriptor = manifest
            .telemetry
            .iter()
            .find(|descriptor| descriptor.channel_id == channel_id)
            .unwrap();
        assert_eq!(descriptor.machine_name, machine_name);
        assert_eq!(descriptor.telemetry_type, expected_type);
    }
}

#[test]
fn default_manifest_uses_utf8_parameter_labels_for_pid_and_encoder_groups() {
    let manifest = SimConfig::default().manifest;
    let pid = manifest
        .parameters
        .iter()
        .find(|descriptor| descriptor.machine_name == "pid.kp")
        .unwrap();
    let left_encoder = manifest
        .parameters
        .iter()
        .find(|descriptor| descriptor.machine_name == "encoder.left.ppr")
        .unwrap();

    assert_eq!(pid.display_name, "速度 Kp");
    assert_eq!(pid.group, "控制");
    assert_eq!(left_encoder.param_id, 100);
    assert_eq!(left_encoder.param_type, ParamType::U32);
    assert_eq!(left_encoder.default_value, ParamValue::U32(512));
    for descriptor in manifest
        .parameters
        .iter()
        .filter(|descriptor| descriptor.machine_name.starts_with("encoder."))
    {
        assert_eq!(descriptor.group, "编码器", "{}", descriptor.machine_name);
        assert!(
            descriptor
                .display_name
                .chars()
                .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character)),
            "{} has non-Chinese display name {}",
            descriptor.machine_name,
            descriptor.display_name
        );
    }
}

#[test]
fn request_cache_is_fixed_fifo_and_lookup_does_not_refresh_age() {
    let mut cache = RequestCache::default();
    for sequence in 0..32 {
        let key = RequestKey {
            session_id: 7,
            message_type: MessageType::Heartbeat,
            sequence,
        };
        cache.get_or_insert(key, || request(MessageType::Heartbeat, sequence, 7, vec![]));
    }
    let oldest = RequestKey {
        session_id: 7,
        message_type: MessageType::Heartbeat,
        sequence: 0,
    };
    let unexpected_build = Cell::new(false);
    cache.get_or_insert(oldest, || {
        unexpected_build.set(true);
        request(MessageType::Heartbeat, 999, 7, vec![])
    });
    assert!(!unexpected_build.get());

    let newest = RequestKey {
        session_id: 7,
        message_type: MessageType::Heartbeat,
        sequence: 32,
    };
    cache.get_or_insert(newest, || request(MessageType::Heartbeat, 32, 7, vec![]));

    let rebuilt = Cell::new(false);
    let response = cache.get_or_insert(oldest, || {
        rebuilt.set(true);
        request(MessageType::Heartbeat, 999, 7, vec![])
    });
    assert!(rebuilt.get());
    assert_eq!(response.header.sequence, 999);
}

#[test]
fn packet_index_faults_are_deterministic_and_direction_specific() {
    let mut faults = FaultInjector::new(vec![
        FaultRule {
            direction: Direction::HostToDevice,
            packet_index: 1,
            action: FaultAction::Drop,
        },
        FaultRule {
            direction: Direction::DeviceToHost,
            packet_index: 0,
            action: FaultAction::CorruptByte {
                offset: 1,
                mask: 0x80,
            },
        },
        FaultRule {
            direction: Direction::HostToDevice,
            packet_index: 2,
            action: FaultAction::Duplicate,
        },
    ])
    .unwrap();

    assert_eq!(
        faults.apply(Direction::HostToDevice, &[1, 2]),
        vec![vec![1, 2]]
    );
    assert!(faults.apply(Direction::HostToDevice, &[3, 4]).is_empty());
    assert_eq!(
        faults.apply(Direction::DeviceToHost, &[5, 6]),
        vec![vec![5, 0x86]]
    );
    assert_eq!(
        faults.apply(Direction::HostToDevice, &[7, 8]),
        vec![vec![7, 8], vec![7, 8]]
    );
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn reserve_loopback_address() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

fn connect_until_ready(address: SocketAddr) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match TcpStream::connect(address) {
            Ok(stream) => return stream,
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("simulator did not listen on {address}: {error}"),
        }
    }
}

#[test]
fn tcp_executable_serves_one_client_and_clearly_rejects_a_second() {
    let address = reserve_loopback_address();
    let child = Command::new(env!("CARGO_BIN_EXE_dctp-sim"))
        .args(["--listen", &address.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _guard = ChildGuard(child);
    let mut first = connect_until_ready(address);
    first
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    let hello = Hello {
        client_nonce: 0x1234_5678,
        min_version: 1,
        max_version: 1,
        max_payload: 1_024,
    };
    let request = request(MessageType::Hello, 1, 0, hello.encode().unwrap());
    first.write_all(&encode_frame(&request).unwrap()).unwrap();
    let mut decoder = StreamDecoder::new();
    let mut buffer = [0u8; 1_100];
    let count = first.read(&mut buffer).unwrap();
    let decoded = decoder.push(&buffer[..count]);
    assert_eq!(decoded.len(), 1);
    let response = decoded.into_iter().next().unwrap().unwrap();
    assert_eq!(response.header.message_type, MessageType::HelloAck);
    assert_ne!(HelloAck::decode(&response.payload).unwrap().session_id, 0);

    let mut second = TcpStream::connect(address).unwrap();
    second
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    second.write_all(&encode_frame(&request).unwrap()).unwrap();
    let mut rejection = [0u8; 128];
    let count = second.read(&mut rejection).unwrap();
    let rejection = std::str::from_utf8(&rejection[..count]).unwrap();
    assert!(rejection.contains("only one client"), "{rejection:?}");
}
