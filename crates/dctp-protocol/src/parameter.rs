use core::ops::BitOr;

use crate::{crc32_iso_hdlc, ProtocolError, WireDecode, WireEncode, WireReader, WireWriter};

pub const MAX_MACHINE_NAME_LEN: usize = 48;
pub const MAX_DISPLAY_NAME_LEN: usize = 64;
pub const MAX_GROUP_LEN: usize = 32;
pub const MAX_UNIT_LEN: usize = 16;
pub const MAX_ENUM_OPTIONS: usize = 32;
pub const MAX_ENUM_LABEL_LEN: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ParamType {
    I32 = 1,
    U32 = 2,
    F32 = 3,
    Bool = 4,
    Enum = 5,
}

impl TryFrom<u8> for ParamType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::I32),
            2 => Ok(Self::U32),
            3 => Ok(Self::F32),
            4 => Ok(Self::Bool),
            5 => Ok(Self::Enum),
            _ => Err(ProtocolError::InvalidValue),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    I32(i32),
    U32(u32),
    F32(f32),
    Bool(bool),
    Enum(i32),
}

impl ParamValue {
    pub const fn param_type(&self) -> ParamType {
        match self {
            Self::I32(_) => ParamType::I32,
            Self::U32(_) => ParamType::U32,
            Self::F32(_) => ParamType::F32,
            Self::Bool(_) => ParamType::Bool,
            Self::Enum(_) => ParamType::Enum,
        }
    }

    pub fn encode_canonical(&self, out: &mut Vec<u8>) {
        match self {
            Self::I32(value) | Self::Enum(value) => out.extend_from_slice(&value.to_le_bytes()),
            Self::U32(value) => out.extend_from_slice(&value.to_le_bytes()),
            Self::F32(value) => out.extend_from_slice(&value.to_bits().to_le_bytes()),
            Self::Bool(value) => out.push(u8::from(*value)),
        }
    }

    fn encode_tagged(&self, writer: &mut WireWriter) {
        writer.put_u8(self.param_type() as u8);
        match self {
            Self::I32(value) | Self::Enum(value) => writer.put_u32(*value as u32),
            Self::U32(value) => writer.put_u32(*value),
            Self::F32(value) => writer.put_f32(*value),
            Self::Bool(value) => writer.put_u8(u8::from(*value)),
        }
    }

    fn decode_tagged(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        match ParamType::try_from(reader.read_u8()?)? {
            ParamType::I32 => Ok(Self::I32(reader.read_u32()? as i32)),
            ParamType::U32 => Ok(Self::U32(reader.read_u32()?)),
            ParamType::F32 => Ok(Self::F32(reader.read_f32()?)),
            ParamType::Bool => match reader.read_u8()? {
                0 => Ok(Self::Bool(false)),
                1 => Ok(Self::Bool(true)),
                _ => Err(ProtocolError::InvalidValue),
            },
            ParamType::Enum => Ok(Self::Enum(reader.read_u32()? as i32)),
        }
    }

