use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use dctp_protocol::{encode_frame, StreamDecoder};

use crate::{PriorityTxQueue, PushOutcome, SimConfig, SimDevice};

const CLIENT_REJECTION: &[u8] = b"DCTP simulator rejected connection: only one client is allowed\n";
const CLIENT_POLL_INTERVAL: Duration = Duration::from_millis(1);
const MAX_READS_PER_POLL: usize = 8;
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub struct SimulatorServer {
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<io::Result<()>>>,
}

impl SimulatorServer {
    pub fn spawn(address: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let listener_shutdown = Arc::clone(&shutdown);
        let listener_thread = thread::spawn(move || run_listener(listener, listener_shutdown));

        Ok(Self {
            local_addr,
            shutdown,
            listener_thread: Some(listener_thread),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn shutdown(mut self) -> io::Result<()> {
        self.signal_shutdown();
        self.join_listener()
    }

    pub fn run_forever(mut self) -> io::Result<()> {
        self.join_listener()
    }

    fn signal_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    fn join_listener(&mut self) -> io::Result<()> {
        match self.listener_thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| io::Error::other("simulator listener thread panicked"))?,
            None => Ok(()),
        }
    }
}

impl Drop for SimulatorServer {
    fn drop(&mut self) {
        self.signal_shutdown();
        let _ = self.join_listener();
    }
}

fn run_listener(listener: TcpListener, shutdown: Arc<AtomicBool>) -> io::Result<()> {
    let device = Arc::new(Mutex::new(SimDevice::new(SimConfig::default())));
    let client_active = Arc::new(AtomicBool::new(false));
    let started_at = Instant::now();

    while !shutdown.load(Ordering::Acquire) {
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

    Ok(())
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
    let result = serve_client_loop(stream, device, started_at);
    let reset_result = lock_device(device).map(|mut device| device.disconnect());
    match result {
        Err(error) => Err(error),
        Ok(()) => reset_result,
    }
}

fn serve_client_loop(
    stream: &mut TcpStream,
    device: &Arc<Mutex<SimDevice>>,
    started_at: Instant,
) -> io::Result<()> {
    stream.set_nonblocking(true)?;
    let mut decoder = StreamDecoder::new();
    let mut queue = PriorityTxQueue::default();
    let mut buffer = [0u8; 1_100];

    loop {
        for _ in 0..MAX_READS_PER_POLL {
            match stream.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    for frame in decoder.push(&buffer[..count]).into_iter().flatten() {
                        let responses = lock_device(device)?.handle(frame, elapsed_ms(started_at));
                        enqueue_responses(&mut queue, device, responses)?;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }

        let responses = lock_device(device)?.tick(elapsed_ms(started_at));
        enqueue_responses(&mut queue, device, responses)?;
        drain_queue(stream, &mut queue)?;
        thread::sleep(CLIENT_POLL_INTERVAL);
    }
}

fn enqueue_responses(
    queue: &mut PriorityTxQueue,
    device: &Arc<Mutex<SimDevice>>,
    responses: Vec<crate::QueuedFrame>,
) -> io::Result<()> {
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
    Ok(())
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
        write_all_nonblocking(stream, &bytes)?;
    }
    Ok(())
}

fn write_all_nonblocking(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + WRITE_TIMEOUT;
    let mut written = 0;
    while written < bytes.len() {
        match stream.write(&bytes[written..]) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::Error::from(io::ErrorKind::TimedOut));
                }
                thread::sleep(CLIENT_POLL_INTERVAL);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
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
