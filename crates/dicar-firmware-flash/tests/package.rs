use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use dicar_firmware_flash::package::{
    build_signed_package, FirmwareManifestInput, FirmwarePackage, PackageError, TrustStore,
};

const KEY_ID: &str = "0102030405060708";
const SIGNING_KEY_BYTES: [u8; 32] = [0x19; 32];

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn manifest(image: &[u8]) -> Value {
    json!({
        "schemaVersion": 1,
        "releaseId": "123e4567-e89b-12d3-a456-426614174000",
        "target": "lckfb-tmx-mspm0g3507",
        "mcu": "MSPM0G3507",
        "firmwareVersion": [0, 3, 0],
        "imageBase": 0,
        "imageLength": image.len(),
        "imageSha256": sha256_hex(image),
        "signingKeyId": KEY_ID,
    })
}

fn signed_package(manifest: &Value, image: &[u8]) -> Vec<u8> {
    let manifest = serde_json::to_vec(manifest).unwrap();
    let mut signed = Vec::new();
    signed.extend_from_slice(b"DiCarFW-v1\0");
    signed.extend_from_slice(&1u16.to_le_bytes());
    signed.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    signed.extend_from_slice(&(image.len() as u32).to_le_bytes());
    signed.extend_from_slice(&manifest);
    signed.extend_from_slice(image);
    let signature = SigningKey::from_bytes(&SIGNING_KEY_BYTES).sign(&signed);

    let mut package = Vec::new();
    package.extend_from_slice(b"DICARFW\0");
    package.extend_from_slice(&signed[b"DiCarFW-v1\0".len()..]);
    package.extend_from_slice(&signature.to_bytes());
    package
}

fn fixture() -> (Vec<u8>, Vec<u8>, TrustStore) {
    let image = (0..1024).map(|value| value as u8).collect::<Vec<_>>();
    let package = signed_package(&manifest(&image), &image);
    let signing = SigningKey::from_bytes(&SIGNING_KEY_BYTES);
    let trust =
        TrustStore::from_keys([(KEY_ID.to_owned(), signing.verifying_key().to_bytes())]).unwrap();
    (package, image, trust)
}

#[test]
fn valid_signed_package_is_bounded_and_verified() {
    let (package, image, trust) = fixture();

    let verified = FirmwarePackage::inspect(&package, &trust).unwrap();

    assert_eq!(
        verified.manifest().release_id().to_string(),
        "123e4567-e89b-12d3-a456-426614174000"
    );
    assert_eq!(verified.manifest().firmware_version(), [0, 3, 0]);
    assert_eq!(verified.manifest().image_length(), 1024);
    assert_eq!(verified.image(), image);
}

#[test]
fn framing_and_bounds_are_rejected_before_manifest_or_signature_work() {
    let (package, _image, trust) = fixture();

    for length in 0..18 {
        assert_eq!(
            FirmwarePackage::inspect(&package[..length], &trust).unwrap_err(),
            PackageError::Truncated
        );
    }

    let mut bad_magic = package.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        FirmwarePackage::inspect(&bad_magic, &trust).unwrap_err(),
        PackageError::BadMagic
    );

    let mut bad_version = package.clone();
    bad_version[8..10].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(
        FirmwarePackage::inspect(&bad_version, &trust).unwrap_err(),
        PackageError::UnsupportedVersion
    );

    let mut huge_manifest = package.clone();
    huge_manifest[10..14].copy_from_slice(&8193u32.to_le_bytes());
    assert_eq!(
        FirmwarePackage::inspect(&huge_manifest, &trust).unwrap_err(),
        PackageError::InvalidManifestLength
    );

    let mut short_image = package.clone();
    short_image[14..18].copy_from_slice(&1023u32.to_le_bytes());
    assert_eq!(
        FirmwarePackage::inspect(&short_image, &trust).unwrap_err(),
        PackageError::InvalidImageLength
    );

    let mut long_image = package.clone();
    long_image[14..18].copy_from_slice(&131_073u32.to_le_bytes());
    assert_eq!(
        FirmwarePackage::inspect(&long_image, &trust).unwrap_err(),
        PackageError::InvalidImageLength
    );

    let mut trailing = package.clone();
    trailing.push(0);
    assert_eq!(
        FirmwarePackage::inspect(&trailing, &trust).unwrap_err(),
        PackageError::LengthMismatch
    );
}

#[test]
fn strict_manifest_metadata_and_image_digest_are_enforced() {
    let (_package, image, trust) = fixture();

    let mut unknown = manifest(&image);
    unknown["unexpected"] = json!(true);
    assert_eq!(
        FirmwarePackage::inspect(&signed_package(&unknown, &image), &trust).unwrap_err(),
        PackageError::InvalidManifest
    );

    let cases = [
        (
            "schemaVersion",
            json!(2),
            PackageError::InvalidManifestSchema,
        ),
        ("target", json!("stm32f4"), PackageError::UnsupportedTarget),
        ("mcu", json!("MSPM0G3519"), PackageError::UnsupportedMcu),
        ("imageBase", json!(4), PackageError::ImageMetadataMismatch),
        (
            "imageLength",
            json!(1025),
            PackageError::ImageMetadataMismatch,
        ),
        (
            "imageSha256",
            json!("A".repeat(64)),
            PackageError::InvalidManifest,
        ),
        (
            "signingKeyId",
            json!("ABCDEF0123456789"),
            PackageError::InvalidManifest,
        ),
    ];
    for (field, value, expected) in cases {
        let mut changed = manifest(&image);
        changed[field] = value;
        assert_eq!(
            FirmwarePackage::inspect(&signed_package(&changed, &image), &trust).unwrap_err(),
            expected,
            "field {field}"
        );
    }

    let mut wrong_hash = manifest(&image);
    wrong_hash["imageSha256"] = json!("00".repeat(32));
    assert_eq!(
        FirmwarePackage::inspect(&signed_package(&wrong_hash, &image), &trust).unwrap_err(),
        PackageError::ImageHashMismatch
    );
}

#[test]
fn unknown_key_and_bad_signature_are_rejected() {
    let (mut package, image, trust) = fixture();
    let mut unknown_key = manifest(&image);
    unknown_key["signingKeyId"] = json!("1111111111111111");
    assert_eq!(
        FirmwarePackage::inspect(&signed_package(&unknown_key, &image), &trust).unwrap_err(),
        PackageError::UnknownSigningKey
    );

    let last = package.len() - 1;
    package[last] ^= 0x80;
    assert_eq!(
        FirmwarePackage::inspect(&package, &trust).unwrap_err(),
        PackageError::InvalidSignature
    );
}

#[test]
fn package_builder_is_deterministic_and_produces_a_verifiable_package() {
    let image = (0..1024).map(|value| value as u8).collect::<Vec<_>>();
    let input = FirmwareManifestInput {
        release_id: "123e4567-e89b-12d3-a456-426614174000".parse().unwrap(),
        firmware_version: [1, 4, 2],
        signing_key_id: KEY_ID.to_owned(),
    };
    let signing = SigningKey::from_bytes(&SIGNING_KEY_BYTES);
    let trust =
        TrustStore::from_keys([(KEY_ID.to_owned(), signing.verifying_key().to_bytes())]).unwrap();

    let first = build_signed_package(&input, &image, &signing).unwrap();
    let second = build_signed_package(&input, &image, &signing).unwrap();

    assert_eq!(first, second);
    let verified = FirmwarePackage::inspect(&first, &trust).unwrap();
    assert_eq!(verified.manifest().firmware_version(), [1, 4, 2]);
    assert_eq!(verified.image(), image);
}
