mod checksum;
mod cobs;
mod codec;
mod error;
mod frame;
mod log;
mod manifest;
mod messages;
mod parameter;
mod stream;
mod telemetry;
mod wire;

pub use checksum::{crc16_ccitt_false, crc32_iso_hdlc};
pub use cobs::{cobs_decode, cobs_encode};
pub use codec::{decode_packet, encode_frame};
pub use error::ProtocolError;
pub use frame::{
    Frame, FrameFlags, FrameHeader, MessageType, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION,
};
pub use log::{LogMessage, LogSeverity, MAX_LOG_TEXT_LEN};
pub use manifest::{
    DeviceManifest, MANIFEST_SCHEMA_VERSION, MAX_MANIFEST_PARAMETERS, MAX_MANIFEST_TELEMETRY,
};
pub use messages::{
    BootloaderProtocol, CapabilityFlags, ErrorCode, ErrorPayload, FirmwareTargetId, Heartbeat,
    Hello, HelloAck, ManifestAssembler, ManifestChunk, ManifestDone, PrepareFlash, PrepareFlashAck,
    MAX_ERROR_CONTEXT_LEN, MAX_MANIFEST_LEN,
};
pub use parameter::{
    canonical_parameter_crc32, EnumOption, ParamCommit, ParamCommitAck, ParamCommitEntry,
    ParamConstraints, ParamDescriptor, ParamFlags, ParamRead, ParamState, ParamType, ParamValue,
    ParamWrite, ParamWriteAck, MAX_DISPLAY_NAME_LEN, MAX_ENUM_LABEL_LEN, MAX_ENUM_OPTIONS,
    MAX_GROUP_LEN, MAX_MACHINE_NAME_LEN, MAX_UNIT_LEN,
};
pub use stream::{StreamDecoder, StreamStats, MAX_ENCODED_PACKET_LEN};
pub use telemetry::{
    TelemetryBatch, TelemetryDescriptor, TelemetrySample, TelemetrySubscription, TelemetryType,
    MAX_TELEMETRY_CHANNELS, MAX_TELEMETRY_RATE_HZ, MAX_TELEMETRY_SAMPLES,
};
pub use wire::{WireDecode, WireEncode, WireReader, WireWriter};
