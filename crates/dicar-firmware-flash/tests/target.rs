use std::io::Cursor;

use dctp_protocol::FirmwareTargetId;
use dicar_firmware_flash::target::{FirmwareTargetAdapter, Mspm0g3507TmxAdapter, TargetError};

#[test]
fn tianmengxing_target_identity_and_rom_bsl_link_settings_are_fixed() {
    let target = Mspm0g3507TmxAdapter;

    assert_eq!(target.target_id(), FirmwareTargetId::LCKFB_TMX_MSPM0G3507);
    assert_eq!(target.package_target(), "lckfb-tmx-mspm0g3507");
    assert_eq!(target.mcu(), "MSPM0G3507");
    assert_eq!(target.board_name(), "立创开发板天猛星 MSPM0G3507");
    assert_eq!(target.initial_baud(), 9_600);
    assert_eq!(target.main_flash_bounds(), 0..0x2_0000);
    assert_eq!(target.flash_sector_size(), 1_024);
}

#[test]
fn image_bounds_and_inclusive_sector_start_erase_range_are_validated() {
    let target = Mspm0g3507TmxAdapter;

    assert_eq!(target.validate_image(0, 1_024), Ok(()));
    assert_eq!(target.erase_range(0, 1_024), Ok((0, 0)));
    assert_eq!(target.erase_range(0, 1_025), Ok((0, 0x400)));
    assert_eq!(target.erase_range(0, 0x2_0000), Ok((0, 0x1_FC00)));
    assert_eq!(
        target.validate_image(4, 1_024),
        Err(TargetError::InvalidImageBase)
    );
    assert_eq!(
        target.validate_image(0, 1_023),
        Err(TargetError::InvalidImageLength)
    );
    assert_eq!(
        target.validate_image(0, 0x2_0001),
        Err(TargetError::ImageOutOfBounds)
    );
}

#[test]
fn adapter_constructs_the_typed_mspm0_rom_bsl_client() {
    let target = Mspm0g3507TmxAdapter;
    let bsl = target.create_bsl(Cursor::new(Vec::<u8>::new()));

    assert!(bsl.into_inner().into_inner().is_empty());
}
