use dctp_protocol::{
    BootloaderProtocol, CapabilityFlags, ErrorCode, ErrorPayload, FirmwareTargetId, Heartbeat,
    Hello, HelloAck, ManifestAssembler, ManifestChunk, ManifestDone, MessageType, PrepareFlash,
    PrepareFlashAck, ProtocolError, WireDecode, WireEncode, WireReader, WireWriter,
};

#[test]
fn hello_payload_round_trips() {
    let hello = Hello {
        client_nonce: 0x1122_3344,
        min_version: 1,
        max_version: 1,
        max_payload: 1024,
    };

    let bytes = hello.encode().unwrap();

    assert_eq!(Hello::decode(&bytes).unwrap(), hello);
}

#[test]
fn reader_rejects_trailing_and_truncated_data() {
    assert!(matches!(
        Hello::decode(&[1, 2]),
        Err(ProtocolError::Truncated)
    ));

    let hello = Hello {
        client_nonce: 1,
        min_version: 1,
        max_version: 1,
        max_payload: 1024,
    };
    let mut bytes = hello.encode().unwrap();
    bytes.push(9);

    assert!(matches!(
        Hello::decode(&bytes),
        Err(ProtocolError::InvalidLength)
    ));
}

#[test]
fn hello_ack_and_heartbeat_round_trip() {
    let ack = HelloAck {
        session_id: 0x1122_3344,
        device_id: [0xAB; 16],
        boot_count: 7,
        firmware_major: 1,
        firmware_minor: 2,
        firmware_patch: 3,
        sdk_major: 4,
        sdk_minor: 5,
        sdk_patch: 6,
        capabilities: CapabilityFlags::PARAMETERS | CapabilityFlags::TELEMETRY,
        manifest_crc32: 0xDEAD_BEEF,
        max_payload: 1024,
    };
    let heartbeat = Heartbeat {
        monotonic_ms: 123_456,
    };

    assert_eq!(HelloAck::decode(&ack.encode().unwrap()).unwrap(), ack);
    assert_eq!(
        Heartbeat::decode(&heartbeat.encode().unwrap()).unwrap(),
        heartbeat
    );
}

#[test]
fn capability_flags_report_individual_supported_features() {
    let capabilities = CapabilityFlags::PARAMETERS | CapabilityFlags::PREPARE_FLASH;

    assert!(capabilities.contains(CapabilityFlags::PARAMETERS));
    assert!(capabilities.contains(CapabilityFlags::PREPARE_FLASH));
    assert!(!capabilities.contains(CapabilityFlags::TELEMETRY));
}

#[test]
fn error_payload_preserves_unknown_error_codes() {
    let payload = ErrorPayload {
        original_message_type: MessageType::ParamWrite,
        original_sequence: 12,
        error_code: ErrorCode::Unknown(99),
        context: "newer-device-detail".to_owned(),
    };

    assert_eq!(
        ErrorPayload::decode(&payload.encode().unwrap()).unwrap(),
        payload
    );
}

#[test]
fn error_payload_rejects_context_over_64_utf8_bytes() {
    let payload = ErrorPayload {
        original_message_type: MessageType::ParamWrite,
        original_sequence: 12,
        error_code: ErrorCode::InvalidLength,
        context: "x".repeat(65),
    };

    assert!(matches!(
        payload.encode(),
        Err(ProtocolError::StringTooLong)
    ));
}

#[test]
fn writer_rejects_oversized_utf8_before_writing() {
    let mut writer = WireWriter::new();

    assert!(matches!(
        writer.put_utf8_u8_len(&"x".repeat(65), 64),
        Err(ProtocolError::StringTooLong)
    ));
    assert!(writer.into_inner().is_empty());
}

#[test]
fn manifest_assembly_requires_contiguous_validated_chunks() {
    let bytes = b"canonical manifest".to_vec();
    let crc = dctp_protocol::crc32_iso_hdlc(&bytes);
    let mut assembler = ManifestAssembler::new();

    assert!(matches!(
        assembler.push_chunk(ManifestChunk {
            manifest_crc32: crc,
            total_len: bytes.len() as u32,
            offset: 1,
            data: bytes[..4].to_vec(),
        }),
        Err(ProtocolError::InvalidLength)
    ));
    assembler
        .push_chunk(ManifestChunk {
            manifest_crc32: crc,
            total_len: bytes.len() as u32,
            offset: 0,
            data: bytes[..4].to_vec(),
        })
        .unwrap();
    assembler
        .push_chunk(ManifestChunk {
            manifest_crc32: crc,
            total_len: bytes.len() as u32,
            offset: 4,
            data: bytes[4..].to_vec(),
        })
        .unwrap();

    assert_eq!(
        assembler
            .finish(ManifestDone {
                manifest_crc32: crc,
                total_len: bytes.len() as u32,
            })
            .unwrap(),
        bytes
    );
}

