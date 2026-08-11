use dctp_protocol::{MessageType, ParamValue, ParamWrite, ParamWriteAck, WireEncode};
use dctp_sim::SimulatorServer;
use dicar_app_core::{
    decode_revision_conflict_context, AccessProfile, AccessRole, CoreError, FixedNonce, LeaseState,
    ParameterWorkspace, ProtocolSession, SystemClock, TcpTransport, Transport, TransportError,
    TransportIdentity, WorkspaceError,
};

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[usize::from(byte >> 4)] as char);
        value.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    value
}

#[test]
fn typed_real_session_write_conflict_and_commit_round_trip() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let transport = TcpTransport::connect(server.local_addr()).unwrap();
    let mut session = ProtocolSession::new(transport, FixedNonce(55), SystemClock::new());
    let connected = session.connect_and_load().unwrap();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(
        &connected.manifest,
        &connected.parameter_states,
    )
    .unwrap();
    let mut stale_workspace = workspace.clone();
    let owner = AccessProfile::new(AccessRole::Owner, LeaseState::Active);
    let pending = workspace
        .queue_write(owner, 1, ParamValue::F32(2.25))
        .unwrap()
        .unwrap();

    let ack = session.execute_write(&mut workspace, &pending).unwrap();
    assert!(ack.value.wire_eq(&ParamValue::F32(2.25)));
    workspace
        .resolve_write(1, &pending, Ok(ack.clone()))
        .unwrap();

    let stale_pending = stale_workspace
        .queue_write(owner, 1, ParamValue::F32(2.5))
        .unwrap()
        .unwrap();
    let conflict = match session.execute_write(&mut stale_workspace, &stale_pending) {
        Err(CoreError::RevisionConflict { current }) => {
            assert!(current.value.wire_eq(&ParamValue::F32(2.25)));
            assert_eq!(current.new_revision, ack.new_revision);
            current
        }
        other => panic!("expected typed revision conflict, got {other:?}"),
    };
    stale_workspace
        .resolve_write(
            1,
            &stale_pending,
            Err(dicar_app_core::WriteFailure::RevisionConflict(conflict)),
        )
        .unwrap();

    let plan = stale_workspace.commit_dirty(owner).unwrap().unwrap();
    let commit_ack = session.execute_commit(&mut stale_workspace, &plan).unwrap();
    assert_eq!(commit_ack.canonical_crc32, plan.canonical_crc32());
    assert_eq!(commit_ack.storage_generation, 1);
    stale_workspace
        .resolve_commit(&plan, Ok(commit_ack))
        .unwrap();
    assert_eq!(stale_workspace.dirty_count(), 0);

    session.close().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn revision_conflict_context_is_strict_lowercase_hex_and_exact_ack_bytes() {
    let ack = ParamWriteAck {
        value: ParamValue::F32(f32::from_bits(0x7fc0_00ab)),
        new_revision: 0xdead_beef,
    };
    let valid = lower_hex(&ack.encode().unwrap());
    let decoded = decode_revision_conflict_context(&valid).unwrap();
    assert!(decoded.value.wire_eq(&ack.value));
    assert_eq!(decoded.new_revision, ack.new_revision);

    for malformed in [
        valid.to_uppercase(),
        valid[..valid.len() - 1].to_owned(),
        format!("{}g0", &valid[..valid.len() - 2]),
        format!("{valid}00"),
    ] {
        assert!(decode_revision_conflict_context(&malformed).is_err());
    }
}

struct CountingTransport<T> {
    inner: T,
    writes: Arc<AtomicUsize>,
}

impl<T: Transport> Transport for CountingTransport<T> {
    fn identity(&self) -> TransportIdentity {
        self.inner.identity()
    }

    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError> {
        self.inner.read(output)
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.inner.write_all(bytes)
    }

    fn close(&mut self) -> Result<(), TransportError> {
        self.inner.close()
    }
}

