use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use dctp_protocol::{
    encode_frame, Frame, FrameFlags, Hello, MessageType, StreamDecoder, WireEncode,
};
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

#[test]
fn shutdown_disconnects_an_active_client_before_returning() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut client = TcpStream::connect(server.local_addr()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    let hello = Hello {
        client_nonce: 77,
        min_version: 1,
        max_version: 1,
        max_payload: 1_024,
    };
    let frame = Frame::new(
        MessageType::Hello,
        FrameFlags::ACK_REQUIRED,
        1,
        0,
        hello.encode().unwrap(),
    )
    .unwrap();
    client.write_all(&encode_frame(&frame).unwrap()).unwrap();
    let mut bytes = [0; 1_100];
    let mut decoder = StreamDecoder::new();
    let deadline = Instant::now() + Duration::from_secs(1);
    let response = loop {
        let count = client.read(&mut bytes).unwrap();
        assert_ne!(count, 0, "simulator closed before HELLO_ACK");
        if let Some(response) = decoder.push(&bytes[..count]).into_iter().flatten().next() {
            break response;
        }
        assert!(
            Instant::now() < deadline,
            "simulator did not return HELLO_ACK"
        );
    };
    assert_eq!(response.header.message_type, MessageType::HelloAck);

    server.shutdown().unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    match client.read(&mut bytes) {
        Ok(0) => {}
        Err(error) if is_disconnect(&error) => {}
        result => panic!("active client remained connected after shutdown: {result:?}"),
    }
}

fn is_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotConnected
            | io::ErrorKind::UnexpectedEof
    )
}
