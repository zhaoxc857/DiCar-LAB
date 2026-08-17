//! 用 Rust 权威协议栈驱动 C 设备的行为交叉验证。
//!
//! 每个测试构造真实的 DCTP 请求字节流喂给 C 实现，再用 dctp-protocol
//! 解码响应并断言语义与 dctp-sim 的参考行为一致。

use dctp_device_c::{FlashTransition, TestDevice};
use dctp_protocol::{
    canonical_parameter_crc32, encode_frame, BootloaderProtocol, CapabilityFlags, DeviceManifest,
    ErrorCode, ErrorPayload, FirmwareTargetId, Frame, FrameFlags, Hello, HelloAck, LogMessage,
    LogSeverity, ManifestAssembler, ManifestChunk, ManifestDone, MessageType, ParamCommit,
    ParamCommitAck, ParamCommitEntry, ParamState, ParamValue, ParamWrite, ParamWriteAck,
    PrepareFlash, PrepareFlashAck, StreamDecoder, TelemetryBatch, TelemetrySubscription,
    WireDecode, WireEncode,
};
use dctp_sim::SimConfig;

const ACK: FrameFlags = FrameFlags::ACK_REQUIRED;

fn reliable_request(
    message_type: MessageType,
    sequence: u16,
    session_id: u32,
    payload: Vec<u8>,
) -> Vec<u8> {
    encode_frame(&Frame::new(message_type, ACK, sequence, session_id, payload).unwrap()).unwrap()
}

fn decode_all(bytes: &[u8]) -> Vec<Frame> {
    StreamDecoder::new()
        .push(bytes)
        .into_iter()
        .map(|frame| frame.expect("device emitted a valid frame"))
        .collect()
}

fn take_one(device: &mut TestDevice) -> Frame {
    let frames = decode_all(&device.take_tx());
    assert_eq!(frames.len(), 1, "expected exactly one response frame");
    frames.into_iter().next().unwrap()
}

fn hello_payload(nonce: u32, max_payload: u16) -> Vec<u8> {
    Hello {
        client_nonce: nonce,
        min_version: 1,
        max_version: 1,
        max_payload,
    }
    .encode()
    .unwrap()
}

fn handshake(device: &mut TestDevice, now_ms: u32) -> HelloAck {
    device.rx(
        &reliable_request(
            MessageType::Hello,
            0x0001,
            0,
            hello_payload(0x1020_3040, 1024),
        ),
        now_ms,
    );
    let frame = take_one(device);
    assert_eq!(frame.header.message_type, MessageType::HelloAck);
    assert_eq!(frame.header.flags.bits(), FrameFlags::RESPONSE.bits());
    HelloAck::decode(&frame.payload).unwrap()
}

fn expect_error(frame: &Frame, original: MessageType, code: ErrorCode) -> ErrorPayload {
    assert_eq!(frame.header.message_type, MessageType::Error);
    assert_eq!(
        frame.header.flags.bits(),
        FrameFlags::RESPONSE.bits() | FrameFlags::ERROR.bits()
    );
    let payload = ErrorPayload::decode(&frame.payload).unwrap();
    assert_eq!(payload.original_message_type, original);
    assert_eq!(payload.error_code, code);
    payload
}

#[test]
fn handshake_reports_identity_capabilities_and_manifest_crc() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    assert_ne!(ack.session_id, 0);
    assert_eq!(ack.device_id, *b"DCTP-SIM-DEVICE!");
    assert_eq!(ack.boot_count, 1);
    assert_eq!(
        (ack.firmware_major, ack.firmware_minor, ack.firmware_patch),
        (1, 0, 0)
    );
    assert_eq!(ack.max_payload, 1024);
    assert_eq!(ack.manifest_crc32, device.manifest_crc32());
    let expected_crc = SimConfig::default().manifest.manifest_crc32().unwrap();
    assert_eq!(
        ack.manifest_crc32, expected_crc,
        "C manifest CRC differs from Rust reference"
    );
    assert!(device.session_active());
}

