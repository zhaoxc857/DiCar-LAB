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
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingWrite {
    pub param_id: u32,
    pub expected_revision: u32,
    pub value: ParamValue,
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
    entries: Vec<ParamCommitEntry>,
    values: Vec<(u32, ParamValue)>,
    canonical_crc32: u32,
}

impl CommitPlan {
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmedChange {
    pub param_id: u32,
    pub previous_value: ParamValue,
    pub accepted_value: ParamValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RevertPlan {
    pub writes: Vec<PendingWrite>,
    pub not_revertible_ids: Vec<u32>,
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
        if let PermissionDecision::Denied(reason) = access.can_write() {
            return Err(WorkspaceError::PermissionDenied(reason));
        }
        let record = self
            .records
            .get_mut(&param_id)
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
        match (&value, &record.descriptor.constraints) {
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
        record.last_error = None;
        record.unresolved_target = None;
        if self.in_flight.contains_key(&param_id) {
            self.queued_latest.insert(param_id, value);
            record.write_state = WriteState::Queued;
            return Ok(None);
        }
        let pending = PendingWrite {
            param_id,
            expected_revision: record.revision,
            value,
        };
        record.write_state = WriteState::InFlight;
        self.in_flight.insert(param_id, pending.clone());
        Ok(Some(pending))
    }

    pub fn resolve_write(
        &mut self,
        param_id: u32,
        result: Result<ParamWriteAck, WriteFailure>,
    ) -> Result<Option<PendingWrite>, WorkspaceError> {
        let pending = self
            .in_flight
            .remove(&param_id)
            .ok_or(WorkspaceError::UnexpectedAck(param_id))?;
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
                let follow_up = PendingWrite {
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
        &self,
        access: AccessProfile,
    ) -> Result<Option<CommitPlan>, WorkspaceError> {
        if let PermissionDecision::Denied(reason) = access.can_commit() {
            return Err(WorkspaceError::PermissionDenied(reason));
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
        Ok(Some(CommitPlan {
            entries,
            values,
            canonical_crc32,
        }))
    }

    pub fn resolve_commit(
        &mut self,
        plan: &CommitPlan,
        result: Result<ParamCommitAck, CommitFailureKind>,
    ) -> Result<(), WorkspaceError> {
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
        if !self.in_flight.is_empty() || !self.queued_latest.is_empty() {
            return Err(WorkspaceError::WritesPending);
        }
        let mut targets = Vec::new();
        let mut not_revertible_ids = Vec::new();
        for (&param_id, record) in &self.records {
            if !record.dirty {
                continue;
            }
            match &record.persisted_value {
                Some(value)
                    if record.descriptor.flags.bits() & ParamFlags::WRITABLE.bits() != 0 =>
                {
                    targets.push((param_id, value.clone()));
                }
                _ => not_revertible_ids.push(param_id),
            }
        }
        let mut writes = Vec::new();
        for (param_id, value) in targets {
            let pending = self
                .queue_write(access, param_id, value)?
                .ok_or(WorkspaceError::WritesPending)?;
            writes.push(pending);
        }
        Ok(RevertPlan {
            writes,
            not_revertible_ids,
        })
    }

    pub fn resolve_revert_all<I>(&mut self, results: I) -> Result<RevertReport, WorkspaceError>
    where
        I: IntoIterator<Item = (u32, Result<ParamWriteAck, WriteFailure>)>,
    {
        let mut confirmed_ids = Vec::new();
        let mut failed_ids = Vec::new();
        for (param_id, result) in results {
            let confirmed = result.is_ok();
            if self.resolve_write(param_id, result)?.is_some() {
                return Err(WorkspaceError::WritesPending);
            }
            if confirmed {
                confirmed_ids.push(param_id);
            } else {
                failed_ids.push(param_id);
            }
        }
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
        let replacement =
            Self::from_manifest_and_states(&connected.manifest, &connected.parameter_states)?;
        *self = replacement;
        Ok(())
    }

    pub fn pending_write_count(&self) -> usize {
        self.in_flight.len() + self.queued_latest.len()
    }

    pub fn records(&self) -> impl ExactSizeIterator<Item = &ParameterRecord> {
        self.records.values()
    }
}
