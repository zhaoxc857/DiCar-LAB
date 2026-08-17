use dicar_firmware_flash::bsl::{
    decode_core_response, encode_command, mspm0_crc32, parse_transport_ack, BslCommand, BslError,
    CoreResponse, CoreStatus, DeviceInfo, TransportAckError,
};

#[test]
fn official_connection_and_unlock_packets_match_literal_uart_bytes() {
    assert_eq!(mspm0_crc32(&[0x12]), 0xDE44_613A);
    assert_eq!(
        encode_command(&BslCommand::Connection).unwrap(),
        [0x80, 0x01, 0x00, 0x12, 0x3A, 0x61, 0x44, 0xDE]
    );

    let password = std::array::from_fn::<_, 32, _>(|index| index as u8);
    let mut expected = vec![0x80, 0x21, 0x00, 0x21];
    expected.extend_from_slice(&password);
    expected.extend_from_slice(&[0x83, 0x7F, 0xBA, 0x53]);
    assert_eq!(
        encode_command(&BslCommand::Unlock(&password)).unwrap(),
        expected
    );
}

#[test]
fn address_commands_use_little_endian_ranges_and_lengths() {
    let erase = encode_command(&BslCommand::EraseRange {
        start: 0,
        end: 0x0003_FFFF,
    })
    .unwrap();
    assert_eq!(
        &erase[..12],
        &[0x80, 0x09, 0x00, 0x23, 0, 0, 0, 0, 0xFF, 0xFF, 3, 0]
    );

    let verify = encode_command(&BslCommand::VerifyCrc {
        address: 0,
        length: 0x400,
    })
    .unwrap();
    assert_eq!(
        &verify[..12],
        &[0x80, 0x09, 0x00, 0x26, 0, 0, 0, 0, 0, 4, 0, 0]
    );

    let program = encode_command(&BslCommand::ProgramData {
        address: 0x1234,
        data: &[0xAA, 0xBB, 0xCC],
    })
    .unwrap();
    assert_eq!(
        &program[..11],
        &[0x80, 0x08, 0x00, 0x20, 0x34, 0x12, 0, 0, 0xAA, 0xBB, 0xCC]
    );
}

#[test]
fn uart_transport_ack_is_distinct_from_core_command_status() {
    assert_eq!(parse_transport_ack(0x00), Ok(()));
    assert_eq!(
        parse_transport_ack(0x51),
        Err(BslError::Transport(TransportAckError::HeaderIncorrect))
    );
    assert_eq!(
        parse_transport_ack(0x52),
        Err(BslError::Transport(TransportAckError::ChecksumIncorrect))
    );
    assert_eq!(
        parse_transport_ack(0x57),
        Err(BslError::Transport(TransportAckError::PacketSize))
    );
}

#[test]
fn literal_core_status_and_crc_response_packets_are_decoded() {
    assert_eq!(
        decode_core_response(&[0x08, 0x02, 0x00, 0x3B, 0x00, 0x38, 0x02, 0x94, 0x82]).unwrap(),
        CoreResponse::Status(CoreStatus::Success)
    );
    assert_eq!(
        decode_core_response(&[0x08, 0x02, 0x00, 0x3B, 0x02, 0x14, 0x63, 0x9A, 0x6C]).unwrap(),
        CoreResponse::Status(CoreStatus::PasswordError)
    );
    assert_eq!(
        decode_core_response(&[
            0x08, 0x05, 0x00, 0x32, 0x78, 0x56, 0x34, 0x12, 0xCA, 0xBB, 0x15, 0x6C,
        ])
        .unwrap(),
        CoreResponse::VerificationCrc(0x1234_5678)
    );
}

#[test]
fn identity_layout_and_response_framing_are_strict() {
    let mut payload = vec![0x31];
    payload.extend_from_slice(&0x0102u16.to_le_bytes());
    payload.extend_from_slice(&0x0304u16.to_le_bytes());
    payload.extend_from_slice(&0x0506_0708u32.to_le_bytes());
    payload.extend_from_slice(&0x0910u16.to_le_bytes());
    payload.extend_from_slice(&0x0120u16.to_le_bytes());
    payload.extend_from_slice(&0x2020_0100u32.to_le_bytes());
    payload.extend_from_slice(&0x1112_1314u32.to_le_bytes());
    payload.extend_from_slice(&0x1516_1718u32.to_le_bytes());
    let mut packet = vec![0x08, payload.len() as u8, 0x00];
    packet.extend_from_slice(&payload);
    packet.extend_from_slice(&mspm0_crc32(&payload).to_le_bytes());

    assert_eq!(
        decode_core_response(&packet).unwrap(),
        CoreResponse::Identity(DeviceInfo {
            command_interpreter_version: 0x0102,
            build_id: 0x0304,
            application_revision: 0x0506_0708,
            plugin_version: 0x0910,
            max_buffer_size: 0x0120,
            buffer_start_address: 0x2020_0100,
            bcr_config_id: 0x1112_1314,
            bsl_config_id: 0x1516_1718,
        })
    );

    let mut bad_crc = packet.clone();
    *bad_crc.last_mut().unwrap() ^= 1;
    assert_eq!(decode_core_response(&bad_crc), Err(BslError::ResponseCrc));
    let mut trailing = packet;
    trailing.push(0);
    assert_eq!(
        decode_core_response(&trailing),
        Err(BslError::ResponseLength)
    );
}