    const fn encoded_tagged_len(&self) -> usize {
        match self {
            Self::Bool(_) => 2,
            Self::I32(_) | Self::U32(_) | Self::F32(_) | Self::Enum(_) => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParamFlags(u8);

impl ParamFlags {
    pub const NONE: Self = Self(0);
    pub const WRITABLE: Self = Self(1 << 0);
    pub const PERSISTENT: Self = Self(1 << 1);
    pub const DANGEROUS: Self = Self(1 << 2);

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl BitOr for ParamFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumOption {
    pub value: i32,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParamConstraints {
    None,
    Numeric {
        min: ParamValue,
        max: ParamValue,
        step: ParamValue,
    },
    Enum {
        options: Vec<EnumOption>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamDescriptor {
    pub param_id: u32,
    pub param_type: ParamType,
    pub flags: ParamFlags,
    pub machine_name: String,
    pub display_name: String,
    pub group: String,
    pub unit: String,
    pub default_value: ParamValue,
    pub constraints: ParamConstraints,
}

impl ParamDescriptor {
    fn validate(&self) -> Result<(), ProtocolError> {
        for (value, limit) in [
            (&self.machine_name, MAX_MACHINE_NAME_LEN),
            (&self.display_name, MAX_DISPLAY_NAME_LEN),
            (&self.group, MAX_GROUP_LEN),
            (&self.unit, MAX_UNIT_LEN),
        ] {
            if value.len() > limit || value.len() > u8::MAX as usize {
                return Err(ProtocolError::StringTooLong);
            }
        }
        if self.default_value.param_type() != self.param_type {
            return Err(ProtocolError::InvalidValue);
        }

        match &self.constraints {
            ParamConstraints::None => Ok(()),
            ParamConstraints::Numeric { min, max, step } => {
                if !matches!(
                    self.param_type,
                    ParamType::I32 | ParamType::U32 | ParamType::F32
                ) || min.param_type() != self.param_type
                    || max.param_type() != self.param_type
                    || step.param_type() != self.param_type
                {
                    return Err(ProtocolError::InvalidValue);
                }
                Ok(())
            }
            ParamConstraints::Enum { options } => {
                if self.param_type != ParamType::Enum || options.len() > MAX_ENUM_OPTIONS {
                    return Err(ProtocolError::InvalidValue);
                }
                for (index, option) in options.iter().enumerate() {
                    if option.label.len() > MAX_ENUM_LABEL_LEN {
                        return Err(ProtocolError::StringTooLong);
                    }
                    if options[..index]
                        .iter()
                        .any(|previous| previous.value == option.value)
                    {
                        return Err(ProtocolError::InvalidValue);
                    }
                }
                Ok(())
            }
        }
    }

    pub(crate) fn encoded_len(&self) -> Result<usize, ProtocolError> {
        self.validate()?;
        let mut len = 6usize;
        for value in [
            &self.machine_name,
            &self.display_name,
            &self.group,
            &self.unit,
        ] {
            len = len
                .checked_add(1)
                .and_then(|value_len| value_len.checked_add(value.len()))
                .ok_or(ProtocolError::InvalidLength)?;
        }
        len = len
            .checked_add(self.default_value.encoded_tagged_len())
            .and_then(|value_len| value_len.checked_add(1))
            .ok_or(ProtocolError::InvalidLength)?;
        match &self.constraints {
            ParamConstraints::None => {}
            ParamConstraints::Numeric { min, max, step } => {
                for value in [min, max, step] {
                    len = len
                        .checked_add(value.encoded_tagged_len())
                        .ok_or(ProtocolError::InvalidLength)?;
                }
            }
            ParamConstraints::Enum { options } => {
                len = len.checked_add(1).ok_or(ProtocolError::InvalidLength)?;
                for option in options {
                    len = len
                        .checked_add(5)
                        .and_then(|value_len| value_len.checked_add(option.label.len()))
                        .ok_or(ProtocolError::InvalidLength)?;
                }
            }
        }
        Ok(len)
    }
}

impl WireEncode for ParamDescriptor {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let mut writer = WireWriter::new();
        writer.put_u32(self.param_id);
        writer.put_u8(self.param_type as u8);
        writer.put_u8(self.flags.bits());
        writer.put_utf8_u8_len(&self.machine_name, MAX_MACHINE_NAME_LEN)?;
        writer.put_utf8_u8_len(&self.display_name, MAX_DISPLAY_NAME_LEN)?;
        writer.put_utf8_u8_len(&self.group, MAX_GROUP_LEN)?;
        writer.put_utf8_u8_len(&self.unit, MAX_UNIT_LEN)?;
        self.default_value.encode_tagged(&mut writer);
        match &self.constraints {
            ParamConstraints::None => writer.put_u8(0),
            ParamConstraints::Numeric { min, max, step } => {
                writer.put_u8(1);
                min.encode_tagged(&mut writer);
                max.encode_tagged(&mut writer);
                step.encode_tagged(&mut writer);
            }
            ParamConstraints::Enum { options } => {
                writer.put_u8(2);
                writer.put_u8(options.len() as u8);
                for option in options {
                    writer.put_u32(option.value as u32);
                    writer.put_utf8_u8_len(&option.label, MAX_ENUM_LABEL_LEN)?;
                }
            }
        }
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamDescriptor {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let param_id = reader.read_u32()?;
        let param_type = ParamType::try_from(reader.read_u8()?)?;
        let flags = ParamFlags::from_bits(reader.read_u8()?);
        let machine_name = reader.read_utf8_u8_len(MAX_MACHINE_NAME_LEN)?;
        let display_name = reader.read_utf8_u8_len(MAX_DISPLAY_NAME_LEN)?;
        let group = reader.read_utf8_u8_len(MAX_GROUP_LEN)?;
        let unit = reader.read_utf8_u8_len(MAX_UNIT_LEN)?;
        let default_value = ParamValue::decode_tagged(&mut reader)?;
        let constraints = match reader.read_u8()? {
            0 => ParamConstraints::None,
            1 => ParamConstraints::Numeric {
                min: ParamValue::decode_tagged(&mut reader)?,
                max: ParamValue::decode_tagged(&mut reader)?,
                step: ParamValue::decode_tagged(&mut reader)?,
            },
            2 => {
                let count = reader.read_u8()? as usize;
                if count > MAX_ENUM_OPTIONS {
                    return Err(ProtocolError::InvalidValue);
                }
                let mut options = Vec::new();
                options
                    .try_reserve(count)
                    .map_err(|_| ProtocolError::InvalidLength)?;
                for _ in 0..count {
                    options.push(EnumOption {
                        value: reader.read_u32()? as i32,
                        label: reader.read_utf8_u8_len(MAX_ENUM_LABEL_LEN)?,
                    });
                }
                ParamConstraints::Enum { options }
            }
            _ => return Err(ProtocolError::InvalidValue),
        };
        reader.finish()?;
        let value = Self {
            param_id,
            param_type,
            flags,
            machine_name,
            display_name,
            group,
            unit,
            default_value,
            constraints,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamState {
    pub param_id: u32,
    pub revision: u32,
    pub value: ParamValue,
}

impl WireEncode for ParamState {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.param_id);
        writer.put_u32(self.revision);
        self.value.encode_tagged(&mut writer);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamState {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            param_id: reader.read_u32()?,
            revision: reader.read_u32()?,
            value: ParamValue::decode_tagged(&mut reader)?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamRead {
    pub param_id: u32,
}

impl WireEncode for ParamRead {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.param_id);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamRead {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            param_id: reader.read_u32()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamWrite {
    pub param_id: u32,
    pub expected_revision: u32,
    pub value: ParamValue,
}

impl WireEncode for ParamWrite {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.param_id);
        writer.put_u32(self.expected_revision);
        self.value.encode_tagged(&mut writer);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamWrite {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            param_id: reader.read_u32()?,
            expected_revision: reader.read_u32()?,
            value: ParamValue::decode_tagged(&mut reader)?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamWriteAck {
    pub value: ParamValue,
    pub new_revision: u32,
}

impl WireEncode for ParamWriteAck {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        self.value.encode_tagged(&mut writer);
        writer.put_u32(self.new_revision);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamWriteAck {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            value: ParamValue::decode_tagged(&mut reader)?,
            new_revision: reader.read_u32()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParamCommitEntry {
    pub param_id: u32,
    pub revision: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamCommit {
    pub entries: Vec<ParamCommitEntry>,
    pub canonical_crc32: u32,
}

impl ParamCommit {
    fn entries_are_strictly_sorted(entries: &[ParamCommitEntry]) -> bool {
        entries
            .windows(2)
            .all(|pair| pair[0].param_id < pair[1].param_id)
    }
}

impl WireEncode for ParamCommit {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.entries.len() > u16::MAX as usize
            || !Self::entries_are_strictly_sorted(&self.entries)
        {
            return Err(ProtocolError::InvalidValue);
        }
        let mut writer = WireWriter::new();
        writer.put_u16(self.entries.len() as u16);
        for entry in &self.entries {
            writer.put_u32(entry.param_id);
            writer.put_u32(entry.revision);
        }
        writer.put_u32(self.canonical_crc32);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamCommit {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let count = reader.read_u16()? as usize;
        let entries_len = count.checked_mul(8).ok_or(ProtocolError::InvalidLength)?;
        let expected_len = 2usize
            .checked_add(entries_len)
            .and_then(|len| len.checked_add(4))
            .ok_or(ProtocolError::InvalidLength)?;
        if bytes.len() != expected_len {
            return Err(ProtocolError::InvalidLength);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve(count)
            .map_err(|_| ProtocolError::InvalidLength)?;
        for _ in 0..count {
            entries.push(ParamCommitEntry {
                param_id: reader.read_u32()?,
                revision: reader.read_u32()?,
            });
        }
        let value = Self {
            entries,
            canonical_crc32: reader.read_u32()?,
        };
        reader.finish()?;
        if !Self::entries_are_strictly_sorted(&value.entries) {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParamCommitAck {
    pub canonical_crc32: u32,
}

impl WireEncode for ParamCommitAck {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut writer = WireWriter::new();
        writer.put_u32(self.canonical_crc32);
        Ok(writer.into_inner())
    }
}

impl WireDecode for ParamCommitAck {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            canonical_crc32: reader.read_u32()?,
        };
        reader.finish()?;
        Ok(value)
    }
}

pub fn canonical_parameter_crc32(entries: &[(u32, ParamValue)]) -> Result<u32, ProtocolError> {
    let mut sorted = Vec::new();
    sorted
        .try_reserve(entries.len())
        .map_err(|_| ProtocolError::InvalidLength)?;
    sorted.extend_from_slice(entries);
    sorted.sort_unstable_by_key(|(param_id, _)| *param_id);
    if sorted.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(ProtocolError::InvalidValue);
    }

    let mut canonical = Vec::new();
    canonical
        .try_reserve(
            sorted
                .len()
                .checked_mul(9)
                .ok_or(ProtocolError::InvalidLength)?,
        )
        .map_err(|_| ProtocolError::InvalidLength)?;
    for (param_id, value) in sorted {
        canonical.extend_from_slice(&param_id.to_le_bytes());
        canonical.push(value.param_type() as u8);
        value.encode_canonical(&mut canonical);
    }
    Ok(crc32_iso_hdlc(&canonical))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_len_rejects_an_overlong_machine_name() {
        let descriptor = ParamDescriptor {
            param_id: 1,
            param_type: ParamType::U32,
            flags: ParamFlags::WRITABLE,
            machine_name: "m".repeat(MAX_MACHINE_NAME_LEN + 1),
            display_name: "Display".into(),
            group: "Group".into(),
            unit: "unit".into(),
            default_value: ParamValue::U32(1),
            constraints: ParamConstraints::None,
        };

        assert!(matches!(
            descriptor.encoded_len(),
            Err(ProtocolError::StringTooLong)
        ));
    }
}
