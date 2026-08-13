//! C 编码路径（wire 布局 + CRC16 + COBS）与已提交黄金向量的逐字节比对。

use std::fs;
use std::path::PathBuf;

use dctp_device_c::build_golden;

const VECTOR_FILES: [&str; 6] = [
    "hello.bin",
    "hello-ack.bin",
    "param-write.bin",
    "param-value.bin",
    "param-commit-ack.bin",
    "telemetry-mixed.bin",
];

fn vector_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-vectors/dctp-v1")
        .join(file)
}

#[test]
fn c_encoders_reproduce_all_committed_golden_vectors() {
    for (which, file) in VECTOR_FILES.iter().enumerate() {
        let committed = fs::read(vector_path(file)).expect("committed vector exists");
        let generated = build_golden(which as i32);
        assert!(!generated.is_empty(), "{file}: C builder produced no bytes");
        assert_eq!(
            generated, committed,
            "{file}: C frame differs from golden vector"
        );
    }
}
