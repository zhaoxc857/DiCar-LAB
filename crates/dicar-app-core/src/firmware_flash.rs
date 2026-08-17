use std::fmt;

use dctp_protocol::CapabilityFlags;

use crate::{AccessProfile, Endpoint, PermissionDecision};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FirmwareFlashStartError {
    PermissionDenied(&'static str),
    RealSerialRequired,
    DirtyParameters { count: usize },
    DeviceCapabilityMissing,
}

impl fmt::Display for FirmwareFlashStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied(reason) => formatter.write_str(reason),
            Self::RealSerialRequired => formatter.write_str("固件烧录仅支持真实串口设备"),
            Self::DirtyParameters { count } => {
                write!(formatter, "仍有 {count} 个参数未固化，不能开始固件烧录")
            }
            Self::DeviceCapabilityMissing => {
                formatter.write_str("当前设备固件未声明 PREPARE_FLASH 能力")
            }
        }
    }
}

impl std::error::Error for FirmwareFlashStartError {}

pub fn validate_firmware_flash_start(
    access: AccessProfile,
    endpoint: &Endpoint,
    capabilities: CapabilityFlags,
    dirty_count: usize,
) -> Result<(), FirmwareFlashStartError> {
    if let PermissionDecision::Denied(reason) = access.can_flash() {
        return Err(FirmwareFlashStartError::PermissionDenied(reason));
    }
    if !matches!(endpoint, Endpoint::Serial { .. }) {
        return Err(FirmwareFlashStartError::RealSerialRequired);
    }
    if dirty_count != 0 {
        return Err(FirmwareFlashStartError::DirtyParameters { count: dirty_count });
    }
    if !capabilities.contains(CapabilityFlags::PREPARE_FLASH) {
        return Err(FirmwareFlashStartError::DeviceCapabilityMissing);
    }
    Ok(())
}
