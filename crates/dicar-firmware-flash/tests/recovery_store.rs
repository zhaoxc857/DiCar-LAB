use std::fs;

use ed25519_dalek::SigningKey;
use tempfile::tempdir;

use dicar_firmware_flash::package::{
    build_signed_package, FirmwareManifestInput, PackageError, TrustStore,
};
use dicar_firmware_flash::recovery_store::{RecoveryError, RecoveryStore};

const KEY_ID: &str = "0102030405060708";

fn package(version: [u16; 3], fill: u8) -> (Vec<u8>, TrustStore) {
    let signing = SigningKey::from_bytes(&[0x29; 32]);
    let trust =
        TrustStore::from_keys([(KEY_ID.to_owned(), signing.verifying_key().to_bytes())]).unwrap();
    let input = FirmwareManifestInput {
        release_id: "123e4567-e89b-12d3-a456-426614174000".parse().unwrap(),
        firmware_version: version,
        signing_key_id: KEY_ID.to_owned(),
    };
    (
        build_signed_package(&input, &vec![fill; 1024], &signing).unwrap(),
        trust,
    )
}

#[test]
fn save_atomically_replaces_only_the_same_devices_single_recovery_package() {
    let directory = tempdir().unwrap();
    let store = RecoveryStore::new(directory.path());
    let device = [0x11; 16];
    let (old, trust) = package([1, 0, 0], 0x10);
    let (new, _) = package([2, 0, 0], 0x20);

    store.save(&device, &old, &trust).unwrap();
    store.save(&device, &new, &trust).unwrap();

    let loaded = store.load(&device, &trust).unwrap();
    assert_eq!(loaded.manifest().firmware_version(), [2, 0, 0]);
    assert_eq!(loaded.image(), &[0x20; 1024]);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn recovery_packages_are_device_scoped_and_corrupt_input_never_replaces_a_valid_copy() {
    let directory = tempdir().unwrap();
    let store = RecoveryStore::new(directory.path());
    let first_device = [0x21; 16];
    let second_device = [0x22; 16];
    let (package, trust) = package([3, 1, 4], 0x33);

    store.save(&first_device, &package, &trust).unwrap();
    store.save(&second_device, &package, &trust).unwrap();
    assert_ne!(
        store.package_path(&first_device),
        store.package_path(&second_device)
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);

    let mut corrupt = package.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        store.save(&first_device, &corrupt, &trust),
        Err(RecoveryError::Package(PackageError::InvalidSignature))
    );
    assert_eq!(
        store
            .load(&first_device, &trust)
            .unwrap()
            .manifest()
            .firmware_version(),
        [3, 1, 4]
    );
}

#[test]
fn missing_and_corrupt_files_are_rejected_on_load() {
    let directory = tempdir().unwrap();
    let store = RecoveryStore::new(directory.path());
    let device = [0x31; 16];
    let (_package, trust) = package([1, 0, 0], 0x44);

    assert_eq!(
        store.load(&device, &trust).unwrap_err(),
        RecoveryError::NotFound
    );
    fs::write(store.package_path(&device), b"corrupt").unwrap();
    assert_eq!(
        store.load(&device, &trust).unwrap_err(),
        RecoveryError::Package(PackageError::Truncated)
    );
}
