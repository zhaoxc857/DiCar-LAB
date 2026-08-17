use std::io::{Read, Write};
use std::ops::Range;

use dctp_protocol::FirmwareTargetId;

use crate::bsl::Mspm0RomBsl;

const MAIN_FLASH_END_EXCLUSIVE: u32 = 0x2_0000;
const FLASH_SECTOR_SIZE: u32 = 1_024;
const MIN_IMAGE_LENGTH: u32 = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetError {
    InvalidImageBase,
    InvalidImageLength,
    ImageOutOfBounds,
}

impl std::fmt::Display for TargetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidImageBase => "firmware image base is invalid for the target",
            Self::InvalidImageLength => "firmware image is shorter than the target minimum",
            Self::ImageOutOfBounds => "firmware image exceeds target main flash",
        })
    }
}

impl std::error::Error for TargetError {}

pub trait FirmwareTargetAdapter {
    fn target_id(&self) -> FirmwareTargetId;
    fn package_target(&self) -> &'static str;
    fn mcu(&self) -> &'static str;
    fn board_name(&self) -> &'static str;
    fn initial_baud(&self) -> u32;
    fn main_flash_bounds(&self) -> Range<u32>;
    fn flash_sector_size(&self) -> u32;
    fn validate_image(&self, base: u32, length: u32) -> Result<(), TargetError>;
    fn erase_range(&self, base: u32, length: u32) -> Result<(u32, u32), TargetError>;

    fn create_bsl<T: Read + Write>(&self, transport: T) -> Mspm0RomBsl<T> {
        Mspm0RomBsl::new(transport)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Mspm0g3507TmxAdapter;

impl FirmwareTargetAdapter for Mspm0g3507TmxAdapter {
    fn target_id(&self) -> FirmwareTargetId {
        FirmwareTargetId::LCKFB_TMX_MSPM0G3507
    }

    fn package_target(&self) -> &'static str {
        "lckfb-tmx-mspm0g3507"
    }

    fn mcu(&self) -> &'static str {
        "MSPM0G3507"
    }

    fn board_name(&self) -> &'static str {
        "立创开发板天猛星 MSPM0G3507"
    }

    fn initial_baud(&self) -> u32 {
        9_600
    }

    fn main_flash_bounds(&self) -> Range<u32> {
        0..MAIN_FLASH_END_EXCLUSIVE
    }

    fn flash_sector_size(&self) -> u32 {
        FLASH_SECTOR_SIZE
    }

    fn validate_image(&self, base: u32, length: u32) -> Result<(), TargetError> {
        if base != 0 {
            return Err(TargetError::InvalidImageBase);
        }
        if length < MIN_IMAGE_LENGTH {
            return Err(TargetError::InvalidImageLength);
        }
        let end = base
            .checked_add(length)
            .ok_or(TargetError::ImageOutOfBounds)?;
        if end > MAIN_FLASH_END_EXCLUSIVE {
            return Err(TargetError::ImageOutOfBounds);
        }
        Ok(())
    }

    fn erase_range(&self, base: u32, length: u32) -> Result<(u32, u32), TargetError> {
        self.validate_image(base, length)?;
        let last_byte = base
            .checked_add(length - 1)
            .ok_or(TargetError::ImageOutOfBounds)?;
        let last_sector_start = last_byte / FLASH_SECTOR_SIZE * FLASH_SECTOR_SIZE;
        Ok((base, last_sector_start))
    }
}
