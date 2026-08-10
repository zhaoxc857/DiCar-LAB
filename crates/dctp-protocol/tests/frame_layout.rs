use dctp_protocol::{Frame, FrameFlags, MessageType, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION};

#[test]
fn frame_constants_match_dctp_v1() {
    assert_eq!(MAGIC, 0x5444);
    assert_eq!(VERSION, 1);
    assert_eq!(HEADER_LEN, 13);
    assert_eq!(MAX_PAYLOAD_LEN, 1024);
}

#[test]
fn frame_rejects_payload_over_limit() {
    let result = Frame::new(
        MessageType::Hello,
        FrameFlags::ACK_REQUIRED,
        7,
        0,
        vec![0; MAX_PAYLOAD_LEN + 1],
    );
    assert!(result.is_err());
}
