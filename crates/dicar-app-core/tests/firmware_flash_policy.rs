use std::net::SocketAddr;

use dctp_protocol::CapabilityFlags;
use dicar_app_core::{
    validate_firmware_flash_start, AccessProfile, AccessRole, Endpoint, FirmwareFlashStartError,
    LeaseState, SerialHardwareProfile,
};

fn serial_endpoint() -> Endpoint {
    Endpoint::Serial {
        port_name: "COM12".into(),
        baud_rate: 115_200,
        hardware_profile: SerialHardwareProfile::Hc05BluetoothSpp,
    }
}

fn owner() -> AccessProfile {
    AccessProfile::new(AccessRole::Owner, LeaseState::Active)
}

#[test]
fn firmware_flash_start_requires_owner_active_lease() {
    let denied = [
        AccessProfile::new(AccessRole::Observer, LeaseState::Active),
        AccessProfile::new(AccessRole::Tuner, LeaseState::Active),
        AccessProfile::new(AccessRole::Owner, LeaseState::Inactive),
    ];

    for access in denied {
        assert!(matches!(
            validate_firmware_flash_start(
                access,
                &serial_endpoint(),
                CapabilityFlags::PREPARE_FLASH,
                0,
            ),
            Err(FirmwareFlashStartError::PermissionDenied(_))
        ));
    }
}

#[test]
fn firmware_flash_start_requires_real_serial_clean_workspace_and_device_capability() {
    let simulator = Endpoint::Simulator {
        address: SocketAddr::from(([127, 0, 0, 1], 9000)),
    };

    assert_eq!(
        validate_firmware_flash_start(owner(), &simulator, CapabilityFlags::PREPARE_FLASH, 0,),
        Err(FirmwareFlashStartError::RealSerialRequired)
    );
    assert_eq!(
        validate_firmware_flash_start(
            owner(),
            &serial_endpoint(),
            CapabilityFlags::PREPARE_FLASH,
            2,
        ),
        Err(FirmwareFlashStartError::DirtyParameters { count: 2 })
    );
    assert_eq!(
        validate_firmware_flash_start(owner(), &serial_endpoint(), CapabilityFlags::PARAMETERS, 0),
        Err(FirmwareFlashStartError::DeviceCapabilityMissing)
    );
}

#[test]
fn firmware_flash_start_allows_a_clean_supported_real_serial_device() {
    assert_eq!(
        validate_firmware_flash_start(
            owner(),
            &serial_endpoint(),
            CapabilityFlags::PARAMETERS | CapabilityFlags::PREPARE_FLASH,
            0,
        ),
        Ok(())
    );
}
