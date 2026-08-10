use crate::ProtocolError;

pub trait WireEncode {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError>;
}

pub trait WireDecode: Sized {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError>;
}

#[derive(Default)]
pub struct WireWriter {
    bytes: Vec<u8>,
}

impl WireWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn put_u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn put_f32(&mut self, value: f32) {
        self.put_u32(value.to_bits());
    }

    pub fn put_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub fn put_utf8_u8_len(
        &mut self,
        value: &str,
        field_limit: usize,
    ) -> Result<(), ProtocolError> {
        let len = value.len();
        if len > field_limit || len > u8::MAX as usize {
            return Err(ProtocolError::StringTooLong);
        }

        self.put_u8(len as u8);
        self.put_bytes(value.as_bytes());
        Ok(())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

pub struct WireReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> WireReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ProtocolError::Truncated)?;
        if end > self.bytes.len() {
            return Err(ProtocolError::Truncated);
        }

        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    pub fn read_u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    pub fn read_u16(&mut self) -> Result<u16, ProtocolError> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub fn read_u32(&mut self) -> Result<u32, ProtocolError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| ProtocolError::Truncated)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_f32(&mut self) -> Result<f32, ProtocolError> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    pub fn read_exact(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
        self.take(len)
    }

    pub fn read_utf8_u8_len(&mut self, field_limit: usize) -> Result<String, ProtocolError> {
        let len = self.read_u8()? as usize;
        if len > field_limit {
            return Err(ProtocolError::StringTooLong);
        }

        let bytes = self.read_exact(len)?;
        let value = core::str::from_utf8(bytes).map_err(|_| ProtocolError::InvalidUtf8)?;
        Ok(value.to_owned())
    }

    pub fn finish(self) -> Result<(), ProtocolError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProtocolError::InvalidLength)
        }
    }
}
