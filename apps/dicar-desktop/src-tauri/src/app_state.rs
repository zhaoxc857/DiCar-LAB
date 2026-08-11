use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use dicar_app_core::{
    ActorError, AppActorHandle, AppSnapshot, CoreCommand, CoreConfig, CoreEventPayload, Endpoint,
    OperationId, OperationResult, OperationStatus, SnapshotPhase,
};
use serde::Serialize;

use crate::{
    CloseDecision, CloseRequestOutcome, CloseResolution, FrontendEventSequencer, FrontendSink,
    WindowCloseCoordinator,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const FORWARDER_POLL: Duration = Duration::from_millis(50);
const COMPLETION_CAPACITY: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeErrorDto {
    pub code: String,
    pub message: String,
    pub operation_id: Option<OperationId>,
}

impl BridgeErrorDto {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            operation_id: None,
        }
    }

    fn with_operation(mut self, operation_id: OperationId) -> Self {
        self.operation_id = Some(operation_id);
        self
    }
}

#[derive(Default)]
struct CompletionState {
    results: VecDeque<OperationResult>,
    closed: bool,
}

#[derive(Default)]
struct CompletionStore {
    state: Mutex<CompletionState>,
    available: Condvar,
}

impl CompletionStore {
    fn record(&self, result: OperationResult) {
        let mut state = lock(&self.state);
        if state.results.len() == COMPLETION_CAPACITY {
            state.results.pop_front();
        }
        state.results.push_back(result);
        self.available.notify_all();
    }

    fn wait(
        &self,
        operation_id: OperationId,
        timeout: Duration,
    ) -> Result<OperationResult, BridgeErrorDto> {
        let deadline = Instant::now() + timeout;
        let mut state = lock(&self.state);
        loop {
            if let Some(index) = state
                .results
                .iter()
                .position(|result| result.operation_id == operation_id)
            {
                return Ok(state
                    .results
                    .remove(index)
                    .expect("completion index is valid"));
            }
            if state.closed {
                return Err(
                    BridgeErrorDto::new("actorClosed", "核心服务已停止，操作没有完成")
                        .with_operation(operation_id),
                );
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(
                    BridgeErrorDto::new("operationTimeout", "等待核心操作结果超时")
                        .with_operation(operation_id),
                );
            }
            let remaining = deadline.saturating_duration_since(now);
            let (next, wait) = self
                .available
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next;
            if wait.timed_out() {
                return Err(
                    BridgeErrorDto::new("operationTimeout", "等待核心操作结果超时")
                        .with_operation(operation_id),
                );
            }
        }
    }

    fn close(&self) {
        lock(&self.state).closed = true;
        self.available.notify_all();
    }
}

pub struct AppState {
    actor: Mutex<Option<AppActorHandle>>,
    configured_endpoint: Endpoint,
    sequencer: Arc<FrontendEventSequencer>,
    completions: Arc<CompletionStore>,
    command_gate: Mutex<()>,
    next_bridge_operation_id: AtomicU64,
    close_coordinator: Mutex<WindowCloseCoordinator>,
    forwarder: Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    pub fn spawn(config: CoreConfig) -> Result<Self, BridgeErrorDto> {
        let configured_endpoint = config.endpoint.clone();
        let actor = AppActorHandle::spawn(config).map_err(actor_error)?;
        let receiver = actor.subscribe().map_err(actor_error)?;
        let sequencer = Arc::new(FrontendEventSequencer::default());
        let completions = Arc::new(CompletionStore::default());
        let forwarder_sequencer = Arc::clone(&sequencer);
        let forwarder_completions = Arc::clone(&completions);
        let forwarder = thread::Builder::new()
            .name("dicar-tauri-channel-forwarder".into())
            .spawn(move || loop {
                match receiver.recv_timeout(FORWARDER_POLL) {
                    Ok(event) => {
                        if let CoreEventPayload::OperationCompleted(result) = &event.payload {
                            forwarder_completions.record(result.clone());
                        }
                        let _ = forwarder_sequencer.publish_core(event);
                    }
                    Err(ActorError::Timeout) => {}
                    Err(_) => break,
                }
            })
            .map_err(|_| BridgeErrorDto::new("forwarderSpawnFailed", "无法启动前端事件线程"))?;
        Ok(Self {
            actor: Mutex::new(Some(actor)),
            configured_endpoint,
            sequencer,
            completions,
            command_gate: Mutex::new(()),
            next_bridge_operation_id: AtomicU64::new(1_u64 << 63),
            close_coordinator: Mutex::new(WindowCloseCoordinator::default()),
            forwarder: Mutex::new(Some(forwarder)),
        })
    }

    pub fn configured_endpoint(&self) -> &Endpoint {
        &self.configured_endpoint
    }

