use crate::{ProtocolError, WireDecode, WireEncode, WireReader, WireWriter};

pub const MAX_LOG_TEXT_LEN: usize = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LogSeverity {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl TryFrom<u8> for LogSeverity {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(Self::Trace),
            1 => Ok(Self::Debug),
            2 => Ok(Self::Info),
            3 => Ok(Self::Warn),
            4 => Ok(Self::Error),
            _ => Err(ProtocolError::InvalidValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogMessage {
    pub timestamp_us: u32,
    pub severity: LogSeverity,
    pub module_id: u16,
    pub text: String,
}

impl WireEncode for LogMessage {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.text.len() > MAX_LOG_TEXT_LEN {
            return Err(ProtocolError::StringTooLong);
        }
        let mut writer = WireWriter::new();
        writer.put_u32(self.timestamp_us);
        writer.put_u8(self.severity as u8);
        writer.put_u16(self.module_id);
        writer.put_utf8_u8_len(&self.text, MAX_LOG_TEXT_LEN)?;
        Ok(writer.into_inner())
    }
}

impl WireDecode for LogMessage {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            timestamp_us: reader.read_u32()?,
            severity: LogSeverity::try_from(reader.read_u8()?)?,
            module_id: reader.read_u16()?,
            text: reader.read_utf8_u8_len(MAX_LOG_TEXT_LEN)?,
        };
        reader.finish()?;
        Ok(value)
    }
}
