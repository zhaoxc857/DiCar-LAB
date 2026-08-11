mod access;
mod actor;
mod bridge_model;
mod clock;
mod error;
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
pub use transport::{Endpoint, TcpTransport, Transport, TransportIdentity};
