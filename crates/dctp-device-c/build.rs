fn main() {
    let firmware_dir = "../../firmware/dctp-device";
    let target_dir = "../../firmware/targets/lckfb-tmx-mspm0g3507";
    println!("cargo:rerun-if-changed={firmware_dir}/include/dctp_device.h");
    println!("cargo:rerun-if-changed={firmware_dir}/src/dctp_internal.h");
    println!("cargo:rerun-if-changed={firmware_dir}/src/dctp_codec.c");
    println!("cargo:rerun-if-changed={firmware_dir}/src/dctp_device.c");
    println!("cargo:rerun-if-changed=shim/dctp_test_shim.c");
    println!("cargo:rerun-if-changed={target_dir}/include/tmx_firmware_flash.h");
    println!("cargo:rerun-if-changed={target_dir}/src/tmx_firmware_flash.c");
    println!("cargo:rerun-if-changed={target_dir}/src/tmx_mspm0_sdk_entry.c");
    println!("cargo:rerun-if-changed=shim/include/ti/driverlib/driverlib.h");
    println!("cargo:rerun-if-changed=shim/tmx_flash_test_shim.c");

    cc::Build::new()
        .include(format!("{firmware_dir}/include"))
        .include(format!("{firmware_dir}/src"))
        .include(format!("{target_dir}/include"))
        .include("shim/include")
        .file(format!("{firmware_dir}/src/dctp_codec.c"))
        .file(format!("{firmware_dir}/src/dctp_device.c"))
        .file(format!("{target_dir}/src/tmx_firmware_flash.c"))
        .file(format!("{target_dir}/src/tmx_mspm0_sdk_entry.c"))
        .file("shim/dctp_test_shim.c")
        .file("shim/tmx_flash_test_shim.c")
        .flag_if_supported("/std:c11")
        .flag_if_supported("/utf-8")
        .flag_if_supported("-std=c99")
        .compile("dctp_device_c");
}
