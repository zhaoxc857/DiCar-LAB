use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialError {
    NotFound,
    InvalidSecret,
    BackendFailure,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "device firmware credential was not found",
            Self::InvalidSecret => "device firmware credential has an invalid format",
            Self::BackendFailure => "device firmware credential backend failed",
        })
    }
}

impl std::error::Error for CredentialError {}

pub struct BslPassword([u8; 32]);

impl BslPassword {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn expose_secret(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for BslPassword {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BslPassword(redacted)")
    }
}

impl Zeroize for BslPassword {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for BslPassword {
    fn drop(&mut self) {
        self.zeroize();
    }
}

pub fn credential_target_name(device_id: &[u8; 16]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut target = String::with_capacity(18 + device_id.len() * 2);
    target.push_str("DiCar/FirmwareBSL/");
    for byte in device_id {
        target.push(DIGITS[usize::from(byte >> 4)] as char);
        target.push(DIGITS[usize::from(byte & 0x0F)] as char);
    }
    target
}

pub trait CredentialStore: Send + Sync {
    fn store(&self, device_id: &[u8; 16], password: &BslPassword) -> Result<(), CredentialError>;
    fn load(&self, device_id: &[u8; 16]) -> Result<BslPassword, CredentialError>;
}

#[derive(Default)]
pub struct MemoryCredentialStore {
    entries: Mutex<BTreeMap<[u8; 16], [u8; 32]>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn store(&self, device_id: &[u8; 16], password: &BslPassword) -> Result<(), CredentialError> {
        self.entries
            .lock()
            .map_err(|_| CredentialError::BackendFailure)?
            .insert(*device_id, *password.expose_secret());
        Ok(())
    }

    fn load(&self, device_id: &[u8; 16]) -> Result<BslPassword, CredentialError> {
        let bytes = self
            .entries
            .lock()
            .map_err(|_| CredentialError::BackendFailure)?
            .get(device_id)
            .copied()
            .ok_or(CredentialError::NotFound)?;
        Ok(BslPassword::new(bytes))
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsCredentialStore;

#[cfg(windows)]
impl CredentialStore for WindowsCredentialStore {
    fn store(&self, device_id: &[u8; 16], password: &BslPassword) -> Result<(), CredentialError> {
        use std::ptr;
        use windows_sys::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };

        let mut target = wide_null(&credential_target_name(device_id));
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: 32,
            CredentialBlob: password.expose_secret().as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: ptr::null_mut(),
            ..CREDENTIALW::default()
        };
        // SAFETY: all pointers reference live buffers for the duration of CredWriteW;
        // the credential blob length exactly matches the 32-byte password.
        let written = unsafe { CredWriteW(&credential, 0) };
        if written == 0 {
            return Err(CredentialError::BackendFailure);
        }
        Ok(())
    }

    fn load(&self, device_id: &[u8; 16]) -> Result<BslPassword, CredentialError> {
        use std::ptr;
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
        use windows_sys::Win32::Security::Credentials::{
            CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };

        let target = wide_null(&credential_target_name(device_id));
        let mut raw: *mut CREDENTIALW = ptr::null_mut();
        // SAFETY: target is NUL-terminated and raw is a valid out pointer.
        let read = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut raw) };
        if read == 0 {
            // SAFETY: GetLastError has no preconditions and is read immediately after failure.
            let error = unsafe { GetLastError() };
            return Err(if error == ERROR_NOT_FOUND {
                CredentialError::NotFound
            } else {
                CredentialError::BackendFailure
            });
        }
        let guard = CredentialGuard(raw);
        // SAFETY: a successful CredReadW returns a valid CREDENTIALW until CredFree.
        let credential = unsafe { &*guard.0 };
        if credential.CredentialBlobSize != 32 || credential.CredentialBlob.is_null() {
            return Err(CredentialError::InvalidSecret);
        }
        let mut bytes = [0u8; 32];
        // SAFETY: the size check above guarantees at least 32 readable bytes.
        unsafe {
            ptr::copy_nonoverlapping(credential.CredentialBlob, bytes.as_mut_ptr(), bytes.len())
        };
        Ok(BslPassword::new(bytes))
    }
}

#[cfg(windows)]
struct CredentialGuard(*mut windows_sys::Win32::Security::Credentials::CREDENTIALW);

#[cfg(windows)]
impl Drop for CredentialGuard {
    fn drop(&mut self) {
        // SAFETY: the pointer was returned by CredReadW and is freed exactly once here.
        unsafe {
            windows_sys::Win32::Security::Credentials::CredFree(self.0.cast());
        }
    }
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
