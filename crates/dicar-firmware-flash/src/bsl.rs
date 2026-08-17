use std::io::{self, Read, Write};

use zeroize::Zeroize;

const REQUEST_HEADER: u8 = 0x80;
const RESPONSE_HEADER: u8 = 0x08;
const MAX_CORE_RESPONSE_PAYLOAD: usize = 1_024;
const MAX_PROGRAM_DATA: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BslCommand<'a> {
    Connection,
    GetIdentity,
    Unlock(&'a [u8; 32]),
    EraseRange { start: u32, end: u32 },
    ProgramData { address: u32, data: &'a [u8] },
    VerifyCrc { address: u32, length: u32 },
    StartApplication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportAckError {
    HeaderIncorrect,
    ChecksumIncorrect,
    PacketSizeZero,
    PacketSizeTooLarge,
    UnknownError,
    UnknownBaudRate,
    PacketSize,
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreStatus {
    Success,
    Locked,
    PasswordError,
    MultiplePasswordError,
    UnknownCommand,
    InvalidMemoryRange,
    InvalidCommand,
    FactoryResetDisabled,
    FactoryResetPasswordError,
    ReadoutDisabled,
    InvalidAddressLengthAlignment,
    VerificationInvalidLength,
    FlashProgramFailed,
    MassEraseFailed,
    FlashEraseFailed,
    FactoryResetFailed,
    Unknown(u8),
}

impl CoreStatus {
    fn from_byte(value: u8) -> Self {
        match value {
            0x00 => Self::Success,
            0x01 => Self::Locked,
            0x02 => Self::PasswordError,
            0x03 => Self::MultiplePasswordError,
            0x04 => Self::UnknownCommand,
            0x05 => Self::InvalidMemoryRange,
            0x06 => Self::InvalidCommand,
            0x07 => Self::FactoryResetDisabled,
            0x08 => Self::FactoryResetPasswordError,
            0x09 => Self::ReadoutDisabled,
            0x0A => Self::InvalidAddressLengthAlignment,
            0x0B => Self::VerificationInvalidLength,
            0xF1 => Self::FlashProgramFailed,
            0xF2 => Self::MassEraseFailed,
            0xF3 => Self::FlashEraseFailed,
            0xF4 => Self::FactoryResetFailed,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceInfo {
    pub command_interpreter_version: u16,
    pub build_id: u16,
    pub application_revision: u32,
    pub plugin_version: u16,
    pub max_buffer_size: u16,
    pub buffer_start_address: u32,
    pub bcr_config_id: u32,
    pub bsl_config_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreResponse {
    Status(CoreStatus),
    Identity(DeviceInfo),
    VerificationCrc(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BslError {
    PacketTooLarge,
    Transport(TransportAckError),
    ResponseHeader,
    ResponseLength,
    ResponseCrc,
    UnknownResponse,
    Timeout,
    Disconnected,
    Io(io::ErrorKind),
    InvalidState,
    InvalidAlignment,
    InvalidBufferSize,
    AddressOverflow,
    Core(CoreStatus),
    VerificationMismatch { expected: u32, actual: u32 },
}

impl std::fmt::Display for BslError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BslError {}

pub fn mspm0_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc
}

pub fn encode_command(command: &BslCommand<'_>) -> Result<Vec<u8>, BslError> {
    let mut payload = Vec::new();
    match command {
        BslCommand::Connection => payload.push(0x12),
        BslCommand::GetIdentity => payload.push(0x19),
        BslCommand::Unlock(password) => {
            payload.push(0x21);
            payload.extend_from_slice(*password);
        }
        BslCommand::EraseRange { start, end } => {
            payload.push(0x23);
            payload.extend_from_slice(&start.to_le_bytes());
            payload.extend_from_slice(&end.to_le_bytes());
        }
        BslCommand::ProgramData { address, data } => {
            payload.push(0x20);
            payload.extend_from_slice(&address.to_le_bytes());
            payload.extend_from_slice(data);
        }
        BslCommand::VerifyCrc { address, length } => {
            payload.push(0x26);
            payload.extend_from_slice(&address.to_le_bytes());
            payload.extend_from_slice(&length.to_le_bytes());
        }
        BslCommand::StartApplication => payload.push(0x40),
    }
    let payload_len = u16::try_from(payload.len()).map_err(|_| BslError::PacketTooLarge)?;
    let mut packet = Vec::with_capacity(3 + payload.len() + 4);
    packet.push(REQUEST_HEADER);
    packet.extend_from_slice(&payload_len.to_le_bytes());
    packet.extend_from_slice(&payload);
    packet.extend_from_slice(&mspm0_crc32(&payload).to_le_bytes());
    Ok(packet)
}

pub fn parse_transport_ack(byte: u8) -> Result<(), BslError> {
    let error = match byte {
        0x00 => return Ok(()),
        0x51 => TransportAckError::HeaderIncorrect,
        0x52 => TransportAckError::ChecksumIncorrect,
        0x53 => TransportAckError::PacketSizeZero,
        0x54 => TransportAckError::PacketSizeTooLarge,
        0x55 => TransportAckError::UnknownError,
        0x56 => TransportAckError::UnknownBaudRate,
        0x57 => TransportAckError::PacketSize,
        value => TransportAckError::Unknown(value),
    };
    Err(BslError::Transport(error))
}

pub fn decode_core_response(packet: &[u8]) -> Result<CoreResponse, BslError> {
    if packet.len() < 8 {
        return Err(BslError::ResponseLength);
    }
    if packet[0] != RESPONSE_HEADER {
        return Err(BslError::ResponseHeader);
    }
    let payload_len = usize::from(u16::from_le_bytes([packet[1], packet[2]]));
    let expected_len = 3usize
        .checked_add(payload_len)
        .and_then(|len| len.checked_add(4))
        .ok_or(BslError::ResponseLength)?;
    if packet.len() != expected_len {
        return Err(BslError::ResponseLength);
    }
    let payload = &packet[3..3 + payload_len];
    let expected_crc = u32::from_le_bytes(
        packet[3 + payload_len..]
            .try_into()
            .map_err(|_| BslError::ResponseLength)?,
    );
    if mspm0_crc32(payload) != expected_crc {
        return Err(BslError::ResponseCrc);
    }
    match payload {
        [0x3B, status] => Ok(CoreResponse::Status(CoreStatus::from_byte(*status))),
        [0x32, crc @ ..] if crc.len() == 4 => Ok(CoreResponse::VerificationCrc(
            u32::from_le_bytes(crc.try_into().map_err(|_| BslError::ResponseLength)?),
        )),
        [0x31, identity @ ..] if identity.len() == 24 => Ok(CoreResponse::Identity(DeviceInfo {
            command_interpreter_version: read_u16(identity, 0)?,
            build_id: read_u16(identity, 2)?,
            application_revision: read_u32(identity, 4)?,
            plugin_version: read_u16(identity, 8)?,
            max_buffer_size: read_u16(identity, 10)?,
            buffer_start_address: read_u32(identity, 12)?,
            bcr_config_id: read_u32(identity, 16)?,
            bsl_config_id: read_u32(identity, 20)?,
        })),
        _ => Err(BslError::UnknownResponse),
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, BslError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(BslError::ResponseLength)?
            .try_into()
            .map_err(|_| BslError::ResponseLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, BslError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(BslError::ResponseLength)?
            .try_into()
            .map_err(|_| BslError::ResponseLength)?,
    ))
}

pub struct Mspm0RomBsl<T: Read + Write> {
    transport: T,
    connected: bool,
    unlocked: bool,
    device_info: Option<DeviceInfo>,
}

impl<T: Read + Write> Mspm0RomBsl<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            connected: false,
            unlocked: false,
            device_info: None,
        }
    }

    pub fn into_inner(self) -> T {
        self.transport
    }

    pub fn connect(&mut self) -> Result<(), BslError> {
        self.write_and_ack(&BslCommand::Connection)?;
        self.connected = true;
        self.unlocked = false;
        self.device_info = None;
        Ok(())
    }

    pub fn device_info(&mut self) -> Result<DeviceInfo, BslError> {
        self.require_connected()?;
        self.write_and_ack(&BslCommand::GetIdentity)?;
        let CoreResponse::Identity(info) = self.read_core_response()? else {
            return Err(BslError::UnknownResponse);
        };
        if info.max_buffer_size <= 5 {
            return Err(BslError::InvalidBufferSize);
        }
        self.device_info = Some(info);
        Ok(info)
    }

    pub fn unlock(&mut self, password: &[u8; 32]) -> Result<(), BslError> {
        self.require_connected()?;
        self.write_and_ack(&BslCommand::Unlock(password))?;
        self.expect_success_status()?;
        self.unlocked = true;
        Ok(())
    }

    pub fn erase_range(&mut self, start: u32, end: u32) -> Result<(), BslError> {
        self.require_unlocked()?;
        if start > end {
            return Err(BslError::InvalidAlignment);
        }
        self.write_and_ack(&BslCommand::EraseRange { start, end })?;
        self.expect_success_status()
    }

    pub fn program(&mut self, address: u32, image: &[u8]) -> Result<usize, BslError> {
        self.require_unlocked()?;
        if address % 8 != 0 {
            return Err(BslError::InvalidAlignment);
        }
        let info = self.device_info.ok_or(BslError::InvalidState)?;
        let available = usize::from(info.max_buffer_size).saturating_sub(5);
        let chunk_size = available.min(MAX_PROGRAM_DATA) / 8 * 8;
        if chunk_size < 8 {
            return Err(BslError::InvalidBufferSize);
        }
        let mut offset = 0usize;
        while offset < image.len() {
            let remaining = image.len() - offset;
            let actual_len = remaining.min(chunk_size);
            let padded_len = actual_len.div_ceil(8) * 8;
            let mut padded = vec![0xFF; padded_len];
            padded[..actual_len].copy_from_slice(&image[offset..offset + actual_len]);
            let chunk_address = address
                .checked_add(u32::try_from(offset).map_err(|_| BslError::AddressOverflow)?)
                .ok_or(BslError::AddressOverflow)?;
            let result = self.write_and_ack(&BslCommand::ProgramData {
                address: chunk_address,
                data: &padded,
            });
            padded.zeroize();
            result?;
            self.expect_success_status()?;
            offset += actual_len;
        }
        Ok(image.len())
    }

    pub fn verify_crc(&mut self, address: u32, length: u32, expected: u32) -> Result<(), BslError> {
        self.require_unlocked()?;
        self.write_and_ack(&BslCommand::VerifyCrc { address, length })?;
        match self.read_core_response()? {
            CoreResponse::VerificationCrc(actual) if actual == expected => Ok(()),
            CoreResponse::VerificationCrc(actual) => {
                Err(BslError::VerificationMismatch { expected, actual })
            }
            CoreResponse::Status(status) if status != CoreStatus::Success => {
                Err(BslError::Core(status))
            }
            _ => Err(BslError::UnknownResponse),
        }
    }

    pub fn start_application(&mut self) -> Result<(), BslError> {
        self.require_connected()?;
        self.write_and_ack(&BslCommand::StartApplication)?;
        self.connected = false;
        self.unlocked = false;
        Ok(())
    }

    fn write_and_ack(&mut self, command: &BslCommand<'_>) -> Result<(), BslError> {
        let mut packet = encode_command(command)?;
        let write_result = self
            .transport
            .write_all(&packet)
            .and_then(|()| self.transport.flush());
        packet.zeroize();
        write_result.map_err(map_io_error)?;
        let mut ack = [0u8; 1];
        self.transport.read_exact(&mut ack).map_err(map_io_error)?;
        parse_transport_ack(ack[0])
    }

    fn read_core_response(&mut self) -> Result<CoreResponse, BslError> {
        let mut header = [0u8; 3];
        self.transport
            .read_exact(&mut header)
            .map_err(map_io_error)?;
        let payload_len = usize::from(u16::from_le_bytes([header[1], header[2]]));
        if payload_len == 0 || payload_len > MAX_CORE_RESPONSE_PAYLOAD {
            return Err(BslError::ResponseLength);
        }
        let mut packet = Vec::with_capacity(3 + payload_len + 4);
        packet.extend_from_slice(&header);
        packet.resize(3 + payload_len + 4, 0);
        self.transport
            .read_exact(&mut packet[3..])
            .map_err(map_io_error)?;
        decode_core_response(&packet)
    }

    fn expect_success_status(&mut self) -> Result<(), BslError> {
        match self.read_core_response()? {
            CoreResponse::Status(CoreStatus::Success) => Ok(()),
            CoreResponse::Status(status) => Err(BslError::Core(status)),
            _ => Err(BslError::UnknownResponse),
        }
    }

    fn require_connected(&self) -> Result<(), BslError> {
        self.connected.then_some(()).ok_or(BslError::InvalidState)
    }

    fn require_unlocked(&self) -> Result<(), BslError> {
        (self.connected && self.unlocked)
            .then_some(())
            .ok_or(BslError::InvalidState)
    }
}

fn map_io_error(error: io::Error) -> BslError {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => BslError::Timeout,
        io::ErrorKind::UnexpectedEof
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::NotConnected => BslError::Disconnected,
        kind => BslError::Io(kind),
    }
}
