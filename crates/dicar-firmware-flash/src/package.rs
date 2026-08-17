use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAGIC: &[u8; 8] = b"DICARFW\0";
const SIGNATURE_DOMAIN: &[u8] = b"DiCarFW-v1\0";
const FORMAT_VERSION: u16 = 1;
const HEADER_LEN: usize = 18;
const SIGNATURE_LEN: usize = 64;
const MAX_MANIFEST_LEN: usize = 8_192;
const MIN_IMAGE_LEN: usize = 1_024;
const MAX_IMAGE_LEN: usize = 131_072;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageError {
    Truncated,
    BadMagic,
    UnsupportedVersion,
    InvalidManifestLength,
    InvalidImageLength,
    LengthMismatch,
    InvalidManifest,
    InvalidManifestSchema,
    UnsupportedTarget,
    UnsupportedMcu,
    ImageMetadataMismatch,
    ImageHashMismatch,
    UnknownSigningKey,
    InvalidPublicKey,
    InvalidSignature,
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Truncated => "firmware package is truncated",
            Self::BadMagic => "firmware package magic is invalid",
            Self::UnsupportedVersion => "firmware package version is unsupported",
            Self::InvalidManifestLength => "firmware manifest length is invalid",
            Self::InvalidImageLength => "firmware image length is invalid",
            Self::LengthMismatch => "firmware package length does not match its header",
            Self::InvalidManifest => "firmware manifest is invalid",
            Self::InvalidManifestSchema => "firmware manifest schema is unsupported",
            Self::UnsupportedTarget => "firmware target is unsupported",
            Self::UnsupportedMcu => "firmware MCU is unsupported",
            Self::ImageMetadataMismatch => "firmware image metadata is inconsistent",
            Self::ImageHashMismatch => "firmware image digest does not match its manifest",
            Self::UnknownSigningKey => "firmware signing key is not trusted",
            Self::InvalidPublicKey => "firmware signing public key is invalid",
            Self::InvalidSignature => "firmware package signature is invalid",
        })
    }
}

impl std::error::Error for PackageError {}

#[derive(Clone, Debug, Default)]
pub struct TrustStore {
    keys: BTreeMap<String, VerifyingKey>,
}

impl TrustStore {
    pub fn from_keys(
        keys: impl IntoIterator<Item = (String, [u8; 32])>,
    ) -> Result<Self, PackageError> {
        let mut store = Self::default();
        for (key_id, bytes) in keys {
            if !is_lower_hex(&key_id, 16) {
                return Err(PackageError::InvalidPublicKey);
            }
            let key =
                VerifyingKey::from_bytes(&bytes).map_err(|_| PackageError::InvalidPublicKey)?;
            store.keys.insert(key_id, key);
        }
        Ok(store)
    }

