use crate::{ProtocolError, HEADER_LEN, MAX_PAYLOAD_LEN};

const MAX_DECODED_PACKET_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN + 2;

pub fn cobs_encode(input: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(input.len() + 1);
    let mut code_index = 0;
    let mut code = 1u8;
    encoded.push(0);

    for &byte in input {
        if byte == 0 {
            encoded[code_index] = code;
            code_index = encoded.len();
            encoded.push(0);
            code = 1;
        } else {
            encoded.push(byte);
            code = code.wrapping_add(1);
            if code == 0xFF {
                encoded[code_index] = code;
                code_index = encoded.len();
                encoded.push(0);
                code = 1;
            }
        }
    }

    encoded[code_index] = code;
    encoded
}

pub fn cobs_decode(input: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if input.is_empty() {
        return Err(ProtocolError::CobsMalformed);
    }

    let mut decoded = Vec::new();
    let mut index = 0;

    while index < input.len() {
        let code = input[index];
        if code == 0 {
            return Err(ProtocolError::CobsMalformed);
        }
        index += 1;

        let data_len = usize::from(code - 1);
        let end = index
            .checked_add(data_len)
            .filter(|&end| end <= input.len())
            .ok_or(ProtocolError::CobsMalformed)?;
        let decoded_len = decoded
            .len()
            .checked_add(data_len)
            .ok_or(ProtocolError::PacketTooLong)?;
        if decoded_len > MAX_DECODED_PACKET_LEN {
            return Err(ProtocolError::PacketTooLong);
        }
        decoded.extend_from_slice(&input[index..end]);
        index = end;

        if code != 0xFF && index < input.len() {
            if decoded.len() == MAX_DECODED_PACKET_LEN {
                return Err(ProtocolError::PacketTooLong);
            }
            decoded.push(0);
        }
    }

    Ok(decoded)
}
