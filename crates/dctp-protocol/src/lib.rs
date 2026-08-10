mod checksum;
mod cobs;
mod codec;
mod error;
mod frame;
mod messages;
mod stream;
mod wire;

pub use checksum::{crc16_ccitt_false, crc32_iso_hdlc};
pub use cobs::{cobs_decode, cobs_encode};
pub use codec::{decode_packet, encode_frame};
pub use error::ProtocolError;
pub use frame::{
    Frame, FrameFlags, FrameHeader, MessageType, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION,
};
pub use messages::{
    CapabilityFlags, ErrorCode, ErrorPayload, Heartbeat, Hello, HelloAck, ManifestAssembler,
    ManifestChunk, ManifestDone, MAX_ERROR_CONTEXT_LEN, MAX_MANIFEST_LEN,
};
pub use stream::{StreamDecoder, StreamStats, MAX_ENCODED_PACKET_LEN};
pub use wire::{WireDecode, WireEncode, WireReader, WireWriter};