#[test]
fn manifest_assembly_rejects_limit_and_final_crc_drift() {
    let mut assembler = ManifestAssembler::new();
    assert!(matches!(
        assembler.push_chunk(ManifestChunk {
            manifest_crc32: 1,
            total_len: 65_537,
            offset: 0,
            data: vec![],
        }),
        Err(ProtocolError::InvalidLength)
    ));

    let bytes = b"manifest".to_vec();
    let crc = dctp_protocol::crc32_iso_hdlc(&bytes);
    assembler
        .push_chunk(ManifestChunk {
            manifest_crc32: crc,
            total_len: bytes.len() as u32,
            offset: 0,
            data: bytes,
        })
        .unwrap();
    assert!(matches!(
        assembler.finish(ManifestDone {
            manifest_crc32: crc ^ 1,
            total_len: 8,
        }),
        Err(ProtocolError::InvalidLength)
    ));

    let mut assembler = ManifestAssembler::new();
    assembler
        .push_chunk(ManifestChunk {
            manifest_crc32: crc,
            total_len: 8,
            offset: 0,
            data: b"manifesz".to_vec(),
        })
        .unwrap();
    assert!(matches!(
        assembler.finish(ManifestDone {
            manifest_crc32: crc,
            total_len: 8,
        }),
        Err(ProtocolError::CrcMismatch)
    ));
}

#[test]
fn wire_reader_finish_rejects_unread_data() {
    let mut reader = WireReader::new(&[1, 2]);
    assert_eq!(reader.read_u8().unwrap(), 1);
    assert!(matches!(reader.finish(), Err(ProtocolError::InvalidLength)));
}

#[test]
fn prepare_flash_payload_has_the_stable_v1_layout_and_round_trips() {
    let request = PrepareFlash {
        operation_id: [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ],
        target_id: FirmwareTargetId::LCKFB_TMX_MSPM0G3507,
        firmware_version: [0, 3, 0],
        image_len: 0x0001_2000,
        image_sha256: [0xA5; 32],
    };

    let bytes = request.encode().unwrap();

    let mut expected = vec![
        0x01, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
        0x0E, 0x0F, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x20, 0x01,
        0x00,
    ];
    expected.extend_from_slice(&[0xA5; 32]);
    assert_eq!(bytes, expected);
    assert_eq!(PrepareFlash::decode(&bytes).unwrap(), request);
}

#[test]
fn prepare_flash_ack_has_the_stable_v1_layout_and_round_trips() {
    let ack = PrepareFlashAck {
        operation_id: [0x11; 16],
        bootloader_protocol: BootloaderProtocol::TI_MSPM0_ROM_BSL_UART,
        entry_delay_ms: 250,
        initial_baud: 9_600,
    };

    let bytes = ack.encode().unwrap();

    let mut expected = vec![0x01];
    expected.extend_from_slice(&[0x11; 16]);
    expected.extend_from_slice(&[0x01, 0xFA, 0x00, 0x80, 0x25, 0x00, 0x00]);
    assert_eq!(bytes, expected);
    assert_eq!(PrepareFlashAck::decode(&bytes).unwrap(), ack);
}

#[test]
fn prepare_flash_payloads_reject_wrong_schema_truncation_and_trailing_bytes() {
    let mut request = vec![0; 63];
    request[0] = 2;
    assert!(matches!(
        PrepareFlash::decode(&request),
        Err(ProtocolError::UnsupportedVersion)
    ));

    request[0] = 1;
    assert!(matches!(
        PrepareFlash::decode(&request[..62]),
        Err(ProtocolError::Truncated)
    ));

    let mut ack = vec![0; 25];
    ack[0] = 1;
    assert!(matches!(
        PrepareFlashAck::decode(&ack),
        Err(ProtocolError::InvalidLength)
    ));
}
