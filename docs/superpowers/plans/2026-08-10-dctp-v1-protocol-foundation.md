# DCTP v1 Protocol Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a tested Rust implementation of the DCTP v1 wire protocol plus a deterministic vehicle simulator and shared golden vectors that the future C SDK, Tauri client, and collaboration services can consume.

**Architecture:** A pure `dctp-protocol` crate owns wire constants, codecs, payload models, and bounded stream parsing without serial-port or UI dependencies. A separate `dctp-sim` crate owns device session state, parameter behavior, priority queues, fault injection, and a TCP test transport. Binary golden vectors lock Rust and future C implementations to the same byte representation.

**Tech Stack:** Rust stable, Rust 2021 edition, Cargo workspace, standard library, `proptest` for parser properties, `serde`/`serde_json` only in vector tooling, and GitHub Actions for repeatable checks.

## Global Constraints

- Follow `docs/superpowers/specs/2026-08-10-dicar-serial-collaboration-protocol-design.md`; DCTP protocol version is exactly `1`.
- All multibyte integers and IEEE 754 `float32` values use little-endian byte order.
- Decode a 13-byte fixed header, allow at most 1024 payload bytes, apply COBS, and terminate every wire packet with `0x00`.
- Use CRC-16/CCITT-FALSE for frames: polynomial `0x1021`, initial value `0xFFFF`, no reflection, output XOR `0x0000`.
- Use CRC-32/ISO-HDLC for manifests and canonical parameter sets: polynomial `0x04C11DB7`, initial value `0xFFFFFFFF`, reflected input/output, output XOR `0xFFFFFFFF`.
- Never use `unsafe`; reject lengths before allocating or copying payload data.
- Keep all receive and transmit buffers bounded; P0/P1 traffic must not wait behind telemetry or logs.
- Heartbeat interval is 500 ms and session expiration is 3000 ms.
- Telemetry supports `f32`, `i32`, `u32`, and `flags32`, with at most 8 active channels, 16 samples per batch, and 1024 decoded payload bytes.
- Do not include serial-port access, the C11 vehicle SDK, Tauri UI, cloud accounts, WebSocket relay, or CMSIS-DAP flashing in this plan; each is an independently testable follow-on plan.

---

## Planned File Structure

```text
Cargo.toml                              # Workspace membership and shared package policy
rust-toolchain.toml                     # Stable toolchain, rustfmt, and clippy
crates/dctp-protocol/
  Cargo.toml                            # Dependency-light protocol crate
  src/lib.rs                            # Public API and module exports
  src/error.rs                          # ProtocolError shared by all codecs
  src/frame.rs                          # Header, flags, message types, and Frame
  src/checksum.rs                       # CRC16 and CRC32 implementations
  src/cobs.rs                           # COBS encode/decode
  src/codec.rs                          # Raw frame and wire packet codec
  src/stream.rs                         # Bounded incremental 0x00 stream parser
  src/wire.rs                           # Checked little-endian reader/writer
  src/messages.rs                       # Handshake, heartbeat, error, and manifest payloads
  src/parameter.rs                      # Parameter descriptors, values, writes, and canonical CRC
  src/telemetry.rs                      # Telemetry manifest and batch payloads
  src/manifest.rs                       # Canonical parameter/telemetry descriptor container
  src/log.rs                            # Structured log payload
  tests/frame_layout.rs                 # Constants and header layout
  tests/checksum_cobs.rs                # Published checksum and COBS vectors
  tests/codec_roundtrip.rs              # Frame encode/decode and corruption behavior
  tests/stream_recovery.rs              # Chunking, noise, overflow, and resynchronization
  tests/messages.rs                     # Payload limits and manifest assembly
  tests/parameter.rs                    # Typed values and canonical CRC ordering
  tests/telemetry.rs                    # Mixed types and batch limits
crates/dctp-sim/
  Cargo.toml                            # Simulator dependencies and binaries
  src/lib.rs                            # Simulator public API
  src/device.rs                         # Session, parameter store, and message dispatch
  src/request_cache.rs                  # 32-entry idempotency cache
  src/priority_queue.rs                 # Bounded P0-P3 queues and drop counters
  src/fault.rs                          # Deterministic drop/corrupt/duplicate rules
  src/main.rs                           # TCP simulator executable
  src/bin/generate_vectors.rs           # Reproducible golden-vector generator
  tests/session_flow.rs                 # Handshake, stale session, duplicate writes
  tests/priority_queue.rs               # Congestion policy
  tests/e2e_wire.rs                     # Full wire-level exchange and injected faults
test-vectors/dctp-v1/
  manifest.json                         # Vector names, protocol version, and SHA-256 values
  hello.bin                             # Encoded HELLO including 0x00 delimiter
  hello-ack.bin                         # Encoded HELLO_ACK
  param-write.bin                       # Encoded typed write
  telemetry-mixed.bin                   # Encoded mixed-type telemetry batch
.github/workflows/ci.yml                # Formatting, linting, tests, and vector drift check
README.md                               # Workspace purpose and developer commands
```

### Task 1: Bootstrap the workspace and lock the frame model

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `crates/dctp-protocol/Cargo.toml`
- Create: `crates/dctp-protocol/src/lib.rs`
- Create: `crates/dctp-protocol/src/error.rs`
- Create: `crates/dctp-protocol/src/frame.rs`
- Test: `crates/dctp-protocol/tests/frame_layout.rs`

**Interfaces:**
- Produces: `MessageType`, `FrameFlags`, `FrameHeader`, `Frame`, `ProtocolError`, `MAGIC`, `VERSION`, `HEADER_LEN`, `MAX_PAYLOAD_LEN`.
- Produces: `Frame::new(message_type: MessageType, flags: FrameFlags, sequence: u16, session_id: u32, payload: Vec<u8>) -> Result<Frame, ProtocolError>`.
- Consumes: only Rust standard library.

