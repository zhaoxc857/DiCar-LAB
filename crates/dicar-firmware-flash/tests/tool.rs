use std::ffi::OsString;
use std::fs;
use std::process::Command;

use ed25519_dalek::SigningKey;
use tempfile::tempdir;

use dicar_firmware_flash::package::{FirmwarePackage, TrustStore};
use dicar_firmware_flash::tool::{execute_package, parse_args, ToolCommand, ToolError};

const KEY_ID: &str = "0102030405060708";

#[test]
fn command_help_describes_both_offline_workflows_without_error() {
    let executable = env!("CARGO_BIN_EXE_dicar-firmware-tool");
    let top_level = Command::new(executable).arg("--help").output().unwrap();
    assert!(top_level.status.success());
    let top_level = String::from_utf8(top_level.stdout).unwrap();
    assert!(top_level.contains("package"));
    assert!(top_level.contains("provision-record"));

    let package = Command::new(executable)
        .args(["package", "--help"])
        .output()
        .unwrap();
    assert!(package.status.success());
    let package = String::from_utf8(package.stdout).unwrap();
    assert!(package.contains("--release-id"));
    assert!(package.contains("--signing-key-id"));
    assert!(package.contains("--output"));

    let provision = Command::new(executable)
        .args(["provision-record", "--help"])
        .output()
        .unwrap();
    assert!(provision.status.success());
    let provision = String::from_utf8(provision.stdout).unwrap();
    assert!(provision.contains("--device-id"));
    assert!(provision.contains("--recovery-package"));
    assert!(provision.contains("stdin"));
    assert!(!provision.contains("--password"));
}

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn package_arguments_are_explicit_and_no_password_option_exists() {
    let parsed = parse_args(args(&[
        "package",
        "--release-id",
        "123e4567-e89b-12d3-a456-426614174000",
        "--version",
        "1.2.3",
        "--signing-key-id",
        KEY_ID,
        "--image",
        "firmware.bin",
        "--key",
        "release.key",
        "--output",
        "release.dicarfw",
    ]))
    .unwrap();

    assert!(matches!(
        parsed,
        ToolCommand::Package {
            firmware_version: [1, 2, 3],
            ..
        }
    ));
    assert_eq!(
        parse_args(args(&[
            "provision-record",
            "--password",
            "do-not-put-secrets-in-argv"
        ])),
        Err(ToolError::InvalidArguments)
    );
}

#[test]
fn provision_arguments_name_only_public_inputs_and_storage() {
    let parsed = parse_args(args(&[
        "provision-record",
        "--device-id",
        "00112233445566778899aabbccddeeff",
        "--signing-key-id",
        KEY_ID,
        "--public-key",
        "release.pub",
        "--recovery-package",
        "known-good.dicarfw",
        "--store-dir",
        "provisioning",
    ]))
    .unwrap();

    assert!(matches!(
        parsed,
        ToolCommand::ProvisionRecord {
            device_id: [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ],
            ..
        }
    ));
}

#[test]
fn package_command_builds_a_verifiable_file_and_refuses_overwrite() {
    let directory = tempdir().unwrap();
    let image_path = directory.path().join("app.bin");
    let key_path = directory.path().join("release.key");
    let output_path = directory.path().join("release.dicarfw");
    let image = vec![0xA7; 1024];
    let signing = SigningKey::from_bytes(&[0x39; 32]);
    fs::write(&image_path, &image).unwrap();
    fs::write(&key_path, signing.to_bytes()).unwrap();
    let command = ToolCommand::Package {
        release_id: "123e4567-e89b-12d3-a456-426614174000".parse().unwrap(),
        firmware_version: [4, 5, 6],
        signing_key_id: KEY_ID.to_owned(),
        image_path,
        key_path,
        output_path: output_path.clone(),
    };

    execute_package(&command).unwrap();

    let trust =
        TrustStore::from_keys([(KEY_ID.to_owned(), signing.verifying_key().to_bytes())]).unwrap();
    let bytes = fs::read(&output_path).unwrap();
    let verified = FirmwarePackage::inspect(&bytes, &trust).unwrap();
    assert_eq!(verified.manifest().firmware_version(), [4, 5, 6]);
    assert_eq!(verified.image(), image);

    assert_eq!(execute_package(&command), Err(ToolError::OutputExists));
    assert_eq!(fs::read(output_path).unwrap(), bytes);
}
