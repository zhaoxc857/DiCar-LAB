use std::{
    collections::VecDeque,
    fmt,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender, TrySendError};
use dctp_protocol::{
    BootloaderProtocol, ErrorCode, FirmwareTargetId, MessageType, ParamValue, PrepareFlash,
    TelemetryBatch, TelemetrySubscription, WireEncode,
};

use crate::{
    bridge_model::{merge_diagnostics, parameter_snapshots},
    link_budget, validate_firmware_flash_start, validate_subscription, AccessProfile,
    ActiveTransport, AppSnapshot, BridgeError, CommitFailureKind, ConnectionLoss, CoreError,
    CoreEvent, CoreEventPayload, Endpoint, LeaseState, OperationId, OperationResult,
    OperationStatus, ParamValueDto, ParameterWorkspace, ProtocolSession, SnapshotPhase,
    SystemClock, SystemNonce, TelemetryEngine, TelemetrySubscriptionSnapshot, Transport,
    TransportIdentity, UiTelemetryBatch, WriteFailure,
};

const COMMAND_CAPACITY: usize = 64;
const RELIABLE_EVENT_CAPACITY: usize = 64;
const TELEMETRY_EVENT_CAPACITY: usize = 4;
const MAX_COMMANDS_PER_TURN: usize = 8;
const MAX_MARKERS: usize = 256;
const MAX_MARKER_BYTES: usize = 64;
const UI_FLUSH_PERIOD: Duration = Duration::from_nanos(33_333_334);
const ACTOR_IDLE_POLL: Duration = Duration::from_millis(2);

struct UiFlushGate {
    period: Duration,
    last_flush: Instant,
}

impl UiFlushGate {
    fn new_at(period: Duration, now: Instant) -> Self {
        Self {
            period,
            last_flush: now,
        }
    }

    #[cfg(test)]
    fn default_at(now: Instant) -> Self {
        Self::new_at(UI_FLUSH_PERIOD, now)
    }

    fn take_if_due(&mut self, now: Instant) -> bool {
        if now.duration_since(self.last_flush) < self.period {
            return false;
        }
        self.last_flush = now;
        true
    }

    fn reset_at(&mut self, now: Instant) {
        self.last_flush = now;
    }
}

#[derive(Clone, Debug)]
pub struct CoreConfig {
    pub endpoint: Endpoint,
    command_capacity: usize,
    startup_delay: Duration,
    ui_flush_period: Duration,
    command_batch_window: Duration,
}

impl CoreConfig {
    pub fn simulator(address: SocketAddr) -> Self {
        Self {
            endpoint: Endpoint::Simulator { address },
            command_capacity: COMMAND_CAPACITY,
            startup_delay: Duration::ZERO,
            ui_flush_period: UI_FLUSH_PERIOD,
            command_batch_window: Duration::from_micros(250),
        }
    }

    #[doc(hidden)]
    pub fn with_command_capacity(mut self, capacity: usize) -> Self {
        self.command_capacity = capacity.max(1);
        self
    }

    #[doc(hidden)]
    pub fn with_startup_delay(mut self, delay: Duration) -> Self {
        self.startup_delay = delay;
        self
    }

    #[doc(hidden)]
    pub fn with_ui_flush_period(mut self, period: Duration) -> Self {
        self.ui_flush_period = period.max(Duration::from_millis(1));
        self
    }