#[test]
fn permission_matrix_sends_zero_frames_when_denied_and_allows_tuner_ram_owner_commit() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let writes = Arc::new(AtomicUsize::new(0));
    let transport = CountingTransport {
        inner: TcpTransport::connect(server.local_addr()).unwrap(),
        writes: writes.clone(),
    };
    let mut session = ProtocolSession::new(transport, FixedNonce(77), SystemClock::new());
    let connected = session.connect_and_load().unwrap();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(
        &connected.manifest,
        &connected.parameter_states,
    )
    .unwrap();

    let before_raw_bypass = writes.load(Ordering::SeqCst);
    assert!(matches!(
        session.request(
            MessageType::ParamWrite,
            ParamWrite {
                param_id: 1,
                expected_revision: 0,
                value: ParamValue::F32(99.0),
            }
            .encode()
            .unwrap(),
        ),
        Err(CoreError::UnauthorizedParameterOperation)
    ));
    assert_eq!(writes.load(Ordering::SeqCst), before_raw_bypass);

    for profile in [
        AccessProfile::new(AccessRole::Observer, LeaseState::Active),
        AccessProfile::new(AccessRole::Owner, LeaseState::Inactive),
        AccessProfile::new(AccessRole::Tuner, LeaseState::Inactive),
    ] {
        let before = writes.load(Ordering::SeqCst);
        assert!(workspace
            .queue_write(profile, 1, ParamValue::F32(2.0))
            .is_err());
        assert_eq!(writes.load(Ordering::SeqCst), before);
    }
    let before_invalid = writes.load(Ordering::SeqCst);
    assert!(workspace
        .queue_write(
            AccessProfile::new(AccessRole::Owner, LeaseState::Active),
            1,
            ParamValue::F32(f32::NAN),
        )
        .is_err());
    assert_eq!(writes.load(Ordering::SeqCst), before_invalid);

    let tuner = AccessProfile::new(AccessRole::Tuner, LeaseState::Active);
    let pending = workspace
        .queue_write(tuner, 1, ParamValue::F32(2.25))
        .unwrap()
        .unwrap();
    let ack = session.execute_write(&mut workspace, &pending).unwrap();
    workspace.resolve_write(1, &pending, Ok(ack)).unwrap();
    for denied in [
        AccessProfile::new(AccessRole::Observer, LeaseState::Active),
        AccessProfile::new(AccessRole::Owner, LeaseState::Inactive),
        AccessProfile::new(AccessRole::Tuner, LeaseState::Inactive),
        tuner,
    ] {
        let before_denied_commit = writes.load(Ordering::SeqCst);
        assert!(workspace.commit_dirty(denied).is_err());
        assert_eq!(writes.load(Ordering::SeqCst), before_denied_commit);
    }

    let owner = AccessProfile::new(AccessRole::Owner, LeaseState::Active);
    let pending = workspace
        .queue_write(owner, 100, ParamValue::U32(640))
        .unwrap()
        .unwrap();
    let ack = session.execute_write(&mut workspace, &pending).unwrap();
    workspace.resolve_write(100, &pending, Ok(ack)).unwrap();

    let plan = workspace.commit_dirty(owner).unwrap().unwrap();
    let ack = session.execute_commit(&mut workspace, &plan).unwrap();
    workspace.resolve_commit(&plan, Ok(ack)).unwrap();
    assert_eq!(workspace.dirty_count(), 0);
    let before_empty_commit = writes.load(Ordering::SeqCst);
    assert!(workspace.commit_dirty(owner).unwrap().is_none());
    assert_eq!(writes.load(Ordering::SeqCst), before_empty_commit);

    session.close().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn active_commit_plan_dispatches_once_and_advances_device_generation_once() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let writes = Arc::new(AtomicUsize::new(0));
    let transport = CountingTransport {
        inner: TcpTransport::connect(server.local_addr()).unwrap(),
        writes: writes.clone(),
    };
    let mut session = ProtocolSession::new(transport, FixedNonce(78), SystemClock::new());
    let connected = session.connect_and_load().unwrap();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(
        &connected.manifest,
        &connected.parameter_states,
    )
    .unwrap();
    let owner = AccessProfile::new(AccessRole::Owner, LeaseState::Active);
    let pending = workspace
        .queue_write(owner, 1, ParamValue::F32(2.25))
        .unwrap()
        .unwrap();
    let write_ack = session.execute_write(&mut workspace, &pending).unwrap();
    workspace.resolve_write(1, &pending, Ok(write_ack)).unwrap();
    let plan = workspace.commit_dirty(owner).unwrap().unwrap();

    let before_commit = writes.load(Ordering::SeqCst);
    let commit_ack = session.execute_commit(&mut workspace, &plan).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), before_commit + 1);
    let after_first_commit = writes.load(Ordering::SeqCst);
    assert!(matches!(
        session.execute_commit(&mut workspace, &plan),
        Err(CoreError::Workspace(
            WorkspaceError::CommitAlreadyDispatched
        ))
    ));
    assert_eq!(writes.load(Ordering::SeqCst), after_first_commit);

    workspace.resolve_commit(&plan, Ok(commit_ack)).unwrap();
    assert_eq!(workspace.storage_generation(), 1);
    assert_eq!(workspace.dirty_count(), 0);

    session.close().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn active_write_dispatches_once_and_rejects_duplicate_wrong_and_stale_handles() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let writes = Arc::new(AtomicUsize::new(0));
    let transport = CountingTransport {
        inner: TcpTransport::connect(server.local_addr()).unwrap(),
        writes: writes.clone(),
    };
    let mut session = ProtocolSession::new(transport, FixedNonce(79), SystemClock::new());
    let connected = session.connect_and_load().unwrap();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(
        &connected.manifest,
        &connected.parameter_states,
    )
    .unwrap();
    let tuner = AccessProfile::new(AccessRole::Tuner, LeaseState::Active);
    let pending = workspace
        .queue_write(tuner, 1, ParamValue::F32(2.25))
        .unwrap()
        .unwrap();

    let before_first = writes.load(Ordering::SeqCst);
    let ack = session.execute_write(&mut workspace, &pending).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), before_first + 1);
    let after_first = writes.load(Ordering::SeqCst);
    assert!(matches!(
        session.execute_write(&mut workspace, &pending),
        Err(CoreError::Workspace(WorkspaceError::WriteAlreadyDispatched))
    ));
    assert_eq!(writes.load(Ordering::SeqCst), after_first);

    workspace.resolve_write(1, &pending, Ok(ack)).unwrap();
    assert!(matches!(
        session.execute_write(&mut workspace, &pending),
        Err(CoreError::Workspace(WorkspaceError::StaleWriteOperation))
    ));
    assert_eq!(writes.load(Ordering::SeqCst), after_first);

    let replacement = workspace
        .queue_write(tuner, 1, ParamValue::F32(2.5))
        .unwrap()
        .unwrap();
    assert!(matches!(
        session.execute_write(&mut workspace, &pending),
        Err(CoreError::Workspace(WorkspaceError::WriteOperationMismatch))
    ));
    assert_eq!(writes.load(Ordering::SeqCst), after_first);
    let replacement_ack = session.execute_write(&mut workspace, &replacement).unwrap();
    workspace
        .resolve_write(1, &replacement, Ok(replacement_ack))
        .unwrap();
    assert!(workspace
        .get(1)
        .unwrap()
        .ram_value
        .wire_eq(&ParamValue::F32(2.5)));

    session.close().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn revert_batch_entry_dispatches_once_and_only_batch_resolution_applies_it() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let writes = Arc::new(AtomicUsize::new(0));
    let transport = CountingTransport {
        inner: TcpTransport::connect(server.local_addr()).unwrap(),
        writes: writes.clone(),
    };
    let mut session = ProtocolSession::new(transport, FixedNonce(80), SystemClock::new());
    let connected = session.connect_and_load().unwrap();
    let mut workspace = ParameterWorkspace::from_manifest_and_states(
        &connected.manifest,
        &connected.parameter_states,
    )
    .unwrap();
    let tuner = AccessProfile::new(AccessRole::Tuner, LeaseState::Active);
    let pending = workspace
        .queue_write(tuner, 1, ParamValue::F32(2.25))
        .unwrap()
        .unwrap();
    let ack = session.execute_write(&mut workspace, &pending).unwrap();
    workspace.resolve_write(1, &pending, Ok(ack)).unwrap();
    let plan = workspace.revert_all(tuner).unwrap();
    let batch_write = plan.writes()[0].clone();

    let before_revert = writes.load(Ordering::SeqCst);
    let revert_ack = session.execute_write(&mut workspace, &batch_write).unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), before_revert + 1);
    let after_first = writes.load(Ordering::SeqCst);
    assert!(matches!(
        session.execute_write(&mut workspace, &batch_write),
        Err(CoreError::Workspace(WorkspaceError::WriteAlreadyDispatched))
    ));
    assert_eq!(writes.load(Ordering::SeqCst), after_first);

    let report = workspace
        .resolve_revert_all(&plan, [(batch_write, Ok(revert_ack))])
        .unwrap();
    assert_eq!(report.confirmed_ids, vec![1]);
    assert_eq!(workspace.pending_write_count(), 0);
    assert_eq!(workspace.dirty_count(), 0);

    session.close().unwrap();
    server.shutdown().unwrap();
}
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