    fn get(&self, key_id: &str) -> Option<&VerifyingKey> {
        self.keys.get(key_id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireManifest {
    schema_version: u16,
    release_id: Uuid,
    target: String,
    mcu: String,
    firmware_version: [u16; 3],
    image_base: u32,
    image_length: u32,
    image_sha256: String,
    signing_key_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareManifest {
    release_id: Uuid,
    firmware_version: [u16; 3],
    image_length: u32,
    image_sha256: [u8; 32],
    signing_key_id: String,
}

impl FirmwareManifest {
    pub const fn release_id(&self) -> Uuid {
        self.release_id
    }

    pub const fn firmware_version(&self) -> [u16; 3] {
        self.firmware_version
    }

    pub const fn image_length(&self) -> u32 {
        self.image_length
    }

    pub const fn image_sha256(&self) -> [u8; 32] {
        self.image_sha256
    }

    pub fn signing_key_id(&self) -> &str {
        &self.signing_key_id
    }
}

#[derive(Clone, Debug)]
pub struct VerifiedFirmwarePackage {
    manifest: FirmwareManifest,
    image: Vec<u8>,
    package_sha256: [u8; 32],
}

impl VerifiedFirmwarePackage {
    pub const fn manifest(&self) -> &FirmwareManifest {
        &self.manifest
    }

    pub fn image(&self) -> &[u8] {
        &self.image
    }

    pub const fn package_sha256(&self) -> [u8; 32] {
        self.package_sha256
    }
}

pub struct FirmwarePackage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareManifestInput {
    pub release_id: Uuid,
    pub firmware_version: [u16; 3],
    pub signing_key_id: String,
}

pub fn build_signed_package(
    input: &FirmwareManifestInput,
    image: &[u8],
    signing_key: &SigningKey,
) -> Result<Vec<u8>, PackageError> {
    if !(MIN_IMAGE_LEN..=MAX_IMAGE_LEN).contains(&image.len()) {
        return Err(PackageError::InvalidImageLength);
    }
    if !is_lower_hex(&input.signing_key_id, 16) {
        return Err(PackageError::InvalidManifest);
    }
    let digest: [u8; 32] = Sha256::digest(image).into();
    let wire = WireManifest {
        schema_version: 1,
        release_id: input.release_id,
        target: "lckfb-tmx-mspm0g3507".to_owned(),
        mcu: "MSPM0G3507".to_owned(),
        firmware_version: input.firmware_version,
        image_base: 0,
        image_length: image.len() as u32,
        image_sha256: encode_hex(&digest),
        signing_key_id: input.signing_key_id.clone(),
    };
    let manifest = serde_json::to_vec(&wire).map_err(|_| PackageError::InvalidManifest)?;
    if manifest.is_empty() || manifest.len() > MAX_MANIFEST_LEN {
        return Err(PackageError::InvalidManifestLength);
    }

    let mut signed = Vec::with_capacity(SIGNATURE_DOMAIN.len() + 10 + manifest.len() + image.len());
    signed.extend_from_slice(SIGNATURE_DOMAIN);
    signed.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    signed.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    signed.extend_from_slice(&(image.len() as u32).to_le_bytes());
    signed.extend_from_slice(&manifest);
    signed.extend_from_slice(image);
    let signature = signing_key.sign(&signed);

    let mut package =
        Vec::with_capacity(MAGIC.len() + signed.len() - SIGNATURE_DOMAIN.len() + SIGNATURE_LEN);
    package.extend_from_slice(MAGIC);
    package.extend_from_slice(&signed[SIGNATURE_DOMAIN.len()..]);
    package.extend_from_slice(&signature.to_bytes());
    Ok(package)
}

impl FirmwarePackage {
    pub fn inspect(
        bytes: &[u8],
        trust_store: &TrustStore,
    ) -> Result<VerifiedFirmwarePackage, PackageError> {
        if bytes.len() < HEADER_LEN {
            return Err(PackageError::Truncated);
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(PackageError::BadMagic);
        }
        let format_version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if format_version != FORMAT_VERSION {
            return Err(PackageError::UnsupportedVersion);
        }
        let manifest_len = read_u32(bytes, 10)? as usize;
        let image_len = read_u32(bytes, 14)? as usize;
        if !(1..=MAX_MANIFEST_LEN).contains(&manifest_len) {
            return Err(PackageError::InvalidManifestLength);
        }
        if !(MIN_IMAGE_LEN..=MAX_IMAGE_LEN).contains(&image_len) {
            return Err(PackageError::InvalidImageLength);
        }
        let expected_len = HEADER_LEN
            .checked_add(manifest_len)
            .and_then(|len| len.checked_add(image_len))
            .and_then(|len| len.checked_add(SIGNATURE_LEN))
            .ok_or(PackageError::LengthMismatch)?;
        if bytes.len() < expected_len {
            return Err(PackageError::Truncated);
        }
        if bytes.len() != expected_len {
            return Err(PackageError::LengthMismatch);
        }

        let manifest_end = HEADER_LEN + manifest_len;
        let image_end = manifest_end + image_len;
        let wire: WireManifest = serde_json::from_slice(&bytes[HEADER_LEN..manifest_end])
            .map_err(|_| PackageError::InvalidManifest)?;
        let manifest = validate_manifest(wire, image_len)?;
        let image = &bytes[manifest_end..image_end];
        if Sha256::digest(image).as_slice() != manifest.image_sha256 {
            return Err(PackageError::ImageHashMismatch);
        }

        let key = trust_store
            .get(&manifest.signing_key_id)
            .ok_or(PackageError::UnknownSigningKey)?;
        let signature_bytes: [u8; SIGNATURE_LEN] = bytes[image_end..]
            .try_into()
            .map_err(|_| PackageError::Truncated)?;
        let signature = Signature::from_bytes(&signature_bytes);
        let mut signed = Vec::with_capacity(SIGNATURE_DOMAIN.len() + image_end - MAGIC.len());
        signed.extend_from_slice(SIGNATURE_DOMAIN);
        signed.extend_from_slice(&bytes[MAGIC.len()..image_end]);
        key.verify_strict(&signed, &signature)
            .map_err(|_| PackageError::InvalidSignature)?;

        Ok(VerifiedFirmwarePackage {
            manifest,
            image: image.to_vec(),
            package_sha256: Sha256::digest(bytes).into(),
        })
    }
}

fn validate_manifest(
    wire: WireManifest,
    image_len: usize,
) -> Result<FirmwareManifest, PackageError> {
    if wire.schema_version != 1 {
        return Err(PackageError::InvalidManifestSchema);
    }
    if wire.target != "lckfb-tmx-mspm0g3507" {
        return Err(PackageError::UnsupportedTarget);
    }
    if wire.mcu != "MSPM0G3507" {
        return Err(PackageError::UnsupportedMcu);
    }
    if !is_lower_hex(&wire.image_sha256, 64) || !is_lower_hex(&wire.signing_key_id, 16) {
        return Err(PackageError::InvalidManifest);
    }
    if wire.image_base != 0 || wire.image_length as usize != image_len {
        return Err(PackageError::ImageMetadataMismatch);
    }
    let image_sha256 = decode_hex_32(&wire.image_sha256).ok_or(PackageError::InvalidManifest)?;
    Ok(FirmwareManifest {
        release_id: wire.release_id,
        firmware_version: wire.firmware_version,
        image_length: wire.image_length,
        image_sha256,
        signing_key_id: wire.signing_key_id,
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PackageError> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or(PackageError::Truncated)?;
    Ok(u32::from_le_bytes(
        raw.try_into().map_err(|_| PackageError::Truncated)?,
    ))
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut output = [0u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
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

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0F)] as char);
    }
    output
}
