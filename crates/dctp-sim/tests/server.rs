use std::net::TcpStream;

use dctp_sim::SimulatorServer;

#[test]
fn spawned_server_reports_an_ephemeral_address_and_releases_it() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = server.local_addr();
    assert_ne!(address.port(), 0);
    TcpStream::connect(address).unwrap();
    server.shutdown().unwrap();
    assert!(TcpStream::connect(address).is_err());
}
