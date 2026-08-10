use crate::{decode_packet, Frame, ProtocolError, HEADER_LEN, MAX_PAYLOAD_LEN};

const MAX_RAW_FRAME_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN + 2;

#[allow(clippy::manual_div_ceil)]
pub const MAX_ENCODED_PACKET_LEN: usize = MAX_RAW_FRAME_LEN + (MAX_RAW_FRAME_LEN + 253) / 254;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamStats {
    pub decoded: u64,
    pub malformed: u64,
    pub overflow: u64,
}

#[derive(Debug, Default)]
pub struct StreamDecoder {
    buffer: Vec<u8>,
    dropping_overlong_packet: bool,
    stats: StreamStats,
}

impl StreamDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(MAX_ENCODED_PACKET_LEN),
            dropping_overlong_packet: false,
            stats: StreamStats::default(),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Frame, ProtocolError>> {
        let mut output = Vec::new();

        for &byte in bytes {
            if self.dropping_overlong_packet {
                if byte == 0 {
                    self.dropping_overlong_packet = false;
                    self.stats.overflow += 1;
                    output.push(Err(ProtocolError::PacketTooLong));
                }
                continue;
            }

            if byte == 0 {
                if self.buffer.is_empty() {
                    continue;
                }

                let result = decode_packet(&self.buffer);
                self.buffer.clear();
                match &result {
                    Ok(_) => self.stats.decoded += 1,
                    Err(_) => self.stats.malformed += 1,
                }
                output.push(result);
            } else if self.buffer.len() < MAX_ENCODED_PACKET_LEN {
                self.buffer.push(byte);
            } else {
                self.buffer.clear();
                self.dropping_overlong_packet = true;
            }
        }

        output
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.dropping_overlong_packet = false;
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub const fn stats(&self) -> StreamStats {
        self.stats
    }
}