#[test]
fn hello_replay_returns_identical_ack_and_new_nonce_rotates_session() {
    let mut device = TestDevice::new(false, false);
    let request = reliable_request(MessageType::Hello, 7, 0, hello_payload(42, 1024));
    device.rx(&request, 0);
    let first = take_one(&mut device);
    device.rx(&request, 10);
    let replayed = take_one(&mut device);
    assert_eq!(
        first, replayed,
        "identical HELLO must replay the identical ACK"
    );

    device.rx(
        &reliable_request(MessageType::Hello, 8, 0, hello_payload(43, 1024)),
        20,
    );
    let second = HelloAck::decode(&take_one(&mut device).payload).unwrap();
    let first = HelloAck::decode(&first.payload).unwrap();
    assert_ne!(first.session_id, second.session_id);

    device.rx(
        &reliable_request(
            MessageType::Heartbeat,
            9,
            first.session_id,
            5u32.to_le_bytes().to_vec(),
        ),
        30,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::Heartbeat,
        ErrorCode::InvalidSession,
    );
}

#[test]
fn heartbeat_echoes_payload_and_wrong_session_is_rejected() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    device.rx(
        &reliable_request(
            MessageType::Heartbeat,
            2,
            ack.session_id,
            0xDEAD_BEEFu32.to_le_bytes().to_vec(),
        ),
        100,
    );
    let frame = take_one(&mut device);
    assert_eq!(frame.header.message_type, MessageType::HeartbeatAck);
    assert_eq!(frame.payload, 0xDEAD_BEEFu32.to_le_bytes().to_vec());

    device.rx(
        &reliable_request(
            MessageType::Heartbeat,
            3,
            ack.session_id ^ 1,
            5u32.to_le_bytes().to_vec(),
        ),
        200,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::Heartbeat,
        ErrorCode::InvalidSession,
    );
}

#[test]
fn manifest_chunks_reassemble_into_the_rust_reference_manifest() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    device.rx(
        &reliable_request(MessageType::ManifestRequest, 2, ack.session_id, Vec::new()),
        10,
    );
    let frames = decode_all(&device.take_tx());
    assert!(frames.len() >= 2);

    let mut assembler = ManifestAssembler::new();
    let (done, chunks) = frames.split_last().unwrap();
    for chunk in chunks {
        assert_eq!(chunk.header.message_type, MessageType::ManifestChunk);
        assert_eq!(
            chunk.header.flags.bits(),
            FrameFlags::RESPONSE.bits() | FrameFlags::MORE_FRAGMENTS.bits()
        );
        assembler
            .push_chunk(ManifestChunk::decode(&chunk.payload).unwrap())
            .unwrap();
    }
    assert_eq!(done.header.message_type, MessageType::ManifestDone);
    let bytes = assembler
        .finish(ManifestDone::decode(&done.payload).unwrap())
        .unwrap();
    let manifest = DeviceManifest::decode(&bytes).unwrap();
    assert_eq!(
        manifest,
        SimConfig::default().manifest,
        "C manifest differs from Rust reference"
    );
}

#[test]
fn negotiated_payload_bounds_manifest_chunks_and_rejects_long_requests() {
    let mut device = TestDevice::new(false, false);
    device.rx(
        &reliable_request(MessageType::Hello, 1, 0, hello_payload(9, 100)),
        0,
    );
    let ack = HelloAck::decode(&take_one(&mut device).payload).unwrap();
    assert_eq!(ack.max_payload, 100);

    device.rx(
        &reliable_request(MessageType::ManifestRequest, 2, ack.session_id, Vec::new()),
        10,
    );
    let frames = decode_all(&device.take_tx());
    for frame in &frames {
        assert!(
            frame.payload.len() <= 100,
            "chunk exceeds the negotiated payload limit"
        );
    }

    let oversized = vec![0u8; 101];
    device.rx(
        &reliable_request(MessageType::TelemetryStop, 3, ack.session_id, oversized),
        20,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::TelemetryStop,
        ErrorCode::InvalidLength,
    );
}

#[test]
fn param_read_reports_ram_flash_and_non_persistent_state() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            2,
            ack.session_id,
            1u32.to_le_bytes().to_vec(),
        ),
        10,
    );
    let state = ParamState::decode(&take_one(&mut device).payload).unwrap();
    assert_eq!(state.param_id, 1);
    assert_eq!(state.revision, 0);
    assert!(state.value.wire_eq(&ParamValue::F32(1.2)));
    assert!(state
        .persisted_value
        .unwrap()
        .wire_eq(&ParamValue::F32(1.2)));

    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            3,
            ack.session_id,
            103u32.to_le_bytes().to_vec(),
        ),
        20,
    );
    let readonly = ParamState::decode(&take_one(&mut device).payload).unwrap();
    assert!(
        readonly.persisted_value.is_none(),
        "non-persistent parameter must not fake a flash value"
    );

    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            4,
            ack.session_id,
            999u32.to_le_bytes().to_vec(),
        ),
        30,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamRead,
        ErrorCode::InvalidParamId,
    );
}

