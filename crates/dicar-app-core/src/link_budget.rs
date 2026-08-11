use std::fmt;

use serde::Serialize;

use crate::{Endpoint, SerialHardwareProfile, TelemetryBudget};

const HIGH_RATE_BUDGET: TelemetryBudget = TelemetryBudget {
    max_channels: 8,
    max_sample_rate_hz: 500,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkBudgetSnapshot {
    pub hardware_profile: Option<SerialHardwareProfile>,
    pub baud_rate: Option<u32>,
    pub max_channels: u8,
    pub max_sample_rate_hz: u16,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkBudgetError {
    TooManyChannels {
        label: &'static str,
        maximum: u8,
    },
    SampleRateTooHigh {
        label: &'static str,
        maximum_hz: u16,
    },
}

impl fmt::Display for LinkBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyChannels { label, maximum } => {
                write!(formatter, "{label} 当前链路最多 {maximum} 个通道")
            }
            Self::SampleRateTooHigh { label, maximum_hz } => {
                write!(formatter, "{label} 当前链路最高 {maximum_hz} Hz")
            }
        }
    }
}

impl std::error::Error for LinkBudgetError {}

pub fn link_budget(endpoint: &Endpoint) -> LinkBudgetSnapshot {
    match endpoint {
        Endpoint::Simulator { .. } => LinkBudgetSnapshot {
            hardware_profile: None,
            baud_rate: None,
            max_channels: HIGH_RATE_BUDGET.max_channels,
            max_sample_rate_hz: HIGH_RATE_BUDGET.max_sample_rate_hz,
            reason: "内置模拟器支持完整 8 通道 × 500 Hz 遥测".into(),
        },
        Endpoint::Serial {
            baud_rate,
            hardware_profile,
            ..
        } => {
            let budget = hardware_profile.telemetry_budget(*baud_rate);
            LinkBudgetSnapshot {
                hardware_profile: Some(*hardware_profile),
                baud_rate: Some(*baud_rate),
                max_channels: budget.max_channels,
                max_sample_rate_hz: budget.max_sample_rate_hz,
                reason: format!(
                    "{} @ {} baud：最多 {} 通道 × {} Hz",
                    profile_label(*hardware_profile),
                    baud_rate,
                    budget.max_channels,
                    budget.max_sample_rate_hz
                ),
            }
        }
    }
}

pub fn validate_subscription(
    endpoint: &Endpoint,
    channel_count: usize,
    sample_rate_hz: u16,
) -> Result<TelemetryBudget, LinkBudgetError> {
    let snapshot = link_budget(endpoint);
    let label = snapshot
        .hardware_profile
        .map(profile_label)
        .unwrap_or("内置模拟器");
    if channel_count > usize::from(snapshot.max_channels) {
        return Err(LinkBudgetError::TooManyChannels {
            label,
            maximum: snapshot.max_channels,
        });
    }
    if sample_rate_hz > snapshot.max_sample_rate_hz {
        return Err(LinkBudgetError::SampleRateTooHigh {
            label,
            maximum_hz: snapshot.max_sample_rate_hz,
        });
    }
    Ok(TelemetryBudget {
        max_channels: snapshot.max_channels,
        max_sample_rate_hz: snapshot.max_sample_rate_hz,
    })
}

const fn profile_label(profile: SerialHardwareProfile) -> &'static str {
    match profile {
        SerialHardwareProfile::NanoUartWl => "nanoUART-wl",
        SerialHardwareProfile::Hc05BluetoothSpp => "HC-05",
        SerialHardwareProfile::GenericSerial => "通用串口",
    }
}
