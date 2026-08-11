use serde::{Deserialize, Serialize};

const NANO_UART_WL_BAUD_RATES: [u32; 3] = [460_800, 230_400, 115_200];
const HC05_BAUD_RATES: [u32; 6] = [115_200, 9_600, 38_400, 57_600, 230_400, 460_800];
const GENERIC_BAUD_RATES: [u32; 1] = [115_200];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SerialHardwareProfile {
    NanoUartWl,
    Hc05BluetoothSpp,
    GenericSerial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryBudget {
    pub max_channels: u8,
    pub max_sample_rate_hz: u16,
}

impl SerialHardwareProfile {
    pub const fn recommended_baud_rate(self) -> u32 {
        match self {
            Self::NanoUartWl => 460_800,
            Self::Hc05BluetoothSpp | Self::GenericSerial => 115_200,
        }
    }

    pub const fn probe_baud_rates(self) -> &'static [u32] {
        match self {
            Self::NanoUartWl => &NANO_UART_WL_BAUD_RATES,
            Self::Hc05BluetoothSpp => &HC05_BAUD_RATES,
            Self::GenericSerial => &GENERIC_BAUD_RATES,
        }
    }

    pub const fn telemetry_budget(self, baud_rate: u32) -> TelemetryBudget {
        match self {
            Self::Hc05BluetoothSpp if baud_rate <= 9_600 => TelemetryBudget {
                max_channels: 2,
                max_sample_rate_hz: 10,
            },
            Self::Hc05BluetoothSpp => TelemetryBudget {
                max_channels: 4,
                max_sample_rate_hz: 50,
            },
            Self::NanoUartWl if baud_rate >= 460_800 => TelemetryBudget {
                max_channels: 8,
                max_sample_rate_hz: 500,
            },
            Self::NanoUartWl => generic_budget(baud_rate),
            Self::GenericSerial => generic_budget(baud_rate),
        }
    }
}

const fn generic_budget(baud_rate: u32) -> TelemetryBudget {
    if baud_rate <= 9_600 {
        TelemetryBudget {
            max_channels: 2,
            max_sample_rate_hz: 10,
        }
    } else if baud_rate <= 57_600 {
        TelemetryBudget {
            max_channels: 4,
            max_sample_rate_hz: 25,
        }
    } else if baud_rate <= 115_200 {
        TelemetryBudget {
            max_channels: 4,
            max_sample_rate_hz: 50,
        }
    } else if baud_rate <= 230_400 {
        TelemetryBudget {
            max_channels: 8,
            max_sample_rate_hz: 100,
        }
    } else {
        TelemetryBudget {
            max_channels: 8,
            max_sample_rate_hz: 500,
        }
    }
}