fn write_request(
    sequence: u16,
    session_id: u32,
    param_id: u32,
    revision: u32,
    value: ParamValue,
) -> Vec<u8> {
    reliable_request(
        MessageType::ParamWrite,
        sequence,
        session_id,
        ParamWrite {
            param_id,
            expected_revision: revision,
            value,
        }
        .encode()
        .unwrap(),
    )
}

#[test]
fn param_write_validation_chain_matches_the_simulator() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;

    device.rx(&write_request(2, session, 999, 0, ParamValue::F32(1.0)), 10);
    expect_error(
        &take_one(&mut device),
        MessageType::ParamWrite,
        ErrorCode::InvalidParamId,
    );

    device.rx(&write_request(3, session, 1, 0, ParamValue::U32(5)), 20);
    expect_error(
        &take_one(&mut device),
        MessageType::ParamWrite,
        ErrorCode::TypeMismatch,
    );

    device.rx(&write_request(4, session, 103, 0, ParamValue::U32(5)), 30);
    expect_error(
        &take_one(&mut device),
        MessageType::ParamWrite,
        ErrorCode::ReadOnly,
    );

    device.rx(
        &write_request(5, session, 1, 0, ParamValue::F32(2000.0)),
        40,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamWrite,
        ErrorCode::OutOfRange,
    );

    device.rx(&write_request(6, session, 102, 0, ParamValue::Enum(3)), 50);
    expect_error(
        &take_one(&mut device),
        MessageType::ParamWrite,
        ErrorCode::OutOfRange,
    );

    device.rx(&write_request(7, session, 1, 0, ParamValue::F32(2.5)), 60);
    let accepted = ParamWriteAck::decode(&take_one(&mut device).payload).unwrap();
    assert!(accepted.value.wire_eq(&ParamValue::F32(2.5)));
    assert_eq!(accepted.new_revision, 1);

    device.rx(&write_request(8, session, 1, 0, ParamValue::F32(3.5)), 70);
    let conflict = expect_error(
        &take_one(&mut device),
        MessageType::ParamWrite,
        ErrorCode::RevisionConflict,
    );
    let context_bytes: Vec<u8> = (0..conflict.context.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&conflict.context[index..index + 2], 16).unwrap())
        .collect();
    let current = ParamWriteAck::decode(&context_bytes).unwrap();
    assert!(current.value.wire_eq(&ParamValue::F32(2.5)));
    assert_eq!(current.new_revision, 1);
}

#[test]
fn reliable_write_retry_replays_the_cached_ack_without_side_effects() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    let request = write_request(2, ack.session_id, 1, 0, ParamValue::F32(2.5));
    device.rx(&request, 10);
    let first = take_one(&mut device);
    device.rx(&request, 20);
    let replayed = take_one(&mut device);
    assert_eq!(first, replayed);

    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            3,
            ack.session_id,
            1u32.to_le_bytes().to_vec(),
        ),
        30,
    );
    let state = ParamState::decode(&take_one(&mut device).payload).unwrap();
    assert_eq!(state.revision, 1, "retry must not apply the write twice");
}

fn commit_request(
    sequence: u16,
    session_id: u32,
    entries: Vec<ParamCommitEntry>,
    crc: u32,
) -> Vec<u8> {
    reliable_request(
        MessageType::ParamCommit,
        sequence,
        session_id,
        ParamCommit {
            entries,
            canonical_crc32: crc,
        }
        .encode()
        .unwrap(),
    )
}

