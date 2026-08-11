use dctp_sim::SimulatorServer;
use dicar_app_core::{ConnectionPhase, FixedNonce, ProtocolSession, TcpTransport, TestClock};

#[test]
fn session_reaches_ready_with_manifest_and_all_parameter_states() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let transport = TcpTransport::connect(server.local_addr()).unwrap();
    let mut session = ProtocolSession::new(transport, FixedNonce(0x1020_3040), TestClock::new());

    let connected = session.connect_and_load().unwrap();

    assert_eq!(connected.phase, ConnectionPhase::Ready);
    assert!(connected.manifest.parameters.len() >= 10);
    assert_eq!(
        connected.parameter_states.len(),
        connected.manifest.parameters.len()
    );
    assert!(connected
        .parameter_states
        .iter()
        .any(|state| state.persisted_value.is_some()));

    session.close().unwrap();
    server.shutdown().unwrap();
}
