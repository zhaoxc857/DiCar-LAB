use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use dctp_protocol::{
    encode_frame, CapabilityFlags, DeviceManifest, ErrorCode, ErrorPayload, Frame, FrameFlags,
    Hello, HelloAck, ManifestAssembler, ManifestChunk, ManifestDone, MessageType, ParamRead,
    ParamState, ParamValue, ParamWrite, StreamDecoder, TelemetryBatch, TelemetryDescriptor,
    TelemetrySubscription, TelemetryType, WireDecode, WireEncode,
};
use dctp_sim::{QueuedFrame, SimConfig, SimDevice};

const MIN_NEGOTIATED_PAYLOAD: u16 = 46;

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

fn only_response(responses: Vec<QueuedFrame>) -> Frame {
    assert_eq!(responses.len(), 1);
    responses.into_iter().next().unwrap().frame
}

fn error_payload(response: &Frame) -> ErrorPayload {
    assert_eq!(response.header.message_type, MessageType::Error);
    ErrorPayload::decode(&response.payload).unwrap()
}

fn hello(device: &mut SimDevice, max_payload: u16, now_ms: u64) -> HelloAck {
    let payload = Hello {
        client_nonce: 0x1234_5678,
        min_version: 1,
        max_version: 1,
        max_payload,
    }
    .encode()
    .unwrap();
    let response = only_response(device.handle(request(MessageType::Hello, 1, 0, payload), now_ms));
    assert_eq!(response.header.message_type, MessageType::HelloAck);
    HelloAck::decode(&response.payload).unwrap()
}

fn subscribe(
    device: &mut SimDevice,
    session_id: u32,
    sample_rate_hz: u16,
    channel_ids: Vec<u32>,
    now_ms: u64,
) {
    let payload = TelemetrySubscription {
        subscription_version: 7,
        sample_rate_hz,
        channel_ids,
    }
    .encode()
    .unwrap();
    let response = only_response(device.handle(
        request(MessageType::TelemetrySubscribe, 2, session_id, payload),
        now_ms,
    ));
    assert_eq!(
        response.header.message_type,
        MessageType::TelemetrySubscribeAck
    );
}

fn batch_at(device: &mut SimDevice, now_ms: u64, channel_count: usize) -> TelemetryBatch {
    let response = only_response(device.tick(now_ms));
    assert_eq!(response.header.message_type, MessageType::TelemetryData);
    TelemetryBatch::decode(&response.payload, channel_count).unwrap()
}

#[test]
fn hello_rejects_a_limit_smaller_than_the_mandatory_fixed_ack() {
    let mut device = SimDevice::new(SimConfig::default());
    let payload = Hello {
        client_nonce: 0x1111_2222,
        min_version: 1,
        max_version: 1,
        max_payload: MIN_NEGOTIATED_PAYLOAD - 1,
    }
    .encode()
    .unwrap();

    let response = only_response(device.handle(request(MessageType::Hello, 9, 0, payload), 0));

    assert_eq!(
        error_payload(&response).error_code,
        ErrorCode::InvalidLength
    );
}

#[test]
fn actual_hello_ack_advertises_parameters_and_telemetry() {
    let mut device = SimDevice::new(SimConfig::default());

    let ack = hello(&mut device, 1_024, 0);

    assert_eq!(
        ack.capabilities.bits(),
        (CapabilityFlags::PARAMETERS | CapabilityFlags::TELEMETRY).bits()
    );
}

