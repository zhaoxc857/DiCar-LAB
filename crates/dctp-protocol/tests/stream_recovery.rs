use dctp_protocol::{
    encode_frame, Frame, FrameFlags, MessageType, ProtocolError, StreamDecoder, StreamStats,
};
use proptest::prelude::*;

#[test]
fn decoder_recovers_after_noise_and_chunk_boundaries() {
    let first =
        encode_frame(&Frame::new(MessageType::Heartbeat, FrameFlags::NONE, 1, 7, vec![]).unwrap())
            .unwrap();
    let second =
        encode_frame(&Frame::new(MessageType::Heartbeat, FrameFlags::NONE, 2, 7, vec![5]).unwrap())
            .unwrap();
    let mut decoder = StreamDecoder::new();
    let mut output = decoder.push(&[0x99, 0x88, 0x00]);
    output.extend(decoder.push(&first[..3]));
    output.extend(decoder.push(&first[3..]));
    output.extend(decoder.push(&second));

    assert!(output[0].is_err());
    assert_eq!(output.iter().filter(|item| item.is_ok()).count(), 2);
}

#[test]
fn overlong_packet_drops_until_next_delimiter() {
    let mut decoder = StreamDecoder::new();
    let output = decoder.push(&vec![0x55; 1100]);
    assert!(output.is_empty());
    let output = decoder.push(&[0x00]);
    assert!(matches!(
        output.as_slice(),
        [Err(ProtocolError::PacketTooLong)]
    ));
}

#[test]
fn decoder_tracks_decode_failure_and_overflow_statistics() {
    let mut decoder = StreamDecoder::new();
    let _ = decoder.push(&[0x99, 0x00]);
    let _ = decoder.push(&vec![0x55; 1100]);
    let _ = decoder.push(&[0x00]);
    let frame =
        encode_frame(&Frame::new(MessageType::Heartbeat, FrameFlags::NONE, 1, 7, vec![]).unwrap())
            .unwrap();
    let _ = decoder.push(&frame);

    assert_eq!(
        decoder.stats(),
        StreamStats {
            decoded: 1,
            malformed: 1,
            overflow: 1,
        }
    );
}

proptest! {
    #[test]
    fn arbitrary_input_never_grows_the_buffer_without_bound(
        data in proptest::collection::vec(any::<u8>(), 0..20_000)
    ) {
        let mut decoder = StreamDecoder::new();
        let _ = decoder.push(&data);
        assert!(decoder.buffered_len() <= dctp_protocol::MAX_ENCODED_PACKET_LEN);
    }
}
