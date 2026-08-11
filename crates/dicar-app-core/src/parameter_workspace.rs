use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use dctp_protocol::{
    canonical_parameter_crc32, DeviceManifest, ParamCommit, ParamCommitAck, ParamCommitEntry,
    ParamConstraints, ParamDescriptor, ParamFlags, ParamState, ParamValue, ParamWriteAck,
};

use crate::{AccessProfile, ConnectedDevice, PermissionDecision};

const HISTORY_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceSyncState {
    Known,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteState {
    Idle,
    InFlight,
    Queued,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterRecord {
    pub descriptor: ParamDescriptor,
    pub ram_value: ParamValue,
    pub persisted_value: Option<ParamValue>,
    pub revision: u32,
    pub dirty: bool,
    pub sync_state: DeviceSyncState,
    pub write_state: WriteState,
    pub unresolved_target: Option<ParamValue>,
    pub last_error: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    DuplicateDescriptor(u32),
    DuplicateState(u32),
    MissingState(u32),
    UnknownState(u32),
    TypeMismatch(u32),
    InvalidPersistence(u32),
    UnknownParameter(u32),
    PermissionDenied(&'static str),
    ReadOnly(u32),
    OutOfRange(u32),
    InvalidEnum(u32),
    UnexpectedAck(u32),
    WritesPending,
    CommitFailed(CommitFailureKind),
    CommitCrcMismatch,
    InvalidCommitPlan,
    NotRevertible(u32),
    DeviceStateUnknown(u32),
    WriteOperationMismatch,
    StaleWriteOperation,
    OldWorkspaceGeneration,
    OperationTokenExhausted,
    WorkspaceGenerationExhausted,
    CommitInFlight,
    CommitOperationMismatch,
    StaleCommitOperation,
    RevertInFlight,
    RevertCoverageMismatch,
    RevertOperationMismatch,
    StaleRevertOperation,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDescriptor(param_id) => {
                write!(formatter, "参数 {param_id} 的描述重复")
            }
            Self::DuplicateState(param_id) => write!(formatter, "参数 {param_id} 的设备状态重复"),
            Self::MissingState(param_id) => write!(formatter, "参数 {param_id} 缺少设备状态"),
            Self::UnknownState(param_id) => write!(formatter, "设备返回未知参数 {param_id}"),
            Self::TypeMismatch(param_id) => write!(formatter, "参数 {param_id} 的类型不匹配"),
            Self::InvalidPersistence(param_id) => {
                write!(formatter, "参数 {param_id} 的固化状态无效")
            }
            Self::UnknownParameter(param_id) => write!(formatter, "未知参数 {param_id}"),
            Self::PermissionDenied(reason) => formatter.write_str(reason),
            Self::ReadOnly(_) => formatter.write_str("参数为只读"),
            Self::OutOfRange(_) => formatter.write_str("参数值超出允许范围"),
            Self::InvalidEnum(_) => formatter.write_str("枚举值无效"),
            Self::UnexpectedAck(param_id) => {
                write!(formatter, "参数 {param_id} 没有对应的在途写入")
            }
            Self::WritesPending => formatter.write_str("仍有参数写入等待确认"),
            Self::CommitFailed(_) => formatter.write_str("参数固化失败，未修改本地固化状态"),
            Self::CommitCrcMismatch => formatter.write_str("固化确认 CRC 不匹配"),
            Self::InvalidCommitPlan => formatter.write_str("固化计划已失效"),
            Self::NotRevertible(param_id) => {
                write!(formatter, "参数 {param_id} 没有可回退的固化值")
            }
            Self::DeviceStateUnknown(_) => formatter.write_str("设备状态未知，不能修改参数"),
            Self::WriteOperationMismatch => formatter.write_str("写入操作与在途请求不匹配"),
            Self::StaleWriteOperation => formatter.write_str("写入操作已过期或已完成"),
            Self::OldWorkspaceGeneration => formatter.write_str("写入操作属于旧设备工作区"),
            Self::OperationTokenExhausted => formatter.write_str("操作令牌已耗尽"),
            Self::WorkspaceGenerationExhausted => formatter.write_str("工作区世代已耗尽"),
            Self::CommitInFlight => formatter.write_str("已有固化操作等待确认"),
            Self::CommitOperationMismatch => formatter.write_str("固化确认与活动计划不匹配"),
            Self::StaleCommitOperation => formatter.write_str("固化计划已过期或已完成"),
            Self::RevertInFlight => formatter.write_str("已有批量回退等待确认"),
            Self::RevertCoverageMismatch => formatter.write_str("批量回退结果覆盖不完整或重复"),
            Self::RevertOperationMismatch => formatter.write_str("批量回退结果与活动计划不匹配"),
            Self::StaleRevertOperation => formatter.write_str("批量回退计划已过期或已完成"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

#[derive(Clone, Debug)]
pub struct ParameterWorkspace {
    records: BTreeMap<u32, ParameterRecord>,
    in_flight: BTreeMap<u32, PendingWrite>,
    queued_latest: BTreeMap<u32, ParamValue>,
    storage_generation: u32,
    history: VecDeque<ConfirmedChange>,
    workspace_generation: WorkspaceGeneration,
    next_operation_token: u64,
    active_commit: Option<CommitPlan>,
    active_revert: Option<RevertPlan>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OperationToken(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkspaceGeneration(u64);

#[derive(Clone, Debug, PartialEq)]
pub struct PendingWrite {
    operation_token: OperationToken,
    workspace_generation: WorkspaceGeneration,
    pub param_id: u32,
    pub expected_revision: u32,
    pub value: ParamValue,
}

impl PendingWrite {
    pub const fn operation_token(&self) -> OperationToken {
        self.operation_token
    }

    pub const fn workspace_generation(&self) -> WorkspaceGeneration {
        self.workspace_generation
    }

    fn wire_matches(&self, other: &Self) -> bool {
        self.operation_token == other.operation_token
            && self.workspace_generation == other.workspace_generation
            && self.param_id == other.param_id
            && self.expected_revision == other.expected_revision
            && self.value.wire_eq(&other.value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WriteFailure {
    RevisionConflict(ParamWriteAck),
    Ordinary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitFailureKind {
    Storage,
    Verify,
    Device,
    Timeout,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommitPlan {
    operation_token: OperationToken,
    workspace_generation: WorkspaceGeneration,
    entries: Vec<ParamCommitEntry>,
    values: Vec<(u32, ParamValue)>,
    canonical_crc32: u32,
}

impl CommitPlan {
    pub const fn operation_token(&self) -> OperationToken {
        self.operation_token
    }

    pub const fn workspace_generation(&self) -> WorkspaceGeneration {
        self.workspace_generation
    }

    pub fn entries(&self) -> &[ParamCommitEntry] {
        &self.entries
    }

    pub fn values(&self) -> &[(u32, ParamValue)] {
        &self.values
    }

    pub const fn canonical_crc32(&self) -> u32 {
        self.canonical_crc32
    }

    pub fn to_protocol_commit(&self) -> ParamCommit {
        ParamCommit {
            entries: self.entries.clone(),
            canonical_crc32: self.canonical_crc32,
        }
    }

    fn wire_matches(&self, other: &Self) -> bool {
        self.operation_token == other.operation_token
            && self.workspace_generation == other.workspace_generation
            && self.canonical_crc32 == other.canonical_crc32
            && self.entries == other.entries
            && self.values.len() == other.values.len()
            && self
                .values
                .iter()
                .zip(&other.values)
                .all(|((left_id, left), (right_id, right))| {
                    left_id == right_id && left.wire_eq(right)
                })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmedChange {
    pub param_id: u32,
    pub previous_value: ParamValue,
    pub accepted_value: ParamValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RevertPlan {
    operation_token: OperationToken,
    workspace_generation: WorkspaceGeneration,
    writes: Vec<PendingWrite>,
}

impl RevertPlan {
    pub const fn operation_token(&self) -> OperationToken {
        self.operation_token
    }

    pub const fn workspace_generation(&self) -> WorkspaceGeneration {
        self.workspace_generation
    }

    pub fn writes(&self) -> &[PendingWrite] {
        &self.writes
    }

    fn wire_matches(&self, other: &Self) -> bool {
        self.operation_token == other.operation_token
            && self.workspace_generation == other.workspace_generation
            && self.writes.len() == other.writes.len()
            && self
                .writes
                .iter()
                .zip(&other.writes)
                .all(|(left, right)| left.wire_matches(right))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevertReport {
    pub confirmed_ids: Vec<u32>,
    pub failed_ids: Vec<u32>,
}

impl ParameterWorkspace {
    pub fn from_manifest_and_states(
        manifest: &DeviceManifest,
        states: &[ParamState],
    ) -> Result<Self, WorkspaceError> {
        let mut states_by_id = BTreeMap::new();
        for state in states {
            if states_by_id.insert(state.param_id, state).is_some() {
                return Err(WorkspaceError::DuplicateState(state.param_id));
            }
        }
        let mut records = BTreeMap::new();
        for descriptor in &manifest.parameters {
            if records.contains_key(&descriptor.param_id) {
                return Err(WorkspaceError::DuplicateDescriptor(descriptor.param_id));
            }
            let state = states_by_id
                .remove(&descriptor.param_id)
                .ok_or(WorkspaceError::MissingState(descriptor.param_id))?;
            if state.value.param_type() != descriptor.param_type
                || state
                    .persisted_value
                    .as_ref()
                    .is_some_and(|value| value.param_type() != descriptor.param_type)
            {
                return Err(WorkspaceError::TypeMismatch(descriptor.param_id));
            }
            let is_persistent = descriptor.flags.bits() & 0b10 != 0;
            if is_persistent != state.persisted_value.is_some() {
                return Err(WorkspaceError::InvalidPersistence(descriptor.param_id));
            }
            let dirty = state
                .persisted_value
                .as_ref()
                .is_some_and(|persisted| !state.value.wire_eq(persisted));
            records.insert(
                descriptor.param_id,
                ParameterRecord {
                    descriptor: descriptor.clone(),
                    ram_value: state.value.clone(),
                    persisted_value: state.persisted_value.clone(),
                    revision: state.revision,
                    dirty,
                    sync_state: DeviceSyncState::Known,
                    write_state: WriteState::Idle,
                    unresolved_target: None,
                    last_error: None,
                },
            );
        }
        if let Some((&param_id, _)) = states_by_id.first_key_value() {
            return Err(WorkspaceError::UnknownState(param_id));
        }
        Ok(Self {
            records,
            in_flight: BTreeMap::new(),
            queued_latest: BTreeMap::new(),
            storage_generation: 0,
            history: VecDeque::with_capacity(HISTORY_CAPACITY),
            workspace_generation: WorkspaceGeneration(1),
            next_operation_token: 1,
            active_commit: None,
            active_revert: None,
        })
    }

    pub fn get(&self, param_id: u32) -> Option<&ParameterRecord> {
        self.records.get(&param_id)
    }

    pub fn queue_write(
        &mut self,
        access: AccessProfile,
        param_id: u32,
        value: ParamValue,
    ) -> Result<Option<PendingWrite>, WorkspaceError> {
        self.validate_write_candidate(access, param_id, &value)?;
        let record = self
            .records
            .get_mut(&param_id)
            .ok_or(WorkspaceError::UnknownParameter(param_id))?;
        record.last_error = None;
        record.unresolved_target = None;
        if self.in_flight.contains_key(&param_id) {
            self.queued_latest.insert(param_id, value);
            record.write_state = WriteState::Queued;
            return Ok(None);
        }
        let operation_token = OperationToken(self.next_operation_token);
        self.next_operation_token = self
            .next_operation_token
            .checked_add(1)
            .ok_or(WorkspaceError::OperationTokenExhausted)?;
        let pending = PendingWrite {
            operation_token,
            workspace_generation: self.workspace_generation,
            param_id,
            expected_revision: record.revision,
            value,
        };
        record.write_state = WriteState::InFlight;
        self.in_flight.insert(param_id, pending.clone());
        Ok(Some(pending))
    }

    fn validate_write_candidate(
        &self,
        access: AccessProfile,
        param_id: u32,
        value: &ParamValue,
    ) -> Result<(), WorkspaceError> {
        if let PermissionDecision::Denied(reason) = access.can_write() {
            return Err(WorkspaceError::PermissionDenied(reason));
        }
        if self.active_commit.is_some() {
            return Err(WorkspaceError::CommitInFlight);
        }
        if self.active_revert.is_some() {
            return Err(WorkspaceError::RevertInFlight);
        }
        let record = self
            .records
            .get(&param_id)
            .ok_or(WorkspaceError::UnknownParameter(param_id))?;
        if record.sync_state == DeviceSyncState::Unknown {
            return Err(WorkspaceError::DeviceStateUnknown(param_id));
        }
        if value.param_type() != record.descriptor.param_type {
            return Err(WorkspaceError::TypeMismatch(param_id));
        }
        if record.descriptor.flags.bits() & ParamFlags::WRITABLE.bits() == 0 {
            return Err(WorkspaceError::ReadOnly(param_id));
        }
        match (value, &record.descriptor.constraints) {
            (_, ParamConstraints::None) => {}
            (
                ParamValue::I32(value),
                ParamConstraints::Numeric {
                    min: ParamValue::I32(min),
                    max: ParamValue::I32(max),
                    ..
                },
            ) if value >= min && value <= max => {}
            (
                ParamValue::U32(value),
                ParamConstraints::Numeric {
                    min: ParamValue::U32(min),
                    max: ParamValue::U32(max),
                    ..
                },
            ) if value >= min && value <= max => {}
            (
                ParamValue::F32(value),
                ParamConstraints::Numeric {
                    min: ParamValue::F32(min),
                    max: ParamValue::F32(max),
                    ..
                },
            ) if value.is_finite() && value >= min && value <= max => {}
            (ParamValue::Enum(value), ParamConstraints::Enum { options })
                if options.iter().any(|option| option.value == *value) => {}
            (ParamValue::Enum(_), ParamConstraints::Enum { .. }) => {
                return Err(WorkspaceError::InvalidEnum(param_id));
            }
            _ => return Err(WorkspaceError::OutOfRange(param_id)),
        }
        Ok(())
    }

    pub fn resolve_write(
        &mut self,
        param_id: u32,
        operation: &PendingWrite,
        result: Result<ParamWriteAck, WriteFailure>,
    ) -> Result<Option<PendingWrite>, WorkspaceError> {
        if operation.workspace_generation != self.workspace_generation {
            return Err(WorkspaceError::OldWorkspaceGeneration);
        }
        if operation.param_id != param_id {
            return Err(WorkspaceError::WriteOperationMismatch);
        }
        let active = self
            .in_flight
            .get(&param_id)
            .ok_or(WorkspaceError::StaleWriteOperation)?;
        if !active.wire_matches(operation) {
            return Err(WorkspaceError::WriteOperationMismatch);
        }
        let pending = self
            .in_flight
            .remove(&param_id)
            .ok_or(WorkspaceError::StaleWriteOperation)?;
        let record = self
            .records
            .get_mut(&param_id)
            .ok_or(WorkspaceError::UnknownParameter(param_id))?;
        let returned_value = match &result {
            Ok(ack) | Err(WriteFailure::RevisionConflict(ack)) => Some(&ack.value),
            Err(WriteFailure::Ordinary) => None,
        };
        if returned_value.is_some_and(|value| value.param_type() != record.descriptor.param_type) {
            self.queued_latest.remove(&param_id);
            record.write_state = WriteState::Idle;
            record.unresolved_target = None;
            record.last_error = Some("设备返回的参数类型无效");
            return Err(WorkspaceError::TypeMismatch(param_id));
        }
        match result {
            Ok(ack) => {
                let previous_value = record.ram_value.clone();
                record.ram_value = ack.value;
                record.revision = ack.new_revision;
                record.dirty = record
                    .persisted_value
                    .as_ref()
                    .is_some_and(|persisted| !record.ram_value.wire_eq(persisted));
                record.last_error = None;
                record.unresolved_target = None;
                if !previous_value.wire_eq(&record.ram_value) {
                    if self.history.len() == HISTORY_CAPACITY {
                        self.history.pop_front();
                    }
                    self.history.push_back(ConfirmedChange {
                        param_id,
                        previous_value,
                        accepted_value: record.ram_value.clone(),
                    });
                }
                let Some(queued) = self.queued_latest.remove(&param_id) else {
                    record.write_state = WriteState::Idle;
                    return Ok(None);
                };
                if queued.wire_eq(&record.ram_value) {
                    record.write_state = WriteState::Idle;
                    return Ok(None);
                }
                let operation_token = OperationToken(self.next_operation_token);
                self.next_operation_token = self
                    .next_operation_token
                    .checked_add(1)
                    .ok_or(WorkspaceError::OperationTokenExhausted)?;
                let follow_up = PendingWrite {
                    operation_token,
                    workspace_generation: self.workspace_generation,
                    param_id,
                    expected_revision: record.revision,
                    value: queued,
                };
                record.write_state = WriteState::InFlight;
                self.in_flight.insert(param_id, follow_up.clone());
                Ok(Some(follow_up))
            }
            Err(WriteFailure::RevisionConflict(current)) => {
                let unresolved = self
                    .queued_latest
                    .remove(&param_id)
                    .unwrap_or(pending.value);
                record.ram_value = current.value;
                record.revision = current.new_revision;
                record.dirty = record
                    .persisted_value
                    .as_ref()
                    .is_some_and(|persisted| !record.ram_value.wire_eq(persisted));
                record.unresolved_target = Some(unresolved);
                record.last_error = Some("参数已在设备端变化，请确认后重试");
                record.write_state = WriteState::Idle;
                Ok(None)
            }
            Err(WriteFailure::Ordinary) => {
                self.queued_latest.remove(&param_id);
                record.unresolved_target = None;
                record.last_error = Some("参数写入失败，请重试");
                record.write_state = WriteState::Idle;
                Ok(None)
            }
        }
    }

    pub fn commit_dirty(
        &mut self,
        access: AccessProfile,
    ) -> Result<Option<CommitPlan>, WorkspaceError> {
        if let PermissionDecision::Denied(reason) = access.can_commit() {
            return Err(WorkspaceError::PermissionDenied(reason));
        }
        if self.active_commit.is_some() {
            return Err(WorkspaceError::CommitInFlight);
        }
        if !self.in_flight.is_empty() || !self.queued_latest.is_empty() {
            return Err(WorkspaceError::WritesPending);
        }
        let mut entries = Vec::new();
        let mut values = Vec::new();
        for (&param_id, record) in &self.records {
            if record.dirty && record.descriptor.flags.bits() & ParamFlags::PERSISTENT.bits() != 0 {
                entries.push(ParamCommitEntry {
                    param_id,
                    revision: record.revision,
                });
                values.push((param_id, record.ram_value.clone()));
            }
        }
        if entries.is_empty() {
            return Ok(None);
        }
        let canonical_crc32 =
            canonical_parameter_crc32(&values).map_err(|_| WorkspaceError::InvalidCommitPlan)?;
        let operation_token = OperationToken(self.next_operation_token);
        self.next_operation_token = self
            .next_operation_token
            .checked_add(1)
            .ok_or(WorkspaceError::OperationTokenExhausted)?;
        let plan = CommitPlan {
            operation_token,
            workspace_generation: self.workspace_generation,
            entries,
            values,
            canonical_crc32,
        };
        self.active_commit = Some(plan.clone());
        Ok(Some(plan))
    }

    pub fn resolve_commit(
        &mut self,
        plan: &CommitPlan,
        result: Result<ParamCommitAck, CommitFailureKind>,
    ) -> Result<(), WorkspaceError> {
        if plan.workspace_generation != self.workspace_generation {
            return Err(WorkspaceError::OldWorkspaceGeneration);
        }
        let active = self
            .active_commit
            .as_ref()
            .ok_or(WorkspaceError::StaleCommitOperation)?;
        if !active.wire_matches(plan) {
            return Err(WorkspaceError::CommitOperationMismatch);
        }
        self.active_commit = None;
        let ack = result.map_err(WorkspaceError::CommitFailed)?;
        if ack.canonical_crc32 != plan.canonical_crc32 {
            return Err(WorkspaceError::CommitCrcMismatch);
        }
        if plan.entries.len() != plan.values.len()
            || canonical_parameter_crc32(&plan.values)
                .map_err(|_| WorkspaceError::InvalidCommitPlan)?
                != plan.canonical_crc32
        {
            return Err(WorkspaceError::InvalidCommitPlan);
        }
        for (entry, (value_param_id, value)) in plan.entries.iter().zip(&plan.values) {
            let Some(record) = self.records.get(&entry.param_id) else {
                return Err(WorkspaceError::InvalidCommitPlan);
            };
            if entry.param_id != *value_param_id
                || entry.revision != record.revision
                || !record.ram_value.wire_eq(value)
                || record.descriptor.flags.bits() & ParamFlags::PERSISTENT.bits() == 0
            {
                return Err(WorkspaceError::InvalidCommitPlan);
            }
        }
        for (param_id, value) in &plan.values {
            let record = self
                .records
                .get_mut(param_id)
                .ok_or(WorkspaceError::InvalidCommitPlan)?;
            record.persisted_value = Some(value.clone());
            record.dirty = false;
        }
        self.storage_generation = ack.storage_generation;
        Ok(())
    }

    pub fn dirty_count(&self) -> usize {
        self.records.values().filter(|record| record.dirty).count()
    }

    pub const fn storage_generation(&self) -> u32 {
        self.storage_generation
    }

    pub fn revert_parameter(
        &mut self,
        access: AccessProfile,
        param_id: u32,
    ) -> Result<PendingWrite, WorkspaceError> {
        if self.in_flight.contains_key(&param_id) || self.queued_latest.contains_key(&param_id) {
            return Err(WorkspaceError::WritesPending);
        }
        let persisted = self
            .records
            .get(&param_id)
            .ok_or(WorkspaceError::UnknownParameter(param_id))?
            .persisted_value
            .clone()
            .ok_or(WorkspaceError::NotRevertible(param_id))?;
        self.queue_write(access, param_id, persisted)?
            .ok_or(WorkspaceError::WritesPending)
    }

    pub fn revert_all(&mut self, access: AccessProfile) -> Result<RevertPlan, WorkspaceError> {
        if let PermissionDecision::Denied(reason) = access.can_write() {
            return Err(WorkspaceError::PermissionDenied(reason));
        }
        if self.active_commit.is_some() {
            return Err(WorkspaceError::CommitInFlight);
        }
        if self.active_revert.is_some() {
            return Err(WorkspaceError::RevertInFlight);
        }
        if !self.in_flight.is_empty() || !self.queued_latest.is_empty() {
            return Err(WorkspaceError::WritesPending);
        }
        let mut targets = Vec::new();
        for (&param_id, record) in &self.records {
            if !record.dirty {
                continue;
            }
            let value = record
                .persisted_value
                .clone()
                .ok_or(WorkspaceError::NotRevertible(param_id))?;
            self.validate_write_candidate(access, param_id, &value)
                .map_err(|error| match error {
                    WorkspaceError::ReadOnly(_) => WorkspaceError::NotRevertible(param_id),
                    other => other,
                })?;
            targets.push((param_id, value));
        }
        let token_count = u64::try_from(targets.len())
            .map_err(|_| WorkspaceError::OperationTokenExhausted)?
            .checked_add(1)
            .ok_or(WorkspaceError::OperationTokenExhausted)?;
        let next_operation_token = self
            .next_operation_token
            .checked_add(token_count)
            .ok_or(WorkspaceError::OperationTokenExhausted)?;
        let batch_token = OperationToken(self.next_operation_token);
        let mut writes = Vec::with_capacity(targets.len());
        for (index, (param_id, value)) in targets.into_iter().enumerate() {
            let offset = u64::try_from(index)
                .map_err(|_| WorkspaceError::OperationTokenExhausted)?
                .checked_add(1)
                .ok_or(WorkspaceError::OperationTokenExhausted)?;
            let operation_token = OperationToken(
                self.next_operation_token
                    .checked_add(offset)
                    .ok_or(WorkspaceError::OperationTokenExhausted)?,
            );
            let record = self
                .records
                .get(&param_id)
                .ok_or(WorkspaceError::UnknownParameter(param_id))?;
            writes.push(PendingWrite {
                operation_token,
                workspace_generation: self.workspace_generation,
                param_id,
                expected_revision: record.revision,
                value,
            });
        }
        let plan = RevertPlan {
            operation_token: batch_token,
            workspace_generation: self.workspace_generation,
            writes,
        };
        for pending in &plan.writes {
            let record = self
                .records
                .get_mut(&pending.param_id)
                .ok_or(WorkspaceError::UnknownParameter(pending.param_id))?;
            record.last_error = None;
            record.unresolved_target = None;
            record.write_state = WriteState::InFlight;
            self.in_flight.insert(pending.param_id, pending.clone());
        }
        self.next_operation_token = next_operation_token;
        self.active_revert = Some(plan.clone());
        Ok(plan)
    }

    pub fn resolve_revert_all<I>(
        &mut self,
        plan: &RevertPlan,
        results: I,
    ) -> Result<RevertReport, WorkspaceError>
    where
        I: IntoIterator<Item = (PendingWrite, Result<ParamWriteAck, WriteFailure>)>,
    {
        if plan.workspace_generation != self.workspace_generation {
            return Err(WorkspaceError::OldWorkspaceGeneration);
        }
        let active = self
            .active_revert
            .as_ref()
            .ok_or(WorkspaceError::StaleRevertOperation)?;
        if !active.wire_matches(plan) {
            return Err(WorkspaceError::RevertOperationMismatch);
        }
        let mut results = results.into_iter().collect::<Vec<_>>();
        if results.len() != plan.writes.len() {
            return Err(WorkspaceError::RevertCoverageMismatch);
        }
        let mut covered = vec![false; plan.writes.len()];
        for (operation, result) in &results {
            let Some(index) = plan
                .writes
                .iter()
                .position(|planned| planned.operation_token == operation.operation_token)
            else {
                return Err(WorkspaceError::RevertCoverageMismatch);
            };
            if covered[index] || !plan.writes[index].wire_matches(operation) {
                return Err(WorkspaceError::RevertCoverageMismatch);
            }
            let Some(in_flight) = self.in_flight.get(&operation.param_id) else {
                return Err(WorkspaceError::RevertCoverageMismatch);
            };
            if !in_flight.wire_matches(operation) {
                return Err(WorkspaceError::RevertCoverageMismatch);
            }
            let returned_value = match result {
                Ok(ack) | Err(WriteFailure::RevisionConflict(ack)) => Some(&ack.value),
                Err(WriteFailure::Ordinary) => None,
            };
            let record = self
                .records
                .get(&operation.param_id)
                .ok_or(WorkspaceError::RevertCoverageMismatch)?;
            if returned_value
                .is_some_and(|value| value.param_type() != record.descriptor.param_type)
            {
                return Err(WorkspaceError::RevertCoverageMismatch);
            }
            covered[index] = true;
        }
        if covered.iter().any(|covered| !covered) {
            return Err(WorkspaceError::RevertCoverageMismatch);
        }
        let mut confirmed_ids = Vec::new();
        let mut failed_ids = Vec::new();
        for planned in &plan.writes {
            let result_index = results
                .iter()
                .position(|(operation, _)| operation.operation_token == planned.operation_token)
                .ok_or(WorkspaceError::RevertCoverageMismatch)?;
            let (operation, result) = results.swap_remove(result_index);
            let confirmed = result.is_ok();
            if self
                .resolve_write(operation.param_id, &operation, result)?
                .is_some()
            {
                return Err(WorkspaceError::WritesPending);
            }
            if confirmed {
                confirmed_ids.push(operation.param_id);
            } else {
                failed_ids.push(operation.param_id);
            }
        }
        self.active_revert = None;
        Ok(RevertReport {
            confirmed_ids,
            failed_ids,
        })
    }

    pub fn undo_last_confirmed_change(
        &mut self,
        access: AccessProfile,
    ) -> Result<Option<PendingWrite>, WorkspaceError> {
        if !self.in_flight.is_empty() || !self.queued_latest.is_empty() {
            return Err(WorkspaceError::WritesPending);
        }
        let Some(change) = self.history.back().cloned() else {
            return Ok(None);
        };
        self.queue_write(access, change.param_id, change.previous_value)
    }

    pub fn history_snapshot(&self) -> Vec<ConfirmedChange> {
        self.history.iter().cloned().collect()
    }

    pub fn mark_disconnected(&mut self) {
        self.in_flight.clear();
        self.queued_latest.clear();
        self.history.clear();
        self.active_commit = None;
        self.active_revert = None;
        for record in self.records.values_mut() {
            record.sync_state = DeviceSyncState::Unknown;
            record.write_state = WriteState::Idle;
            record.unresolved_target = None;
            record.last_error = Some("设备已断开，状态未知");
        }
    }

    pub fn replace_from_connected(
        &mut self,
        connected: &ConnectedDevice,
    ) -> Result<(), WorkspaceError> {
        let next_generation = self
            .workspace_generation
            .0
            .checked_add(1)
            .map(WorkspaceGeneration)
            .ok_or(WorkspaceError::WorkspaceGenerationExhausted)?;
        let mut replacement =
            Self::from_manifest_and_states(&connected.manifest, &connected.parameter_states)?;
        replacement.workspace_generation = next_generation;
        replacement.next_operation_token = self.next_operation_token;
        *self = replacement;
        Ok(())
    }

    pub fn pending_write_count(&self) -> usize {
        self.in_flight.len() + self.queued_latest.len()
    }

    pub fn records(&self) -> impl ExactSizeIterator<Item = &ParameterRecord> {
        self.records.values()
    }

    pub(crate) fn validate_pending_execution(
        &self,
        operation: &PendingWrite,
    ) -> Result<(), WorkspaceError> {
        if operation.workspace_generation != self.workspace_generation {
            return Err(WorkspaceError::OldWorkspaceGeneration);
        }
        let active = self
            .in_flight
            .get(&operation.param_id)
            .ok_or(WorkspaceError::StaleWriteOperation)?;
        if !active.wire_matches(operation) {
            return Err(WorkspaceError::WriteOperationMismatch);
        }
        Ok(())
    }

    pub(crate) fn validate_commit_execution(
        &self,
        plan: &CommitPlan,
    ) -> Result<(), WorkspaceError> {
        if plan.workspace_generation != self.workspace_generation {
            return Err(WorkspaceError::OldWorkspaceGeneration);
        }
        let active = self
            .active_commit
            .as_ref()
            .ok_or(WorkspaceError::StaleCommitOperation)?;
        if !active.wire_matches(plan) {
            return Err(WorkspaceError::CommitOperationMismatch);
        }
        Ok(())
    }
}