#[test]
fn commit_persists_atomically_and_replays_idempotently() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;
    device.rx(&write_request(2, session, 1, 0, ParamValue::F32(2.5)), 10);
    take_one(&mut device);

    let crc = canonical_parameter_crc32(&[(1, ParamValue::F32(2.5))]).unwrap();
    let request = commit_request(
        3,
        session,
        vec![ParamCommitEntry {
            param_id: 1,
            revision: 1,
        }],
        crc,
    );
    device.rx(&request, 20);
    let commit_ack = ParamCommitAck::decode(&take_one(&mut device).payload).unwrap();
    assert_eq!(commit_ack.canonical_crc32, crc);
    assert_eq!(commit_ack.storage_generation, 1);
    assert_eq!(device.persist_calls(), 1);
    assert_eq!(device.storage_generation(), 1);

    device.rx(&request, 30);
    let replayed = ParamCommitAck::decode(&take_one(&mut device).payload).unwrap();
    assert_eq!(replayed, commit_ack);
    assert_eq!(device.persist_calls(), 1, "replay must not persist twice");

    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            4,
            session,
            1u32.to_le_bytes().to_vec(),
        ),
        40,
    );
    let state = ParamState::decode(&take_one(&mut device).payload).unwrap();
    assert!(state
        .persisted_value
        .unwrap()
        .wire_eq(&ParamValue::F32(2.5)));
}

#[test]
fn commit_rejects_stale_revisions_bad_crc_and_unsorted_entries() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;
    device.rx(&write_request(2, session, 1, 0, ParamValue::F32(2.5)), 10);
    take_one(&mut device);
    let crc = canonical_parameter_crc32(&[(1, ParamValue::F32(2.5))]).unwrap();

    device.rx(
        &commit_request(
            3,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 0,
            }],
            crc,
        ),
        20,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamCommit,
        ErrorCode::RevisionConflict,
    );

    device.rx(
        &commit_request(
            4,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 1,
            }],
            crc ^ 1,
        ),
        30,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamCommit,
        ErrorCode::InvalidLength,
    );

    device.rx(
        &commit_request(
            5,
            session,
            vec![ParamCommitEntry {
                param_id: 103,
                revision: 0,
            }],
            0,
        ),
        40,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamCommit,
        ErrorCode::ReadOnly,
    );

    let mut unsorted = Vec::new();
    unsorted.extend_from_slice(&2u16.to_le_bytes());
    for (param_id, revision) in [(100u32, 0u32), (1u32, 1u32)] {
        unsorted.extend_from_slice(&param_id.to_le_bytes());
        unsorted.extend_from_slice(&revision.to_le_bytes());
    }
    unsorted.extend_from_slice(&0u32.to_le_bytes());
    device.rx(
        &reliable_request(MessageType::ParamCommit, 6, session, unsorted),
        50,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamCommit,
        ErrorCode::InvalidLength,
    );

    assert_eq!(device.persist_calls(), 0);
    assert_eq!(device.storage_generation(), 0);
}

#[test]
fn commit_failure_paths_keep_the_previous_flash_state() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;
    device.rx(&write_request(2, session, 1, 0, ParamValue::F32(2.5)), 10);
    take_one(&mut device);
    let crc = canonical_parameter_crc32(&[(1, ParamValue::F32(2.5))]).unwrap();

    device.set_persist_result(1);
    device.rx(
        &commit_request(
            3,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 1,
            }],
            crc,
        ),
        20,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamCommit,
        ErrorCode::StorageFailed,
    );

    device.set_persist_result(2);
    device.rx(
        &commit_request(
            4,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 1,
            }],
            crc,
        ),
        30,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::ParamCommit,
        ErrorCode::VerifyFailed,
    );

    assert_eq!(device.storage_generation(), 0);
    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            5,
            session,
            1u32.to_le_bytes().to_vec(),
        ),
        40,
    );
    let state = ParamState::decode(&take_one(&mut device).payload).unwrap();
    assert!(
        state
            .persisted_value
            .unwrap()
            .wire_eq(&ParamValue::F32(1.2)),
        "failed commit must keep the previous flash value"
    );

    let mut no_storage = TestDevice::new(false, false);
    let ack = handshake(&mut no_storage, 0);
    no_storage.rx(
        &commit_request(
            2,
            ack.session_id,
            Vec::new(),
            canonical_parameter_crc32(&[]).unwrap(),
        ),
        10,
    );
    expect_error(
        &take_one(&mut no_storage),
        MessageType::ParamCommit,
        ErrorCode::StorageFailed,
    );
}

