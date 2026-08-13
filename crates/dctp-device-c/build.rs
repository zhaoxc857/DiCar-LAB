fn main() {
    let firmware_dir = "../../firmware/dctp-device";
    println!("cargo:rerun-if-changed={firmware_dir}/include/dctp_device.h");
    println!("cargo:rerun-if-changed={firmware_dir}/src/dctp_internal.h");
    println!("cargo:rerun-if-changed={firmware_dir}/src/dctp_codec.c");
    println!("cargo:rerun-if-changed={firmware_dir}/src/dctp_device.c");
    println!("cargo:rerun-if-changed=shim/dctp_test_shim.c");

    cc::Build::new()
        .include(format!("{firmware_dir}/include"))
        .include(format!("{firmware_dir}/src"))
        .file(format!("{firmware_dir}/src/dctp_codec.c"))
        .file(format!("{firmware_dir}/src/dctp_device.c"))
        .file("shim/dctp_test_shim.c")
        .flag_if_supported("/std:c11")
        .flag_if_supported("/utf-8")
        .flag_if_supported("-std=c99")
        .compile("dctp_device_c");
}