    #[doc(hidden)]
    pub fn with_command_batch_window(mut self, window: Duration) -> Self {
        self.command_batch_window = window;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CoreCommand {
    Connect,
    ConnectTo {
        endpoint: Endpoint,
    },
    Disconnect,
    WriteParameter {
        param_id: u32,
        value: ParamValueDto,
    },
    CommitParameters,
    PrepareFirmwareFlash {
        flash_operation_id: [u8; 16],
        target_id: FirmwareTargetId,
        firmware_version: [u16; 3],
        image_len: u32,
        image_sha256: [u8; 32],
    },
    RevertAllPendingChanges,
    UndoLastConfirmedChange,
    SetTelemetrySubscription {
        channel_ids: Vec<u32>,
        sample_rate_hz: u16,
    },
    ClearTelemetrySubscription,
    SetPaused {
        paused: bool,
    },
    SelectAccessProfile {
        profile: crate::AccessProfileDto,
    },
    AddMarker {
        label: String,
    },
    GetSnapshot,
    Shutdown,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ActorSendError {
    Overloaded { capacity: usize },
    Closed,
}

impl fmt::Display for ActorSendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ActorSendError {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ActorError {
    Closed,
    Timeout,
    AlreadySubscribed,
    ThreadPanicked,
}

impl fmt::Display for ActorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ActorError {}

#[derive(Clone, Debug)]
struct CommandEnvelope {
    operation_id: OperationId,
    command: CoreCommand,
}

#[derive(Default)]
struct MailboxState {
    queue: VecDeque<CoreEvent>,
    reliable_count: usize,
    telemetry_count: usize,
    ui_dropped_batches: u64,
    next_order: u64,
    sticky_fatal: Option<CoreEvent>,
    terminal: bool,
    closed: bool,
}

#[derive(Default)]
struct EventMailbox {
    state: Mutex<MailboxState>,
    available: Condvar,
}

impl EventMailbox {
    fn publish(&self, payload: CoreEventPayload) -> bool {
        let mut state = lock(&self.state);
        if state.closed || (state.terminal && !matches!(payload, CoreEventPayload::FatalError(_))) {
            return false;
        }
        let kind = EventKind::of(&payload);
        match kind {
            EventKind::Snapshot => {
                if let Some(index) = state
                    .queue
                    .iter()
                    .position(|event| matches!(event.payload, CoreEventPayload::SnapshotChanged(_)))
                {
                    state.queue.remove(index);
                }
            }
            EventKind::Telemetry => {
                if state.telemetry_count == TELEMETRY_EVENT_CAPACITY {
                    if let Some(index) = state.queue.iter().position(|event| {
                        matches!(event.payload, CoreEventPayload::TelemetryBatch(_))
                    }) {
                        state.queue.remove(index);
                        state.telemetry_count -= 1;
                        state.ui_dropped_batches = state.ui_dropped_batches.saturating_add(1);
                    }
                }
            }
            EventKind::Reliable if state.reliable_count == RELIABLE_EVENT_CAPACITY => {
                if state.sticky_fatal.is_none() {
                    let operation_id = match &payload {
                        CoreEventPayload::OperationCompleted(result) => Some(result.operation_id),
                        _ => None,
                    };
                    let order = take_order(&mut state);
                    state.sticky_fatal = Some(CoreEvent {
                        actor_order: order,
                        payload: CoreEventPayload::FatalError(BridgeError {
                            code: "frontendOverrun".into(),
                            message: "前端可靠事件队列已满，已停止设备写入".into(),
                            operation_id,
                        }),
                    });
                }
                state.terminal = true;
                self.available.notify_all();
                return false;
            }
            EventKind::Reliable => {}
        }
        let order = take_order(&mut state);
        state.queue.push_back(CoreEvent {
            actor_order: order,
            payload,
        });
        match kind {
            EventKind::Reliable => state.reliable_count += 1,
            EventKind::Telemetry => state.telemetry_count += 1,
            EventKind::Snapshot => {}
        }
        self.available.notify_all();
        true
    }

    fn close(&self) {
        let mut state = lock(&self.state);
        state.closed = true;
        self.available.notify_all();
    }

    fn ui_dropped_batches(&self) -> u64 {
        lock(&self.state).ui_dropped_batches
    }

    fn is_terminal(&self) -> bool {
        lock(&self.state).terminal
    }

    fn receive(&self, timeout: Duration) -> Result<CoreEvent, ActorError> {
        let deadline = Instant::now() + timeout;
        let mut state = lock(&self.state);
        loop {
            if let Some(event) = state.queue.pop_front() {
                match EventKind::of(&event.payload) {
                    EventKind::Reliable => state.reliable_count -= 1,
                    EventKind::Telemetry => state.telemetry_count -= 1,
                    EventKind::Snapshot => {}
                }
                return Ok(event);
            }
            if let Some(event) = state.sticky_fatal.take() {
                return Ok(event);
            }
            if state.closed {
                return Err(ActorError::Closed);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(ActorError::Timeout);
            }
            let remaining = deadline.saturating_duration_since(now);
            let (next, wait) = self
                .available
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next;
            if wait.timed_out() && state.queue.is_empty() && state.sticky_fatal.is_none() {
                return Err(ActorError::Timeout);
            }
        }
    }

    fn drain(&self) -> Vec<CoreEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receive(Duration::ZERO) {
            events.push(event);
        }
        events
    }
}

#[derive(Clone, Copy)]
enum EventKind {
    Reliable,
    Telemetry,
    Snapshot,
}

impl EventKind {
    fn of(payload: &CoreEventPayload) -> Self {
        match payload {
            CoreEventPayload::SnapshotChanged(_) => Self::Snapshot,
            CoreEventPayload::TelemetryBatch(_) => Self::Telemetry,
            CoreEventPayload::OperationCompleted(_)
            | CoreEventPayload::ConnectionLost(_)
            | CoreEventPayload::FatalError(_) => Self::Reliable,
        }
    }
}

fn take_order(state: &mut MailboxState) -> u64 {
    let order = state.next_order;
    state.next_order = state.next_order.saturating_add(1);
    order
}

pub struct CoreEventReceiver {
    mailbox: Arc<EventMailbox>,
}

impl CoreEventReceiver {
    pub fn recv_timeout(&self, timeout: Duration) -> Result<CoreEvent, ActorError> {
        self.mailbox.receive(timeout)
    }

    pub fn drain(&self) -> Vec<CoreEvent> {
        self.mailbox.drain()
    }
}

pub struct AppActorHandle {
    command_tx: Sender<CommandEnvelope>,
    snapshot: Arc<RwLock<AppSnapshot>>,
    mailbox: Arc<EventMailbox>,
    next_operation_id: AtomicU64,
    receiver_taken: AtomicBool,
    shutdown_requested: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
    command_capacity: usize,
}

impl AppActorHandle {
    pub fn spawn(config: CoreConfig) -> Result<Self, ActorError> {
        let access = AccessProfile::new(crate::AccessRole::Owner, LeaseState::Active);
        let snapshot = Arc::new(RwLock::new(AppSnapshot::disconnected(access)));
        let mailbox = Arc::new(EventMailbox::default());
        let (command_tx, command_rx) = bounded(config.command_capacity);
        let thread_snapshot = Arc::clone(&snapshot);
        let thread_mailbox = Arc::clone(&mailbox);
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown_requested);
        let command_capacity = config.command_capacity;
        let join = thread::Builder::new()
            .name("dicar-app-actor".into())
            .spawn(move || {
                actor_loop(
                    config,
                    command_rx,
                    thread_snapshot,
                    thread_mailbox,
                    thread_shutdown,
                )
            })
            .map_err(|_| ActorError::ThreadPanicked)?;
        Ok(Self {
            command_tx,
            snapshot,
            mailbox,
            next_operation_id: AtomicU64::new(1),
            receiver_taken: AtomicBool::new(false),
            shutdown_requested,
            join: Mutex::new(Some(join)),
            command_capacity,
        })
    }

    pub fn send(&self, command: CoreCommand) -> Result<OperationId, ActorSendError> {
        if self.mailbox.is_terminal() {
            return Err(ActorSendError::Closed);
        }
        let operation_id = OperationId(self.next_operation_id.fetch_add(1, Ordering::Relaxed));
        match self.command_tx.try_send(CommandEnvelope {
            operation_id,
            command,
        }) {
            Ok(()) => Ok(operation_id),
            Err(TrySendError::Full(_)) => Err(ActorSendError::Overloaded {
                capacity: self.command_capacity,
            }),
            Err(TrySendError::Disconnected(_)) => Err(ActorSendError::Closed),
        }
    }

    pub fn snapshot(&self) -> AppSnapshot {
        read(&self.snapshot).clone()
    }

    pub fn subscribe(&self) -> Result<CoreEventReceiver, ActorError> {
        if self.receiver_taken.swap(true, Ordering::AcqRel) {
            return Err(ActorError::AlreadySubscribed);
        }
        Ok(CoreEventReceiver {
            mailbox: Arc::clone(&self.mailbox),
        })
    }

    pub fn shutdown(self) -> Result<(), ActorError> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&self) -> Result<(), ActorError> {
        if lock(&self.join).is_none() {
            return Ok(());
        }
        self.shutdown_requested.store(true, Ordering::Release);
        let operation_id = OperationId(self.next_operation_id.fetch_add(1, Ordering::Relaxed));
        let _ = self.command_tx.send_timeout(
            CommandEnvelope {
                operation_id,
                command: CoreCommand::Shutdown,
            },
            Duration::from_millis(100),
        );
        if let Some(join) = lock(&self.join).take() {
            join.join().map_err(|_| ActorError::ThreadPanicked)?;
        }
        Ok(())
    }
}

impl Drop for AppActorHandle {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn actor_loop(
    config: CoreConfig,
    command_rx: Receiver<CommandEnvelope>,
    snapshot: Arc<RwLock<AppSnapshot>>,
    mailbox: Arc<EventMailbox>,
    shutdown_requested: Arc<AtomicBool>,
) {
    if !config.startup_delay.is_zero() {
        thread::sleep(config.startup_delay);
    }
    let command_batch_window = config.command_batch_window;
    let mut runtime = ActorRuntime::new(config, snapshot, mailbox);
    let mut running = true;
    while running && !runtime.mailbox.is_terminal() && !shutdown_requested.load(Ordering::Acquire) {
        let mut commands = Vec::with_capacity(MAX_COMMANDS_PER_TURN);
        match command_rx.recv_timeout(ACTOR_IDLE_POLL) {
            Ok(command) => commands.push(command),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while commands.len() < MAX_COMMANDS_PER_TURN {
            match command_rx.recv_timeout(command_batch_window) {
                Ok(command) => commands.push(command),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    running = false;
                    break;
                }
            }
        }
        let commands = runtime.coalesce_writes(commands);
        for command in commands {
            if matches!(command.command, CoreCommand::Shutdown) {
                runtime.complete(command.operation_id, Ok("核心服务已停止"));
                running = false;
                break;
            }
            runtime.handle_command(command);
            if runtime.mailbox.is_terminal() {
                running = false;
                break;
            }
        }
        if running {
            runtime.poll_protocol();
            runtime.flush_telemetry(false);
        }
    }
    runtime.close();
    runtime.mailbox.close();
}

struct ActorRuntime {
    config: CoreConfig,
    snapshot: Arc<RwLock<AppSnapshot>>,
    mailbox: Arc<EventMailbox>,
    session: Option<ProtocolSession<ActiveTransport>>,
    workspace: Option<ParameterWorkspace>,
    device: Option<crate::ConnectedDevice>,
    transport_identity: Option<TransportIdentity>,
    telemetry: TelemetryEngine,
    desired_subscription: Option<TelemetrySubscriptionSnapshot>,
    active_subscription: Option<TelemetrySubscriptionSnapshot>,
    next_subscription_version: u16,
    paused: bool,
    access: AccessProfile,
    markers: VecDeque<String>,
    accumulator: Option<UiTelemetryBatch>,
    ui_flush_gate: UiFlushGate,
    last_protocol_diagnostics: crate::DiagnosticsSnapshot,
    last_disconnect_reason: Option<String>,
    snapshot_revision: u64,
}

impl ActorRuntime {
    fn new(
        config: CoreConfig,
        snapshot: Arc<RwLock<AppSnapshot>>,
        mailbox: Arc<EventMailbox>,
    ) -> Self {
        let ui_flush_gate = UiFlushGate::new_at(config.ui_flush_period, Instant::now());
        Self {
            config,
            snapshot,
            mailbox,
            session: None,
            workspace: None,
            device: None,
            transport_identity: None,
            telemetry: TelemetryEngine::default(),
            desired_subscription: None,
            active_subscription: None,
            next_subscription_version: 1,
            paused: false,
            access: AccessProfile::new(crate::AccessRole::Owner, LeaseState::Active),
            markers: VecDeque::new(),
            accumulator: None,
            ui_flush_gate,
            last_protocol_diagnostics: crate::DiagnosticsSnapshot::default(),
            last_disconnect_reason: None,
            snapshot_revision: 0,
        }
    }