#[test]
fn storage_blob_round_trips_into_a_fresh_device() {
    let mut device = TestDevice::new(true, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;
    device.rx(&write_request(2, session, 1, 0, ParamValue::F32(2.5)), 10);
    take_one(&mut device);
    let crc = canonical_parameter_crc32(&[(1, ParamValue::F32(2.5))]).unwrap();
    device.rx(
        &commit_request(
            3,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 1,
            }],
            crc,
        ),
        20,
    );
    take_one(&mut device);
    let blob = device.last_blob();
    assert!(!blob.is_empty());

    let mut restored = TestDevice::new(true, false);
    assert!(restored.storage_apply(Some(&blob), None));
    assert_eq!(restored.storage_generation(), 1);
    assert_eq!(restored.get_value_bits(1), Some((3, 2.5f32.to_bits())));

    let mut corrupted = blob.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xFF;
    let mut rejecting = TestDevice::new(true, false);
    assert!(!rejecting.storage_apply(Some(&corrupted), None));
    assert_eq!(rejecting.get_value_bits(1), Some((3, 1.2f32.to_bits())));

    let mut newer = TestDevice::new(true, false);
    let ack = handshake(&mut newer, 0);
    let session = ack.session_id;
    newer.rx(&write_request(2, session, 1, 0, ParamValue::F32(7.5)), 10);
    take_one(&mut newer);
    let crc = canonical_parameter_crc32(&[(1, ParamValue::F32(7.5))]).unwrap();
    newer.rx(
        &commit_request(
            3,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 1,
            }],
            crc,
        ),
        20,
    );
    take_one(&mut newer);
    newer.rx(&write_request(4, session, 1, 1, ParamValue::F32(9.5)), 30);
    take_one(&mut newer);
    let crc = canonical_parameter_crc32(&[(1, ParamValue::F32(9.5))]).unwrap();
    newer.rx(
        &commit_request(
            5,
            session,
            vec![ParamCommitEntry {
                param_id: 1,
                revision: 2,
            }],
            crc,
        ),
        40,
    );
    take_one(&mut newer);
    let newer_blob = newer.last_blob();

    let mut both = TestDevice::new(true, false);
    assert!(both.storage_apply(Some(&blob), Some(&newer_blob)));
    assert_eq!(
        both.storage_generation(),
        2,
        "the newer generation slot must win"
    );
    assert_eq!(both.get_value_bits(1), Some((3, 9.5f32.to_bits())));
}

#[test]
fn firmware_side_set_value_bumps_the_revision() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    assert!(device.set_value_f32(1, 3.25));
    assert!(
        !device.set_value_f32(1, 2000.0),
        "out-of-range firmware writes are rejected"
    );
    device.rx(
        &reliable_request(
            MessageType::ParamRead,
            2,
            ack.session_id,
            1u32.to_le_bytes().to_vec(),
        ),
        10,
    );
    let state = ParamState::decode(&take_one(&mut device).payload).unwrap();
    assert_eq!(state.revision, 1);
    assert!(state.value.wire_eq(&ParamValue::F32(3.25)));
}

fn subscribe_request(sequence: u16, session_id: u32, channel_ids: Vec<u32>, rate: u16) -> Vec<u8> {
    reliable_request(
        MessageType::TelemetrySubscribe,
        sequence,
        session_id,
        TelemetrySubscription {
            subscription_version: 1,
            sample_rate_hz: rate,
            channel_ids,
        }
        .encode()
        .unwrap(),
    )
}