#[test]
fn negotiated_limit_bounds_manifest_chunks_and_inbound_session_payloads() {
    let mut device = SimDevice::new(SimConfig::default());
    let ack = hello(&mut device, MIN_NEGOTIATED_PAYLOAD, 0);
    assert_eq!(ack.max_payload, MIN_NEGOTIATED_PAYLOAD);

    let responses = device.handle(
        request(MessageType::ManifestRequest, 2, ack.session_id, Vec::new()),
        1,
    );
    let mut assembler = ManifestAssembler::new();
    let mut done = None;
    for response in responses {
        assert!(
            response.frame.payload.len() <= usize::from(MIN_NEGOTIATED_PAYLOAD),
            "outbound payload exceeded the negotiated limit: {}",
            response.frame.payload.len()
        );
        match response.frame.header.message_type {
            MessageType::ManifestChunk => {
                let chunk = ManifestChunk::decode(&response.frame.payload).unwrap();
                assert!(chunk.data.len() <= usize::from(MIN_NEGOTIATED_PAYLOAD) - 12);
                assembler.push_chunk(chunk).unwrap();
            }
            MessageType::ManifestDone => {
                done = Some(ManifestDone::decode(&response.frame.payload).unwrap())
            }
            other => panic!("unexpected Manifest response: {other:?}"),
        }
    }
    let bytes = assembler.finish(done.unwrap()).unwrap();
    DeviceManifest::decode(&bytes).unwrap();

    let response = only_response(device.handle(
        request(
            MessageType::ParamCommit,
            3,
            ack.session_id,
            vec![0; usize::from(MIN_NEGOTIATED_PAYLOAD) + 1],
        ),
        2,
    ));
    assert_eq!(
        error_payload(&response).error_code,
        ErrorCode::InvalidLength
    );
}

#[test]
fn session_close_exact_retry_replays_the_completed_ack() {
    let mut device = SimDevice::new(SimConfig::default());
    let session_id = device.open_session(17, 0).unwrap();
    let close = request(MessageType::SessionClose, 77, session_id, Vec::new());

    let first = device.handle(close.clone(), 1);
    let retry = device.handle(close, 2);

    assert_eq!(retry, first);
    let changed_sequence = only_response(device.handle(
        request(MessageType::SessionClose, 78, session_id, Vec::new()),
        3,
    ));
    assert_eq!(
        error_payload(&changed_sequence).error_code,
        ErrorCode::InvalidSession
    );
}

#[test]
fn one_hertz_telemetry_fires_only_on_deterministic_deadlines() {
    let mut device = SimDevice::new(SimConfig::default());
    let session_id = device.open_session(18, 0).unwrap();
    subscribe(&mut device, session_id, 1, vec![200], 0);

    assert!(device.tick(999).is_empty());
    let first = batch_at(&mut device, 1_000, 1);
    assert_eq!(first.first_sample_sequence, 0);
    assert_eq!(first.base_timestamp_us, 1_000_000);
    assert!(device.tick(1_000).is_empty());
    assert!(device.tick(1_999).is_empty());
    let second = batch_at(&mut device, 2_000, 1);
    assert_eq!(second.first_sample_sequence, 1);
    assert_eq!(second.base_timestamp_us, 2_000_000);
}

#[test]
fn five_hundred_hertz_telemetry_has_two_ms_resolution_and_bounded_catch_up() {
    let mut device = SimDevice::new(SimConfig::default());
    let session_id = device.open_session(19, 0).unwrap();
    subscribe(&mut device, session_id, 500, vec![200, 201, 202, 203], 0);

    assert!(device.tick(1).is_empty());
    let first = batch_at(&mut device, 2, 4);
    assert_eq!(first.samples.len(), 1);
    assert_eq!(first.base_timestamp_us, 2_000);
    assert!(device.tick(2).is_empty());

    let catch_up = batch_at(&mut device, 1_000, 4);
    assert_eq!(catch_up.samples.len(), 16);
    assert_eq!(catch_up.first_sample_sequence, 484);
    assert_eq!(catch_up.dropped_samples, 483);
    assert_eq!(catch_up.base_timestamp_us, 970_000);
    assert!(device.tick(1_000).is_empty());
}

#[test]
fn telemetry_timestamp_wraps_modulo_u32_microseconds() {
    let start_ms = 4_294_962;
    let mut device = SimDevice::new(SimConfig::default());
    let session_id = device.open_session(20, start_ms).unwrap();
    subscribe(&mut device, session_id, 500, vec![200], start_ms);

    assert_eq!(
        batch_at(&mut device, start_ms + 2, 1).base_timestamp_us,
        4_294_964_000
    );
    assert_eq!(
        batch_at(&mut device, start_ms + 4, 1).base_timestamp_us,
        4_294_966_000
    );
    assert_eq!(
        batch_at(&mut device, start_ms + 6, 1).base_timestamp_us,
        704
    );
}

