use core::ops::BitOr;

use crate::{
    crc32_iso_hdlc, MessageType, ProtocolError, WireDecode, WireEncode, WireReader, WireWriter,
};

pub const MAX_ERROR_CONTEXT_LEN: usize = 64;
pub const MAX_MANIFEST_LEN: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityFlags(u32);

impl CapabilityFlags {
    pub const PARAMETERS: Self = Self(1 << 0);
    pub const TELEMETRY: Self = Self(1 << 1);
    pub const PERSISTENCE: Self = Self(1 << 2);
    pub const STRUCTURED_LOG: Self = Self(1 << 3);
    pub const PREPARE_FLASH: Self = Self(1 << 4);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl BitOr for CapabilityFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    UnsupportedVersion,
    InvalidSession,
    UnknownMessage,
    InvalidLength,
    InvalidParamId,
    TypeMismatch,
    OutOfRange,
    ReadOnly,
    RevisionConflict,
    Busy,
    QueueFull,
    StorageFailed,
    VerifyFailed,
    NotReady,
    InternalError,
    Unknown(u16),
}

impl ErrorCode {
    pub const fn from_u16(value: u16) -> Self {
        match value {
            1 => Self::UnsupportedVersion,
            2 => Self::InvalidSession,
            3 => Self::UnknownMessage,
            4 => Self::InvalidLength,
            5 => Self::InvalidParamId,
            6 => Self::TypeMismatch,
            7 => Self::OutOfRange,
            8 => Self::ReadOnly,
            9 => Self::RevisionConflict,
            10 => Self::Busy,
            11 => Self::QueueFull,
            12 => Self::StorageFailed,
            13 => Self::VerifyFailed,
            14 => Self::NotReady,
            15 => Self::InternalError,
            value => Self::Unknown(value),
        }
    }

    pub const fn as_u16(self) -> u16 {
        match self {
            Self::UnsupportedVersion => 1,
            Self::InvalidSession => 2,
            Self::UnknownMessage => 3,
            Self::InvalidLength => 4,
            Self::InvalidParamId => 5,
            Self::TypeMismatch => 6,
            Self::OutOfRange => 7,
            Self::ReadOnly => 8,
            Self::RevisionConflict => 9,
            Self::Busy => 10,
            Self::QueueFull => 11,
            Self::StorageFailed => 12,
            Self::VerifyFailed => 13,
            Self::NotReady => 14,
            Self::InternalError => 15,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hello {
    pub client_nonce: u32,
    pub min_version: u8,
    pub max_version: u8,
    pub max_payload: u16,
}

impl WireEncode for Hello {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.client_nonce);
        writer.put_u8(self.min_version);
        writer.put_u8(self.max_version);
        writer.put_u16(self.max_payload);
        Ok(writer.into_inner())
    }
}

impl WireDecode for Hello {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            client_nonce: reader.read_u32()?,
            min_version: reader.read_u8()?,
            max_version: reader.read_u8()?,
            max_payload: reader.read_u16()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelloAck {
    pub session_id: u32,
    pub device_id: [u8; 16],
    pub boot_count: u32,
    pub firmware_major: u16,
    pub firmware_minor: u16,
    pub firmware_patch: u16,
    pub sdk_major: u16,
    pub sdk_minor: u16,
    pub sdk_patch: u16,
    pub capabilities: CapabilityFlags,
    pub manifest_crc32: u32,
    pub max_payload: u16,
}

impl WireEncode for HelloAck {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.session_id);
        writer.put_bytes(&self.device_id);
        writer.put_u32(self.boot_count);
        writer.put_u16(self.firmware_major);
        writer.put_u16(self.firmware_minor);
        writer.put_u16(self.firmware_patch);
        writer.put_u16(self.sdk_major);
        writer.put_u16(self.sdk_minor);
        writer.put_u16(self.sdk_patch);
        writer.put_u32(self.capabilities.bits());
        writer.put_u32(self.manifest_crc32);
        writer.put_u16(self.max_payload);
        Ok(writer.into_inner())
    }
}

impl WireDecode for HelloAck {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let session_id = reader.read_u32()?;
        let device_id: [u8; 16] = reader
            .read_exact(16)?
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?;
        let value = Self {
            session_id,
            device_id,
            boot_count: reader.read_u32()?,
            firmware_major: reader.read_u16()?,
            firmware_minor: reader.read_u16()?,
            firmware_patch: reader.read_u16()?,
            sdk_major: reader.read_u16()?,
            sdk_minor: reader.read_u16()?,
            sdk_patch: reader.read_u16()?,
            capabilities: CapabilityFlags::from_bits(reader.read_u32()?),
            manifest_crc32: reader.read_u32()?,
            max_payload: reader.read_u16()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Heartbeat {
    pub monotonic_ms: u32,
}

impl WireEncode for Heartbeat {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.monotonic_ms);
        Ok(writer.into_inner())
    }
}

