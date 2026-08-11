use dctp_protocol::{CapabilityFlags, DeviceManifest, ParamState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionPhase {
    Disconnected,
    Connecting,
    LoadingManifest,
    LoadingParameters,
    Ready,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentity {
    pub device_id: [u8; 16],
    pub boot_count: u32,
    pub firmware_version: [u16; 3],
    pub sdk_version: [u16; 3],
    pub capabilities: CapabilityFlags,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticsSnapshot {
    pub inbound_bytes: u64,
    pub outbound_bytes: u64,
    pub last_rtt_ms: u64,
    pub last_valid_frame_at_ms: u64,
    pub valid_frames: u64,
    pub malformed_frames: u64,
    pub crc_errors: u64,
    pub decoder_overflows: u64,
    pub retries: u64,
    pub unsolicited_dropped: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectedDevice {
    pub phase: ConnectionPhase,
    pub session_id: u32,
    pub negotiated_max_payload: u16,
    pub identity: DeviceIdentity,
    pub manifest: DeviceManifest,
    pub parameter_states: Vec<ParamState>,
    pub diagnostics: DiagnosticsSnapshot,
}
