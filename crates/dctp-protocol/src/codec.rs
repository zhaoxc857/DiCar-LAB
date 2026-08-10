use crate::{
    cobs_decode, cobs_encode, crc16_ccitt_false, Frame, FrameFlags, MessageType, ProtocolError,
    HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION,
};

const CRC_LEN: usize = 2;

pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, ProtocolError> {
    if frame.payload.len() > MAX_PAYLOAD_LEN
        || usize::from(frame.header.payload_len) != frame.payload.len()
    {
        return Err(ProtocolError::InvalidLength);
    }

    let mut raw = Vec::with_capacity(HEADER_LEN + frame.payload.len() + CRC_LEN);
    raw.extend_from_slice(&MAGIC.to_le_bytes());
    raw.push(frame.header.version);
    raw.push(frame.header.message_type as u8);
    raw.push(frame.header.flags.bits());
    raw.extend_from_slice(&frame.header.sequence.to_le_bytes());
    raw.extend_from_slice(&frame.header.session_id.to_le_bytes());
    raw.extend_from_slice(&frame.header.payload_len.to_le_bytes());
    raw.extend_from_slice(&frame.payload);
    let crc = crc16_ccitt_false(&raw);
    raw.extend_from_slice(&crc.to_le_bytes());

    let mut encoded = cobs_encode(&raw);
    encoded.push(0);
    Ok(encoded)
}

pub fn decode_packet(encoded_without_delimiter: &[u8]) -> Result<Frame, ProtocolError> {
    let raw = cobs_decode(encoded_without_delimiter)?;
    if raw.len() < HEADER_LEN + CRC_LEN {
        return Err(ProtocolError::Truncated);
    }

    let magic = u16::from_le_bytes([raw[0], raw[1]]);
    if magic != MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    if raw[2] != VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }

    let payload_len = usize::from(u16::from_le_bytes([raw[11], raw[12]]));
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge(payload_len));
    }
    let expected_len = HEADER_LEN + payload_len + CRC_LEN;
    if raw.len() != expected_len {
        return Err(ProtocolError::InvalidLength);
    }

    let expected_crc = u16::from_le_bytes([raw[expected_len - 2], raw[expected_len - 1]]);
    if crc16_ccitt_false(&raw[..expected_len - CRC_LEN]) != expected_crc {
        return Err(ProtocolError::CrcMismatch);
    }

    let message_type = MessageType::try_from(raw[3])?;
    let sequence = u16::from_le_bytes([raw[5], raw[6]]);
    let session_id = u32::from_le_bytes([raw[7], raw[8], raw[9], raw[10]]);
    let payload = raw[HEADER_LEN..HEADER_LEN + payload_len].to_vec();

    Ok(Frame {
        header: crate::FrameHeader {
            version: raw[2],
            message_type,
            flags: FrameFlags::from_bits(raw[4]),
            sequence,
            session_id,
            payload_len: payload_len as u16,
        },
        payload,
    })
}
