use dicar_firmware_flash::credentials::{
    credential_target_name, BslPassword, CredentialError, CredentialStore, MemoryCredentialStore,
};
use zeroize::Zeroize;

#[test]
fn credential_target_is_stable_lowercase_and_device_scoped() {
    assert_eq!(
        credential_target_name(&[0xAB; 16]),
        "DiCar/FirmwareBSL/abababababababababababababababab"
    );
    assert_ne!(
        credential_target_name(&[0xAB; 16]),
        credential_target_name(&[0xAC; 16])
    );
}

#[test]
fn password_debug_and_errors_never_render_secret_material() {
    let password = BslPassword::new([0xA5; 32]);
    let rendered = format!("{password:?} {}", CredentialError::BackendFailure);

    assert!(!rendered.contains("a5a5"));
    assert!(!rendered.contains("165"));
    assert!(rendered.contains("redacted"));
}

#[test]
fn password_zeroization_and_in_memory_backend_round_trip_are_explicit() {
    let device_id = [0x31; 16];
    let mut password = BslPassword::new([0x5C; 32]);
    let store = MemoryCredentialStore::default();

    store.store(&device_id, &password).unwrap();
    assert_eq!(store.load(&device_id).unwrap().expose_secret(), &[0x5C; 32]);
    assert_eq!(
        store.load(&[0x32; 16]).unwrap_err(),
        CredentialError::NotFound
    );

    password.zeroize();
    assert_eq!(password.expose_secret(), &[0; 32]);
}
