use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    time::Duration,
};

use dctp_protocol::{
    TelemetryBatch, TelemetryDescriptor, TelemetrySubscription, TelemetryType,
    MAX_TELEMETRY_CHANNELS, MAX_TELEMETRY_RATE_HZ,
};
use serde::Serialize;

const MAX_SAMPLE_RATE_HZ: u128 = MAX_TELEMETRY_RATE_HZ as u128;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum TelemetryValue {
    F32(f32),
    I32(i32),
    U32(u32),
    Flags32(u32),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryPoint {
    pub channel_id: u32,
    pub timestamp_us: u64,
    pub sample_sequence: u16,
    pub value: TelemetryValue,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiTelemetryBatch {
    pub subscription_version: u16,
    pub first_sample_sequence: u16,
    pub dropped_samples: u16,
    pub points: Vec<TelemetryPoint>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryDiagnostics {
    pub accepted_batches: u64,
    pub rejected_batches: u64,
    pub sequence_gap_samples: u64,
    pub device_dropped_samples: u64,
    pub evicted_points: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TelemetryError {
    InvalidSubscription,
    TooManyChannels { limit: usize, actual: usize },
    DuplicateChannel(u32),
    UnknownChannel(u32),
    StaleSubscription { expected: u16, actual: u16 },
    ChannelWidth { expected: usize, actual: usize },
    InvalidBatch,
    TimestampRegression,
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TelemetryError {}

#[derive(Clone, Debug)]
struct ActiveChannel {
    descriptor: TelemetryDescriptor,
    points: VecDeque<TelemetryPoint>,
}

#[derive(Clone, Debug)]
pub struct TelemetryEngine {
    max_channels: usize,
    max_points_per_channel: usize,
    subscription: Option<TelemetrySubscription>,
    channels: BTreeMap<u32, ActiveChannel>,
    next_sample_sequence: Option<u16>,
    timestamp_epoch: u64,
    last_base_timestamp_raw: Option<u32>,
    latest_timestamp_us: Option<u64>,
    diagnostics: TelemetryDiagnostics,
}

impl Default for TelemetryEngine {
    fn default() -> Self {
        Self::new(Duration::from_secs(60), MAX_TELEMETRY_CHANNELS)
    }
}

impl TelemetryEngine {
    pub fn new(retention: Duration, max_channels: usize) -> Self {
        let max_channels = max_channels.clamp(1, MAX_TELEMETRY_CHANNELS);
        let points = retention
            .as_micros()
            .saturating_mul(MAX_SAMPLE_RATE_HZ)
            .checked_div(1_000_000)
            .unwrap_or(0)
            .max(1)
            .min(usize::MAX as u128) as usize;
        Self {
            max_channels,
            max_points_per_channel: points,
            subscription: None,
            channels: BTreeMap::new(),
            next_sample_sequence: None,
            timestamp_epoch: 0,
            last_base_timestamp_raw: None,
            latest_timestamp_us: None,
            diagnostics: TelemetryDiagnostics::default(),
        }
    }

    pub fn activate(
        &mut self,
        subscription: TelemetrySubscription,
        descriptors: &[TelemetryDescriptor],
    ) -> Result<(), TelemetryError> {
        if subscription.subscription_version == 0
            || subscription.sample_rate_hz == 0
            || subscription.sample_rate_hz > MAX_TELEMETRY_RATE_HZ
            || subscription.channel_ids.is_empty()
        {
            return Err(TelemetryError::InvalidSubscription);
        }
        if subscription.channel_ids.len() > self.max_channels {
            return Err(TelemetryError::TooManyChannels {
                limit: self.max_channels,
                actual: subscription.channel_ids.len(),
            });
        }
        let descriptor_map = descriptors
            .iter()
            .map(|descriptor| (descriptor.channel_id, descriptor))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();
        let mut channels = BTreeMap::new();
        for channel_id in &subscription.channel_ids {
            if !seen.insert(*channel_id) {
                return Err(TelemetryError::DuplicateChannel(*channel_id));
            }
            let descriptor = descriptor_map
                .get(channel_id)
                .ok_or(TelemetryError::UnknownChannel(*channel_id))?;
            channels.insert(
                *channel_id,
                ActiveChannel {
                    descriptor: (*descriptor).clone(),
                    points: VecDeque::new(),
                },
            );
        }
        self.subscription = Some(subscription);
        self.channels = channels;
        self.next_sample_sequence = None;
        self.timestamp_epoch = 0;
        self.last_base_timestamp_raw = None;
        self.latest_timestamp_us = None;
        self.diagnostics = TelemetryDiagnostics::default();
        Ok(())
    }

    pub fn accept(&mut self, batch: TelemetryBatch) -> Result<UiTelemetryBatch, TelemetryError> {
        let (subscription_version, channel_ids) = self
            .subscription
            .as_ref()
            .map(|subscription| {
                (
                    subscription.subscription_version,
                    subscription.channel_ids.clone(),
                )
            })
            .ok_or(TelemetryError::InvalidSubscription)?;
        if batch.subscription_version != subscription_version {
            self.diagnostics.rejected_batches = self.diagnostics.rejected_batches.saturating_add(1);
            return Err(TelemetryError::StaleSubscription {
                expected: subscription_version,
                actual: batch.subscription_version,
            });
        }
        if batch.samples.is_empty() || batch.samples[0].dt_us != 0 {
            self.diagnostics.rejected_batches = self.diagnostics.rejected_batches.saturating_add(1);
            return Err(TelemetryError::InvalidBatch);
        }
        let expected_width = channel_ids.len();
        if let Some(actual) = batch
            .samples
            .iter()
            .map(|sample| sample.values.len())
            .find(|actual| *actual != expected_width)
        {
            self.diagnostics.rejected_batches = self.diagnostics.rejected_batches.saturating_add(1);
            return Err(TelemetryError::ChannelWidth {
                expected: expected_width,
                actual,
            });
        }

        let base_timestamp_us = self.unwrap_timestamp(batch.base_timestamp_us)?;
        let mut timestamp_us = base_timestamp_us;
        let mut points = Vec::with_capacity(batch.samples.len().saturating_mul(expected_width));
        for (sample_index, sample) in batch.samples.iter().enumerate() {
            if sample_index != 0 {
                timestamp_us = timestamp_us.saturating_add(u64::from(sample.dt_us));
            }
            let sample_sequence = batch
                .first_sample_sequence
                .wrapping_add(sample_index as u16);
            for (slot, channel_id) in sample.values.iter().zip(&channel_ids) {
                let channel = self
                    .channels
                    .get(channel_id)
                    .ok_or(TelemetryError::UnknownChannel(*channel_id))?;
                points.push(TelemetryPoint {
                    channel_id: *channel_id,
                    timestamp_us,
                    sample_sequence,
                    value: decode_value(channel.descriptor.telemetry_type, *slot),
                });
            }
        }

        if let Some(expected) = self.next_sample_sequence {
            self.diagnostics.sequence_gap_samples = self
                .diagnostics
                .sequence_gap_samples
                .saturating_add(u64::from(
                    batch.first_sample_sequence.wrapping_sub(expected),
                ));
        }
        self.next_sample_sequence = Some(
            batch
                .first_sample_sequence
                .wrapping_add(batch.samples.len() as u16),
        );
        self.last_base_timestamp_raw = Some(batch.base_timestamp_us);
        self.latest_timestamp_us = Some(timestamp_us);
        self.diagnostics.accepted_batches = self.diagnostics.accepted_batches.saturating_add(1);
        self.diagnostics.device_dropped_samples = self
            .diagnostics
            .device_dropped_samples
            .saturating_add(u64::from(batch.dropped_samples));

        for point in &points {
            let channel = self
                .channels
                .get_mut(&point.channel_id)
                .ok_or(TelemetryError::UnknownChannel(point.channel_id))?;
            if channel.points.len() == self.max_points_per_channel {
                channel.points.pop_front();
                self.diagnostics.evicted_points = self.diagnostics.evicted_points.saturating_add(1);
            }
            channel.points.push_back(point.clone());
        }

        Ok(UiTelemetryBatch {
            subscription_version: batch.subscription_version,
            first_sample_sequence: batch.first_sample_sequence,
            dropped_samples: batch.dropped_samples,
            points,
        })
    }

    fn unwrap_timestamp(&mut self, raw: u32) -> Result<u64, TelemetryError> {
        if let Some(last_raw) = self.last_base_timestamp_raw {
            if raw < last_raw {
                if last_raw.wrapping_sub(raw) > u32::MAX / 2 {
                    self.timestamp_epoch = self.timestamp_epoch.saturating_add(1_u64 << 32);
                } else {
                    self.diagnostics.rejected_batches =
                        self.diagnostics.rejected_batches.saturating_add(1);
                    return Err(TelemetryError::TimestampRegression);
                }
            }
        }
        Ok(self.timestamp_epoch.saturating_add(u64::from(raw)))
    }

    pub fn latest_timestamp_us(&self) -> Option<u64> {
        self.latest_timestamp_us
    }

    pub fn channel_len(&self, channel_id: u32) -> usize {
        self.channels
            .get(&channel_id)
            .map_or(0, |channel| channel.points.len())
    }

    pub fn total_points(&self) -> usize {
        self.channels
            .values()
            .map(|channel| channel.points.len())
            .sum()
    }

    pub fn diagnostics(&self) -> TelemetryDiagnostics {
        self.diagnostics
    }

    pub fn channel_points(&self, channel_id: u32) -> Option<&VecDeque<TelemetryPoint>> {
        self.channels
            .get(&channel_id)
            .map(|channel| &channel.points)
    }
}

fn decode_value(telemetry_type: TelemetryType, raw: u32) -> TelemetryValue {
    match telemetry_type {
        TelemetryType::F32 => TelemetryValue::F32(f32::from_bits(raw)),
        TelemetryType::I32 => TelemetryValue::I32(raw as i32),
        TelemetryType::U32 => TelemetryValue::U32(raw),
        TelemetryType::Flags32 => TelemetryValue::Flags32(raw),
    }
}