    fn coalesce_writes(&mut self, commands: Vec<CommandEnvelope>) -> Vec<CommandEnvelope> {
        let mut output: Vec<CommandEnvelope> = Vec::with_capacity(commands.len());
        for command in commands {
            let same_parameter = match (output.last(), &command.command) {
                (
                    Some(CommandEnvelope {
                        command:
                            CoreCommand::WriteParameter {
                                param_id: previous, ..
                            },
                        ..
                    }),
                    CoreCommand::WriteParameter { param_id, .. },
                ) => previous == param_id,
                _ => false,
            };
            if same_parameter {
                if let Some(superseded) = output.pop() {
                    self.publish_reliable(CoreEventPayload::OperationCompleted(OperationResult {
                        operation_id: superseded.operation_id,
                        status: OperationStatus::Superseded,
                        message: "已被同参数的更新值合并".into(),
                    }));
                }
            }
            output.push(command);
        }
        output
    }

    fn handle_command(&mut self, envelope: CommandEnvelope) {
        let result = match envelope.command {
            CoreCommand::Connect => self.connect(self.config.endpoint.clone()),
            CoreCommand::ConnectTo { endpoint } => self.connect(endpoint),
            CoreCommand::Disconnect => self.disconnect_explicit(),
            CoreCommand::WriteParameter { param_id, value } => {
                self.write_parameter(param_id, value.into())
            }
            CoreCommand::CommitParameters => self.commit_parameters(),
            CoreCommand::PrepareFirmwareFlash {
                flash_operation_id,
                target_id,
                firmware_version,
                image_len,
                image_sha256,
            } => self.prepare_firmware_flash(PrepareFlash {
                operation_id: flash_operation_id,
                target_id,
                firmware_version,
                image_len,
                image_sha256,
            }),
            CoreCommand::RevertAllPendingChanges => self.revert_all(),
            CoreCommand::UndoLastConfirmedChange => self.undo_last(),
            CoreCommand::SetTelemetrySubscription {
                channel_ids,
                sample_rate_hz,
            } => self.set_subscription(channel_ids, sample_rate_hz),
            CoreCommand::ClearTelemetrySubscription => self.clear_subscription(),
            CoreCommand::SetPaused { paused } => self.set_paused(paused),
            CoreCommand::SelectAccessProfile { profile } => {
                self.access = profile.into();
                Ok("本地演示权限已切换")
            }
            CoreCommand::AddMarker { label } => self.add_marker(label),
            CoreCommand::GetSnapshot => Ok("快照已刷新"),
            CoreCommand::Shutdown => unreachable!("shutdown is handled by the actor loop"),
        };
        self.refresh_snapshot(true);
        self.complete(envelope.operation_id, result);
    }

