use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use dctp_sim::SimulatorServer;
use dicar_app_core::{CoreCommand, CoreConfig, OperationStatus};
use dicar_desktop_lib::{
    FirmwareFlashErrorDto, FirmwareFlashEvent, FirmwareFlashServiceState,
    FirmwareFlashStartRequest, FirmwareSerial, FirmwareTransportFactory,
};
use dicar_firmware_flash::credentials::MemoryCredentialStore;
use dicar_firmware_flash::package::{build_signed_package, FirmwareManifestInput};
use dicar_firmware_flash::provisioning_store::ProvisioningStore;
use ed25519_dalek::SigningKey;
use tempfile::tempdir;

#[derive(Default)]
struct RefusingTransportFactory {
    opens: Mutex<usize>,
}

impl FirmwareTransportFactory for RefusingTransportFactory {
    fn wait_for_transition(&self) {}

    fn open(
        &self,
        _port_name: &str,
        _baud_rate: u32,
    ) -> Result<Box<dyn FirmwareSerial>, FirmwareFlashErrorDto> {
        *self.opens.lock().unwrap() += 1;
        panic!("simulator rejection must happen before opening BSL serial")
    }
}

#[test]
fn simulator_start_is_rejected_before_any_bsl_transport_is_opened() {
    let directory = tempdir().unwrap();
    let credentials = Arc::new(MemoryCredentialStore::default());
    let transport = Arc::new(RefusingTransportFactory::default());
    let service = FirmwareFlashServiceState::new(directory.path(), credentials, transport.clone());
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let app =
        dicar_desktop_lib::AppState::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    assert_eq!(
        app.dispatch(CoreCommand::Connect).unwrap().status,
        OperationStatus::Succeeded
    );
    let events = Mutex::new(Vec::<FirmwareFlashEvent>::new());

    let error = service
        .start(
            &app,
            FirmwareFlashStartRequest {
                operation_id: uuid::Uuid::new_v4(),
                package_bytes: vec![0; 1024],
                allow_downgrade: false,
            },
            |event| {
                events.lock().unwrap().push(event);
                Ok(())
            },
        )
        .unwrap_err();

    assert_eq!(error.code, "realSerialRequired");
    assert_eq!(*transport.opens.lock().unwrap(), 0);
    assert!(events.lock().unwrap().is_empty());
    drop(app);
    server.shutdown().unwrap();
}

#[test]
fn recovery_and_cancel_commands_reject_unknown_operations() {
    let directory = tempdir().unwrap();
    let service = FirmwareFlashServiceState::new(
        directory.path(),
        Arc::new(MemoryCredentialStore::default()),
        Arc::new(RefusingTransportFactory::default()),
    );
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let app =
        dicar_desktop_lib::AppState::spawn(CoreConfig::simulator(server.local_addr())).unwrap();
    let operation_id = uuid::Uuid::new_v4();

    assert_eq!(
        service.cancel(operation_id).unwrap_err().code,
        "firmwareOperationNotFound"
    );
    assert_eq!(
        service
            .retry(&app, operation_id, |_| Ok(()))
            .unwrap_err()
            .code,
        "firmwareOperationNotFound"
    );
    assert_eq!(
        service
            .rollback(&app, operation_id, |_| Ok(()))
            .unwrap_err()
            .code,
        "firmwareOperationNotFound"
    );
    drop(app);
    server.shutdown().unwrap();
}

#[test]
fn inspect_returns_only_verified_public_summary_fields() {
    let directory = tempdir().unwrap();
    let credentials = Arc::new(MemoryCredentialStore::default());
    let transport = Arc::new(RefusingTransportFactory::default());
    let service = FirmwareFlashServiceState::new(directory.path(), credentials, transport);
    let device_id = [0x19; 16];
    let signing = SigningKey::from_bytes(&[0x39; 32]);
    let key_id = "0102030405060708";
    ProvisioningStore::new(directory.path().join("trust"))
        .save(&device_id, key_id, signing.verifying_key().to_bytes())
        .unwrap();
    let package = build_signed_package(
        &FirmwareManifestInput {
            release_id: "123e4567-e89b-12d3-a456-426614174000".parse().unwrap(),
            firmware_version: [2, 3, 4],
            signing_key_id: key_id.to_owned(),
        },
        &vec![0xA5; 1024],
        &signing,
    )
    .unwrap();

    let summary = service.inspect(&device_id, &package).unwrap();

    assert_eq!(summary.firmware_version, [2, 3, 4]);
    assert_eq!(summary.image_length, 1024);
    assert_eq!(summary.signing_key_id, key_id);
    assert_eq!(summary.target, "lckfb-tmx-mspm0g3507");
    assert_eq!(summary.mcu, "MSPM0G3507");
    assert_eq!(summary.package_sha256.len(), 64);
    let serialized = serde_json::to_string(&summary).unwrap();
    assert!(!serialized.contains("packageBytes"));
    assert!(!serialized.contains("password"));
}

#[allow(dead_code)]
fn assert_firmware_serial_is_read_write<T: Read + Write + Send>() {}
