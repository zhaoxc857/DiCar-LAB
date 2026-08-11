use std::sync::{Arc, Mutex};

use dctp_sim::SimulatorServer;
use dicar_app_core::{
    AccessProfileDto, AccessRoleDto, CoreCommand, CoreConfig, ParamValueDto, SnapshotPhase,
};
use dicar_desktop_lib::{
    AppState, CloseDecision, CloseRequestOutcome, CloseResolution, FrontendEvent,
    FrontendEventPayload, FrontendSink,
};

#[derive(Default)]
struct RecordingSink(Mutex<Vec<FrontendEvent>>);

impl FrontendSink for RecordingSink {
    fn send(&self, event: FrontendEvent) -> Result<(), String> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

impl RecordingSink {
    fn events(&self) -> Vec<FrontendEvent> {
        self.0.lock().unwrap().clone()
    }
}

fn connected_dirty_state() -> (SimulatorServer, AppState, Arc<RecordingSink>) {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let state = AppState::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let sink = Arc::new(RecordingSink::default());
    state.replace_frontend_sink(sink.clone()).unwrap();
    state.dispatch(CoreCommand::Connect).unwrap();
    state
        .dispatch(CoreCommand::WriteParameter {
            param_id: 1,
            value: ParamValueDto::F32(2.5),
        })
        .unwrap();
    assert_eq!(state.snapshot().dirty_count, 1);
    (server, state, sink)
}

#[test]
fn clean_close_is_allowed_and_dirty_close_is_serialized_until_cancelled() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let state = AppState::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    assert_eq!(
        state.request_window_close().unwrap(),
        CloseRequestOutcome::Allow
    );
    drop(state);
    server.shutdown().unwrap();

    let (server, state, sink) = connected_dirty_state();
    let first = state.request_window_close().unwrap();
    let request_id = match first {
        CloseRequestOutcome::Prevented { request_id, .. } => request_id,
        CloseRequestOutcome::Allow => panic!("dirty state must prevent close"),
    };
    assert_eq!(state.request_window_close().unwrap(), first);
    assert!(sink
        .events()
        .iter()
        .any(|event| matches!(event.payload, FrontendEventPayload::WindowCloseRequested(_))));

    assert!(state
        .resolve_window_close(request_id + 1, CloseDecision::Cancel)
        .is_err());
    assert_eq!(state.snapshot().phase, SnapshotPhase::Ready);
    assert_eq!(
        state
            .resolve_window_close(request_id, CloseDecision::Cancel)
            .unwrap(),
        CloseResolution::KeepOpen
    );
    assert_eq!(state.snapshot().phase, SnapshotPhase::Ready);
    drop(state);
    server.shutdown().unwrap();
}

#[test]
fn disconnect_close_marks_truth_unknown_and_revert_failure_keeps_window_open() {
    let (server, state, _) = connected_dirty_state();
    let request_id = match state.request_window_close().unwrap() {
        CloseRequestOutcome::Prevented { request_id, .. } => request_id,
        CloseRequestOutcome::Allow => panic!("dirty state must prevent close"),
    };
    assert_eq!(
        state
            .resolve_window_close(request_id, CloseDecision::DisconnectKeepUnknown)
            .unwrap(),
        CloseResolution::CloseWindow
    );
    let snapshot = state.snapshot();
    assert_eq!(snapshot.phase, SnapshotPhase::Disconnected);
    assert!(snapshot
        .parameters
        .iter()
        .all(|parameter| !parameter.sync_known));
    drop(state);
    server.shutdown().unwrap();

    let (server, state, _) = connected_dirty_state();
    state
        .dispatch(CoreCommand::SelectAccessProfile {
            profile: AccessProfileDto {
                role: AccessRoleDto::Observer,
                lease_active: true,
            },
        })
        .unwrap();
    let request_id = match state.request_window_close().unwrap() {
        CloseRequestOutcome::Prevented { request_id, .. } => request_id,
        CloseRequestOutcome::Allow => panic!("dirty state must prevent close"),
    };
    assert!(state
        .resolve_window_close(request_id, CloseDecision::RevertThenClose)
        .is_err());
    assert_eq!(state.snapshot().phase, SnapshotPhase::Ready);
    assert_eq!(state.snapshot().dirty_count, 1);
    drop(state);
    server.shutdown().unwrap();
}

#[test]
fn a_dirty_close_can_be_reissued_after_the_frontend_channel_opens() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let state = AppState::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    state.dispatch(CoreCommand::Connect).unwrap();
    state
        .dispatch(CoreCommand::WriteParameter {
            param_id: 1,
            value: ParamValueDto::F32(2.5),
        })
        .unwrap();

    let error = state.request_window_close().unwrap_err();
    assert_eq!(error.code, "frontendChannelUnavailable");

    let sink = Arc::new(RecordingSink::default());
    state.replace_frontend_sink(sink.clone()).unwrap();
    assert!(matches!(
        state.request_window_close().unwrap(),
        CloseRequestOutcome::Prevented { .. }
    ));
    assert_eq!(
        sink.events()
            .iter()
            .filter(|event| matches!(event.payload, FrontendEventPayload::WindowCloseRequested(_)))
            .count(),
        1
    );
    drop(state);
    server.shutdown().unwrap();
}
