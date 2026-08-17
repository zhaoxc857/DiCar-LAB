use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::package::{PackageError, TrustStore};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvisioningError {
    NotFound,
    InvalidRecord,
    Io,
    Package(PackageError),
}

impl std::fmt::Display for ProvisioningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "device provisioning record was not found",
            Self::InvalidRecord => "device provisioning record is invalid",
            Self::Io => "device provisioning record storage failed",
            Self::Package(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for ProvisioningError {}

impl From<PackageError> for ProvisioningError {
    fn from(error: PackageError) -> Self {
        Self::Package(error)
    }
}

#[derive(Clone, Debug)]
pub struct ProvisioningStore {
    directory: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProvisioningRecord {
    schema_version: u16,
    device_id: String,
    signing_key_id: String,
    public_key: String,
}

impl ProvisioningStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn save(
        &self,
        device_id: &[u8; 16],
        signing_key_id: &str,
        public_key: [u8; 32],
    ) -> Result<(), ProvisioningError> {
        TrustStore::from_keys([(signing_key_id.to_owned(), public_key)])?;
        let record = WireProvisioningRecord {
            schema_version: 1,
            device_id: encode_hex(device_id),
            signing_key_id: signing_key_id.to_owned(),
            public_key: encode_hex(&public_key),
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| ProvisioningError::InvalidRecord)?;
        fs::create_dir_all(&self.directory).map_err(|_| ProvisioningError::Io)?;
        let final_path = self.record_path(device_id);
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_path = self.directory.join(format!(
            ".{}.{}.{}.tmp",
            encode_hex(device_id),
            std::process::id(),
            counter
        ));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary_path)
                .map_err(|_| ProvisioningError::Io)?;
            file.write_all(&bytes).map_err(|_| ProvisioningError::Io)?;
            file.sync_all().map_err(|_| ProvisioningError::Io)?;
            atomic_replace(&temporary_path, &final_path)
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        write_result
    }

    pub fn load_trust_store(&self, device_id: &[u8; 16]) -> Result<TrustStore, ProvisioningError> {
        let bytes = fs::read(self.record_path(device_id)).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ProvisioningError::NotFound
            } else {
                ProvisioningError::Io
            }
        })?;
        if bytes.len() > 1_024 {
            return Err(ProvisioningError::InvalidRecord);
        }
        let record: WireProvisioningRecord =
            serde_json::from_slice(&bytes).map_err(|_| ProvisioningError::InvalidRecord)?;
        if record.schema_version != 1 || record.device_id != encode_hex(device_id) {
            return Err(ProvisioningError::InvalidRecord);
        }
        let public_key =
            decode_hex_32(&record.public_key).ok_or(ProvisioningError::InvalidRecord)?;
        Ok(TrustStore::from_keys([(
            record.signing_key_id,
            public_key,
        )])?)
    }

    fn record_path(&self, device_id: &[u8; 16]) -> PathBuf {
        self.directory
            .join(format!("{}.json", encode_hex(device_id)))
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0F)] as char);
    }
    output
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = hex_nibble(pair[0])?.checked_mul(16)? + hex_nibble(pair[1])?;
    }
    Some(output)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> Result<(), ProvisioningError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both path buffers are NUL-terminated and live for the call.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(ProvisioningError::Io);
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> Result<(), ProvisioningError> {
    fs::rename(source, destination).map_err(|_| ProvisioningError::Io)
}