    pub fn replace_frontend_sink(&self, sink: Arc<dyn FrontendSink>) -> Result<(), BridgeErrorDto> {
        self.sequencer.replace_sink(sink)
    }

    pub fn close_frontend_sink(&self) {
        self.sequencer.close_sink();
    }

    pub fn dispatch(&self, command: CoreCommand) -> Result<OperationResult, BridgeErrorDto> {
        let _command_guard = lock(&self.command_gate);
        let operation_id = {
            let actor = lock(&self.actor);
            let actor = actor
                .as_ref()
                .ok_or_else(|| BridgeErrorDto::new("actorClosed", "核心服务已停止"))?;
            actor.send(command).map_err(|error| {
                BridgeErrorDto::new("commandRejected", format!("核心命令未进入队列：{error}"))
            })?
        };
        self.completions.wait(operation_id, COMMAND_TIMEOUT)
    }

    pub fn snapshot(&self) -> AppSnapshot {
        lock(&self.actor)
            .as_ref()
            .map(AppActorHandle::snapshot)
            .unwrap_or_else(|| panic!("snapshot requested after AppState shutdown"))
    }

    pub fn complete_bridge_operation(
        &self,
        message: impl Into<String>,
    ) -> Result<OperationResult, BridgeErrorDto> {
        let operation_id = OperationId(
            self.next_bridge_operation_id
                .fetch_add(1, Ordering::Relaxed),
        );
        let result = OperationResult {
            operation_id,
            status: OperationStatus::Succeeded,
            message: message.into(),
        };
        self.sequencer.publish_core(dicar_app_core::CoreEvent {
            actor_order: 0,
            payload: CoreEventPayload::OperationCompleted(result.clone()),
        })?;
        Ok(result)
    }

    pub fn request_window_close(&self) -> Result<CloseRequestOutcome, BridgeErrorDto> {
        let (outcome, created) = lock(&self.close_coordinator).request(&self.snapshot())?;
        if created {
            if let CloseRequestOutcome::Prevented {
                request_id,
                dirty_count,
                can_revert,
            } = outcome
            {
                if self
                    .sequencer
                    .publish_window_close(request_id, dirty_count, can_revert)?
                    .is_none()
                {
                    lock(&self.close_coordinator).discard(request_id);
                    return Err(BridgeErrorDto::new(
                        "frontendChannelUnavailable",
                        "前端事件通道尚未打开，无法确认未固化修改",
                    ));
                }
            }
        }
        Ok(outcome)
    }

    pub fn resolve_window_close(
        &self,
        request_id: u64,
        decision: CloseDecision,
    ) -> Result<CloseResolution, BridgeErrorDto> {
        lock(&self.close_coordinator).begin_resolution(request_id)?;
        let result = self.resolve_window_close_inner(decision);
        match result {
            Ok(resolution) => {
                lock(&self.close_coordinator).complete(request_id)?;
                Ok(resolution)
            }
            Err(error) => {
                lock(&self.close_coordinator).retryable_failure(request_id);
                Err(error)
            }
        }
    }

    fn resolve_window_close_inner(
        &self,
        decision: CloseDecision,
    ) -> Result<CloseResolution, BridgeErrorDto> {
        match decision {
            CloseDecision::Cancel => Ok(CloseResolution::KeepOpen),
            CloseDecision::DisconnectKeepUnknown => {
                require_success(self.dispatch(CoreCommand::Disconnect)?)?;
                Ok(CloseResolution::CloseWindow)
            }
            CloseDecision::RevertThenClose => {
                if self.snapshot().phase != SnapshotPhase::Ready {
                    return Err(BridgeErrorDto::new(
                        "revertUnavailable",
                        "设备未就绪，无法安全回退未固化修改",
                    ));
                }
                require_success(self.dispatch(CoreCommand::RevertAllPendingChanges)?)?;
                require_success(self.dispatch(CoreCommand::Disconnect)?)?;
                Ok(CloseResolution::CloseWindow)
            }
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Some(actor) = lock(&self.actor).take() {
            let _ = actor.shutdown();
        }
        if let Some(forwarder) = lock(&self.forwarder).take() {
            let _ = forwarder.join();
        }
        self.completions.close();
    }
}

fn require_success(result: OperationResult) -> Result<OperationResult, BridgeErrorDto> {
    if result.status == OperationStatus::Succeeded {
        return Ok(result);
    }
    Err(BridgeErrorDto {
        code: "operationFailed".into(),
        message: result.message,
        operation_id: Some(result.operation_id),
    })
}

fn actor_error(error: ActorError) -> BridgeErrorDto {
    BridgeErrorDto::new("actorUnavailable", format!("核心服务不可用：{error}"))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
