mod checksum;
mod cobs;
mod codec;
mod error;
mod frame;
mod messages;
mod parameter;
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
pub use parameter::{
    canonical_parameter_crc32, EnumOption, ParamCommit, ParamCommitAck, ParamCommitEntry,
    ParamConstraints, ParamDescriptor, ParamFlags, ParamRead, ParamState, ParamType, ParamValue,
    ParamWrite, ParamWriteAck, MAX_DISPLAY_NAME_LEN, MAX_ENUM_LABEL_LEN, MAX_ENUM_OPTIONS,
    MAX_GROUP_LEN, MAX_MACHINE_NAME_LEN, MAX_UNIT_LEN,
};
pub use stream::{StreamDecoder, StreamStats, MAX_ENCODED_PACKET_LEN};
pub use wire::{WireDecode, WireEncode, WireReader, WireWriter};
