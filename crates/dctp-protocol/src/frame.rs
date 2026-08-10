use crate::ProtocolError;

pub const MAGIC: u16 = 0x5444;
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 13;
pub const MAX_PAYLOAD_LEN: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    Hello = 0x01,
    HelloAck = 0x02,
    Heartbeat = 0x03,
    HeartbeatAck = 0x04,
    SessionClose = 0x05,
    ManifestRequest = 0x10,
    ManifestChunk = 0x11,
    ManifestDone = 0x12,
    ParamRead = 0x20,
    ParamValue = 0x21,
    ParamWrite = 0x22,
    ParamWriteAck = 0x23,
    ParamCommit = 0x24,
    ParamCommitAck = 0x25,
    TelemetrySubscribe = 0x30,
    TelemetrySubscribeAck = 0x31,
    TelemetryData = 0x32,
    TelemetryStop = 0x33,
    LogMessage = 0x40,
    DeviceEvent = 0x41,
    PrepareFlash = 0x50,
    PrepareFlashAck = 0x51,
    Error = 0x7f,
}

impl TryFrom<u8> for MessageType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0x01 => Ok(Self::Hello),
            0x02 => Ok(Self::HelloAck),
            0x03 => Ok(Self::Heartbeat),
            0x04 => Ok(Self::HeartbeatAck),
            0x05 => Ok(Self::SessionClose),
            0x10 => Ok(Self::ManifestRequest),
            0x11 => Ok(Self::ManifestChunk),
            0x12 => Ok(Self::ManifestDone),
            0x20 => Ok(Self::ParamRead),
            0x21 => Ok(Self::ParamValue),
            0x22 => Ok(Self::ParamWrite),
            0x23 => Ok(Self::ParamWriteAck),
            0x24 => Ok(Self::ParamCommit),
            0x25 => Ok(Self::ParamCommitAck),
            0x30 => Ok(Self::TelemetrySubscribe),
            0x31 => Ok(Self::TelemetrySubscribeAck),
            0x32 => Ok(Self::TelemetryData),
            0x33 => Ok(Self::TelemetryStop),
            0x40 => Ok(Self::LogMessage),
            0x41 => Ok(Self::DeviceEvent),
            0x50 => Ok(Self::PrepareFlash),
            0x51 => Ok(Self::PrepareFlashAck),
            0x7f => Ok(Self::Error),
            _ => Err(ProtocolError::UnknownMessageType(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameFlags(u8);

impl FrameFlags {
    pub const NONE: Self = Self(0);
    pub const ACK_REQUIRED: Self = Self(1 << 0);
    pub const RESPONSE: Self = Self(1 << 1);
    pub const ERROR: Self = Self(1 << 2);
    pub const MORE_FRAGMENTS: Self = Self(1 << 3);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub version: u8,
    pub message_type: MessageType,
    pub flags: FrameFlags,
    pub sequence: u16,
    pub session_id: u32,
    pub payload_len: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(
        message_type: MessageType,
        flags: FrameFlags,
        sequence: u16,
        session_id: u32,
        payload: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(payload.len()));
        }

        Ok(Self {
            header: FrameHeader {
                version: VERSION,
                message_type,
                flags,
                sequence,
                session_id,
                payload_len: payload.len() as u16,
            },
            payload,
        })
    }
}
