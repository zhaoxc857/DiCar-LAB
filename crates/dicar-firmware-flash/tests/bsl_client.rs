use std::io::{self, Cursor, Read, Write};

use dicar_firmware_flash::bsl::{mspm0_crc32, BslError, CoreStatus, Mspm0RomBsl};

#[derive(Default)]
struct FakeSerial {
    reads: Cursor<Vec<u8>>,
    writes: Vec<u8>,
    fail_read: Option<io::ErrorKind>,
}

impl FakeSerial {
    fn scripted(reads: Vec<u8>) -> Self {
        Self {
            reads: Cursor::new(reads),
            ..Self::default()
        }
    }

    fn failing(kind: io::ErrorKind) -> Self {
        Self {
            fail_read: Some(kind),
            ..Self::default()
        }
    }
}

impl Read for FakeSerial {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if let Some(kind) = self.fail_read {
            return Err(io::Error::from(kind));
        }
        self.reads.read(output)
    }
}

impl Write for FakeSerial {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn response(payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![0x08];
    packet.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    packet.extend_from_slice(payload);
    packet.extend_from_slice(&mspm0_crc32(payload).to_le_bytes());
    packet
}

fn status(status: u8) -> Vec<u8> {
    let mut bytes = vec![0x00];
    bytes.extend_from_slice(&response(&[0x3B, status]));
    bytes
}

fn identity(max_buffer_size: u16) -> Vec<u8> {
    let mut payload = vec![0x31];
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&2u16.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&0x1331u16.to_le_bytes());
    payload.extend_from_slice(&max_buffer_size.to_le_bytes());
    payload.extend_from_slice(&0x2020_0000u32.to_le_bytes());
    payload.extend_from_slice(&0x1111_1111u32.to_le_bytes());
    payload.extend_from_slice(&0x2222_2222u32.to_le_bytes());
    let mut bytes = vec![0x00];
    bytes.extend_from_slice(&response(&payload));
    bytes
}

fn verification(crc: u32) -> Vec<u8> {
    let mut payload = vec![0x32];
    payload.extend_from_slice(&crc.to_le_bytes());
    let mut bytes = vec![0x00];
    bytes.extend_from_slice(&response(&payload));
    bytes
}

fn split_requests(bytes: &[u8]) -> Vec<&[u8]> {
    let mut requests = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let payload_len = usize::from(u16::from_le_bytes([bytes[offset + 1], bytes[offset + 2]]));
        let packet_len = 3 + payload_len + 4;
        requests.push(&bytes[offset..offset + packet_len]);
        offset += packet_len;
    }
    requests
}

#[test]
fn full_update_uses_device_buffer_chunks_padding_and_crc_verification() {
    let image = (0..33).map(|value| value as u8).collect::<Vec<_>>();
    let image_crc = mspm0_crc32(&image);
    let mut reads = vec![0x00];
    reads.extend_from_slice(&identity(21));
    reads.extend_from_slice(&status(0));
    reads.extend_from_slice(&status(0));
    reads.extend_from_slice(&status(0));
    reads.extend_from_slice(&status(0));
    reads.extend_from_slice(&status(0));
    reads.extend_from_slice(&verification(image_crc));
    reads.push(0x00);
    let mut bsl = Mspm0RomBsl::new(FakeSerial::scripted(reads));

    bsl.connect().unwrap();
    let info = bsl.device_info().unwrap();
    assert_eq!(info.max_buffer_size, 21);
    bsl.unlock(&[0xA5; 32]).unwrap();
    bsl.erase_range(0, 0x3FF).unwrap();
    assert_eq!(bsl.program(0, &image).unwrap(), image.len());
    bsl.verify_crc(0, image.len() as u32, image_crc).unwrap();
    bsl.start_application().unwrap();

    let serial = bsl.into_inner();
    let requests = split_requests(&serial.writes);
    let programs = requests
        .iter()
        .filter(|packet| packet[3] == 0x20)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(programs.len(), 3);
    assert_eq!(&programs[0][4..8], &0u32.to_le_bytes());
    assert_eq!(&programs[1][4..8], &16u32.to_le_bytes());
    assert_eq!(&programs[2][4..8], &32u32.to_le_bytes());
    assert_eq!(&programs[0][8..24], &image[..16]);
    assert_eq!(&programs[1][8..24], &image[16..32]);
    assert_eq!(
        &programs[2][8..16],
        &[image[32], 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn timeout_disconnect_bad_password_and_crc_mismatch_are_distinct() {
    let mut timeout = Mspm0RomBsl::new(FakeSerial::failing(io::ErrorKind::TimedOut));
    assert_eq!(timeout.connect(), Err(BslError::Timeout));

    let mut disconnected = Mspm0RomBsl::new(FakeSerial::scripted(Vec::new()));
    assert_eq!(disconnected.connect(), Err(BslError::Disconnected));

    let mut bad_password_reads = vec![0x00];
    bad_password_reads.extend_from_slice(&status(2));
    let mut bad_password = Mspm0RomBsl::new(FakeSerial::scripted(bad_password_reads));
    bad_password.connect().unwrap();
    assert_eq!(
        bad_password.unlock(&[0x44; 32]),
        Err(BslError::Core(CoreStatus::PasswordError))
    );

    let expected = 0x1234_5678;
    let mut mismatch_reads = vec![0x00];
    mismatch_reads.extend_from_slice(&status(0));
    mismatch_reads.extend_from_slice(&verification(0xDEAD_BEEF));
    let mut mismatch = Mspm0RomBsl::new(FakeSerial::scripted(mismatch_reads));
    mismatch.connect().unwrap();
    mismatch.unlock(&[0x55; 32]).unwrap();
    assert_eq!(
        mismatch.verify_crc(0, 1024, expected),
        Err(BslError::VerificationMismatch {
            expected,
            actual: 0xDEAD_BEEF,
        })
    );
}