- [ ] **Step 1: Write the workspace manifests**

```toml
# Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.80"
license = "MIT OR Apache-2.0"
```

```toml
# rust-toolchain.toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
```

```toml
# crates/dctp-protocol/Cargo.toml
[package]
name = "dctp-protocol"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dev-dependencies]
proptest = "1"
```

- [ ] **Step 2: Write the failing frame-layout test**

```rust
use dctp_protocol::{Frame, FrameFlags, MessageType, HEADER_LEN, MAGIC, MAX_PAYLOAD_LEN, VERSION};

#[test]
fn frame_constants_match_dctp_v1() {
    assert_eq!(MAGIC, 0x5444);
    assert_eq!(VERSION, 1);
    assert_eq!(HEADER_LEN, 13);
    assert_eq!(MAX_PAYLOAD_LEN, 1024);
}

#[test]
fn frame_rejects_payload_over_limit() {
    let result = Frame::new(
        MessageType::Hello,
        FrameFlags::ACK_REQUIRED,
        7,
        0,
        vec![0; MAX_PAYLOAD_LEN + 1],
    );
    assert!(result.is_err());
}
```

- [ ] **Step 3: Run the test and verify the expected failure**

Run: `cargo test -p dctp-protocol --test frame_layout`

Expected: compilation fails because `dctp_protocol` exports do not exist.

- [ ] **Step 4: Implement the frame domain types**

Define every DCTP v1 message value from the spec with `#[repr(u8)]`, implement `TryFrom<u8>`, and reject unknown values with `ProtocolError::UnknownMessageType(value)`. Keep flags as a checked byte wrapper:

```rust
pub const MAGIC: u16 = 0x5444;
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 13;
pub const MAX_PAYLOAD_LEN: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameFlags(u8);

impl FrameFlags {
    pub const NONE: Self = Self(0);
    pub const ACK_REQUIRED: Self = Self(1 << 0);
    pub const RESPONSE: Self = Self(1 << 1);
    pub const ERROR: Self = Self(1 << 2);
    pub const MORE_FRAGMENTS: Self = Self(1 << 3);

    pub const fn bits(self) -> u8 { self.0 }
    pub const fn from_bits(bits: u8) -> Self { Self(bits) }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub version: u8,
    pub message_type: MessageType,
    pub flags: FrameFlags,
    pub sequence: u16,
    pub session_id: u32,
    pub payload_len: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(
        message_type: MessageType,
        flags: FrameFlags,
        sequence: u16,
        session_id: u32,
        payload: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge(payload.len()));
        }
        Ok(Self {
            header: FrameHeader {
                version: VERSION,
                message_type,
                flags,
                sequence,
                session_id,
                payload_len: payload.len() as u16,
            },
            payload,
        })
    }
}
```

Define concrete `ProtocolError` variants used by this plan: `PayloadTooLarge`, `UnknownMessageType`, `InvalidMagic`, `UnsupportedVersion`, `InvalidLength`, `Truncated`, `CrcMismatch`, `CobsMalformed`, `PacketTooLong`, `InvalidUtf8`, `StringTooLong`, `InvalidValue`, `InvalidSession`, and `RevisionConflict`.

- [ ] **Step 5: Run tests and formatting**

Run: `cargo fmt --check`

Expected: exit 0.

Run: `cargo test -p dctp-protocol --test frame_layout`

Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/dctp-protocol
git commit -m "feat(protocol): define DCTP v1 frame model"
```

### Task 2: Implement checksums, COBS, and complete-frame encoding

**Files:**
- Create: `crates/dctp-protocol/src/checksum.rs`
- Create: `crates/dctp-protocol/src/cobs.rs`
- Create: `crates/dctp-protocol/src/codec.rs`
- Modify: `crates/dctp-protocol/src/lib.rs`
- Test: `crates/dctp-protocol/tests/checksum_cobs.rs`
- Test: `crates/dctp-protocol/tests/codec_roundtrip.rs`

**Interfaces:**
- Produces: `crc16_ccitt_false(bytes: &[u8]) -> u16`.
- Produces: `crc32_iso_hdlc(bytes: &[u8]) -> u32`.
- Produces: `cobs_encode(input: &[u8]) -> Vec<u8>` and `cobs_decode(input: &[u8]) -> Result<Vec<u8>, ProtocolError>`.
- Produces: `encode_frame(frame: &Frame) -> Result<Vec<u8>, ProtocolError>` including the trailing `0x00`.
- Produces: `decode_packet(encoded_without_delimiter: &[u8]) -> Result<Frame, ProtocolError>`.
- Consumes: Task 1 frame and error types.

- [ ] **Step 1: Write published-vector tests before implementation**

```rust
use dctp_protocol::{cobs_decode, cobs_encode, crc16_ccitt_false, crc32_iso_hdlc};

#[test]
fn checksum_check_values_match_the_spec() {
    assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    assert_eq!(crc32_iso_hdlc(b"123456789"), 0xCBF4_3926);
}

