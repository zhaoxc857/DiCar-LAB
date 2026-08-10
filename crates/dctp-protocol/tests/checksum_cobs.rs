use dctp_protocol::{cobs_decode, cobs_encode, crc16_ccitt_false, crc32_iso_hdlc};

#[test]
fn checksum_check_values_match_the_spec() {
    assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    assert_eq!(crc32_iso_hdlc(b"123456789"), 0xCBF4_3926);
}

#[test]
fn cobs_known_vector_round_trips() {
    let raw = [0x11, 0x00, 0x22];
    let encoded = vec![0x02, 0x11, 0x02, 0x22];
    assert_eq!(cobs_encode(&raw), encoded);
    assert_eq!(cobs_decode(&encoded).unwrap(), raw);
}
