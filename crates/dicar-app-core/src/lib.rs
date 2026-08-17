mod access;
mod actor;
mod bridge_model;
mod clock;
mod error;
mod firmware_flash;
mod hardware_profile;
mod link_budget;
mod model;
mod parameter_workspace;
mod session;
mod telemetry_engine;
pub mod transport;

pub use access::{AccessProfile, AccessRole, LeaseState, PermissionDecision};
pub use actor::{
    ActorError, ActorSendError, AppActorHandle, CoreCommand, CoreConfig, CoreEventReceiver,
};
pub use bridge_model::{
    AccessProfileDto, AccessRoleDto, ActorDiagnosticsSnapshot, AppSnapshot, BridgeError,
    ConnectionLoss, CoreEvent, CoreEventPayload, OperationId, OperationResult, OperationStatus,
    ParamValueDto, ParameterSnapshot, SnapshotPhase, TelemetryDescriptorDto,
    TelemetrySubscriptionSnapshot,
};
pub use clock::{Clock, FixedNonce, NonceSource, SystemClock, SystemNonce, TestClock};
pub use error::{CoreError, TransportError};
pub use firmware_flash::{validate_firmware_flash_start, FirmwareFlashStartError};
pub use hardware_profile::{SerialHardwareProfile, TelemetryBudget};
pub use link_budget::{link_budget, validate_subscription, LinkBudgetError, LinkBudgetSnapshot};
pub use model::{ConnectedDevice, ConnectionPhase, DeviceIdentity, DiagnosticsSnapshot};
pub use parameter_workspace::{
    CommitFailureKind, CommitPlan, ConfirmedChange, DeviceSyncState, OperationToken,
    ParameterRecord, ParameterWorkspace, PendingWrite, RevertPlan, RevertReport, WorkspaceError,
    WorkspaceGeneration, WriteFailure, WriteState,
};
pub use session::{decode_revision_conflict_context, ProtocolSession};
pub use telemetry_engine::{
    TelemetryDiagnostics, TelemetryEngine, TelemetryError, TelemetryPoint, TelemetryValue,
    UiTelemetryBatch,
};
pub use transport::{
    available_serial_ports, ActiveTransport, Endpoint, SerialPortDescriptor, SerialPortKind,
    SerialTransport, TcpTransport, Transport, TransportIdentity,
};
