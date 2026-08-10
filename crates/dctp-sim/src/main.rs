use std::env;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dctp_protocol::{encode_frame, StreamDecoder};
use dctp_sim::{PriorityTxQueue, PushOutcome, SimConfig, SimDevice};

const DEFAULT_LISTEN_ADDRESS: &str = "127.0.0.1:7100";
const CLIENT_REJECTION: &[u8] = b"DCTP simulator rejected connection: only one client is allowed\n";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dctp-sim: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let address = parse_listen_address(env::args().skip(1))?;
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    let device = Arc::new(Mutex::new(SimDevice::new(SimConfig::default())));
    let client_active = Arc::new(AtomicBool::new(false));
    let started_at = Instant::now();

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if client_active
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    let _ = stream.write_all(CLIENT_REJECTION);
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }

                let device = Arc::clone(&device);
                let client_active = Arc::clone(&client_active);
                thread::spawn(move || {
                    let _slot = ClientSlot(client_active);
                    if let Err(error) = serve_client(&mut stream, &device, started_at) {
                        eprintln!("dctp-sim client: {error}");
                    }
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
}

struct ClientSlot(Arc<AtomicBool>);

impl Drop for ClientSlot {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn serve_client(
    stream: &mut TcpStream,
    device: &Arc<Mutex<SimDevice>>,
    started_at: Instant,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut decoder = StreamDecoder::new();
    let mut queue = PriorityTxQueue::default();
    let mut buffer = [0u8; 1_100];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                for frame in decoder.push(&buffer[..count]).into_iter().flatten() {
                    let responses = lock_device(device)?.handle(frame, elapsed_ms(started_at));
                    for response in responses {
                        let sample_count = telemetry_sample_count(&response.frame);
                        match queue.push(response.priority, response.frame) {
                            PushOutcome::Backpressure => {
                                return Err(io::Error::other("P0/P1 transmit queue backpressure"));
                            }
                            PushOutcome::DroppedTelemetry => {
                                lock_device(device)?.note_telemetry_drop(sample_count);
                            }
                            PushOutcome::Enqueued | PushOutcome::DroppedLog => {}
                        }
                    }
                }
                drain_queue(stream, &mut queue)?;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                let responses = lock_device(device)?.tick(elapsed_ms(started_at));
                for response in responses {
                    let sample_count = telemetry_sample_count(&response.frame);
                    match queue.push(response.priority, response.frame) {
                        PushOutcome::Backpressure => {
                            return Err(io::Error::other("P0/P1 transmit queue backpressure"));
                        }
                        PushOutcome::DroppedTelemetry => {
                            lock_device(device)?.note_telemetry_drop(sample_count);
                        }
                        PushOutcome::Enqueued | PushOutcome::DroppedLog => {}
                    }
                }
                drain_queue(stream, &mut queue)?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn lock_device(device: &Arc<Mutex<SimDevice>>) -> io::Result<std::sync::MutexGuard<'_, SimDevice>> {
    device
        .lock()
        .map_err(|_| io::Error::other("simulator device lock poisoned"))
}

fn drain_queue(stream: &mut TcpStream, queue: &mut PriorityTxQueue) -> io::Result<()> {
    while let Some(frame) = queue.pop() {
        let bytes = encode_frame(&frame)
            .map_err(|error| io::Error::other(format!("frame encode failed: {error:?}")))?;
        stream.write_all(&bytes)?;
    }
    Ok(())
}

fn telemetry_sample_count(frame: &dctp_protocol::Frame) -> u16 {
    if frame.header.message_type == dctp_protocol::MessageType::TelemetryData {
        u16::from(frame.payload.get(4).copied().unwrap_or(0))
    } else {
        0
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn parse_listen_address(args: impl Iterator<Item = String>) -> io::Result<SocketAddr> {
    let mut address = DEFAULT_LISTEN_ADDRESS.to_owned();
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--listen" => {
                address = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--listen requires an address")
                })?;
            }
            "--help" | "-h" => {
                println!("Usage: dctp-sim [--listen ADDRESS]");
                std::process::exit(0);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {argument}"),
                ));
            }
        }
    }
    address.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid listen address {address:?}: {error}"),
        )
    })
}
