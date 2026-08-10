use crate::{
    crc32_iso_hdlc, ParamDescriptor, ProtocolError, TelemetryDescriptor, WireDecode, WireEncode,
    MAX_MANIFEST_LEN,
};

pub const MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const MAX_MANIFEST_PARAMETERS: usize = 64;
pub const MAX_MANIFEST_TELEMETRY: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceManifest {
    pub schema_version: u16,
    pub parameters: Vec<ParamDescriptor>,
    pub telemetry: Vec<TelemetryDescriptor>,
}

impl DeviceManifest {
    pub fn encode_canonical(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate_shape()?;
        let mut total_len = 6usize;
        for descriptor in &self.parameters {
            let record_len = descriptor.encoded_len()?;
            if record_len > u16::MAX as usize {
                return Err(ProtocolError::InvalidLength);
            }
            total_len = total_len
                .checked_add(2)
                .and_then(|len| len.checked_add(record_len))
                .ok_or(ProtocolError::InvalidLength)?;
        }
        for descriptor in &self.telemetry {
            let record_len = descriptor.encoded_len()?;
            if record_len > u16::MAX as usize {
                return Err(ProtocolError::InvalidLength);
            }
            total_len = total_len
                .checked_add(2)
                .and_then(|len| len.checked_add(record_len))
                .ok_or(ProtocolError::InvalidLength)?;
        }
        if total_len > MAX_MANIFEST_LEN {
            return Err(ProtocolError::PayloadTooLarge(total_len));
        }

        let mut parameters = self.parameters.clone();
        let mut telemetry = self.telemetry.clone();
        parameters.sort_unstable_by_key(|descriptor| descriptor.param_id);
        telemetry.sort_unstable_by_key(|descriptor| descriptor.channel_id);
        Self::ensure_unique_parameter_ids(&parameters)?;
        Self::ensure_unique_telemetry_ids(&telemetry)?;

        let mut bytes = Vec::new();
        bytes
            .try_reserve(total_len)
            .map_err(|_| ProtocolError::InvalidLength)?;
        bytes.extend_from_slice(&MANIFEST_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(parameters.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(telemetry.len() as u16).to_le_bytes());
        for descriptor in &parameters {
            let record = descriptor.encode()?;
            bytes.extend_from_slice(&(record.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        for descriptor in &telemetry {
            let record = descriptor.encode()?;
            bytes.extend_from_slice(&(record.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_MANIFEST_LEN {
            return Err(ProtocolError::PayloadTooLarge(bytes.len()));
        }
        if bytes.len() < 6 {
            return Err(ProtocolError::Truncated);
        }
        let schema_version = u16::from_le_bytes([bytes[0], bytes[1]]);
        if schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        let parameter_count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
        let telemetry_count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        if parameter_count > MAX_MANIFEST_PARAMETERS || telemetry_count > MAX_MANIFEST_TELEMETRY {
            return Err(ProtocolError::InvalidValue);
        }

        let mut offset = 6usize;
        let mut parameters = Vec::new();
        parameters
            .try_reserve(parameter_count)
            .map_err(|_| ProtocolError::InvalidLength)?;
        for _ in 0..parameter_count {
            let (record, next_offset) = Self::record_at(bytes, offset)?;
            parameters.push(ParamDescriptor::decode(record)?);
            offset = next_offset;
        }
        let mut telemetry = Vec::new();
        telemetry
            .try_reserve(telemetry_count)
            .map_err(|_| ProtocolError::InvalidLength)?;
        for _ in 0..telemetry_count {
            let (record, next_offset) = Self::record_at(bytes, offset)?;
            telemetry.push(TelemetryDescriptor::decode(record)?);
            offset = next_offset;
        }
        if offset != bytes.len() {
            return Err(ProtocolError::InvalidLength);
        }

        let value = Self {
            schema_version,
            parameters,
            telemetry,
        };
        value.validate_shape()?;
        Self::ensure_strictly_sorted_parameter_ids(&value.parameters)?;
        Self::ensure_strictly_sorted_telemetry_ids(&value.telemetry)?;
        Ok(value)
    }

    pub fn manifest_crc32(&self) -> Result<u32, ProtocolError> {
        Ok(crc32_iso_hdlc(&self.encode_canonical()?))
    }

    fn record_at(bytes: &[u8], offset: usize) -> Result<(&[u8], usize), ProtocolError> {
        let length_end = offset.checked_add(2).ok_or(ProtocolError::InvalidLength)?;
        if length_end > bytes.len() {
            return Err(ProtocolError::Truncated);
        }
        let record_len = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        let next_offset = length_end
            .checked_add(record_len)
            .ok_or(ProtocolError::InvalidLength)?;
        if next_offset > bytes.len() {
            return Err(ProtocolError::Truncated);
        }
        Ok((&bytes[length_end..next_offset], next_offset))
    }

    fn validate_shape(&self) -> Result<(), ProtocolError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if self.parameters.len() > MAX_MANIFEST_PARAMETERS
            || self.telemetry.len() > MAX_MANIFEST_TELEMETRY
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }

    fn ensure_unique_parameter_ids(parameters: &[ParamDescriptor]) -> Result<(), ProtocolError> {
        if parameters
            .windows(2)
            .any(|pair| pair[0].param_id == pair[1].param_id)
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }

    fn ensure_unique_telemetry_ids(telemetry: &[TelemetryDescriptor]) -> Result<(), ProtocolError> {
        if telemetry
            .windows(2)
            .any(|pair| pair[0].channel_id == pair[1].channel_id)
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }

    fn ensure_strictly_sorted_parameter_ids(
        parameters: &[ParamDescriptor],
    ) -> Result<(), ProtocolError> {
        if parameters
            .windows(2)
            .any(|pair| pair[0].param_id >= pair[1].param_id)
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }

    fn ensure_strictly_sorted_telemetry_ids(
        telemetry: &[TelemetryDescriptor],
    ) -> Result<(), ProtocolError> {
        if telemetry
            .windows(2)
            .any(|pair| pair[0].channel_id >= pair[1].channel_id)
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }
}

impl WireDecode for DeviceManifest {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        Self::decode(bytes)
    }
}
