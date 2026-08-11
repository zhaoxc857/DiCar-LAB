use std::sync::{Arc, Mutex};

use dicar_app_core::{
    AppSnapshot, BridgeError, ConnectionLoss, CoreEvent, CoreEventPayload, OperationResult,
    UiTelemetryBatch,
};
use serde::Serialize;

use crate::BridgeErrorDto;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowCloseRequest {
    pub request_id: u64,
    pub dirty_count: usize,
    pub can_revert: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum FrontendEventPayload {
    SnapshotChanged(Box<AppSnapshot>),
    TelemetryBatch(UiTelemetryBatch),
    OperationCompleted(OperationResult),
    ConnectionLost(ConnectionLoss),
    FatalError(BridgeError),
    WindowCloseRequested(WindowCloseRequest),
}

impl From<CoreEventPayload> for FrontendEventPayload {
    fn from(payload: CoreEventPayload) -> Self {
        match payload {
            CoreEventPayload::SnapshotChanged(snapshot) => Self::SnapshotChanged(snapshot),
            CoreEventPayload::TelemetryBatch(batch) => Self::TelemetryBatch(batch),
            CoreEventPayload::OperationCompleted(result) => Self::OperationCompleted(result),
            CoreEventPayload::ConnectionLost(loss) => Self::ConnectionLost(loss),
            CoreEventPayload::FatalError(error) => Self::FatalError(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendEvent {
    pub event_index: u64,
    #[serde(flatten)]
    pub payload: FrontendEventPayload,
}

pub trait FrontendSink: Send + Sync + 'static {
    fn send(&self, event: FrontendEvent) -> Result<(), String>;
}

#[derive(Default)]
struct SequencerState {
    next_event_index: u64,
    sink: Option<Arc<dyn FrontendSink>>,
}

#[derive(Default)]
pub struct FrontendEventSequencer {
    state: Mutex<SequencerState>,
}

impl FrontendEventSequencer {
    pub fn replace_sink(&self, sink: Arc<dyn FrontendSink>) -> Result<(), BridgeErrorDto> {
        lock(&self.state).sink = Some(sink);
        Ok(())
    }

    pub fn close_sink(&self) {
        lock(&self.state).sink = None;
    }

    pub fn publish_core(&self, event: CoreEvent) -> Result<Option<u64>, BridgeErrorDto> {
        self.publish(event.payload.into())
    }

    pub fn publish_window_close(
        &self,
        request_id: u64,
        dirty_count: usize,
        can_revert: bool,
    ) -> Result<Option<u64>, BridgeErrorDto> {
        self.publish(FrontendEventPayload::WindowCloseRequested(
            WindowCloseRequest {
                request_id,
                dirty_count,
                can_revert,
            },
        ))
    }

    fn publish(&self, payload: FrontendEventPayload) -> Result<Option<u64>, BridgeErrorDto> {
        let mut state = lock(&self.state);
        let Some(sink) = state.sink.as_ref().cloned() else {
            return Ok(None);
        };
        let event_index = state
            .next_event_index
            .checked_add(1)
            .ok_or_else(|| BridgeErrorDto::new("eventIndexExhausted", "前端事件序号已耗尽"))?;
        sink.send(FrontendEvent {
            event_index,
            payload,
        })
        .map_err(|message| BridgeErrorDto::new("channelSendFailed", message))?;
        state.next_event_index = event_index;
        Ok(Some(event_index))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
