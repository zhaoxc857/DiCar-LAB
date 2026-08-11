use std::fmt;
use std::io;

#[derive(Debug)]
pub enum TransportError {
    Disconnected,
    Io(io::Error),
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str("transport disconnected"),
            Self::Io(error) => write!(formatter, "transport I/O error: {error}"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Disconnected => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
