mod error;
mod frame;

pub use error::ProtocolError;
pub use frame::{
    Frame, FrameFlags, FrameHeader, MessageType, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION,
};
