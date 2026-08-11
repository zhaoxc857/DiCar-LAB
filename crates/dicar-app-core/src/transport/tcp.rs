use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

use crate::transport::{Endpoint, Transport, TransportIdentity};
use crate::TransportError;

const READ_TIMEOUT: Duration = Duration::from_millis(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub struct TcpTransport {
    stream: Option<TcpStream>,
    identity: TransportIdentity,
}

impl TcpTransport {
    pub fn connect(address: SocketAddr) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream: Some(stream),
            identity: TransportIdentity {
                endpoint: Endpoint::Simulator { address },
            },
        })
    }

    fn stream(&mut self) -> Result<&mut TcpStream, TransportError> {
        self.stream.as_mut().ok_or(TransportError::Disconnected)
    }
}

impl Transport for TcpTransport {
    fn identity(&self) -> TransportIdentity {
        self.identity.clone()
    }

    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            match self.stream()?.read(output) {
                Ok(0) => return Err(TransportError::Disconnected),
                Ok(count) => return Ok(count),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(0);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if is_disconnect(&error) => {
                    return Err(TransportError::Disconnected);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        match self.stream()?.write_all(bytes) {
            Ok(()) => Ok(()),
            Err(error) if is_disconnect(&error) => Err(TransportError::Disconnected),
            Err(error) => Err(error.into()),
        }
    }

    fn close(&mut self) -> Result<(), TransportError> {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
        Ok(())
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        let _ = self.close();
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
