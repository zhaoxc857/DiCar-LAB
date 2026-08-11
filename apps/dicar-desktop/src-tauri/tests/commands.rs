use std::sync::{Arc, Mutex};

use dctp_sim::SimulatorServer;
use dicar_app_core::{
    CoreCommand, CoreConfig, CoreEvent, CoreEventPayload, OperationId, OperationResult,
    OperationStatus, SerialHardwareProfile, SnapshotPhase,
};
use dicar_desktop_lib::{
    connect_core, list_serial_ports_core, spawn_bundled_runtime, AppState, EndpointDto,
    FrontendEvent, FrontendEventPayload, FrontendEventSequencer, FrontendSink,
};

#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<FrontendEvent>>,
}

#[test]
fn bundled_runtime_connects_to_its_own_simulator_without_an_external_service() {
    let (simulator, state) = spawn_bundled_runtime().unwrap();
    let advertised = EndpointDto::Simulator {
        address: "127.0.0.1:1".into(),
    };

    let result = connect_core(&state, advertised).unwrap();

    assert_eq!(result.status, OperationStatus::Succeeded);
    assert_eq!(state.snapshot().phase, SnapshotPhase::Ready);
    assert_eq!(
        state.snapshot().transport_identity.unwrap().endpoint,
        dicar_app_core::Endpoint::Simulator {
            address: simulator.local_addr()
        }
    );
}

#[test]
fn typed_connect_routes_simulator_to_the_configured_runtime_and_validates_serial() {
    let configured_server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let selected_server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let state = AppState::spawn(CoreConfig::simulator(configured_server.local_addr())).unwrap();
    let selected: EndpointDto = serde_json::from_value(serde_json::json!({
        "kind": "simulator",
        "address": selected_server.local_addr().to_string()
    }))
    .unwrap();

    assert_eq!(
        connect_core(&state, selected).unwrap().status,
        OperationStatus::Succeeded
    );
    let snapshot = serde_json::to_value(state.snapshot()).unwrap();
    assert_eq!(
        snapshot["transportIdentity"]["endpoint"],
        serde_json::json!({
            "kind": "simulator",
            "address": configured_server.local_addr().to_string()
        })
    );
    state.dispatch(CoreCommand::Disconnect).unwrap();

    let serial: EndpointDto = serde_json::from_value(serde_json::json!({
        "kind": "serial",
        "portName": "",
        "baudRate": 921600,
        "hardwareProfile": "genericSerial"
    }))
    .unwrap();
    let error = connect_core(&state, serial).unwrap_err();
    assert_eq!(error.code, "invalidEndpoint");
    assert_eq!(state.snapshot().phase, SnapshotPhase::Disconnected);

    drop(state);
    configured_server.shutdown().unwrap();
    selected_server.shutdown().unwrap();
}

#[test]
fn serial_endpoint_dto_accepts_hc05_profile_and_low_data_mode_baud() {
    let serial: EndpointDto = serde_json::from_value(serde_json::json!({
        "kind": "serial",
        "portName": "COM12",
        "baudRate": 9600,
        "hardwareProfile": "hc05BluetoothSpp"
    }))
    .unwrap();

    assert!(matches!(
        serial,
        EndpointDto::Serial {
            port_name,
            baud_rate: 9_600,
            hardware_profile: SerialHardwareProfile::Hc05BluetoothSpp,
        } if port_name == "COM12"
    ));
}

#[test]
fn serial_discovery_returns_ports_in_stable_name_order_when_available() {
    if let Ok(ports) = list_serial_ports_core() {
        assert!(ports
            .windows(2)
            .all(|pair| pair[0].port_name <= pair[1].port_name));
    }
}

#[test]
fn frontend_event_serializes_to_the_typescript_discriminated_union() {
    let sequencer = FrontendEventSequencer::default();
    let sink = Arc::new(RecordingSink::default());
    sequencer.replace_sink(sink.clone()).unwrap();
    sequencer.publish_window_close(7, 3, true).unwrap();

    let value = serde_json::to_value(&sink.events()[0]).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "eventIndex": 1,
            "event": "windowCloseRequested",
            "data": {"requestId": 7, "dirtyCount": 3, "canRevert": true}
        })
    );
}

impl FrontendSink for RecordingSink {
    fn send(&self, event: FrontendEvent) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

impl RecordingSink {
    fn events(&self) -> Vec<FrontendEvent> {
        self.events.lock().unwrap().clone()
    }
}

fn operation_event(actor_order: u64) -> CoreEvent {
    CoreEvent {
        actor_order,
        payload: CoreEventPayload::OperationCompleted(OperationResult {
            operation_id: OperationId(actor_order + 1),
            status: OperationStatus::Succeeded,
            message: format!("operation-{actor_order}"),
        }),
    }
}

#[test]
fn one_sequencer_serializes_core_and_window_events_without_gaps() {
    let sequencer = Arc::new(FrontendEventSequencer::default());
    let sink = Arc::new(RecordingSink::default());
    sequencer.replace_sink(sink.clone()).unwrap();

    let core_sequencer = sequencer.clone();
    let core = std::thread::spawn(move || {
        for order in 0..50 {
            core_sequencer.publish_core(operation_event(order)).unwrap();
        }
    });
    let window_sequencer = sequencer.clone();
    let window = std::thread::spawn(move || {
        for request_id in 1..=50 {
            window_sequencer
                .publish_window_close(request_id, 1, true)
                .unwrap();
        }
    });
    core.join().unwrap();
    window.join().unwrap();

    let events = sink.events();
    assert_eq!(events.len(), 100);
    assert_eq!(
        events
            .iter()
            .map(|event| event.event_index)
            .collect::<Vec<_>>(),
        (1..=100).collect::<Vec<_>>()
    );
    assert!(events
        .iter()
        .any(|event| matches!(event.payload, FrontendEventPayload::WindowCloseRequested(_))));
    assert!(events
        .iter()
        .any(|event| matches!(event.payload, FrontendEventPayload::OperationCompleted(_))));
}

#[test]
fn app_state_dispatches_real_actor_commands_and_forwards_their_results() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let state = AppState::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let sink = Arc::new(RecordingSink::default());
    state.replace_frontend_sink(sink.clone()).unwrap();

    let connected = state.dispatch(CoreCommand::Connect).unwrap();
    assert_eq!(connected.status, OperationStatus::Succeeded);
    assert_eq!(state.snapshot().phase, SnapshotPhase::Ready);
    assert!(sink
        .events()
        .iter()
        .any(|event| matches!(event.payload, FrontendEventPayload::SnapshotChanged(_))));

    let disconnected = state.dispatch(CoreCommand::Disconnect).unwrap();
    assert_eq!(disconnected.status, OperationStatus::Succeeded);
    assert_eq!(state.snapshot().phase, SnapshotPhase::Disconnected);
    drop(state);
    server.shutdown().unwrap();
}
