use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::package::{FirmwarePackage, PackageError, TrustStore, VerifiedFirmwarePackage};

const MAX_PACKAGE_LEN: u64 = 18 + 8_192 + 131_072 + 64;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    NotFound,
    TooLarge,
    Io,
    Package(PackageError),
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("device recovery package was not found"),
            Self::TooLarge => formatter.write_str("device recovery package exceeds its size limit"),
            Self::Io => formatter.write_str("device recovery package storage failed"),
            Self::Package(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for RecoveryError {}

impl From<PackageError> for RecoveryError {
    fn from(error: PackageError) -> Self {
        Self::Package(error)
    }
}

#[derive(Clone, Debug)]
pub struct RecoveryStore {
    directory: PathBuf,
}

impl RecoveryStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn package_path(&self, device_id: &[u8; 16]) -> PathBuf {
        self.directory
            .join(format!("{}.dicarfw", device_hex(device_id)))
    }

    pub fn save(
        &self,
        device_id: &[u8; 16],
        package: &[u8],
        trust_store: &TrustStore,
    ) -> Result<(), RecoveryError> {
        FirmwarePackage::inspect(package, trust_store)?;
        if package.len() as u64 > MAX_PACKAGE_LEN {
            return Err(RecoveryError::TooLarge);
        }
        fs::create_dir_all(&self.directory).map_err(|_| RecoveryError::Io)?;
        let final_path = self.package_path(device_id);
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_path = self.directory.join(format!(
            ".{}.{}.{}.tmp",
            device_hex(device_id),
            std::process::id(),
            counter
        ));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary_path)
                .map_err(|_| RecoveryError::Io)?;
            file.write_all(package).map_err(|_| RecoveryError::Io)?;
            file.sync_all().map_err(|_| RecoveryError::Io)?;
            atomic_replace(&temporary_path, &final_path)
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        write_result
    }

    pub fn load(
        &self,
        device_id: &[u8; 16],
        trust_store: &TrustStore,
    ) -> Result<VerifiedFirmwarePackage, RecoveryError> {
        let path = self.package_path(device_id);
        let metadata = fs::metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RecoveryError::NotFound
            } else {
                RecoveryError::Io
            }
        })?;
        if metadata.len() > MAX_PACKAGE_LEN {
            return Err(RecoveryError::TooLarge);
        }
        let bytes = fs::read(path).map_err(|_| RecoveryError::Io)?;
        Ok(FirmwarePackage::inspect(&bytes, trust_store)?)
    }
}

fn device_hex(device_id: &[u8; 16]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(32);
    for byte in device_id {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0F)] as char);
    }
    output
}

#[cfg(windows)]
fn atomic_replace(source: &Path, destination: &Path) -> Result<(), RecoveryError> {
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
    // SAFETY: both path buffers are NUL-terminated and remain live during the call.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(RecoveryError::Io);
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, destination: &Path) -> Result<(), RecoveryError> {
    fs::rename(source, destination).map_err(|_| RecoveryError::Io)
}
