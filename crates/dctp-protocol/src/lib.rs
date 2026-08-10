mod checksum;
mod cobs;
mod codec;
mod error;
mod frame;

pub use checksum::{crc16_ccitt_false, crc32_iso_hdlc};
pub use cobs::{cobs_decode, cobs_encode};
pub use codec::{decode_packet, encode_frame};
pub use error::ProtocolError;
pub use frame::{
    Frame, FrameFlags, FrameHeader, MessageType, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION,
};
