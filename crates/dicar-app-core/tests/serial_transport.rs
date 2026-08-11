use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
    time::Duration,
};

use dctp_sim::SimulatorServer;
use dicar_app_core::{
    CoreCommand, Endpoint, ProtocolSession, SerialTransport, SystemClock, SystemNonce, Transport,
    TransportError,
};

#[derive(Clone, Default)]
struct Probe(Arc<Mutex<Vec<u8>>>);

enum ReadAction {
    Timeout,
    Bytes(Vec<u8>),
}

struct ScriptedPort {
    reads: VecDeque<ReadAction>,
    written: Probe,
}

impl Read for ScriptedPort {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        match self.reads.pop_front().unwrap_or(ReadAction::Timeout) {
            ReadAction::Timeout => Err(io::Error::new(io::ErrorKind::TimedOut, "idle")),
            ReadAction::Bytes(bytes) => {
                let count = output.len().min(bytes.len());
                output[..count].copy_from_slice(&bytes[..count]);
                Ok(count)
            }
        }
    }
}

impl Write for ScriptedPort {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.written.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn serial_transport_has_typed_identity_bounded_idle_reads_and_idempotent_close() {
    let probe = Probe::default();
    let scripted = ScriptedPort {
        reads: VecDeque::from([
            ReadAction::Timeout,
            ReadAction::Bytes(vec![0x11, 0x22, 0x00]),
        ]),
        written: probe.clone(),
    };
    let mut transport = SerialTransport::from_io("COM7", 921_600, scripted).unwrap();

    assert_eq!(
        transport.identity().endpoint,
        Endpoint::Serial {
            port_name: "COM7".to_owned(),
            baud_rate: 921_600,
        }
    );
    let mut output = [0_u8; 8];
    assert_eq!(transport.read(&mut output).unwrap(), 0);
    assert_eq!(transport.read(&mut output).unwrap(), 3);
    assert_eq!(&output[..3], &[0x11, 0x22, 0x00]);
    transport.write_all(&[0xaa, 0xbb]).unwrap();
    assert_eq!(*probe.0.lock().unwrap(), vec![0xaa, 0xbb]);

    transport.close().unwrap();
    transport.close().unwrap();
    assert!(matches!(
        transport.read(&mut output),
        Err(TransportError::Disconnected)
    ));
}

#[test]
fn serial_transport_rejects_empty_ports_and_unapproved_baud_rates_before_io() {
    for (port_name, baud_rate) in [("", 921_600), ("COM7", 230_400)] {
        let result = SerialTransport::from_io(
            port_name,
            baud_rate,
            ScriptedPort {
                reads: VecDeque::new(),
                written: Probe::default(),
            },
        );
        assert!(result.is_err());
    }

    for baud_rate in [115_200, 460_800, 921_600] {
        SerialTransport::from_io(
            "COM7",
            baud_rate,
            ScriptedPort {
                reads: VecDeque::new(),
                written: Probe::default(),
            },
        )
        .unwrap();
    }
}

#[test]
fn actor_command_accepts_a_runtime_serial_endpoint() {
    let endpoint = Endpoint::Serial {
        port_name: "COM7".to_owned(),
        baud_rate: 921_600,
    };
    assert!(matches!(
        CoreCommand::ConnectTo {
            endpoint: endpoint.clone()
        },
        CoreCommand::ConnectTo { endpoint: actual } if actual == endpoint
    ));
}

#[test]
fn dctp_handshake_manifest_and_parameters_load_over_the_serial_byte_stream() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let stream = TcpStream::connect(server.local_addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let transport = SerialTransport::from_io("COM7", 921_600, stream).unwrap();
    let mut session = ProtocolSession::new(transport, SystemNonce::default(), SystemClock::new());

    let connected = session.connect_and_load().unwrap();

    assert!(!connected.manifest.parameters.is_empty());
    assert_eq!(
        connected.parameter_states.len(),
        connected.manifest.parameters.len()
    );
    session.close().unwrap();
    server.shutdown().unwrap();
}