#[test]
fn cobs_known_vector_round_trips() {
    let raw = [0x11, 0x00, 0x22];
    let encoded = vec![0x02, 0x11, 0x02, 0x22];
    assert_eq!(cobs_encode(&raw), encoded);
    assert_eq!(cobs_decode(&encoded).unwrap(), raw);
}
```

- [ ] **Step 2: Run checksum/COBS tests and verify failure**

Run: `cargo test -p dctp-protocol --test checksum_cobs`

Expected: compilation fails because checksum and COBS functions are missing.

- [ ] **Step 3: Implement both checksum algorithms**

```rust
pub fn crc16_ccitt_false(bytes: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &byte in bytes {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

pub fn crc32_iso_hdlc(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    crc ^ 0xFFFF_FFFF
}
```

Implement standard COBS with these rejection rules: an empty encoded packet is malformed; a zero code byte is malformed; a code byte cannot advance past the encoded packet; decoded output may not exceed `HEADER_LEN + MAX_PAYLOAD_LEN + 2`.

- [ ] **Step 4: Write frame round-trip and corruption tests**

```rust
use dctp_protocol::{decode_packet, encode_frame, Frame, FrameFlags, MessageType, ProtocolError};

#[test]
fn encoded_frame_has_delimiter_and_round_trips() {
    let frame = Frame::new(MessageType::Hello, FrameFlags::ACK_REQUIRED, 9, 0, vec![1, 0, 2]).unwrap();
    let wire = encode_frame(&frame).unwrap();
    assert_eq!(wire.last(), Some(&0));
    assert_eq!(decode_packet(&wire[..wire.len() - 1]).unwrap(), frame);
}

#[test]
fn corrupted_packet_is_rejected() {
    let frame = Frame::new(MessageType::Heartbeat, FrameFlags::ACK_REQUIRED, 10, 77, vec![]).unwrap();
    let mut wire = encode_frame(&frame).unwrap();
    wire[4] ^= 0x40;
    assert!(matches!(decode_packet(&wire[..wire.len() - 1]), Err(ProtocolError::CrcMismatch)));
}
```

- [ ] **Step 5: Implement raw-header serialization and frame decoding**

`encode_frame` must append fields in this exact order before CRC and COBS:

```rust
raw.extend_from_slice(&MAGIC.to_le_bytes());
raw.push(frame.header.version);
raw.push(frame.header.message_type as u8);
raw.push(frame.header.flags.bits());
raw.extend_from_slice(&frame.header.sequence.to_le_bytes());
raw.extend_from_slice(&frame.header.session_id.to_le_bytes());
raw.extend_from_slice(&frame.header.payload_len.to_le_bytes());
raw.extend_from_slice(&frame.payload);
let crc = crc16_ccitt_false(&raw);
raw.extend_from_slice(&crc.to_le_bytes());
```

`decode_packet` must COBS-decode first, require at least `HEADER_LEN + 2` bytes, verify Magic and Version, reject payload lengths over 1024, require exact length equality, verify CRC, convert Message Type, then construct `Frame`. It must not use struct transmutation or unchecked slicing.

- [ ] **Step 6: Run targeted and crate tests**

Run: `cargo test -p dctp-protocol --test checksum_cobs --test codec_roundtrip`

Expected: 4 tests pass.

Run: `cargo clippy -p dctp-protocol --all-targets -- -D warnings`

Expected: exit 0 with no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/dctp-protocol
git commit -m "feat(protocol): add COBS and checksum codec"
```

### Task 3: Add the bounded incremental stream decoder

**Files:**
- Create: `crates/dctp-protocol/src/stream.rs`
- Modify: `crates/dctp-protocol/src/lib.rs`
- Test: `crates/dctp-protocol/tests/stream_recovery.rs`

**Interfaces:**
- Produces: `StreamDecoder::new() -> StreamDecoder`.
- Produces: `StreamDecoder::push(&mut self, bytes: &[u8]) -> Vec<Result<Frame, ProtocolError>>`.
- Produces: `StreamDecoder::reset(&mut self)` and `StreamStats { decoded, malformed, overflow }`.
- Produces: public `MAX_ENCODED_PACKET_LEN: usize` and diagnostic `StreamDecoder::buffered_len() -> usize`.
- Consumes: `decode_packet`, `Frame`, and `ProtocolError` from Tasks 1–2.

- [ ] **Step 1: Write split-packet, noise, and overflow tests**

```rust
use dctp_protocol::{encode_frame, Frame, FrameFlags, MessageType, ProtocolError, StreamDecoder};

#[test]
fn decoder_recovers_after_noise_and_chunk_boundaries() {
    let first = encode_frame(&Frame::new(MessageType::Heartbeat, FrameFlags::NONE, 1, 7, vec![]).unwrap()).unwrap();
    let second = encode_frame(&Frame::new(MessageType::Heartbeat, FrameFlags::NONE, 2, 7, vec![5]).unwrap()).unwrap();
    let mut decoder = StreamDecoder::new();
    let mut output = decoder.push(&[0x99, 0x88, 0x00]);
    output.extend(decoder.push(&first[..3]));
    output.extend(decoder.push(&first[3..]));
    output.extend(decoder.push(&second));
    assert!(output[0].is_err());
    assert_eq!(output.iter().filter(|item| item.is_ok()).count(), 2);
}

#[test]
fn overlong_packet_drops_until_next_delimiter() {
    let mut decoder = StreamDecoder::new();
    let output = decoder.push(&vec![0x55; 1100]);
    assert!(output.is_empty());
    let output = decoder.push(&[0x00]);
    assert!(matches!(output.as_slice(), [Err(ProtocolError::PacketTooLong)]));
}
```

- [ ] **Step 2: Run the stream tests and verify failure**

Run: `cargo test -p dctp-protocol --test stream_recovery`

Expected: compilation fails because `StreamDecoder` is missing.

- [ ] **Step 3: Implement the bounded parser state machine**

Use a `Vec<u8>` whose length never exceeds:

```rust
const MAX_RAW_FRAME_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN + 2;
const MAX_ENCODED_PACKET_LEN: usize =
    MAX_RAW_FRAME_LEN + (MAX_RAW_FRAME_LEN + 253) / 254;
```

For every pushed byte:

1. If currently dropping an overlong packet, ignore bytes until `0x00` and then emit one `PacketTooLong`.
2. If byte is `0x00` and the buffer is empty, ignore it.
3. If byte is `0x00` and the buffer is nonempty, call `decode_packet`, clear the buffer, update statistics, and emit the result.
4. Otherwise append the byte if below the bound; on the first excess byte, clear the buffer and enter dropping mode.

- [ ] **Step 4: Add parser property tests**

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn arbitrary_input_never_grows_the_buffer_without_bound(data in proptest::collection::vec(any::<u8>(), 0..20_000)) {
        let mut decoder = StreamDecoder::new();
        let _ = decoder.push(&data);
        assert!(decoder.buffered_len() <= dctp_protocol::MAX_ENCODED_PACKET_LEN);
    }
}
```

Expose `buffered_len()` only as a diagnostic getter; do not expose the internal vector.

- [ ] **Step 5: Run recovery and property tests**

Run: `cargo test -p dctp-protocol --test stream_recovery`

Expected: all tests pass, including the proptest case.

- [ ] **Step 6: Commit**

```bash
git add crates/dctp-protocol
git commit -m "feat(protocol): add bounded stream recovery"
```

### Task 4: Implement checked payload primitives, handshake, and Manifest assembly

**Files:**
- Create: `crates/dctp-protocol/src/wire.rs`
- Create: `crates/dctp-protocol/src/messages.rs`
- Modify: `crates/dctp-protocol/src/lib.rs`
- Test: `crates/dctp-protocol/tests/messages.rs`

**Interfaces:**
- Produces: `WireWriter` methods `put_u8`, `put_u16`, `put_u32`, `put_f32`, `put_bytes`, and `put_utf8_u8_len`.
- Produces: `WireReader` checked methods `read_u8`, `read_u16`, `read_u32`, `read_f32`, `read_exact`, `read_utf8_u8_len`, and `finish`.
- Produces: `Hello`, `HelloAck`, `Heartbeat`, `ErrorPayload`, `ManifestChunk`, `ManifestDone`, and `ManifestAssembler`.
- Consumes: checksum functions and protocol errors from Tasks 1–2.

- [ ] **Step 1: Write reader-boundary and handshake round-trip tests**

```rust
use dctp_protocol::{Hello, ProtocolError, WireDecode, WireEncode, WireReader};

#[test]
fn hello_payload_round_trips() {
    let hello = Hello { client_nonce: 0x1122_3344, min_version: 1, max_version: 1, max_payload: 1024 };
    let bytes = hello.encode().unwrap();
    assert_eq!(Hello::decode(&bytes).unwrap(), hello);
}

#[test]
fn reader_rejects_trailing_and_truncated_data() {
    assert!(matches!(Hello::decode(&[1, 2]), Err(ProtocolError::Truncated)));
    let hello = Hello { client_nonce: 1, min_version: 1, max_version: 1, max_payload: 1024 };
    let mut bytes = hello.encode().unwrap();
    bytes.push(9);
    assert!(matches!(Hello::decode(&bytes), Err(ProtocolError::InvalidLength)));
}
```

- [ ] **Step 2: Run message tests and verify failure**

Run: `cargo test -p dctp-protocol --test messages`

Expected: compilation fails because wire traits and payload structs are missing.

- [ ] **Step 3: Implement checked wire helpers**

```rust
pub trait WireEncode {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError>;
}

pub trait WireDecode: Sized {
    fn decode(bytes: &[u8]) -> Result<Self, ProtocolError>;
}

pub struct WireReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
```

Every `WireReader` method must call one shared checked `take(len)` function using `checked_add`; `finish()` succeeds only when `offset == bytes.len()`. `put_utf8_u8_len` rejects strings over the caller-provided field limit or 255 bytes before writing the length.

- [ ] **Step 4: Implement handshake and error payloads**

Use these exact field layouts:

- `Hello`: `client_nonce: u32`, `min_version: u8`, `max_version: u8`, `max_payload: u16`.
- `HelloAck`: `session_id: u32`, `device_id: [u8; 16]`, `boot_count: u32`, firmware `major/minor/patch: u16 × 3`, SDK `major/minor/patch: u16 × 3`, `capabilities: u32`, `manifest_crc32: u32`, `max_payload: u16`.
- `Heartbeat`: `monotonic_ms: u32`.
- `ErrorPayload`: original Message Type, original Sequence, numeric error code, and a `u8`-length context of at most 64 bytes.

Assign capability bits as `PARAMETERS = 1 << 0`, `TELEMETRY = 1 << 1`, `PERSISTENCE = 1 << 2`, `STRUCTURED_LOG = 1 << 3`, and `PREPARE_FLASH = 1 << 4`. Assign error codes in spec order: `UNSUPPORTED_VERSION = 1`, `INVALID_SESSION = 2`, `UNKNOWN_MESSAGE = 3`, `INVALID_LENGTH = 4`, `INVALID_PARAM_ID = 5`, `TYPE_MISMATCH = 6`, `OUT_OF_RANGE = 7`, `READ_ONLY = 8`, `REVISION_CONFLICT = 9`, `BUSY = 10`, `QUEUE_FULL = 11`, `STORAGE_FAILED = 12`, `VERIFY_FAILED = 13`, `NOT_READY = 14`, and `INTERNAL_ERROR = 15`. Unknown numeric error codes remain decodable as `ErrorCode::Unknown(u16)` so a newer device can explain itself to an older client.

- [ ] **Step 5: Implement Manifest fragmentation and assembly**

```rust
pub struct ManifestChunk {
    pub manifest_crc32: u32,
    pub total_len: u32,
    pub offset: u32,
    pub data: Vec<u8>,
}

pub struct ManifestDone {
    pub manifest_crc32: u32,
    pub total_len: u32,
}
```

`ManifestAssembler` must require a single CRC/length pair, accept only the next contiguous offset, reject total lengths over 64 KiB, and return bytes only after `ManifestDone` matches both length and `crc32_iso_hdlc`.

- [ ] **Step 6: Run tests and lint**

Run: `cargo test -p dctp-protocol --test messages`

Expected: all tests pass.

Run: `cargo clippy -p dctp-protocol --all-targets -- -D warnings`

Expected: exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/dctp-protocol
git commit -m "feat(protocol): add handshake and manifest payloads"
```

### Task 5: Implement typed parameter messages and canonical value CRC

**Files:**
- Create: `crates/dctp-protocol/src/parameter.rs`
- Modify: `crates/dctp-protocol/src/lib.rs`
- Test: `crates/dctp-protocol/tests/parameter.rs`

**Interfaces:**
- Produces: `ParamType`, `ParamValue`, `ParamFlags`, `ParamDescriptor`, `ParamState`.
- Produces: `ParamRead`, `ParamWrite`, `ParamWriteAck`, `ParamCommit`, and `ParamCommitAck` wire payloads.
- Produces: `canonical_parameter_crc32(entries: &[(u32, ParamValue)]) -> u32`.
- Consumes: checked wire helpers and CRC32 from prior tasks.

- [ ] **Step 1: Write typed-value and ordering tests**

```rust
use dctp_protocol::{canonical_parameter_crc32, ParamValue, ParamWrite, WireDecode, WireEncode};

#[test]
fn typed_write_round_trips_with_expected_revision() {
    let write = ParamWrite { param_id: 42, expected_revision: 7, value: ParamValue::F32(1.25) };
    assert_eq!(ParamWrite::decode(&write.encode().unwrap()).unwrap(), write);
}

#[test]
fn canonical_crc_is_independent_of_input_order() {
    let a = vec![(20, ParamValue::U32(4)), (10, ParamValue::I32(-2))];
    let b = vec![(10, ParamValue::I32(-2)), (20, ParamValue::U32(4))];
    assert_eq!(canonical_parameter_crc32(&a), canonical_parameter_crc32(&b));
}
```

- [ ] **Step 2: Run parameter tests and verify failure**

Run: `cargo test -p dctp-protocol --test parameter`

Expected: compilation fails because parameter types are missing.

- [ ] **Step 3: Implement the parameter value representation**

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    I32(i32),
    U32(u32),
    F32(f32),
    Bool(bool),
    Enum(i32),
}

