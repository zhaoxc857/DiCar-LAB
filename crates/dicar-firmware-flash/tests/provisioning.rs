use std::fs;
use std::io::Cursor;

use dicar_firmware_flash::credentials::{CredentialStore, MemoryCredentialStore};
use dicar_firmware_flash::package::{build_signed_package, FirmwareManifestInput};
use dicar_firmware_flash::provisioning_store::ProvisioningStore;
use dicar_firmware_flash::recovery_store::RecoveryStore;
use dicar_firmware_flash::tool::{execute_provision_record, ToolCommand, ToolError};
use ed25519_dalek::SigningKey;
use tempfile::tempdir;

const DEVICE_ID: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const KEY_ID: &str = "0102030405060708";

#[test]
fn provision_record_imports_public_trust_recovery_and_stdin_password() {
    let directory = tempdir().unwrap();
    let public_key_path = directory.path().join("release.pub");
    let recovery_path = directory.path().join("known-good.dicarfw");
    let store_dir = directory.path().join("store");
    let signing = SigningKey::from_bytes(&[0x39; 32]);
    fs::write(&public_key_path, signing.verifying_key().to_bytes()).unwrap();
    let package = build_signed_package(
        &FirmwareManifestInput {
            release_id: "123e4567-e89b-12d3-a456-426614174000".parse().unwrap(),
            firmware_version: [1, 0, 0],
            signing_key_id: KEY_ID.to_owned(),
        },
        &vec![0xA5; 1024],
        &signing,
    )
    .unwrap();
    fs::write(&recovery_path, &package).unwrap();
    let command = ToolCommand::ProvisionRecord {
        device_id: DEVICE_ID,
        signing_key_id: KEY_ID.to_owned(),
        public_key_path,
        recovery_package_path: recovery_path,
        store_dir: store_dir.clone(),
    };
    let credentials = MemoryCredentialStore::default();
    let password_hex = format!("{}\n", "ab".repeat(32));

    execute_provision_record(
        &command,
        Cursor::new(password_hex.into_bytes()),
        &credentials,
    )
    .unwrap();

    assert_eq!(
        credentials.load(&DEVICE_ID).unwrap().expose_secret(),
        &[0xAB; 32]
    );
    let trust = ProvisioningStore::new(store_dir.join("trust"))
        .load_trust_store(&DEVICE_ID)
        .unwrap();
    let restored = RecoveryStore::new(store_dir.join("recovery"))
        .load(&DEVICE_ID, &trust)
        .unwrap();
    assert_eq!(restored.image(), vec![0xA5; 1024]);
}

#[test]
fn invalid_stdin_password_does_not_store_a_credential() {
    let directory = tempdir().unwrap();
    let command = ToolCommand::ProvisionRecord {
        device_id: DEVICE_ID,
        signing_key_id: KEY_ID.to_owned(),
        public_key_path: directory.path().join("missing.pub"),
        recovery_package_path: directory.path().join("missing.dicarfw"),
        store_dir: directory.path().join("store"),
    };
    let credentials = MemoryCredentialStore::default();

    assert_eq!(
        execute_provision_record(&command, Cursor::new(b"password\n"), &credentials),
        Err(ToolError::InvalidPassword)
    );
    assert!(credentials.load(&DEVICE_ID).is_err());
}
