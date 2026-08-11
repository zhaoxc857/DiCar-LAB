use std::fmt;
use std::io;

use dctp_protocol::{ErrorCode, MessageType, ParamWriteAck, ProtocolError};

use crate::WorkspaceError;

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

#[derive(Debug)]
pub enum CoreError {
    Transport(TransportError),
    Protocol(ProtocolError),
    Workspace(WorkspaceError),
    UnauthorizedParameterOperation,
    Device {
        original_message_type: MessageType,
        original_sequence: u16,
        code: ErrorCode,
        context: String,
    },
    RevisionConflict {
        current: ParamWriteAck,
    },
    Timeout {
        message_type: MessageType,
        attempts: u8,
    },
    Disconnected,
    ManifestCrcMismatch {
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Protocol(error) => write!(formatter, "protocol error: {error:?}"),
            Self::Workspace(error) => write!(formatter, "workspace error: {error}"),
            Self::UnauthorizedParameterOperation => {
                formatter.write_str("raw parameter write or commit is not authorized")
            }
            Self::Device { code, context, .. } => {
                write!(formatter, "device error {code:?}: {context}")
            }
            Self::RevisionConflict { .. } => formatter.write_str("parameter revision conflict"),
            Self::Timeout {
                message_type,
                attempts,
            } => write!(
                formatter,
                "{message_type:?} timed out after {attempts} attempts"
            ),
            Self::Disconnected => formatter.write_str("protocol session disconnected"),
            Self::ManifestCrcMismatch { expected, actual } => write!(
                formatter,
                "manifest CRC mismatch: expected {expected:#010x}, got {actual:#010x}"
            ),
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Workspace(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TransportError> for CoreError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

impl From<ProtocolError> for CoreError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<WorkspaceError> for CoreError {
    fn from(error: WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}