#[test]
fn telemetry_batches_pace_sequence_and_report_gaps() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 10);
    let session = ack.session_id;
    device.rx(
        &subscribe_request(2, session, vec![200, 201, 202, 203], 500),
        10,
    );
    let frame = take_one(&mut device);
    assert_eq!(
        frame.header.message_type,
        MessageType::TelemetrySubscribeAck
    );

    device.poll(10, 10_000);
    assert!(
        device.take_tx().is_empty(),
        "first poll only seeds the pacing clock"
    );

    device.poll(12, 12_000);
    let frame = take_one(&mut device);
    assert_eq!(frame.header.message_type, MessageType::TelemetryData);
    let batch = TelemetryBatch::decode(&frame.payload, 4).unwrap();
    assert_eq!(batch.subscription_version, 1);
    assert_eq!(batch.first_sample_sequence, 0);
    assert_eq!(batch.dropped_samples, 0);
    assert_eq!(batch.base_timestamp_us, 12_000);
    assert_eq!(batch.samples.len(), 1);
    assert_eq!(batch.samples[0].dt_us, 0);
    assert_eq!(
        batch.samples[0].values,
        vec![1, 2, 3, 4],
        "values follow the subscription order"
    );

    device.poll(80, 80_000);
    let frame = take_one(&mut device);
    let batch = TelemetryBatch::decode(&frame.payload, 4).unwrap();
    assert_eq!(
        batch.samples.len(),
        16,
        "catch-up batches are capped at 16 samples"
    );
    assert_eq!(batch.dropped_samples, 18);
    assert_eq!(batch.first_sample_sequence, 19);
    assert_eq!(batch.base_timestamp_us, 50_000);
    assert_eq!(batch.samples[1].dt_us, 2_000);

    device.rx(
        &reliable_request(MessageType::TelemetryStop, 3, session, Vec::new()),
        90,
    );
    let frame = take_one(&mut device);
    assert_eq!(frame.header.message_type, MessageType::TelemetryStop);
    device.poll(120, 120_000);
    assert!(device.take_tx().is_empty(), "stop must halt telemetry");
}

#[test]
fn telemetry_subscribe_validation_and_tx_budget_drops() {
    let mut device = TestDevice::new(false, true);
    device.set_tx_free(4096);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;

    device.rx(&subscribe_request(2, session, vec![200, 999], 500), 0);
    expect_error(
        &take_one(&mut device),
        MessageType::TelemetrySubscribe,
        ErrorCode::InvalidParamId,
    );

    device.rx(&subscribe_request(3, session, vec![200, 201], 500), 0);
    take_one(&mut device);
    device.poll(0, 0);

    device.set_tx_free(0);
    device.poll(2, 2_000);
    assert!(
        device.take_tx().is_empty(),
        "an exhausted TX budget must drop the whole batch"
    );

    device.set_tx_free(4096);
    device.poll(4, 4_000);
    let frame = take_one(&mut device);
    let batch = TelemetryBatch::decode(&frame.payload, 2).unwrap();
    assert!(
        batch.dropped_samples >= 1,
        "dropped batches surface in the next gap counter"
    );
}

#[test]
fn session_expires_after_exactly_three_seconds_of_silence() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;

    device.rx(
        &reliable_request(
            MessageType::Heartbeat,
            2,
            session,
            1u32.to_le_bytes().to_vec(),
        ),
        2_999,
    );
    assert_eq!(
        take_one(&mut device).header.message_type,
        MessageType::HeartbeatAck
    );

    device.rx(
        &reliable_request(
            MessageType::Heartbeat,
            3,
            session,
            2u32.to_le_bytes().to_vec(),
        ),
        5_999,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::Heartbeat,
        ErrorCode::InvalidSession,
    );
    assert!(!device.session_active());
}

#[test]
fn session_close_responds_replays_and_invalidates_the_session() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;
    let close = reliable_request(MessageType::SessionClose, 2, session, Vec::new());
    device.rx(&close, 10);
    let response = take_one(&mut device);
    assert_eq!(response.header.message_type, MessageType::SessionClose);
    assert!(!device.session_active());

    device.rx(&close, 20);
    assert_eq!(
        take_one(&mut device),
        response,
        "duplicate close must replay the same response"
    );

    device.rx(
        &reliable_request(
            MessageType::Heartbeat,
            3,
            session,
            1u32.to_le_bytes().to_vec(),
        ),
        30,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::Heartbeat,
        ErrorCode::InvalidSession,
    );
}

#[test]
fn unsupported_requests_and_bad_hello_are_rejected() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    device.rx(
        &reliable_request(MessageType::PrepareFlash, 2, ack.session_id, Vec::new()),
        10,
    );
    expect_error(
        &take_one(&mut device),
        MessageType::PrepareFlash,
        ErrorCode::UnknownMessage,
    );

    let mut fresh = TestDevice::new(false, false);
    fresh.rx(
        &reliable_request(MessageType::Hello, 1, 5, hello_payload(1, 1024)),
        0,
    );
    expect_error(
        &take_one(&mut fresh),
        MessageType::Hello,
        ErrorCode::InvalidSession,
    );

    let unsupported = Hello {
        client_nonce: 1,
        min_version: 2,
        max_version: 2,
        max_payload: 1024,
    };
    fresh.rx(
        &reliable_request(MessageType::Hello, 2, 0, unsupported.encode().unwrap()),
        10,
    );
    expect_error(
        &take_one(&mut fresh),
        MessageType::Hello,
        ErrorCode::UnsupportedVersion,
    );

    fresh.rx(
        &reliable_request(MessageType::Hello, 3, 0, hello_payload(1, 10)),
        20,
    );
    expect_error(
        &take_one(&mut fresh),
        MessageType::Hello,
        ErrorCode::InvalidLength,
    );
}

