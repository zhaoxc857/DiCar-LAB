use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::credentials::{BslPassword, CredentialError, CredentialStore};
use crate::package::{build_signed_package, FirmwareManifestInput, PackageError};
use crate::provisioning_store::{ProvisioningError, ProvisioningStore};
use crate::recovery_store::{RecoveryError, RecoveryStore};

const MAX_IMAGE_LEN: u64 = 131_072;
const MAX_PACKAGE_LEN: u64 = 18 + 8_192 + 131_072 + 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCommand {
    Package {
        release_id: Uuid,
        firmware_version: [u16; 3],
        signing_key_id: String,
        image_path: PathBuf,
        key_path: PathBuf,
        output_path: PathBuf,
    },
    ProvisionRecord {
        device_id: [u8; 16],
        signing_key_id: String,
        public_key_path: PathBuf,
        recovery_package_path: PathBuf,
        store_dir: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolError {
    InvalidArguments,
    InvalidImage,
    InvalidSigningKey,
    InvalidPublicKey,
    InvalidPassword,
    OutputExists,
    Io,
    Package(PackageError),
    Credential(CredentialError),
    Provisioning(ProvisioningError),
    Recovery(RecoveryError),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidArguments => "invalid command arguments",
            Self::InvalidImage => "firmware image is invalid",
            Self::InvalidSigningKey => "firmware signing key is invalid",
            Self::InvalidPublicKey => "firmware signing public key is invalid",
            Self::InvalidPassword => "device firmware password is invalid",
            Self::OutputExists => "output file already exists",
            Self::Io => "firmware tool I/O operation failed",
            Self::Package(error) => return error.fmt(formatter),
            Self::Credential(error) => return error.fmt(formatter),
            Self::Provisioning(error) => return error.fmt(formatter),
            Self::Recovery(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for ToolError {}

impl From<PackageError> for ToolError {
    fn from(error: PackageError) -> Self {
        Self::Package(error)
    }
}

impl From<CredentialError> for ToolError {
    fn from(error: CredentialError) -> Self {
        Self::Credential(error)
    }
}

impl From<ProvisioningError> for ToolError {
    fn from(error: ProvisioningError) -> Self {
        Self::Provisioning(error)
    }
}

impl From<RecoveryError> for ToolError {
    fn from(error: RecoveryError) -> Self {
        Self::Recovery(error)
    }
}

pub fn parse_args(args: Vec<OsString>) -> Result<ToolCommand, ToolError> {
    let mut args = args.into_iter();
    let command = args.next().ok_or(ToolError::InvalidArguments)?;
    match command.to_str() {
        Some("package") => parse_package_args(args),
        Some("provision-record") => parse_provision_args(args),
        _ => Err(ToolError::InvalidArguments),
    }
}

fn parse_package_args(mut args: impl Iterator<Item = OsString>) -> Result<ToolCommand, ToolError> {
    let mut release_id = None;
    let mut firmware_version = None;
    let mut signing_key_id = None;
    let mut image_path = None;
    let mut key_path = None;
    let mut output_path = None;

    while let Some(option) = args.next() {
        let value = args.next().ok_or(ToolError::InvalidArguments)?;
        match option.to_str() {
            Some("--release-id") if release_id.is_none() => {
                release_id = Some(parse_uuid(value)?);
            }
            Some("--version") if firmware_version.is_none() => {
                firmware_version = Some(parse_version(value)?);
            }
            Some("--signing-key-id") if signing_key_id.is_none() => {
                let value = value
                    .into_string()
                    .map_err(|_| ToolError::InvalidArguments)?;
                if !is_lower_hex(&value, 16) {
                    return Err(ToolError::InvalidArguments);
                }
                signing_key_id = Some(value);
            }
            Some("--image") if image_path.is_none() => image_path = Some(PathBuf::from(value)),
            Some("--key") if key_path.is_none() => key_path = Some(PathBuf::from(value)),
            Some("--output") if output_path.is_none() => output_path = Some(PathBuf::from(value)),
            _ => return Err(ToolError::InvalidArguments),
        }
    }

    Ok(ToolCommand::Package {
        release_id: release_id.ok_or(ToolError::InvalidArguments)?,
        firmware_version: firmware_version.ok_or(ToolError::InvalidArguments)?,
        signing_key_id: signing_key_id.ok_or(ToolError::InvalidArguments)?,
        image_path: image_path.ok_or(ToolError::InvalidArguments)?,
        key_path: key_path.ok_or(ToolError::InvalidArguments)?,
        output_path: output_path.ok_or(ToolError::InvalidArguments)?,
    })
}

fn parse_provision_args(
    mut args: impl Iterator<Item = OsString>,
) -> Result<ToolCommand, ToolError> {
    let mut device_id = None;
    let mut signing_key_id = None;
    let mut public_key_path = None;
    let mut recovery_package_path = None;
    let mut store_dir = None;

    while let Some(option) = args.next() {
        let value = args.next().ok_or(ToolError::InvalidArguments)?;
        match option.to_str() {
            Some("--device-id") if device_id.is_none() => {
                let value = value
                    .into_string()
                    .map_err(|_| ToolError::InvalidArguments)?;
                device_id = Some(decode_hex_16(&value).ok_or(ToolError::InvalidArguments)?);
            }
            Some("--signing-key-id") if signing_key_id.is_none() => {
                let value = value
                    .into_string()
                    .map_err(|_| ToolError::InvalidArguments)?;
                if !is_lower_hex(&value, 16) {
                    return Err(ToolError::InvalidArguments);
                }
                signing_key_id = Some(value);
            }
            Some("--public-key") if public_key_path.is_none() => {
                public_key_path = Some(PathBuf::from(value));
            }
            Some("--recovery-package") if recovery_package_path.is_none() => {
                recovery_package_path = Some(PathBuf::from(value));
            }
            Some("--store-dir") if store_dir.is_none() => store_dir = Some(PathBuf::from(value)),
            _ => return Err(ToolError::InvalidArguments),
        }
    }

    Ok(ToolCommand::ProvisionRecord {
        device_id: device_id.ok_or(ToolError::InvalidArguments)?,
        signing_key_id: signing_key_id.ok_or(ToolError::InvalidArguments)?,
        public_key_path: public_key_path.ok_or(ToolError::InvalidArguments)?,
        recovery_package_path: recovery_package_path.ok_or(ToolError::InvalidArguments)?,
        store_dir: store_dir.ok_or(ToolError::InvalidArguments)?,
    })
}

pub fn execute_package(command: &ToolCommand) -> Result<(), ToolError> {
    let ToolCommand::Package {
        release_id,
        firmware_version,
        signing_key_id,
        image_path,
        key_path,
        output_path,
    } = command
    else {
        return Err(ToolError::InvalidArguments);
    };

    let image_metadata = fs::metadata(image_path).map_err(|_| ToolError::Io)?;
    if !image_metadata.is_file() || image_metadata.len() > MAX_IMAGE_LEN {
        return Err(ToolError::InvalidImage);
    }
    let image = fs::read(image_path).map_err(|_| ToolError::Io)?;

    let mut key_file = fs::read(key_path).map_err(|_| ToolError::Io)?;
    let mut key_bytes = decode_signing_key(&key_file).ok_or(ToolError::InvalidSigningKey)?;
    key_file.zeroize();
    let signing_key = SigningKey::from_bytes(&key_bytes);
    key_bytes.zeroize();

    let package = build_signed_package(
        &FirmwareManifestInput {
            release_id: *release_id,
            firmware_version: *firmware_version,
            signing_key_id: signing_key_id.clone(),
        },
        &image,
        &signing_key,
    )?;

    let mut output = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(ToolError::OutputExists);
        }
        Err(_) => return Err(ToolError::Io),
    };
    output.write_all(&package).map_err(|_| ToolError::Io)?;
    output.sync_all().map_err(|_| ToolError::Io)
}

pub fn execute_provision_record(
    command: &ToolCommand,
    input: impl Read,
    credentials: &impl CredentialStore,
) -> Result<(), ToolError> {
    let ToolCommand::ProvisionRecord {
        device_id,
        signing_key_id,
        public_key_path,
        recovery_package_path,
        store_dir,
    } = command
    else {
        return Err(ToolError::InvalidArguments);
    };

    let password = read_password(input)?;
    let public_key_metadata = fs::metadata(public_key_path).map_err(|_| ToolError::Io)?;
    if !public_key_metadata.is_file() || public_key_metadata.len() > 64 {
        return Err(ToolError::InvalidPublicKey);
    }
    let mut public_key_file = fs::read(public_key_path).map_err(|_| ToolError::Io)?;
    let public_key = decode_32(&public_key_file).ok_or(ToolError::InvalidPublicKey)?;
    public_key_file.zeroize();
    let trust_store =
        crate::package::TrustStore::from_keys([(signing_key_id.clone(), public_key)])?;

    let recovery_metadata = fs::metadata(recovery_package_path).map_err(|_| ToolError::Io)?;
    if !recovery_metadata.is_file() || recovery_metadata.len() > MAX_PACKAGE_LEN {
        return Err(ToolError::InvalidImage);
    }
    let recovery_package = fs::read(recovery_package_path).map_err(|_| ToolError::Io)?;
    crate::package::FirmwarePackage::inspect(&recovery_package, &trust_store)?;

    ProvisioningStore::new(store_dir.join("trust")).save(device_id, signing_key_id, public_key)?;
    RecoveryStore::new(store_dir.join("recovery")).save(
        device_id,
        &recovery_package,
        &trust_store,
    )?;
    credentials.store(device_id, &password)?;
    Ok(())
}

fn read_password(input: impl Read) -> Result<BslPassword, ToolError> {
    let mut bytes = Vec::with_capacity(68);
    input
        .take(68)
        .read_to_end(&mut bytes)
        .map_err(|_| ToolError::Io)?;
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    let decoded = decode_32(&bytes).ok_or(ToolError::InvalidPassword);
    bytes.zeroize();
    Ok(BslPassword::new(decoded?))
}

fn parse_uuid(value: OsString) -> Result<Uuid, ToolError> {
    value
        .into_string()
        .map_err(|_| ToolError::InvalidArguments)?
        .parse()
        .map_err(|_| ToolError::InvalidArguments)
}

fn parse_version(value: OsString) -> Result<[u16; 3], ToolError> {
    let value = value
        .into_string()
        .map_err(|_| ToolError::InvalidArguments)?;
    let mut parts = value.split('.');
    let major = parse_version_part(parts.next())?;
    let minor = parse_version_part(parts.next())?;
    let patch = parse_version_part(parts.next())?;
    if parts.next().is_some() {
        return Err(ToolError::InvalidArguments);
    }
    Ok([major, minor, patch])
}

fn parse_version_part(part: Option<&str>) -> Result<u16, ToolError> {
    let part = part.ok_or(ToolError::InvalidArguments)?;
    if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
        return Err(ToolError::InvalidArguments);
    }
    part.parse().map_err(|_| ToolError::InvalidArguments)
}

fn decode_signing_key(bytes: &[u8]) -> Option<[u8; 32]> {
    decode_32(bytes)
}

fn decode_32(bytes: &[u8]) -> Option<[u8; 32]> {
    if let Ok(raw) = <[u8; 32]>::try_from(bytes) {
        return Some(raw);
    }
    if bytes.len() != 64 {
        return None;
    }
    let mut output = [0u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        output[index] = hex_nibble(pair[0])?.checked_mul(16)? + hex_nibble(pair[1])?;
    }
    Some(output)
}

fn decode_hex_16(value: &str) -> Option<[u8; 16]> {
    if !is_lower_hex(value, 32) {
        return None;
    }
    let mut output = [0u8; 16];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = hex_nibble(pair[0])?.checked_mul(16)? + hex_nibble(pair[1])?;
    }
    Some(output)
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