    fn connect(&mut self, endpoint: Endpoint) -> Result<&'static str, String> {
        if self.session.is_some() {
            return Err("设备已经连接".into());
        }
        self.set_phase(SnapshotPhase::Connecting);
        self.config.endpoint = endpoint.clone();
        let transport = ActiveTransport::connect(&endpoint).map_err(|error| error.to_string())?;
        let transport_identity = transport.identity();
        let mut session =
            ProtocolSession::new(transport, SystemNonce::default(), SystemClock::new());
        let connected = session
            .connect_and_load()
            .map_err(|error| error.to_string())?;
        let workspace = ParameterWorkspace::from_manifest_and_states(
            &connected.manifest,
            &connected.parameter_states,
        )
        .map_err(|error| error.to_string())?;
        self.workspace = Some(workspace);
        self.device = Some(connected);
        self.session = Some(session);
        self.transport_identity = Some(transport_identity);
        self.last_disconnect_reason = None;
        self.paused = false;
        self.active_subscription = None;
        Ok("设备连接并加载完成")
    }

    fn disconnect_explicit(&mut self) -> Result<&'static str, String> {
        self.capture_protocol_diagnostics();
        let close_error = self
            .session
            .take()
            .and_then(|mut session| session.close().err())
            .map(|error| error.to_string());
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.mark_disconnected();
        }
        self.device = None;
        self.active_subscription = None;
        self.paused = true;
        self.accumulator = None;
        self.last_disconnect_reason = Some("用户主动断开".into());
        match close_error {
            Some(error) => Err(format!("设备已在本地断开；链路关闭返回错误: {error}")),
            None => Ok("设备已断开"),
        }
    }

    fn write_parameter(
        &mut self,
        param_id: u32,
        value: ParamValue,
    ) -> Result<&'static str, String> {
        let workspace = self.workspace.as_mut().ok_or("设备未连接")?;
        let session = self.session.as_mut().ok_or("设备未连接")?;
        let Some(mut pending) = workspace
            .queue_write(self.access, param_id, value)
            .map_err(|error| error.to_string())?
        else {
            return Ok("参数值未变化");
        };
        loop {
            let (result, operation_error) = match session.execute_write(workspace, &pending) {
                Ok(ack) => (Ok(ack), None),
                Err(CoreError::RevisionConflict { current }) => (
                    Err(WriteFailure::RevisionConflict(current)),
                    Some("参数版本冲突，已刷新设备当前值".to_owned()),
                ),
                Err(error) => (Err(WriteFailure::Ordinary), Some(error.to_string())),
            };
            let next = workspace
                .resolve_write(param_id, &pending, result)
                .map_err(|error| error.to_string())?;
            let Some(next) = next else {
                if let Some(error) = operation_error {
                    return Err(error);
                }
                break;
            };
            pending = next;
        }
        Ok("RAM 参数已确认")
    }

    fn commit_parameters(&mut self) -> Result<&'static str, String> {
        let workspace = self.workspace.as_mut().ok_or("设备未连接")?;
        let session = self.session.as_mut().ok_or("设备未连接")?;
        let Some(plan) = workspace
            .commit_dirty(self.access)
            .map_err(|error| error.to_string())?
        else {
            return Ok("没有需要固化的参数");
        };
        let result = match session.execute_commit(workspace, &plan) {
            Ok(ack) => Ok(ack),
            Err(CoreError::Device {
                code: ErrorCode::StorageFailed,
                ..
            }) => Err(CommitFailureKind::Storage),
            Err(CoreError::Device {
                code: ErrorCode::VerifyFailed,
                ..
            }) => Err(CommitFailureKind::Verify),
            Err(CoreError::Timeout { .. }) => Err(CommitFailureKind::Timeout),
            Err(_) => Err(CommitFailureKind::Device),
        };
        workspace
            .resolve_commit(&plan, result)
            .map_err(|error| error.to_string())?;
        Ok("参数已固化到 Flash")
    }

    fn prepare_firmware_flash(&mut self, request: PrepareFlash) -> Result<&'static str, String> {
        let workspace = self.workspace.as_ref().ok_or("设备未连接")?;
        let device = self.device.as_ref().ok_or("设备未连接")?;
        let endpoint = &self
            .transport_identity
            .as_ref()
            .ok_or("设备未连接")?
            .endpoint;
        validate_firmware_flash_start(
            self.access,
            endpoint,
            device.identity.capabilities,
            workspace.dirty_count(),
        )
        .map_err(|error| error.to_string())?;

        let session = self.session.as_mut().ok_or("设备未连接")?;
        let ack = session
            .prepare_firmware_flash(&request)
            .map_err(|error| error.to_string())?;
        if ack.bootloader_protocol != BootloaderProtocol::TI_MSPM0_ROM_BSL_UART
            || ack.initial_baud != 9_600
        {
            return Err("设备返回了不支持的 Bootloader 切换参数".into());
        }

        self.capture_protocol_diagnostics();
        if let Some(session) = self.session.take() {
            let mut transport = session.into_transport();
            let _ = transport.close();
        }
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.mark_disconnected();
        }
        self.device = None;
        self.active_subscription = None;
        self.paused = true;
        self.accumulator = None;
        self.last_disconnect_reason = Some("设备已切换到 TI ROM BSL".into());
        Ok("设备已确认固件烧录并释放串口")
    }

    fn revert_all(&mut self) -> Result<&'static str, String> {
        let workspace = self.workspace.as_mut().ok_or("设备未连接")?;
        let session = self.session.as_mut().ok_or("设备未连接")?;
        let plan = workspace
            .revert_all(self.access)
            .map_err(|error| error.to_string())?;
        let mut results = Vec::with_capacity(plan.writes().len());
        for pending in plan.writes() {
            let result = match session.execute_write(workspace, pending) {
                Ok(ack) => Ok(ack),
                Err(CoreError::RevisionConflict { current }) => {
                    Err(WriteFailure::RevisionConflict(current))
                }
                Err(_) => Err(WriteFailure::Ordinary),
            };
            results.push((pending.clone(), result));
        }
        let report = workspace
            .resolve_revert_all(&plan, results)
            .map_err(|error| error.to_string())?;
        if report.failed_ids.is_empty() {
            Ok("全部未固化修改已回退")
        } else {
            Err(format!("部分参数回退失败: {:?}", report.failed_ids))
        }
    }

    fn undo_last(&mut self) -> Result<&'static str, String> {
        let workspace = self.workspace.as_mut().ok_or("设备未连接")?;
        let session = self.session.as_mut().ok_or("设备未连接")?;
        let Some(pending) = workspace
            .undo_last_confirmed_change(self.access)
            .map_err(|error| error.to_string())?
        else {
            return Ok("没有可撤销的已确认修改");
        };
        let (result, operation_error) = match session.execute_write(workspace, &pending) {
            Ok(ack) => (Ok(ack), None),
            Err(CoreError::RevisionConflict { current }) => (
                Err(WriteFailure::RevisionConflict(current)),
                Some("撤销时发生参数版本冲突".to_owned()),
            ),
            Err(error) => (Err(WriteFailure::Ordinary), Some(error.to_string())),
        };
        workspace
            .resolve_write(pending.param_id, &pending, result)
            .map_err(|error| error.to_string())?;
        if let Some(error) = operation_error {
            return Err(error);
        }
        Ok("已撤销最近一次参数修改")
    }

    fn set_subscription(
        &mut self,
        channel_ids: Vec<u32>,
        sample_rate_hz: u16,
    ) -> Result<&'static str, String> {
        let telemetry_descriptors = self
            .device
            .as_ref()
            .ok_or("设备未连接")?
            .manifest
            .telemetry
            .clone();
        let endpoint = &self
            .transport_identity
            .as_ref()
            .ok_or("设备未连接")?
            .endpoint;
        validate_subscription(endpoint, channel_ids.len(), sample_rate_hz)
            .map_err(|error| error.to_string())?;
        let version = self.take_subscription_version();
        let subscription = TelemetrySubscription {
            subscription_version: version,
            sample_rate_hz,
            channel_ids: channel_ids.clone(),
        };
        let mut candidate = self.telemetry.clone();
        candidate
            .activate(subscription.clone(), &telemetry_descriptors)
            .map_err(|error| error.to_string())?;
        let session = self.session.as_mut().ok_or("设备未连接")?;
        session
            .request(
                MessageType::TelemetrySubscribe,
                subscription
                    .encode()
                    .map_err(|error| format!("{error:?}"))?,
            )
            .map_err(|error| error.to_string())?;
        self.telemetry = candidate;
        let snapshot = TelemetrySubscriptionSnapshot {
            subscription_version: version,
            sample_rate_hz,
            channel_ids,
        };
        self.desired_subscription = Some(snapshot.clone());
        self.active_subscription = Some(snapshot);
        self.paused = false;
        self.accumulator = None;
        self.ui_flush_gate.reset_at(Instant::now());
        Ok("遥测订阅已生效")
    }

    fn set_paused(&mut self, paused: bool) -> Result<&'static str, String> {
        if self.paused == paused {
            return Ok("波形状态未变化");
        }
        if paused {
            let session = self.session.as_mut().ok_or("设备未连接")?;
            session
                .request(MessageType::TelemetryStop, Vec::new())
                .map_err(|error| error.to_string())?;
            self.flush_telemetry(true);
            self.paused = true;
            self.active_subscription = None;
            Ok("波形已暂停")
        } else {
            let desired = self
                .desired_subscription
                .clone()
                .ok_or("尚未选择遥测通道")?;
            self.set_subscription(desired.channel_ids, desired.sample_rate_hz)?;
            Ok("波形已恢复")
        }
    }

    fn clear_subscription(&mut self) -> Result<&'static str, String> {
        let session = self.session.as_mut().ok_or("设备未连接")?;
        session
            .request(MessageType::TelemetryStop, Vec::new())
            .map_err(|error| error.to_string())?;
        self.flush_telemetry(true);
        self.desired_subscription = None;
        self.active_subscription = None;
        self.paused = true;
        self.accumulator = None;
        Ok("遥测订阅已清除")
    }

    fn add_marker(&mut self, label: String) -> Result<&'static str, String> {
        if label.is_empty() || label.len() > MAX_MARKER_BYTES {
            return Err("标记文字必须为 1–64 字节".into());
        }
        if self.markers.len() == MAX_MARKERS {
            self.markers.pop_front();
        }
        self.markers.push_back(label);
        Ok("波形标记已添加")
    }

    fn take_subscription_version(&mut self) -> u16 {
        let version = self.next_subscription_version.max(1);
        self.next_subscription_version = version.wrapping_add(1).max(1);
        version
    }

    fn poll_protocol(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if let Err(error) = session.poll() {
            self.unexpected_disconnect(error.to_string());
            return;
        }
        let mut frames = Vec::new();
        while let Some(frame) = session.pop_unsolicited() {
            frames.push(frame);
        }
        for frame in frames {
            if frame.header.message_type != MessageType::TelemetryData {
                continue;
            }
            let Some(active) = self.active_subscription.as_ref() else {
                continue;
            };
            let batch = match TelemetryBatch::decode(&frame.payload, active.channel_ids.len()) {
                Ok(batch) => batch,
                Err(_) => continue,
            };
            if let Ok(batch) = self.telemetry.accept(batch) {
                if !self.paused {
                    self.accumulate(batch);
                }
            }
        }
    }

    fn accumulate(&mut self, batch: UiTelemetryBatch) {
        match &mut self.accumulator {
            Some(accumulator) if accumulator.subscription_version == batch.subscription_version => {
                accumulator.dropped_samples = accumulator
                    .dropped_samples
                    .saturating_add(batch.dropped_samples);
                accumulator.points.extend(batch.points);
            }
            Some(_) => {
                self.flush_telemetry(true);
                self.accumulator = Some(batch);
            }
            None => self.accumulator = Some(batch),
        }
    }

    fn flush_telemetry(&mut self, force: bool) {
        if self.accumulator.is_none() {
            return;
        }
        let now = Instant::now();
        if !force && !self.ui_flush_gate.take_if_due(now) {
            return;
        }
        if force {
            self.ui_flush_gate.reset_at(now);
        }
        if let Some(batch) = self.accumulator.take() {
            self.mailbox
                .publish(CoreEventPayload::TelemetryBatch(batch));
            self.refresh_snapshot(true);
        }
    }

    fn unexpected_disconnect(&mut self, reason: String) {
        self.capture_protocol_diagnostics();
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.mark_disconnected();
        }
        self.session = None;
        self.device = None;
        self.active_subscription = None;
        self.paused = true;
        self.accumulator = None;
        self.last_disconnect_reason = Some(reason.clone());
        self.refresh_snapshot(true);
        self.publish_reliable(CoreEventPayload::ConnectionLost(ConnectionLoss {
            message: reason,
        }));
    }

    fn refresh_snapshot(&mut self, publish: bool) {
        self.snapshot_revision = self.snapshot_revision.saturating_add(1);
        self.capture_protocol_diagnostics();
        let protocol_diagnostics = self.last_protocol_diagnostics;
        let telemetry_diagnostics = self.telemetry.diagnostics();
        let mut current = read(&self.snapshot).clone();
        current.revision = self.snapshot_revision;
        current.phase = self
            .session
            .as_ref()
            .map_or(SnapshotPhase::Disconnected, |session| {
                session.phase().into()
            });
        current.transport_identity = self.transport_identity.clone();
        current.session_id = self.device.as_ref().map(|device| device.session_id);
        current.device_id_hex = self.device.as_ref().map(|device| {
            device
                .identity
                .device_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        });
        current.firmware_version = self
            .device
            .as_ref()
            .map(|device| device.identity.firmware_version);
        current.parameters = parameter_snapshots(self.workspace.as_ref());
        current.telemetry_descriptors = self
            .device
            .as_ref()
            .map(|device| device.manifest.telemetry.iter().map(Into::into).collect())
            .unwrap_or_default();
        current.dirty_count = self
            .workspace
            .as_ref()
            .map_or(0, ParameterWorkspace::dirty_count);
        current.storage_generation = self
            .workspace
            .as_ref()
            .map_or(0, ParameterWorkspace::storage_generation);
        current.access_profile = self.access.into();
        current.desired_subscription = self.desired_subscription.clone();
        current.active_subscription = self.active_subscription.clone();
        current.link_budget = self
            .session
            .as_ref()
            .and(self.transport_identity.as_ref())
            .map(|identity| link_budget(&identity.endpoint));
        current.paused = self.paused;
        current.telemetry_points = self.telemetry.total_points();
        current.diagnostics = merge_diagnostics(
            protocol_diagnostics,
            telemetry_diagnostics,
            self.mailbox.ui_dropped_batches(),
        );
        current.last_disconnect_reason = self.last_disconnect_reason.clone();
        current.markers = self.markers.iter().cloned().collect();
        *write(&self.snapshot) = current.clone();
        if publish {
            self.mailbox
                .publish(CoreEventPayload::SnapshotChanged(Box::new(current)));
        }
    }

    fn set_phase(&mut self, phase: SnapshotPhase) {
        let mut current = read(&self.snapshot).clone();
        current.phase = phase;
        self.snapshot_revision = self.snapshot_revision.saturating_add(1);
        current.revision = self.snapshot_revision;
        *write(&self.snapshot) = current.clone();
        self.mailbox
            .publish(CoreEventPayload::SnapshotChanged(Box::new(current)));
    }

    fn complete(&self, operation_id: OperationId, result: Result<&str, String>) {
        let (status, message) = match result {
            Ok(message) => (OperationStatus::Succeeded, message.to_owned()),
            Err(message) => (OperationStatus::Failed, message),
        };
        self.publish_reliable(CoreEventPayload::OperationCompleted(OperationResult {
            operation_id,
            status,
            message,
        }));
    }

    fn publish_reliable(&self, payload: CoreEventPayload) {
        self.mailbox.publish(payload);
    }

    fn close(&mut self) {
        self.flush_telemetry(true);
        self.capture_protocol_diagnostics();
        if let Some(mut session) = self.session.take() {
            let _ = session.close();
        }
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.mark_disconnected();
        }
        self.refresh_snapshot(false);
    }

    fn capture_protocol_diagnostics(&mut self) {
        if let Some(session) = self.session.as_ref() {
            self.last_protocol_diagnostics = session.diagnostics();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ui_flush_gate_enforces_the_thirty_hertz_nanosecond_boundary() {
        let started = Instant::now();
        let mut gate = UiFlushGate::default_at(started);

        assert!(!gate.take_if_due(started + Duration::from_nanos(33_333_333)));
        assert!(gate.take_if_due(started + Duration::from_nanos(33_333_334)));
        assert!(!gate.take_if_due(started + Duration::from_nanos(66_666_667)));
        assert!(gate.take_if_due(started + Duration::from_nanos(66_666_668)));
    }
}
