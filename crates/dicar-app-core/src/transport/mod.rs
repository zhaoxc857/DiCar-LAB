mod serial;
mod tcp;

use std::net::SocketAddr;

use serde::Serialize;

use crate::{SerialHardwareProfile, TransportError};

pub use serial::{available_serial_ports, SerialPortDescriptor, SerialPortKind, SerialTransport};
pub use tcp::TcpTransport;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Endpoint {
    Simulator {
        address: SocketAddr,
    },
    Serial {
        port_name: String,
        baud_rate: u32,
        hardware_profile: SerialHardwareProfile,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportIdentity {
    pub endpoint: Endpoint,
}

pub trait Transport: Send {
    fn identity(&self) -> TransportIdentity;
    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError>;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    fn close(&mut self) -> Result<(), TransportError>;
}

pub enum ActiveTransport {
    Simulator(TcpTransport),
    Serial(SerialTransport),
}

impl ActiveTransport {
    pub fn connect(endpoint: &Endpoint) -> Result<Self, TransportError> {
        match endpoint {
            Endpoint::Simulator { address } => TcpTransport::connect(*address).map(Self::Simulator),
            Endpoint::Serial {
                port_name,
                baud_rate,
                hardware_profile,
            } => SerialTransport::open(port_name, *baud_rate, *hardware_profile).map(Self::Serial),
        }
    }
}

impl Transport for ActiveTransport {
    fn identity(&self) -> TransportIdentity {
        match self {
            Self::Simulator(transport) => transport.identity(),
            Self::Serial(transport) => transport.identity(),
        }
    }

    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError> {
        match self {
            Self::Simulator(transport) => transport.read(output),
            Self::Serial(transport) => transport.read(output),
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        match self {
            Self::Simulator(transport) => transport.write_all(bytes),
            Self::Serial(transport) => transport.write_all(bytes),
        }
    }

    fn close(&mut self) -> Result<(), TransportError> {
        match self {
            Self::Simulator(transport) => transport.close(),
            Self::Serial(transport) => transport.close(),
        }
    }
}
