use std::io::{self, Read, Write};
use std::time::Duration;

use serde::Serialize;

use crate::transport::{Endpoint, Transport, TransportIdentity};
use crate::TransportError;

const READ_TIMEOUT: Duration = Duration::from_millis(10);
const ALLOWED_BAUD_RATES: [u32; 3] = [115_200, 460_800, 921_600];

trait SerialIo: Read + Write + Send {}
impl<T: Read + Write + Send> SerialIo for T {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialPortDescriptor {
    pub port_name: String,
    pub display_name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

pub struct SerialTransport {
    port: Option<Box<dyn SerialIo>>,
    identity: TransportIdentity,
}

impl SerialTransport {
    pub fn open(port_name: &str, baud_rate: u32) -> Result<Self, TransportError> {
        validate(port_name, baud_rate)?;
        let port = serialport::new(port_name, baud_rate)
            .timeout(READ_TIMEOUT)
            .open()
            .map_err(serial_error)?;
        Self::from_io(port_name, baud_rate, SerialPortIo(port))
    }

    #[doc(hidden)]
    pub fn from_io<T>(port_name: &str, baud_rate: u32, port: T) -> Result<Self, TransportError>
    where
        T: Read + Write + Send + 'static,
    {
        validate(port_name, baud_rate)?;
        Ok(Self {
            port: Some(Box::new(port)),
            identity: TransportIdentity {
                endpoint: Endpoint::Serial {
                    port_name: port_name.to_owned(),
                    baud_rate,
                },
            },
        })
    }

    fn port(&mut self) -> Result<&mut (dyn SerialIo + 'static), TransportError> {
        self.port.as_deref_mut().ok_or(TransportError::Disconnected)
    }
}

impl Transport for SerialTransport {
    fn identity(&self) -> TransportIdentity {
        self.identity.clone()
    }

    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            match self.port()?.read(output) {
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
        self.port()?.write_all(bytes).map_err(|error| {
            if is_disconnect(&error) {
                TransportError::Disconnected
            } else {
                error.into()
            }
        })
    }

    fn close(&mut self) -> Result<(), TransportError> {
        self.port.take();
        Ok(())
    }
}

impl Drop for SerialTransport {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

pub fn available_serial_ports() -> Result<Vec<SerialPortDescriptor>, TransportError> {
    let mut ports = serialport::available_ports()
        .map_err(serial_error)?
        .into_iter()
        .map(|port| {
            let (display_name, vendor_id, product_id) = match port.port_type {
                serialport::SerialPortType::UsbPort(info) => (
                    info.product
                        .or(info.manufacturer)
                        .unwrap_or_else(|| "USB 串口".to_owned()),
                    Some(info.vid),
                    Some(info.pid),
                ),
                serialport::SerialPortType::BluetoothPort => {
                    ("Bluetooth 串口".to_owned(), None, None)
                }
                serialport::SerialPortType::PciPort => ("PCI 串口".to_owned(), None, None),
                serialport::SerialPortType::Unknown => ("串口设备".to_owned(), None, None),
            };
            SerialPortDescriptor {
                port_name: port.port_name,
                display_name,
                vendor_id,
                product_id,
            }
        })
        .collect::<Vec<_>>();
    ports.sort_by(|left, right| left.port_name.cmp(&right.port_name));
    Ok(ports)
}

struct SerialPortIo(Box<dyn serialport::SerialPort>);

impl Read for SerialPortIo {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.0.read(output)
    }
}

impl Write for SerialPortIo {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

fn validate(port_name: &str, baud_rate: u32) -> Result<(), TransportError> {
    if port_name.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "串口名称不能为空").into());
    }
    if !ALLOWED_BAUD_RATES.contains(&baud_rate) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "波特率必须是 115200、460800 或 921600",
        )
        .into());
    }
    Ok(())
}

fn serial_error(error: serialport::Error) -> TransportError {
    io::Error::other(error.to_string()).into()
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
