use crate::{
    ProtocolError, WireDecode, WireEncode, WireReader, WireWriter, MAX_DISPLAY_NAME_LEN,
    MAX_GROUP_LEN, MAX_MACHINE_NAME_LEN, MAX_PAYLOAD_LEN, MAX_UNIT_LEN,
};

pub const MAX_TELEMETRY_CHANNELS: usize = 8;
pub const MAX_TELEMETRY_SAMPLES: usize = 16;
pub const MAX_TELEMETRY_RATE_HZ: u16 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TelemetryType {
    F32 = 1,
    I32 = 2,
    U32 = 3,
    Flags32 = 4,
}

impl TryFrom<u8> for TelemetryType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::F32),
            2 => Ok(Self::I32),
            3 => Ok(Self::U32),
            4 => Ok(Self::Flags32),
            _ => Err(ProtocolError::InvalidValue),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelemetryDescriptor {
    pub channel_id: u32,
    pub telemetry_type: TelemetryType,
    pub machine_name: String,
    pub display_name: String,
    pub group: String,
    pub unit: String,
}

impl TelemetryDescriptor {
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
        Ok(())
    }

    pub(crate) fn encoded_len(&self) -> Result<usize, ProtocolError> {
        self.validate()?;
        [
            &self.machine_name,
            &self.display_name,
            &self.group,
            &self.unit,
        ]
        .iter()
        .try_fold(5usize, |len, value| {
            len.checked_add(1)
                .and_then(|value_len| value_len.checked_add(value.len()))
                .ok_or(ProtocolError::InvalidLength)
        })
    }
}

impl WireEncode for TelemetryDescriptor {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let mut writer = WireWriter::new();
        writer.put_u32(self.channel_id);
        writer.put_u8(self.telemetry_type as u8);
        writer.put_utf8_u8_len(&self.machine_name, MAX_MACHINE_NAME_LEN)?;
        writer.put_utf8_u8_len(&self.display_name, MAX_DISPLAY_NAME_LEN)?;
        writer.put_utf8_u8_len(&self.group, MAX_GROUP_LEN)?;
        writer.put_utf8_u8_len(&self.unit, MAX_UNIT_LEN)?;
        Ok(writer.into_inner())
    }
}

impl WireDecode for TelemetryDescriptor {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut reader = WireReader::new(bytes);
        let value = Self {
            channel_id: reader.read_u32()?,
            telemetry_type: TelemetryType::try_from(reader.read_u8()?)?,
            machine_name: reader.read_utf8_u8_len(MAX_MACHINE_NAME_LEN)?,
            display_name: reader.read_utf8_u8_len(MAX_DISPLAY_NAME_LEN)?,
            group: reader.read_utf8_u8_len(MAX_GROUP_LEN)?,
            unit: reader.read_utf8_u8_len(MAX_UNIT_LEN)?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelemetrySubscription {
    pub subscription_version: u16,
    pub sample_rate_hz: u16,
    pub channel_ids: Vec<u32>,
}

impl TelemetrySubscription {
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.channel_ids.is_empty()
            || self.channel_ids.len() > MAX_TELEMETRY_CHANNELS
            || self.sample_rate_hz == 0
            || self.sample_rate_hz > MAX_TELEMETRY_RATE_HZ
        {
            return Err(ProtocolError::InvalidValue);
        }
        if self
            .channel_ids
            .iter()
            .enumerate()
            .any(|(index, id)| self.channel_ids[..index].contains(id))
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(())
    }
}

impl WireEncode for TelemetrySubscription {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let mut writer = WireWriter::new();
        writer.put_u16(self.subscription_version);
        writer.put_u16(self.sample_rate_hz);
        writer.put_u8(self.channel_ids.len() as u8);
        for channel_id in &self.channel_ids {
            writer.put_u32(*channel_id);
        }
        Ok(writer.into_inner())
    }
}

impl WireDecode for TelemetrySubscription {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < 5 {
            return Err(ProtocolError::Truncated);
        }
        let channel_count = bytes[4] as usize;
        if channel_count == 0 || channel_count > MAX_TELEMETRY_CHANNELS {
            return Err(ProtocolError::InvalidValue);
        }
        let expected_len = 5usize
            .checked_add(
                channel_count
                    .checked_mul(4)
                    .ok_or(ProtocolError::InvalidLength)?,
            )
            .ok_or(ProtocolError::InvalidLength)?;
        if bytes.len() != expected_len {
            return Err(ProtocolError::InvalidLength);
        }

