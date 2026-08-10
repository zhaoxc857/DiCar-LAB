#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    PayloadTooLarge(usize),
    UnknownMessageType(u8),
    InvalidMagic,
    UnsupportedVersion,
    InvalidLength,
    Truncated,
    CrcMismatch,
    CobsMalformed,
    PacketTooLong,
    InvalidUtf8,
    StringTooLong,
    InvalidValue,
    InvalidSession,
    RevisionConflict,
}
