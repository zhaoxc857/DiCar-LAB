use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use dctp_protocol::{
    encode_frame, CapabilityFlags, Frame, FrameFlags, Hello, HelloAck, MessageType, ParamCommitAck,
    ParamState, ParamValue, ParamWrite, TelemetryBatch, TelemetrySample, WireEncode,
};
use sha2::{Digest, Sha256};

const VECTOR_DIRECTORY: &str = "test-vectors/dctp-v1";

struct Vector {
    file: &'static str,
    description: &'static str,
    bytes: Vec<u8>,
}

fn main() -> ExitCode {
    let check = match parse_check_argument() {
        Ok(check) => check,
        Err(error) => {
            eprintln!("generate_vectors: {error}");
            return ExitCode::FAILURE;
        }
    };
    match run(check) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("generate_vectors: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_check_argument() -> Result<bool, &'static str> {
    let mut check = false;
    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--check" => check = true,
            "--help" | "-h" => {
                println!("Usage: generate_vectors [--check]");
                std::process::exit(0);
            }
            _ => return Err("expected --check or no arguments"),
        }
    }
    Ok(check)
}

fn run(check: bool) -> Result<(), String> {
    let vectors = build_vectors().map_err(|error| format!("build failed: {error:?}"))?;
    let root = workspace_root();
    let committed_directory = root.join(VECTOR_DIRECTORY);
    if check {
        let temporary_directory = TemporaryDirectory::new()?;
        write_outputs(temporary_directory.path(), &vectors)?;
        compare_outputs(temporary_directory.path(), &committed_directory, &vectors)?;
        println!("DCTP v1 vectors match");
    } else {
        write_outputs(&committed_directory, &vectors)?;
        println!("wrote DCTP v1 vectors to {}", committed_directory.display());
    }
    Ok(())
}

fn build_vectors() -> Result<Vec<Vector>, dctp_protocol::ProtocolError> {
    let hello = Hello {
        client_nonce: 0x1020_3040,
        min_version: 1,
        max_version: 1,
        max_payload: 1_024,
    };
    let hello_ack = HelloAck {
        session_id: 0xA1B2_C3D4,
        device_id: *b"DCTP-VECTOR-0001",
        boot_count: 7,
        firmware_major: 1,
        firmware_minor: 2,
        firmware_patch: 3,
        sdk_major: 1,
        sdk_minor: 0,
        sdk_patch: 0,
        capabilities: CapabilityFlags::PARAMETERS | CapabilityFlags::TELEMETRY,
        manifest_crc32: 0x89AB_CDEF,
        max_payload: 1_024,
    };
    let write = ParamWrite {
        param_id: 1,
        expected_revision: 7,
        value: ParamValue::F32(2.5),
    };
    let parameter_state = ParamState {
        param_id: 1,
        revision: 7,
        value: ParamValue::F32(2.5),
        persisted_value: Some(ParamValue::F32(1.0)),
    };
    let commit_ack = ParamCommitAck {
        canonical_crc32: 0x1234_5678,
        storage_generation: 42,
    };
    let telemetry = TelemetryBatch {
        subscription_version: 7,
        first_sample_sequence: 42,
        dropped_samples: 3,
        base_timestamp_us: 0x1122_3344,
        samples: vec![
            TelemetrySample {
                dt_us: 0,
                values: vec![1.5f32.to_bits(), (-4i32) as u32, 8, 0b101],
            },
            TelemetrySample {
                dt_us: 2_000,
                values: vec![1.75f32.to_bits(), (-3i32) as u32, 9, 0b001],
            },
        ],
    };

    Ok(vec![
        Vector {
            file: "hello.bin",
            description: "HELLO with a fixed client nonce and protocol limits",
            bytes: encode_frame(&Frame::new(
                MessageType::Hello,
                FrameFlags::ACK_REQUIRED,
                0x1001,
                0,
                hello.encode()?,
            )?)?,
        },
        Vector {
            file: "hello-ack.bin",
            description: "HELLO_ACK with a fixed session, identity, and capabilities",
            bytes: encode_frame(&Frame::new(
                MessageType::HelloAck,
                FrameFlags::RESPONSE,
                0x1001,
                0xA1B2_C3D4,
                hello_ack.encode()?,
            )?)?,
        },
        Vector {
            file: "param-write.bin",
            description: "PARAM_WRITE for f32 PID Kp with an expected revision",
            bytes: encode_frame(&Frame::new(
                MessageType::ParamWrite,
                FrameFlags::ACK_REQUIRED,
                0x1003,
                0xA1B2_C3D4,
                write.encode()?,
            )?)?,
        },
        Vector {
            file: "param-value.bin",
            description: "PARAM_VALUE with RAM and persisted f32 values",
            bytes: encode_frame(&Frame::new(
                MessageType::ParamValue,
                FrameFlags::RESPONSE,
                0x1003,
                0xA1B2_C3D4,
                parameter_state.encode()?,
            )?)?,
        },
        Vector {
            file: "param-commit-ack.bin",
            description: "PARAM_COMMIT_ACK with canonical CRC and storage generation",
            bytes: encode_frame(&Frame::new(
                MessageType::ParamCommitAck,
                FrameFlags::RESPONSE,
                0x1003,
                0xA1B2_C3D4,
                commit_ack.encode()?,
            )?)?,
        },
        Vector {
            file: "telemetry-mixed.bin",
            description: "TELEMETRY_DATA with f32, i32, u32, and flags32 values",
            bytes: encode_frame(&Frame::new(
                MessageType::TelemetryData,
                FrameFlags::NONE,
                0x1004,
                0xA1B2_C3D4,
                telemetry.encode()?,
            )?)?,
        },
    ])
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate directory is below workspace root")
        .to_path_buf()
}

