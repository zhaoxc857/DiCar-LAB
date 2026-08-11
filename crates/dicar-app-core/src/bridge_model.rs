use dctp_protocol::{ParamFlags, ParamValue, TelemetryDescriptor};
use serde::Serialize;

use crate::{
    AccessProfile, AccessRole, ConnectionPhase, DeviceSyncState, LeaseState, ParameterWorkspace,
    TelemetryDiagnostics, TransportIdentity, UiTelemetryBatch, WriteState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct OperationId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotPhase {
    Disconnected,
    Connecting,
    LoadingManifest,
    LoadingParameters,
    Ready,
}

impl From<ConnectionPhase> for SnapshotPhase {
    fn from(value: ConnectionPhase) -> Self {
        match value {
            ConnectionPhase::Disconnected => Self::Disconnected,
            ConnectionPhase::Connecting => Self::Connecting,
            ConnectionPhase::LoadingManifest => Self::LoadingManifest,
            ConnectionPhase::LoadingParameters => Self::LoadingParameters,
            ConnectionPhase::Ready => Self::Ready,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ParamValueDto {
    F32(f32),
    I32(i32),
    U32(u32),
    Bool(bool),
    Enum(i32),
}

impl From<&ParamValue> for ParamValueDto {
    fn from(value: &ParamValue) -> Self {
        match value {
            ParamValue::F32(value) => Self::F32(*value),
            ParamValue::I32(value) => Self::I32(*value),
            ParamValue::U32(value) => Self::U32(*value),
            ParamValue::Bool(value) => Self::Bool(*value),
            ParamValue::Enum(value) => Self::Enum(*value),
        }
    }
}

impl From<ParamValueDto> for ParamValue {
    fn from(value: ParamValueDto) -> Self {
        match value {
            ParamValueDto::F32(value) => Self::F32(value),
            ParamValueDto::I32(value) => Self::I32(value),
            ParamValueDto::U32(value) => Self::U32(value),
            ParamValueDto::Bool(value) => Self::Bool(value),
            ParamValueDto::Enum(value) => Self::Enum(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessRoleDto {
    Owner,
    Tuner,
    Observer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessProfileDto {
    pub role: AccessRoleDto,
    pub lease_active: bool,
}

impl From<AccessProfile> for AccessProfileDto {
    fn from(value: AccessProfile) -> Self {
        Self {
            role: match value.role {
                AccessRole::Owner => AccessRoleDto::Owner,
                AccessRole::Tuner => AccessRoleDto::Tuner,
                AccessRole::Observer => AccessRoleDto::Observer,
            },
            lease_active: value.lease == LeaseState::Active,
        }
    }
}

impl From<AccessProfileDto> for AccessProfile {
    fn from(value: AccessProfileDto) -> Self {
        AccessProfile::new(
            match value.role {
                AccessRoleDto::Owner => AccessRole::Owner,
                AccessRoleDto::Tuner => AccessRole::Tuner,
                AccessRoleDto::Observer => AccessRole::Observer,
            },
            if value.lease_active {
                LeaseState::Active
            } else {
                LeaseState::Inactive
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterSnapshot {
    pub param_id: u32,
    pub machine_name: String,
    pub display_name: String,
    pub group: String,
    pub unit: String,
    pub ram_value: ParamValueDto,
    pub persisted_value: Option<ParamValueDto>,
    pub revision: u32,
    pub dirty: bool,
    pub sync_known: bool,
    pub write_state: String,
    pub writable: bool,
    pub dangerous: bool,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryDescriptorDto {
    pub channel_id: u32,
    pub telemetry_type: String,
    pub machine_name: String,
    pub display_name: String,
    pub group: String,
    pub unit: String,
}

impl From<&TelemetryDescriptor> for TelemetryDescriptorDto {
    fn from(value: &TelemetryDescriptor) -> Self {
        Self {
            channel_id: value.channel_id,
            telemetry_type: format!("{:?}", value.telemetry_type),
            machine_name: value.machine_name.clone(),
            display_name: value.display_name.clone(),
            group: value.group.clone(),
            unit: value.unit.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySubscriptionSnapshot {
    pub subscription_version: u16,
    pub sample_rate_hz: u16,
    pub channel_ids: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorDiagnosticsSnapshot {
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
    pub sequence_gap_samples: u64,
    pub device_dropped_samples: u64,
    pub rejected_telemetry_batches: u64,
    pub ui_dropped_batches: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub revision: u64,
    pub phase: SnapshotPhase,
    pub transport_identity: Option<TransportIdentity>,
    pub session_id: Option<u32>,
    pub device_id_hex: Option<String>,
    pub firmware_version: Option<[u16; 3]>,
    pub parameters: Vec<ParameterSnapshot>,
    pub telemetry_descriptors: Vec<TelemetryDescriptorDto>,
    pub dirty_count: usize,
    pub storage_generation: u32,
    pub access_profile: AccessProfileDto,
    pub desired_subscription: Option<TelemetrySubscriptionSnapshot>,
    pub active_subscription: Option<TelemetrySubscriptionSnapshot>,
    pub paused: bool,
    pub telemetry_points: usize,
    pub diagnostics: ActorDiagnosticsSnapshot,
    pub last_disconnect_reason: Option<String>,
    pub markers: Vec<String>,
}

impl AppSnapshot {
    pub(crate) fn disconnected(access_profile: AccessProfile) -> Self {
        Self {
            revision: 0,
            phase: SnapshotPhase::Disconnected,
            transport_identity: None,
            session_id: None,
            device_id_hex: None,
            firmware_version: None,
            parameters: Vec::new(),
            telemetry_descriptors: Vec::new(),
            dirty_count: 0,
            storage_generation: 0,
            access_profile: access_profile.into(),
            desired_subscription: None,
            active_subscription: None,
            paused: false,
            telemetry_points: 0,
            diagnostics: ActorDiagnosticsSnapshot {
                inbound_bytes: 0,
                outbound_bytes: 0,
                last_rtt_ms: 0,
                last_valid_frame_at_ms: 0,
                valid_frames: 0,
                malformed_frames: 0,
                crc_errors: 0,
                decoder_overflows: 0,
                retries: 0,
                unsolicited_dropped: 0,
                sequence_gap_samples: 0,
                device_dropped_samples: 0,
                rejected_telemetry_batches: 0,
                ui_dropped_batches: 0,
            },
            last_disconnect_reason: None,
            markers: Vec::new(),
        }
    }
}

pub(crate) fn parameter_snapshots(
    workspace: Option<&ParameterWorkspace>,
) -> Vec<ParameterSnapshot> {
    workspace
        .into_iter()
        .flat_map(ParameterWorkspace::records)
        .map(|record| ParameterSnapshot {
            param_id: record.descriptor.param_id,
            machine_name: record.descriptor.machine_name.clone(),
            display_name: record.descriptor.display_name.clone(),
            group: record.descriptor.group.clone(),
            unit: record.descriptor.unit.clone(),
            ram_value: (&record.ram_value).into(),
            persisted_value: record.persisted_value.as_ref().map(Into::into),
            revision: record.revision,
            dirty: record.dirty,
            sync_known: record.sync_state == DeviceSyncState::Known,
            write_state: match record.write_state {
                WriteState::Idle => "idle",
                WriteState::InFlight => "inFlight",
                WriteState::Queued => "queued",
            }
            .into(),
            writable: record.descriptor.flags.bits() & ParamFlags::WRITABLE.bits() != 0,
            dangerous: record.descriptor.flags.bits() & ParamFlags::DANGEROUS.bits() != 0,
            last_error: record.last_error.map(str::to_owned),
        })
        .collect()
}

pub(crate) fn merge_diagnostics(
    protocol: crate::DiagnosticsSnapshot,
    telemetry: TelemetryDiagnostics,
    ui_dropped_batches: u64,
) -> ActorDiagnosticsSnapshot {
    ActorDiagnosticsSnapshot {
        inbound_bytes: protocol.inbound_bytes,
        outbound_bytes: protocol.outbound_bytes,
        last_rtt_ms: protocol.last_rtt_ms,
        last_valid_frame_at_ms: protocol.last_valid_frame_at_ms,
        valid_frames: protocol.valid_frames,
        malformed_frames: protocol.malformed_frames,
        crc_errors: protocol.crc_errors,
        decoder_overflows: protocol.decoder_overflows,
        retries: protocol.retries,
        unsolicited_dropped: protocol.unsolicited_dropped,
        sequence_gap_samples: telemetry.sequence_gap_samples,
        device_dropped_samples: telemetry.device_dropped_samples,
        rejected_telemetry_batches: telemetry.rejected_batches,
        ui_dropped_batches,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationStatus {
    Succeeded,
    Failed,
    Superseded,
    Aborted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub operation_id: OperationId,
    pub status: OperationStatus,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionLoss {
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeError {
    pub code: String,
    pub message: String,
    pub operation_id: Option<OperationId>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum CoreEventPayload {
    SnapshotChanged(Box<AppSnapshot>),
    TelemetryBatch(UiTelemetryBatch),
    OperationCompleted(OperationResult),
    ConnectionLost(ConnectionLoss),
    FatalError(BridgeError),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreEvent {
    pub actor_order: u64,
    pub payload: CoreEventPayload,
}