impl ParamValue {
    pub fn param_type(&self) -> ParamType;
    pub fn encode_canonical(&self, out: &mut Vec<u8>);
}
```

Assign stable one-byte tags to `ParamType`; decode Bool only from `0` or `1`; preserve `f32::to_bits()` exactly. `ParamDescriptor` enforces byte limits from the spec: machine name 48, display name 64, group 32, and unit 16.

Use exact tags `I32 = 1`, `U32 = 2`, `F32 = 3`, `Bool = 4`, and `Enum = 5`. Use parameter flag bits `WRITABLE = 1 << 0`, `PERSISTENT = 1 << 1`, and `DANGEROUS = 1 << 2`; absence of `WRITABLE` means read-only. A descriptor encodes `param_id`, type, flags, four length-prefixed strings, one tagged default value, and one constraint union:

```rust
pub enum ParamConstraints {
    None,
    Numeric { min: ParamValue, max: ParamValue, step: ParamValue },
    Enum { options: Vec<EnumOption> },
}

pub struct EnumOption {
    pub value: i32,
    pub label: String,
}
```

`None = 0`, `Numeric = 1`, and `Enum = 2`. Numeric constraint values must match the descriptor type. Enum descriptors allow at most 32 options, each label at most 32 UTF-8 bytes, and unique numeric values.

- [ ] **Step 4: Implement parameter messages and canonical CRC**

Sort a temporary vector by `param_id`, reject duplicate IDs, and hash exactly:

```rust
canonical.extend_from_slice(&param_id.to_le_bytes());
canonical.push(value.param_type() as u8);
value.encode_canonical(&mut canonical);
```

`ParamCommit` carries sorted `(param_id, revision)` pairs plus the canonical CRC. `ParamWriteAck` carries the actual accepted value and new Revision. Error responses transport current value and Revision in `ErrorPayload` context for `REVISION_CONFLICT`.

- [ ] **Step 5: Add descriptor-limit and float-bit tests**

Test that a 49-byte machine name is rejected, duplicate IDs are rejected, `-0.0f32` retains its sign bit, and two distinct NaN payloads remain distinct on the wire.

- [ ] **Step 6: Run parameter and full protocol tests**

Run: `cargo test -p dctp-protocol --test parameter`

Expected: all tests pass.

Run: `cargo test -p dctp-protocol`

Expected: all protocol tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/dctp-protocol
git commit -m "feat(protocol): add typed parameter payloads"
```

