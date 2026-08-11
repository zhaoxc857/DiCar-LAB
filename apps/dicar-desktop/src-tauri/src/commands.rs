use std::net::SocketAddr;

use dicar_app_core::{CoreCommand, Endpoint, OperationResult};
use serde::Deserialize;

use crate::{AppState, BridgeErrorDto};

#[cfg(any(target_env = "msvc", feature = "native-check"))]
use crate::{CloseDecision, CloseResolution, FrontendEvent, FrontendSink};
#[cfg(any(target_env = "msvc", feature = "native-check"))]
use dicar_app_core::{AccessProfileDto, AccessRoleDto, AppSnapshot, ParamValueDto};
#[cfg(any(target_env = "msvc", feature = "native-check"))]
use std::sync::Arc;
#[cfg(any(target_env = "msvc", feature = "native-check"))]
use tauri::{ipc::Channel, State, WebviewWindow};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EndpointDto {
    Simulator { address: String },
}

impl EndpointDto {
    fn into_core(self) -> Result<Endpoint, BridgeErrorDto> {
        match self {
            Self::Simulator { address } => {
                let address = address.parse::<SocketAddr>().map_err(|_| {
                    BridgeErrorDto::new("invalidEndpoint", "模拟器地址必须是有效的 IP:端口")
                })?;
                Ok(Endpoint::Simulator { address })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[cfg(any(target_env = "msvc", feature = "native-check"))]
pub enum ParameterValueDto {
    F32(f32),
    I32(i32),
    U32(u32),
    Bool(bool),
    Enum(i32),
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
impl From<ParameterValueDto> for ParamValueDto {
    fn from(value: ParameterValueDto) -> Self {
        match value {
            ParameterValueDto::F32(value) => Self::F32(value),
            ParameterValueDto::I32(value) => Self::I32(value),
            ParameterValueDto::U32(value) => Self::U32(value),
            ParameterValueDto::Bool(value) => Self::Bool(value),
            ParameterValueDto::Enum(value) => Self::Enum(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg(any(target_env = "msvc", feature = "native-check"))]
pub enum AccessProfileId {
    Owner,
    Tuner,
    Observer,
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
impl AccessProfileId {
    fn into_core(self) -> AccessProfileDto {
        AccessProfileDto {
            role: match self {
                Self::Owner => AccessRoleDto::Owner,
                Self::Tuner => AccessRoleDto::Tuner,
                Self::Observer => AccessRoleDto::Observer,
            },
            lease_active: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg(any(target_env = "msvc", feature = "native-check"))]
pub struct TelemetrySubscriptionRequestDto {
    pub channel_ids: Vec<u32>,
    pub sample_rate_hz: u16,
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
struct TauriChannelSink(Channel<FrontendEvent>);

#[cfg(any(target_env = "msvc", feature = "native-check"))]
impl FrontendSink for TauriChannelSink {
    fn send(&self, event: FrontendEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

pub fn connect_core(
    state: &AppState,
    endpoint: EndpointDto,
) -> Result<OperationResult, BridgeErrorDto> {
    let endpoint = endpoint.into_core()?;
    if &endpoint != state.configured_endpoint() {
        return Err(BridgeErrorDto::new(
            "endpointMismatch",
            "当前核心服务已绑定到另一个模拟器地址",
        ));
    }
    state.dispatch(CoreCommand::Connect)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn connect(
    state: State<'_, AppState>,
    endpoint: EndpointDto,
) -> Result<OperationResult, BridgeErrorDto> {
    connect_core(state.inner(), endpoint)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn disconnect(state: State<'_, AppState>) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::Disconnect)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn write_parameter(
    state: State<'_, AppState>,
    param_id: u32,
    value: ParameterValueDto,
) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::WriteParameter {
        param_id,
        value: value.into(),
    })
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn commit_parameters(state: State<'_, AppState>) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::CommitParameters)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn revert_all(state: State<'_, AppState>) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::RevertAllPendingChanges)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn undo_last(state: State<'_, AppState>) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::UndoLastConfirmedChange)
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn set_telemetry_subscription(
    state: State<'_, AppState>,
    request: TelemetrySubscriptionRequestDto,
) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::SetTelemetrySubscription {
        channel_ids: request.channel_ids,
        sample_rate_hz: request.sample_rate_hz,
    })
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn set_paused(
    state: State<'_, AppState>,
    paused: bool,
) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::SetPaused { paused })
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn add_marker(
    state: State<'_, AppState>,
    label: String,
) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::AddMarker { label })
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn select_access_profile(
    state: State<'_, AppState>,
    profile: AccessProfileId,
) -> Result<OperationResult, BridgeErrorDto> {
    state.dispatch(CoreCommand::SelectAccessProfile {
        profile: profile.into_core(),
    })
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> AppSnapshot {
    state.snapshot()
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn open_core_channel(
    state: State<'_, AppState>,
    on_event: Channel<FrontendEvent>,
) -> Result<(), BridgeErrorDto> {
    state.replace_frontend_sink(Arc::new(TauriChannelSink(on_event)))
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn close_core_channel(state: State<'_, AppState>) {
    state.close_frontend_sink();
}

#[cfg(any(target_env = "msvc", feature = "native-check"))]
#[tauri::command]
pub fn resolve_window_close(
    window: WebviewWindow,
    state: State<'_, AppState>,
    request_id: u64,
    decision: CloseDecision,
) -> Result<OperationResult, BridgeErrorDto> {
    match state.resolve_window_close(request_id, decision)? {
        CloseResolution::KeepOpen => state.complete_bridge_operation("已取消关闭"),
        CloseResolution::CloseWindow => {
            let result = state.complete_bridge_operation("窗口可安全关闭")?;
            window.destroy().map_err(|error| {
                BridgeErrorDto::new("windowDestroyFailed", format!("无法关闭窗口：{error}"))
            })?;
            Ok(result)
        }
    }
}
