use std::net::{Shutdown, TcpListener};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use dctp_protocol::{
    encode_frame, Frame, FrameFlags, Hello, MessageType, StreamDecoder, WireEncode,
};
use dctp_sim::SimulatorServer;
use dicar_app_core::{TcpTransport, Transport, TransportError};

fn hello_frame(client_nonce: u32) -> Frame {
    let hello = Hello {
        client_nonce,
        min_version: 1,
        max_version: 1,
        max_payload: 1_024,
    };
    Frame::new(
        MessageType::Hello,
        FrameFlags::ACK_REQUIRED,
        1,
        0,
        hello.encode().unwrap(),
    )
    .unwrap()
}

#[test]
fn tcp_transport_reads_and_writes_the_simulator_byte_stream() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut transport = TcpTransport::connect(server.local_addr()).unwrap();
    transport
        .write_all(&encode_frame(&hello_frame(77)).unwrap())
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut decoder = StreamDecoder::new();
    let message_type = loop {
        let mut bytes = [0; 1_100];
        let count = transport.read(&mut bytes).unwrap();
        if let Some(frame) = decoder.push(&bytes[..count]).into_iter().flatten().next() {
            break frame.header.message_type;
        }
        assert!(
            Instant::now() < deadline,
            "simulator did not return a frame"
        );
    };

    assert_eq!(message_type, MessageType::HelloAck);
    transport.close().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn tcp_transport_returns_zero_when_the_peer_sends_no_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        accepted_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(Duration::from_secs(2));
    });

    let mut transport = TcpTransport::connect(address).unwrap();
    accepted_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut bytes = [0; 16];
    assert_eq!(transport.read(&mut bytes).unwrap(), 0);

    transport.close().unwrap();
    release_tx.send(()).unwrap();
    peer.join().unwrap();
}

#[test]
fn tcp_transport_reports_peer_eof_as_disconnected() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (closed_tx, closed_rx) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.shutdown(Shutdown::Both).unwrap();
        drop(stream);
        closed_tx.send(()).unwrap();
    });

    let mut transport = TcpTransport::connect(address).unwrap();
    closed_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        transport.read(&mut [0; 16]),
        Err(TransportError::Disconnected)
    ));

    transport.close().unwrap();
    peer.join().unwrap();
}

#[test]
fn tcp_transport_close_is_idempotent() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut byte = [0];
        let _ = std::io::Read::read(&mut stream, &mut byte);
    });

    let mut transport = TcpTransport::connect(address).unwrap();
    transport.close().unwrap();
    transport.close().unwrap();
    peer.join().unwrap();
}