### Task 6: Implement mixed-type telemetry and structured logs

**Files:**
- Create: `crates/dctp-protocol/src/telemetry.rs`
- Create: `crates/dctp-protocol/src/manifest.rs`
- Create: `crates/dctp-protocol/src/log.rs`
- Modify: `crates/dctp-protocol/src/lib.rs`
- Test: `crates/dctp-protocol/tests/telemetry.rs`

**Interfaces:**
- Produces: `TelemetryType`, `TelemetryDescriptor`, `TelemetrySubscription`, `TelemetrySample`, `TelemetryBatch`.
- Produces: `DeviceManifest::encode_canonical()` and `DeviceManifest::decode()`.
- Produces: `LogSeverity` and `LogMessage`.
- Produces: `TelemetryBatch::encode()` and `TelemetryBatch::decode(bytes, expected_channel_count)`.
- Consumes: checked wire helpers and protocol errors.

- [ ] **Step 1: Write maximum-batch and mixed-value tests**

```rust
use dctp_protocol::{TelemetryBatch, TelemetrySample, WireDecode, WireEncode};

#[test]
fn mixed_telemetry_batch_round_trips() {
    let batch = TelemetryBatch {
        subscription_version: 3,
        first_sample_sequence: 99,
        dropped_samples: 2,
        base_timestamp_us: 1_000_000,
        samples: vec![
            TelemetrySample { dt_us: 0, values: vec![1.5f32.to_bits(), (-4i32) as u32, 8, 0b101] },
            TelemetrySample { dt_us: 2_000, values: vec![1.75f32.to_bits(), (-3i32) as u32, 9, 0b001] },
        ],
    };
    let bytes = batch.encode().unwrap();
    assert_eq!(TelemetryBatch::decode(&bytes, 4).unwrap(), batch);
}
```