#[test]
fn telemetry_stop_returns_same_sequence_response_and_clears_the_schedule() {
    let mut device = SimDevice::new(SimConfig::default());
    let session_id = device.open_session(21, 0).unwrap();
    subscribe(&mut device, session_id, 100, vec![200], 0);

    let response = only_response(device.handle(
        request(MessageType::TelemetryStop, 91, session_id, Vec::new()),
        1,
    ));

    assert_eq!(response.header.message_type, MessageType::TelemetryStop);
    assert_eq!(response.header.sequence, 91);
    assert_eq!(response.header.flags, FrameFlags::RESPONSE);
    assert!(response.payload.is_empty());
    assert!(device.tick(100).is_empty());
}

#[test]
fn telemetry_stop_rejects_non_empty_payload_without_clearing_the_schedule() {
    let mut device = SimDevice::new(SimConfig::default());
    let session_id = device.open_session(22, 0).unwrap();
    subscribe(&mut device, session_id, 100, vec![200], 0);

    let response = only_response(device.handle(
        request(MessageType::TelemetryStop, 92, session_id, vec![1]),
        1,
    ));

    assert_eq!(
        error_payload(&response).error_code,
        ErrorCode::InvalidLength
    );
    assert_eq!(batch_at(&mut device, 10, 1).samples.len(), 1);
}

#[test]
fn small_negotiated_limit_bounds_catch_up_telemetry_payload() {
    let mut config = SimConfig::default();
    config.manifest.telemetry = (0..8)
        .map(|index| TelemetryDescriptor {
            channel_id: 900 + index,
            telemetry_type: match index % 4 {
                0 => TelemetryType::F32,
                1 => TelemetryType::I32,
                2 => TelemetryType::U32,
                _ => TelemetryType::Flags32,
            },
            machine_name: format!("custom.{index}"),
            display_name: format!("Custom {index}"),
            group: "Custom".into(),
            unit: String::new(),
        })
        .collect();
    let channel_ids = config
        .manifest
        .telemetry
        .iter()
        .map(|descriptor| descriptor.channel_id)
        .collect();
    let mut device = SimDevice::new(config);
    let ack = hello(&mut device, MIN_NEGOTIATED_PAYLOAD, 0);
    subscribe(&mut device, ack.session_id, 500, channel_ids, 1);

    let response = only_response(device.tick(101));
    assert!(response.payload.len() <= usize::from(MIN_NEGOTIATED_PAYLOAD));
    let batch = TelemetryBatch::decode(&response.payload, 8).unwrap();
    assert_eq!(batch.samples.len(), 1);
    assert_eq!(batch.first_sample_sequence, 49);
    assert_eq!(batch.dropped_samples, 49);
    assert!(device.tick(101).is_empty());
}

#[test]
fn custom_manifest_telemetry_emits_deterministic_type_correct_raw_slots() {
    let mut config = SimConfig::default();
    config.manifest.telemetry = vec![
        telemetry_descriptor(900, TelemetryType::F32),
        telemetry_descriptor(901, TelemetryType::I32),
        telemetry_descriptor(902, TelemetryType::U32),
        telemetry_descriptor(903, TelemetryType::Flags32),
    ];
    let mut device = SimDevice::new(config);
    let session_id = device.open_session(23, 0).unwrap();
    subscribe(&mut device, session_id, 100, vec![900, 901, 902, 903], 0);

    let batch = batch_at(&mut device, 10, 4);
    let values = &batch.samples[0].values;
    assert_eq!(f32::from_bits(values[0]), 1.5);
    assert_eq!(values[1] as i32, -4);
    assert_eq!(values[2], 8);
    assert_eq!(values[3], 0b101);
}