impl WireDecode for Heartbeat {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            monotonic_ms: reader.read_u32()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorPayload {
    pub original_message_type: MessageType,
    pub original_sequence: u16,
    pub error_code: ErrorCode,
    pub context: String,
}

impl WireEncode for ErrorPayload {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u8(self.original_message_type as u8);
        writer.put_u16(self.original_sequence);
        writer.put_u16(self.error_code.as_u16());
        writer.put_utf8_u8_len(&self.context, MAX_ERROR_CONTEXT_LEN)?;
        Ok(writer.into_inner())
    }
}

impl WireDecode for ErrorPayload {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            original_message_type: MessageType::try_from(reader.read_u8()?)?,
            original_sequence: reader.read_u16()?,
            error_code: ErrorCode::from_u16(reader.read_u16()?),
            context: reader.read_utf8_u8_len(MAX_ERROR_CONTEXT_LEN)?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestChunk {
    pub manifest_crc32: u32,
    pub total_len: u32,
    pub offset: u32,
    pub data: Vec<u8>,
}

impl WireEncode for ManifestChunk {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.manifest_crc32);
        writer.put_u32(self.total_len);
        writer.put_u32(self.offset);
        writer.put_bytes(&self.data);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ManifestChunk {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let manifest_crc32 = reader.read_u32()?;
        let total_len = reader.read_u32()?;
        let offset = reader.read_u32()?;
        let data = reader
            .read_exact(
                bytes
                    .len()
                    .checked_sub(12)
                    .ok_or(ProtocolError::Truncated)?,
            )?
            .to_vec();
        reader.finish()?;
        Ok(Self {
            manifest_crc32,
            total_len,
            offset,
            data,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDone {
    pub manifest_crc32: u32,
    pub total_len: u32,
}

impl WireEncode for ManifestDone {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.manifest_crc32);
        writer.put_u32(self.total_len);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ManifestDone {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            manifest_crc32: reader.read_u32()?,
            total_len: reader.read_u32()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Default)]
pub struct ManifestAssembler {
    manifest_crc32: Option<u32>,
    total_len: Option<usize>,
    bytes: Vec<u8>,
}

impl ManifestAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_chunk(&mut self, chunk: ManifestChunk) -> Result<(), ProtocolError> {
        let total_len =
            usize::try_from(chunk.total_len).map_err(|_| ProtocolError::InvalidLength)?;
        let offset = usize::try_from(chunk.offset).map_err(|_| ProtocolError::InvalidLength)?;
        let end = offset
            .checked_add(chunk.data.len())
            .ok_or(ProtocolError::InvalidLength)?;
        if total_len > MAX_MANIFEST_LEN || offset != self.bytes.len() || end > total_len {
            return Err(ProtocolError::InvalidLength);
        }

        match (self.manifest_crc32, self.total_len) {
            (Some(expected_crc32), Some(expected_len))
                if expected_crc32 != chunk.manifest_crc32 || expected_len != total_len =>
            {
                return Err(ProtocolError::InvalidLength);
            }
            (None, None) => {
                self.manifest_crc32 = Some(chunk.manifest_crc32);
                self.total_len = Some(total_len);
            }
            (Some(_), Some(_)) => {}
            _ => return Err(ProtocolError::InvalidLength),
        }

        self.bytes
            .try_reserve(chunk.data.len())
            .map_err(|_| ProtocolError::InvalidLength)?;
        self.bytes.extend_from_slice(&chunk.data);
        Ok(())
    }

    pub fn finish(self, done: ManifestDone) -> Result<Vec<u8>, ProtocolError> {
        let total_len =
            usize::try_from(done.total_len).map_err(|_| ProtocolError::InvalidLength)?;
        if total_len > MAX_MANIFEST_LEN
            || self.manifest_crc32 != Some(done.manifest_crc32)
            || self.total_len != Some(total_len)
            || self.bytes.len() != total_len
        {
            return Err(ProtocolError::InvalidLength);
        }
        if crc32_iso_hdlc(&self.bytes) != done.manifest_crc32 {
            return Err(ProtocolError::CrcMismatch);
        }

        Ok(self.bytes)
    }
}