#[test]
fn prepare_flash_is_advertised_acked_once_and_exposes_a_one_shot_transition() {
    let mut device = TestDevice::new_with_flash();
    let hello = handshake(&mut device, 0);
    assert!(hello.capabilities.contains(CapabilityFlags::PREPARE_FLASH));
    let prepare = PrepareFlash {
        operation_id: [0x42; 16],
        target_id: FirmwareTargetId::LCKFB_TMX_MSPM0G3507,
        firmware_version: [2, 3, 4],
        image_len: 0x1_2345,
        image_sha256: [0xA6; 32],
    };
    let request = reliable_request(
        MessageType::PrepareFlash,
        2,
        hello.session_id,
        prepare.encode().unwrap(),
    );

    device.rx(&request, 10);
    let first = take_one(&mut device);
    let ack = PrepareFlashAck::decode(&first.payload).unwrap();
    assert_eq!(first.header.message_type, MessageType::PrepareFlashAck);
    assert_eq!(ack.operation_id, prepare.operation_id);
    assert_eq!(
        ack.bootloader_protocol,
        BootloaderProtocol::TI_MSPM0_ROM_BSL_UART
    );
    assert_eq!(ack.entry_delay_ms, 250);
    assert_eq!(ack.initial_baud, 9_600);
    assert_eq!(device.prepare_flash_calls(), 1);
    assert_eq!(
        device.take_flash_transition(),
        Some(FlashTransition {
            operation_id: prepare.operation_id,
            bootloader_protocol: BootloaderProtocol::TI_MSPM0_ROM_BSL_UART.bits(),
            entry_delay_ms: 250,
            initial_baud: 9_600,
        })
    );
    assert_eq!(device.take_flash_transition(), None);

    device.rx(&request, 20);
    assert_eq!(take_one(&mut device), first);
    assert_eq!(device.prepare_flash_calls(), 1);
    assert_eq!(device.take_flash_transition(), None);
}

#[test]
fn corrupted_input_is_dropped_and_the_stream_resynchronizes() {
    let mut device = TestDevice::new(false, false);
    let ack = handshake(&mut device, 0);
    let session = ack.session_id;

    device.rx(&[0x11, 0x22, 0x33, 0x00], 10);
    let mut corrupted = reliable_request(
        MessageType::Heartbeat,
        2,
        session,
        1u32.to_le_bytes().to_vec(),
    );
    corrupted[5] ^= 0xFF;
    device.rx(&corrupted, 20);
    assert!(
        device.take_tx().is_empty(),
        "corrupted frames must not produce responses"
    );

    let request = reliable_request(
        MessageType::Heartbeat,
        3,
        session,
        2u32.to_le_bytes().to_vec(),
    );
    let (first_half, second_half) = request.split_at(5);
    device.rx(first_half, 30);
    device.rx(second_half, 31);
    assert_eq!(
        take_one(&mut device).header.message_type,
        MessageType::HeartbeatAck
    );
}

#[test]
fn structured_logs_require_a_session_and_carry_text() {
    let mut device = TestDevice::new(false, false);
    assert!(!device.log(2, 7, 123, "before session"));

    handshake(&mut device, 0);
    assert!(device.log(4, 7, 456, "编码器丢脉冲"));
    let frame = take_one(&mut device);
    assert_eq!(frame.header.message_type, MessageType::LogMessage);
    let log = LogMessage::decode(&frame.payload).unwrap();
    assert_eq!(log.severity, LogSeverity::Error);
    assert_eq!(log.module_id, 7);
    assert_eq!(log.timestamp_us, 456);
    assert_eq!(log.text, "编码器丢脉冲");
}