fn telemetry_descriptor(channel_id: u32, telemetry_type: TelemetryType) -> TelemetryDescriptor {
    TelemetryDescriptor {
        channel_id,
        telemetry_type,
        machine_name: format!("custom.{channel_id}"),
        display_name: format!("Custom {channel_id}"),
        group: "Custom".into(),
        unit: String::new(),
    }
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
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("simulator did not listen on {address}: {error}"),
        }
    }
}

fn send_and_read_frame(stream: &mut TcpStream, request: &Frame) -> Frame {
    stream.write_all(&encode_frame(request).unwrap()).unwrap();
    let mut decoder = StreamDecoder::new();
    let mut buffer = [0u8; 1_100];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert_ne!(count, 0, "server closed before responding");
        if let Some(frame) = decoder.push(&buffer[..count]).into_iter().flatten().next() {
            return frame;
        }
    }
}

fn connect_after_prior_client_exit(
    address: SocketAddr,
    stale_request: &Frame,
) -> (TcpStream, Frame) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(250)))
            .unwrap();
        stream
            .write_all(&encode_frame(stale_request).unwrap())
            .unwrap();
        let mut decoder = StreamDecoder::new();
        let mut buffer = [0u8; 1_100];
        match stream.read(&mut buffer) {
            Ok(count) if count > 0 && buffer[..count].starts_with(b"DCTP simulator rejected") => {}
            Ok(count) if count > 0 => {
                if let Some(frame) = decoder.push(&buffer[..count]).into_iter().flatten().next() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    return (stream, frame);
                }
            }
            Ok(_) | Err(_) => {}
        }
        assert!(
            Instant::now() < deadline,
            "simulator did not release the prior client slot"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn sequential_tcp_client_cannot_use_the_disconnected_clients_session() {
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

    let first_hello = request(
        MessageType::Hello,
        1,
        0,
        Hello {
            client_nonce: 0xAAAA,
            min_version: 1,
            max_version: 1,
            max_payload: 1_024,
        }
        .encode()
        .unwrap(),
    );
    let first_ack = send_and_read_frame(&mut first, &first_hello);
    let old_session = HelloAck::decode(&first_ack.payload).unwrap().session_id;
    let accepted_write = request(
        MessageType::ParamWrite,
        2,
        old_session,
        ParamWrite {
            param_id: 1,
            expected_revision: 0,
            value: ParamValue::F32(2.0),
        }
        .encode()
        .unwrap(),
    );
    let write_ack = send_and_read_frame(&mut first, &accepted_write);
    assert_eq!(write_ack.header.message_type, MessageType::ParamWriteAck);
    first.shutdown(Shutdown::Both).unwrap();
    drop(first);

    let stale_write = request(
        MessageType::ParamWrite,
        3,
        old_session,
        ParamWrite {
            param_id: 1,
            expected_revision: 1,
            value: ParamValue::F32(3.0),
        }
        .encode()
        .unwrap(),
    );
    let (mut second, stale_response) = connect_after_prior_client_exit(address, &stale_write);
    assert_eq!(
        error_payload(&stale_response).error_code,
        ErrorCode::InvalidSession
    );

    let second_hello = request(
        MessageType::Hello,
        4,
        0,
        Hello {
            client_nonce: 0xBBBB,
            min_version: 1,
            max_version: 1,
            max_payload: 1_024,
        }
        .encode()
        .unwrap(),
    );
    let second_ack = send_and_read_frame(&mut second, &second_hello);
    let new_session = HelloAck::decode(&second_ack.payload).unwrap().session_id;
    assert_ne!(new_session, old_session);
    let read = request(
        MessageType::ParamRead,
        5,
        new_session,
        ParamRead { param_id: 1 }.encode().unwrap(),
    );
    let state = ParamState::decode(&send_and_read_frame(&mut second, &read).payload).unwrap();
    assert_eq!(state.value, ParamValue::F32(2.0));
    assert_eq!(state.revision, 1);
}