fn write_outputs(directory: &Path, vectors: &[Vector]) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| format!("create output directory: {error}"))?;
    for vector in vectors {
        fs::write(directory.join(vector.file), &vector.bytes)
            .map_err(|error| format!("write {}: {error}", vector.file))?;
    }
    fs::write(directory.join("manifest.json"), manifest_text(vectors))
        .map_err(|error| format!("write manifest.json: {error}"))?;
    Ok(())
}

fn compare_outputs(
    generated_directory: &Path,
    committed_directory: &Path,
    vectors: &[Vector],
) -> Result<(), String> {
    for vector in vectors {
        if fs::read(generated_directory.join(vector.file)).ok()
            != fs::read(committed_directory.join(vector.file)).ok()
        {
            return Err(format!("vectors differ: {}", vector.file));
        }
    }
    let generated_manifest = fs::read_to_string(generated_directory.join("manifest.json"))
        .map_err(|error| format!("read generated manifest: {error}"))?;
    let committed_manifest = fs::read_to_string(committed_directory.join("manifest.json"))
        .map_err(|_| "vectors differ: manifest.json".to_owned())?;
    if normalize_lf(&generated_manifest) != normalize_lf(&committed_manifest) {
        return Err("vectors differ: manifest.json".to_owned());
    }
    Ok(())
}

fn manifest_text(vectors: &[Vector]) -> String {
    let mut text = String::from("{\n  \"protocol_version\": 1,\n  \"vectors\": [\n");
    for (index, vector) in vectors.iter().enumerate() {
        let separator = if index + 1 == vectors.len() { "" } else { "," };
        text.push_str(&format!(
            "    {{\n      \"file\": \"{}\",\n      \"description\": \"{}\",\n      \"byte_length\": {},\n      \"sha256\": \"{}\"\n    }}{}\n",
            vector.file,
            vector.description,
            vector.bytes.len(),
            sha256_hex(&vector.bytes),
            separator
        ));
    }
    text.push_str("  ]\n}\n");
    text
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

static TEMPORARY_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Result<Self, String> {
        for _ in 0..100 {
            let counter = TEMPORARY_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("dctp-v1-vectors-{}-{counter}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("create temporary directory: {error}")),
            }
        }
        Err("create temporary directory: exhausted unique names".to_owned())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::build_vectors;

    #[test]
    fn fixed_messages_produce_the_six_named_vectors() {
        let vectors = build_vectors().unwrap();
        assert_eq!(
            vectors.iter().map(|vector| vector.file).collect::<Vec<_>>(),
            [
                "hello.bin",
                "hello-ack.bin",
                "param-write.bin",
                "param-value.bin",
                "param-commit-ack.bin",
                "telemetry-mixed.bin",
            ]
        );
        assert!(vectors.iter().all(|vector| vector.bytes.last() == Some(&0)));
    }
}
