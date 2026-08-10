use dctp_protocol::{decode_packet, encode_frame, Frame, FrameFlags, MessageType, ProtocolError};

#[test]
fn encoded_frame_has_delimiter_and_round_trips() {
    let frame = Frame::new(
        MessageType::Hello,
        FrameFlags::ACK_REQUIRED,
        9,
        0,
        vec![1, 0, 2],
    )
    .unwrap();
    let wire = encode_frame(&frame).unwrap();
    assert_eq!(wire.last(), Some(&0));
    assert_eq!(decode_packet(&wire[..wire.len() - 1]).unwrap(), frame);
}

#[test]
fn corrupted_packet_is_rejected() {
    let frame = Frame::new(
        MessageType::Heartbeat,
        FrameFlags::ACK_REQUIRED,
        10,
        77,
        vec![],
    )
    .unwrap();
    let mut wire = encode_frame(&frame).unwrap();
    wire[4] ^= 0x40;
    assert!(matches!(
        decode_packet(&wire[..wire.len() - 1]),
        Err(ProtocolError::CrcMismatch)
    ));
}

#[test]
fn unsupported_frame_version_is_rejected_before_encoding() {
    let mut frame = Frame::new(MessageType::Heartbeat, FrameFlags::NONE, 1, 0, vec![]).unwrap();
    frame.header.version = 2;

    assert!(matches!(
        encode_frame(&frame),
        Err(ProtocolError::UnsupportedVersion)
    ));
}