- [ ] **Step 2: Run telemetry tests and verify failure**

Run: `cargo test -p dctp-protocol --test telemetry`

Expected: compilation fails because telemetry types are missing.

- [ ] **Step 3: Implement telemetry descriptors and batches**

Encode the fixed prefix as `subscription_version`, `first_sample_sequence`, `sample_count`, `channel_count`, `dropped_samples`, and `base_timestamp_us`. Encode each sample as `dt_us` plus exactly `channel_count` little-endian `u32` slots.

Assign telemetry type tags `F32 = 1`, `I32 = 2`, `U32 = 3`, and `Flags32 = 4`. `TelemetryDescriptor` encodes `channel_id: u32`, the type tag, machine name, display name, group, and unit using the same string limits as parameters. `TelemetrySubscription` encodes `subscription_version: u16`, `sample_rate_hz: u16`, `channel_count: u8`, followed by unique `channel_id: u32` values; reject more than 8 IDs or `sample_rate_hz > 500`.

Reject:

- zero samples or more than 16 samples;
- zero channels or more than 8 channels;
- any sample whose value count differs from the prefix;
- a first sample whose `dt_us` is not zero;
- decoded bytes remaining after the expected samples;
- an encoded payload over 1024 bytes.

- [ ] **Step 4: Implement structured logs**

```rust
pub struct LogMessage {
    pub timestamp_us: u32,
    pub severity: LogSeverity,
    pub module_id: u16,
    pub text: String,
}
```

Assign severity tags `Trace = 0`, `Debug = 1`, `Info = 2`, `Warn = 3`, and `Error = 4`. Encode text using `u8` length and reject more than 192 UTF-8 bytes. Decode only known severity values and valid UTF-8.

- [ ] **Step 5: Implement the canonical Device Manifest**

```rust
pub struct DeviceManifest {
    pub schema_version: u16,
    pub parameters: Vec<ParamDescriptor>,
    pub telemetry: Vec<TelemetryDescriptor>,
}
```

Canonical encoding is `schema_version = 1`, `parameter_count: u16`, `telemetry_count: u16`, parameter records sorted by `param_id`, then telemetry records sorted by `channel_id`. Prefix every record with `record_len: u16`. Reject duplicate IDs, more than 64 parameters, more than 16 telemetry descriptors, a record whose decoder does not consume exactly `record_len`, and total encoded size over 64 KiB. `manifest_crc32` is `crc32_iso_hdlc(DeviceManifest::encode_canonical())`.

- [ ] **Step 6: Add all limit tests**

Test 8 channels × 16 samples, 9-channel rejection, 17-sample rejection, inconsistent sample width, a 193-byte log, invalid UTF-8, duplicate Manifest IDs, noncanonical input ordering normalized on encode, and Manifest CRC drift after one descriptor changes.

- [ ] **Step 7: Run telemetry and protocol tests**

Run: `cargo test -p dctp-protocol --test telemetry`

Expected: all tests pass.

Run: `cargo test -p dctp-protocol`

