use dctp_protocol::{
    CapabilityFlags, ErrorCode, ErrorPayload, Heartbeat, Hello, HelloAck, ManifestAssembler,
    ManifestChunk, ManifestDone, MessageType, ProtocolError, WireDecode, WireEncode, WireReader,
    WireWriter,
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
