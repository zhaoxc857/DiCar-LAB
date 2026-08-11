mod tcp;

use std::net::SocketAddr;

use serde::Serialize;

use crate::TransportError;

pub use tcp::TcpTransport;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Endpoint {
    Simulator { address: SocketAddr },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TransportIdentity {
    pub endpoint: Endpoint,
}

pub trait Transport: Send {
    fn identity(&self) -> TransportIdentity;
    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError>;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    fn close(&mut self) -> Result<(), TransportError>;
}