Expected: all protocol tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/dctp-protocol
git commit -m "feat(protocol): add telemetry and log payloads"
```

### Task 7: Build the deterministic vehicle simulator and priority queues

**Files:**
- Create: `crates/dctp-sim/Cargo.toml`
- Create: `crates/dctp-sim/src/lib.rs`
- Create: `crates/dctp-sim/src/device.rs`
- Create: `crates/dctp-sim/src/request_cache.rs`
- Create: `crates/dctp-sim/src/priority_queue.rs`
- Create: `crates/dctp-sim/src/fault.rs`
- Create: `crates/dctp-sim/src/main.rs`
- Test: `crates/dctp-sim/tests/session_flow.rs`
- Test: `crates/dctp-sim/tests/priority_queue.rs`

**Interfaces:**
- Produces: `SimDevice::new(config: SimConfig) -> SimDevice`.
- Produces: `SimDevice::handle(&mut self, request: Frame, now_ms: u64) -> Vec<QueuedFrame>`.
- Produces: `SimDevice::tick(&mut self, now_ms: u64) -> Vec<QueuedFrame>`.
- Produces: `SimDevice::open_session(&mut self, client_nonce: u32, now_ms: u64) -> Result<u32, ProtocolError>`.
- Produces: `SimDevice::validate_session(&self, session_id: u32) -> Result<(), ProtocolError>` and `SimDevice::parameter_revision(&self, param_id: u32) -> Option<u32>`.
- Produces: `RequestCache::get_or_insert(key: RequestKey, build: impl FnOnce() -> Frame) -> Frame` with capacity 32.
- Produces: `PriorityTxQueue::push(priority: Priority, frame: Frame) -> PushOutcome` and `pop() -> Option<Frame>`.
- Produces: `QueuedFrame { priority: Priority, frame: Frame }`; `SimConfig::default()` installs a fixed Manifest with writable PID `f32` parameter ID 1 and encoder calibration parameters.
- Consumes: all protocol models from Tasks 1–6.

- [ ] **Step 1: Create the simulator manifest**

```toml
[package]
name = "dctp-sim"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
dctp-protocol = { path = "../dctp-protocol" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"

[dev-dependencies]
proptest = "1"
```

- [ ] **Step 2: Write session and duplicate-write tests**

```rust
use dctp_protocol::{Frame, FrameFlags, MessageType, ParamValue, ParamWrite, WireEncode};
use dctp_sim::{SimConfig, SimDevice};

#[test]
fn new_hello_invalidates_the_previous_session() {
    let mut device = SimDevice::new(SimConfig::default());
    let first = device.open_session(11, 0).unwrap();
    let second = device.open_session(22, 10).unwrap();
    assert_ne!(first, second);
    assert!(device.validate_session(first).is_err());
    assert!(device.validate_session(second).is_ok());
}

#[test]
fn duplicate_parameter_write_returns_cached_ack_once() {
    let mut device = SimDevice::new(SimConfig::default());
    let session = device.open_session(11, 0).unwrap();
    let payload = ParamWrite { param_id: 1, expected_revision: 0, value: ParamValue::F32(2.0) }.encode().unwrap();
    let request = Frame::new(MessageType::ParamWrite, FrameFlags::ACK_REQUIRED, 5, session, payload).unwrap();
    let first = device.handle(request.clone(), 1);
    let second = device.handle(request, 2);
    assert_eq!(first, second);
    assert_eq!(device.parameter_revision(1), Some(1));
}
```

- [ ] **Step 3: Run session tests and verify failure**

Run: `cargo test -p dctp-sim --test session_flow`

Expected: compilation fails because simulator types are missing.

- [ ] **Step 4: Implement session and parameter behavior**

`SimDevice` must:

- generate a nonzero Session ID from `client_nonce`, boot count, and an incrementing session counter;
- invalidate the old session when accepting `HELLO`;
- reject every non-HELLO request with the wrong Session ID;
- expose a fixed test Manifest containing PID and encoder parameters;
- apply `PARAM_WRITE` only when ID, type, range, and expected Revision match;
- increment Revision exactly once and return cached responses for duplicates;
- expire the session when `now_ms - last_valid_frame_ms >= 3000`;
- keep RAM values unchanged when the session expires.

- [ ] **Step 5: Implement the fixed 32-entry request cache**

Use `VecDeque<(RequestKey, Frame)>`; update no entry on lookup, append new results, and evict only the oldest entry when inserting the 33rd result. `RequestKey` contains Session ID, Message Type, and Sequence.

```rust
pub struct RequestKey {
    pub session_id: u32,
    pub message_type: MessageType,
    pub sequence: u16,
}
```

- [ ] **Step 6: Write and implement priority queue tests**

```rust
fn frame(sequence: u16) -> Frame {
    Frame::new(MessageType::Heartbeat, FrameFlags::NONE, sequence, 1, vec![]).unwrap()
}

#[test]
fn control_frames_precede_telemetry_and_logs() {
    let mut queue = PriorityTxQueue::with_capacities([4, 4, 2, 2]);
    queue.push(Priority::Log, frame(1));
    queue.push(Priority::Telemetry, frame(2));
    queue.push(Priority::Reliable, frame(3));
    queue.push(Priority::Safety, frame(4));
    assert_eq!(queue.pop().unwrap().header.sequence, 4);
    assert_eq!(queue.pop().unwrap().header.sequence, 3);
}
```

When a P3 queue is full, drop the new log and increment `dropped_logs`. When P2 is full, drop the oldest complete telemetry frame and increment `dropped_telemetry`. P0/P1 insertion into a full own queue returns `PushOutcome::Backpressure`; it must never evict another reliable frame silently.

Use these exact enums:

```rust
pub enum Priority { Safety, Reliable, Telemetry, Log }
pub enum PushOutcome { Enqueued, DroppedTelemetry, DroppedLog, Backpressure }
```

- [ ] **Step 7: Implement deterministic fault rules**

```rust
pub enum Direction { HostToDevice, DeviceToHost }
pub enum FaultAction { Pass, Drop, Duplicate, CorruptByte { offset: usize, mask: u8 } }

pub struct FaultRule {
    pub direction: Direction,
    pub packet_index: u64,
    pub action: FaultAction,
}
```

Apply rules by packet index, never random global state, so failing tests reproduce with the same configuration.

- [ ] **Step 8: Add the TCP executable**

`dctp-sim --listen 127.0.0.1:7100` accepts one TCP client, feeds bytes to `StreamDecoder`, passes decoded frames to `SimDevice`, drains `PriorityTxQueue`, and writes `encode_frame` output. A second concurrent client receives a clear connection rejection and cannot create another session.

- [ ] **Step 9: Run simulator tests**

Run: `cargo test -p dctp-sim --test session_flow --test priority_queue`

Expected: all tests pass.

Run: `cargo clippy -p dctp-sim --all-targets -- -D warnings`

Expected: exit 0.

- [ ] **Step 10: Commit**

```bash
git add crates/dctp-sim
git commit -m "feat(sim): add deterministic DCTP vehicle simulator"
```

### Task 8: Lock golden vectors, end-to-end faults, CI, and developer commands

**Files:**
- Create: `crates/dctp-sim/src/bin/generate_vectors.rs`
- Create: `crates/dctp-sim/tests/e2e_wire.rs`
- Create: `test-vectors/dctp-v1/manifest.json`
- Create: `test-vectors/dctp-v1/hello.bin`
- Create: `test-vectors/dctp-v1/hello-ack.bin`
- Create: `test-vectors/dctp-v1/param-write.bin`
- Create: `test-vectors/dctp-v1/telemetry-mixed.bin`
- Create: `.github/workflows/ci.yml`
- Create: `README.md`

**Interfaces:**
- Produces: stable DCTP v1 byte vectors consumed by the future C SDK and other languages.
- Produces: `cargo run -p dctp-sim --bin generate_vectors -- --check` for drift detection.
- Produces: test-only `WireHarness::new()`, `hello(client_nonce)`, `heartbeat(session_id)`, `write_f32(session_id, param_id, expected_revision, value)`, and `inject_corrupt_next_device_packet(offset, mask)`.
- Consumes: codec and simulator APIs from all prior tasks.

- [ ] **Step 1: Write the failing end-to-end test**

```rust
#[test]
fn wire_session_survives_corruption_and_rejects_stale_write() {
    let mut harness = WireHarness::new();
    let session_a = harness.hello(0xAAAA).unwrap();
    harness.inject_corrupt_next_device_packet(3, 0x20);
    assert!(harness.heartbeat(session_a).is_err());
    assert!(harness.heartbeat(session_a).is_ok());
    let session_b = harness.hello(0xBBBB).unwrap();
    assert_ne!(session_a, session_b);
    assert!(harness.write_f32(session_a, 1, 0, 2.0).is_err());
    assert!(harness.write_f32(session_b, 1, 0, 2.0).is_ok());
}
```

`WireHarness` must always pass frames through `encode_frame`, optional fault rules, chunked byte delivery, `StreamDecoder`, `SimDevice`, and the reverse codec. It cannot call message handlers with already-decoded payloads.

- [ ] **Step 2: Run the end-to-end test and verify failure**

Run: `cargo test -p dctp-sim --test e2e_wire`

Expected: compilation fails because `WireHarness` is missing.

- [ ] **Step 3: Implement the wire harness and scenarios**

Add tests for:

- HELLO → Manifest → parameter read → typed RAM write → telemetry subscription;
- corrupted CRC followed by successful resynchronization;
- duplicate `PARAM_WRITE` with one Revision increment;
- Session expiration after exactly 3000 ms and no expiration at 2999 ms;
- a full P3 log queue while heartbeat and parameter ACK still pass;
- mixed telemetry sequence gaps and dropped-sample counters.

- [ ] **Step 4: Implement reproducible vector generation**

The generator must construct fixed semantic messages, call `encode_frame`, write the four `.bin` files, calculate SHA-256 for each file, and write `manifest.json` with protocol version, semantic description, byte length, and lowercase SHA-256. Use fixed nonces, Session IDs, Sequences, timestamps, and values.

`--check` writes generated bytes into a temporary directory, compares them byte-for-byte and compares the manifest text after normalized LF line endings. It exits nonzero and prints the first differing file without modifying committed vectors.

- [ ] **Step 5: Generate and test the vectors**

Run: `cargo run -p dctp-sim --bin generate_vectors`

Expected: four `.bin` files and `manifest.json` are created under `test-vectors/dctp-v1/`.

Run: `cargo run -p dctp-sim --bin generate_vectors -- --check`

Expected: exit 0 and output `DCTP v1 vectors match`.

- [ ] **Step 6: Add CI**

```yaml
name: ci
on:
  push:
  pull_request:
jobs:
  rust:
    strategy:
      matrix:
        os: [windows-latest, ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - run: cargo run -p dctp-sim --bin generate_vectors -- --check
```

- [ ] **Step 7: Add the developer README**

Document only commands verified in this task:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p dctp-sim -- --listen 127.0.0.1:7100
cargo run -p dctp-sim --bin generate_vectors -- --check
```

Explain that `dctp-protocol` performs no serial I/O and that `dctp-sim` is the test transport for the next desktop-client plan.

- [ ] **Step 8: Run the full verification gate**

Run: `cargo fmt --check`

Expected: exit 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: exit 0 with no warnings.

Run: `cargo test --workspace`

Expected: all tests pass with zero failures.

Run: `cargo run -p dctp-sim --bin generate_vectors -- --check`

Expected: `DCTP v1 vectors match`.

Run: `git status --short`

Expected before commit: only Task 8 files are listed.

- [ ] **Step 9: Commit**

```bash
git add .github README.md crates/dctp-sim test-vectors
git commit -m "test(protocol): lock DCTP v1 vectors and CI"
```

## Plan Completion Criteria

The protocol-foundation phase is complete only when:

1. Every Task 8 verification command exits successfully.
2. The simulator rejects stale Session IDs and duplicate writes change Revision exactly once.
3. Parser property tests demonstrate bounded memory for arbitrary input.
4. The four committed vectors match regenerated output byte-for-byte.
5. A future C implementation can determine every field order, scalar width, checksum, string limit, and rejection rule without reading Rust internals.

After this phase, create separate implementation plans in dependency order for the C11 vehicle SDK, Tauri Windows client, collaboration/relay service, wireless flashing adapter, and cross-platform/AI/multi-vehicle extensions.