        let mut reader = WireReader::new(bytes);
        let subscription_version = reader.read_u16()?;
        let sample_rate_hz = reader.read_u16()?;
        let decoded_count = reader.read_u8()? as usize;
        let mut channel_ids = Vec::new();
        channel_ids
            .try_reserve(decoded_count)
            .map_err(|_| ProtocolError::InvalidLength)?;
        for _ in 0..decoded_count {
            channel_ids.push(reader.read_u32()?);
        }
        reader.finish()?;
        let value = Self {
            subscription_version,
            sample_rate_hz,
            channel_ids,
        };
        value.validate()?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelemetrySample {
    pub dt_us: u16,
    pub values: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelemetryBatch {
    pub subscription_version: u16,
    pub first_sample_sequence: u16,
    pub dropped_samples: u16,
    pub base_timestamp_us: u32,
    pub samples: Vec<TelemetrySample>,
}

impl TelemetryBatch {
    const PREFIX_LEN: usize = 12;

    fn channel_count(&self) -> Result<usize, ProtocolError> {
        let Some(first) = self.samples.first() else {
            return Err(ProtocolError::InvalidValue);
        };
        let channel_count = first.values.len();
        if self.samples.len() > MAX_TELEMETRY_SAMPLES
            || channel_count == 0
            || channel_count > MAX_TELEMETRY_CHANNELS
            || first.dt_us != 0
            || self
                .samples
                .iter()
                .any(|sample| sample.values.len() != channel_count)
        {
            return Err(ProtocolError::InvalidValue);
        }
        Ok(channel_count)
    }

    fn encoded_len(sample_count: usize, channel_count: usize) -> Result<usize, ProtocolError> {
        let sample_len = 2usize
            .checked_add(
                channel_count
                    .checked_mul(4)
                    .ok_or(ProtocolError::InvalidLength)?,
            )
            .ok_or(ProtocolError::InvalidLength)?;
        Self::PREFIX_LEN
            .checked_add(
                sample_count
                    .checked_mul(sample_len)
                    .ok_or(ProtocolError::InvalidLength)?,
            )
            .ok_or(ProtocolError::InvalidLength)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let channel_count = self.channel_count()?;
        let encoded_len = Self::encoded_len(self.samples.len(), channel_count)?;
        if encoded_len > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(encoded_len));
        }

        let mut writer = WireWriter::new();
        writer.put_u16(self.subscription_version);
        writer.put_u16(self.first_sample_sequence);
        writer.put_u8(self.samples.len() as u8);
        writer.put_u8(channel_count as u8);
        writer.put_u16(self.dropped_samples);
        writer.put_u32(self.base_timestamp_us);
        for sample in &self.samples {
            writer.put_u16(sample.dt_us);
            for value in &sample.values {
                writer.put_u32(*value);
            }
        }
        Ok(writer.into_inner())
    }

    pub fn decode(bytes: &[u8], expected_channel_count: usize) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(bytes.len()));
        }
        if bytes.len() < Self::PREFIX_LEN {
            return Err(ProtocolError::Truncated);
        }
        let sample_count = bytes[4] as usize;
        let channel_count = bytes[5] as usize;
        if sample_count == 0
            || sample_count > MAX_TELEMETRY_SAMPLES
            || channel_count == 0
            || channel_count > MAX_TELEMETRY_CHANNELS
            || channel_count != expected_channel_count
        {
            return Err(ProtocolError::InvalidValue);
        }
        let expected_len = Self::encoded_len(sample_count, channel_count)?;
        if bytes.len() != expected_len {
            return Err(ProtocolError::InvalidLength);
        }

        let mut reader = WireReader::new(bytes);
        let subscription_version = reader.read_u16()?;
        let first_sample_sequence = reader.read_u16()?;
        let decoded_sample_count = reader.read_u8()? as usize;
        let decoded_channel_count = reader.read_u8()? as usize;
        let dropped_samples = reader.read_u16()?;
        let base_timestamp_us = reader.read_u32()?;
        let mut samples = Vec::new();
        samples
            .try_reserve(decoded_sample_count)
            .map_err(|_| ProtocolError::InvalidLength)?;
        for index in 0..decoded_sample_count {
            let dt_us = reader.read_u16()?;
            if index == 0 && dt_us != 0 {
                return Err(ProtocolError::InvalidValue);
            }
            let mut values = Vec::new();
            values
                .try_reserve(decoded_channel_count)
                .map_err(|_| ProtocolError::InvalidLength)?;
            for _ in 0..decoded_channel_count {
                values.push(reader.read_u32()?);
            }
            samples.push(TelemetrySample { dt_us, values });
        }
        reader.finish()?;
        Ok(Self {
            subscription_version,
            first_sample_sequence,
            dropped_samples,
            base_timestamp_us,
            samples,
        })
    }
}

impl WireEncode for TelemetryBatch {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        Self::encode(self)
    }
}
