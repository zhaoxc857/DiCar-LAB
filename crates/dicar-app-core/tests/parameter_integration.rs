use dctp_protocol::{
    canonical_parameter_crc32, ParamCommit, ParamCommitEntry, ParamValue, ParamWriteAck, WireEncode,
};
use dctp_sim::SimulatorServer;
use dicar_app_core::{
    decode_revision_conflict_context, AccessProfile, AccessRole, CoreError, FixedNonce, LeaseState,
    ParameterWorkspace, ProtocolSession, SystemClock, TcpTransport, Transport, TransportError,
    TransportIdentity,
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
    let initial = connected
        .parameter_states
        .iter()
        .find(|state| state.param_id == 1)
        .unwrap();

    let ack = session
        .write_parameter(1, initial.revision, ParamValue::F32(2.25))
        .unwrap();
    assert!(ack.value.wire_eq(&ParamValue::F32(2.25)));
    assert_eq!(ack.new_revision, initial.revision + 1);

    match session.write_parameter(1, initial.revision, ParamValue::F32(3.0)) {
        Err(CoreError::RevisionConflict { current }) => {
            assert!(current.value.wire_eq(&ParamValue::F32(2.25)));
            assert_eq!(current.new_revision, ack.new_revision);
        }
        other => panic!("expected typed revision conflict, got {other:?}"),
    }

    let crc = canonical_parameter_crc32(&[(1, ack.value.clone())]).unwrap();
    let commit_ack = session
        .commit_parameters(&ParamCommit {
            entries: vec![ParamCommitEntry {
                param_id: 1,
                revision: ack.new_revision,
            }],
            canonical_crc32: crc,
        })
        .unwrap();
    assert_eq!(commit_ack.canonical_crc32, crc);
    assert_eq!(commit_ack.storage_generation, 1);

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

    let tuner = AccessProfile::new(AccessRole::Tuner, LeaseState::Active);
    let pending = workspace
        .queue_write(tuner, 1, ParamValue::F32(2.25))
        .unwrap()
        .unwrap();
    let ack = session
        .write_parameter(pending.param_id, pending.expected_revision, pending.value)
        .unwrap();
    workspace.resolve_write(1, Ok(ack)).unwrap();
    let before_denied_commit = writes.load(Ordering::SeqCst);
    assert!(workspace.commit_dirty(tuner).is_err());
    assert_eq!(writes.load(Ordering::SeqCst), before_denied_commit);

    let owner = AccessProfile::new(AccessRole::Owner, LeaseState::Active);
    let pending = workspace
        .queue_write(owner, 100, ParamValue::U32(640))
        .unwrap()
        .unwrap();
    let ack = session
        .write_parameter(pending.param_id, pending.expected_revision, pending.value)
        .unwrap();
    workspace.resolve_write(100, Ok(ack)).unwrap();

    let plan = workspace.commit_dirty(owner).unwrap().unwrap();
    let ack = session
        .commit_parameters(&plan.to_protocol_commit())
        .unwrap();
    workspace.resolve_commit(&plan, Ok(ack)).unwrap();
    assert_eq!(workspace.dirty_count(), 0);
    let before_empty_commit = writes.load(Ordering::SeqCst);
    assert!(workspace.commit_dirty(owner).unwrap().is_none());
    assert_eq!(writes.load(Ordering::SeqCst), before_empty_commit);

    session.close().unwrap();
    server.shutdown().unwrap();
}
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
