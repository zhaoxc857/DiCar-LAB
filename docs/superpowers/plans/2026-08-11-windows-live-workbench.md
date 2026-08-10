# DiCar Tune Windows Live Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Deliver a runnable Windows-first DiCar Tune application in which the B-style menu page connects to the real DCTP simulator and opens the A-style workbench for Manifest-driven parameter/encoder tuning, explicit persistence, and eight-channel live waveforms.

**Architecture:** A Tauri-independent Rust actor owns the one active Transport, DCTP session, parameter workspace, telemetry buffers, and diagnostics. Tauri commands send bounded CoreCommand values and one typed Tauri Channel streams ordered BridgeEvent batches to a React/Vite frontend; browser tests use the same DesktopBridge contract through a deterministic MockBridge. The stage begins by closing the persisted-value and simulator telemetry gaps in DCTP v1.

**Tech Stack:** Rust 1.80+ workspace, dctp-protocol, crossbeam-channel, serde, Tauri 2, React + TypeScript + Vite, Tailwind CSS 4 Vite plugin, shadcn/Radix primitives, Zustand, Phosphor icons, Vitest + Testing Library, Playwright + axe.

## Global Constraints

- Base all work on feature/windows-app-shell at commit cb83f5c or a descendant; never reimplement DCTP framing, COBS, CRC, payload encoding, or StreamDecoder in TypeScript.
- Preserve exactly one active physical/simulator session per selected vehicle. A second simulator TCP client remains rejected.
- DCTP payload stays bounded at the negotiated limit and never exceeds 1024 bytes. All actor queues and telemetry buffers are bounded.
- Heartbeat is 500 ms; a session expires after 3000 ms without a valid frame. Ordinary commands use 300 ms plus at most 3 retries (4 total sends), Manifest uses 500 ms plus at most 3 retries (4 total sends), and Commit uses 3000 ms plus at most 2 retries (3 total sends); every retry reuses Sequence.
- PARAM_WRITE changes RAM only. PARAM_COMMIT is the sole persistence operation and must be idempotent.
- PARAM_VALUE exposes RAM and optional Flash values. PARAM_COMMIT_ACK exposes canonical CRC32 and Storage Generation.
- Encoder PPR, quadrature multiplier, and effective CPR remain separate fields. Raw counts and calculated speed channels remain separately observable.
- A subscription contains 1–8 unique channels at 1–500 Hz. React receives batches at no more than 30 Hz; Canvas performs drawing and pixel downsampling.
- Owner/Tuner/Observer are local demo fixtures only in this stage. The backend still enforces their gates, but the UI must label them as local demonstration permissions.
- Dark engineering UI follows design-system/dicar-tune/MASTER.md and page overrides. WCAG AA contrast, visible focus, keyboard operation, reduced motion, and 200% zoom are release gates.
- Node must satisfy Vite current requirements (20.19+ or 22.12+); use the available Node 24.14.0 and pnpm 11.16.0. Commit pnpm-lock.yaml and the root Cargo.lock once app binaries exist.
- Native Tauri packaging requires the official Windows prerequisites: x86_64-pc-windows-msvc Rust, Microsoft C++ Build Tools with Desktop development with C++, Windows SDK, and WebView2. Do not claim native completion until the MSVC build passes.
- Every production behavior starts with an honest failing test, then minimal implementation, focused green, self-review, full regression, and a focused commit.
- DCTP v1 is still pre-release and has no deployed firmware compatibility obligation. This stage deliberately finalizes the v1 persisted-value/Generation schema before its first release; lock the revised bytes with new golden vectors rather than adding an ambiguous legacy decoder.
- Retain every existing DCTP behavior test and golden vector, then add the new cases. The binding baseline is the full workspace suite plus byte-for-byte vector checker, not a fragile fixed test count.

---

### Task 1: Complete persisted parameter state and realistic simulator telemetry

**Files:**
- Modify: crates/dctp-protocol/src/parameter.rs
- Modify: crates/dctp-protocol/tests/parameter.rs
- Modify: crates/dctp-sim/src/device.rs
- Modify: crates/dctp-sim/tests/session_flow.rs
- Modify: crates/dctp-sim/tests/e2e_wire.rs
- Modify: crates/dctp-sim/tests/final_review.rs
- Modify: crates/dctp-sim/src/bin/generate_vectors.rs
- Create through the generator: test-vectors/dctp-v1/param-value.bin
- Create through the generator: test-vectors/dctp-v1/param-commit-ack.bin
- Modify through the generator: test-vectors/dctp-v1/manifest.json
- Modify: README.md

**Interfaces:**
- Produces: ParamState { param_id, revision, value, persisted_value: Option<ParamValue> }.
- Produces: ParamCommitAck { canonical_crc32, storage_generation }.
- Produces: ParamValue::wire_eq() for type/tag-aware and f32 bit-exact state comparison.
- Produces: CommitFailure::Storage and CommitFailure::Verify test controls on SimDevice.
- Produces: default DeviceManifest with at least 16 telemetry descriptors and deterministic time-varying values.
- Produces: dedicated PARAM_VALUE and PARAM_COMMIT_ACK DCTP v1 golden frames.
- Consumed by: Tasks 4–11.

- [ ] **Step 1: Write the failing protocol payload tests**

Add focused tests that demand an optional persisted value and Storage Generation:

~~~rust
#[test]
fn parameter_state_round_trips_ram_and_flash_values() {
    let state = ParamState {
        param_id: 7,
        revision: 9,
        value: ParamValue::F32(1.25),
        persisted_value: Some(ParamValue::F32(1.0)),
    };
    assert_eq!(ParamState::decode(&state.encode().unwrap()).unwrap(), state);
}

#[test]
fn non_persistent_state_round_trips_without_a_flash_value() {
    let state = ParamState {
        param_id: 8,
        revision: 0,
        value: ParamValue::Bool(true),
        persisted_value: None,
    };
    assert_eq!(ParamState::decode(&state.encode().unwrap()).unwrap(), state);
}

#[test]
fn commit_ack_round_trips_crc_and_generation() {
    let ack = ParamCommitAck {
        canonical_crc32: 0x1234_5678,
        storage_generation: 42,
    };
    assert_eq!(ParamCommitAck::decode(&ack.encode().unwrap()).unwrap(), ack);
}

#[test]
fn wire_equality_distinguishes_signed_zero_and_nan_payload_bits() {
    assert!(!ParamValue::F32(-0.0).wire_eq(&ParamValue::F32(0.0)));
    let first = ParamValue::F32(f32::from_bits(0x7fc0_0001));
    let same = ParamValue::F32(f32::from_bits(0x7fc0_0001));
    let other = ParamValue::F32(f32::from_bits(0x7fc0_0002));
    assert!(first.wire_eq(&same));
    assert!(!first.wire_eq(&other));
}
~~~

- [ ] **Step 2: Run the focused tests and record RED**

Run:

~~~powershell
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu test --offline -p dctp-protocol --test parameter
~~~

Expected: compilation fails because persisted_value and storage_generation do not exist.

- [ ] **Step 3: Extend the wire types without an ambiguous fallback**

Encode a one-byte presence marker after the RAM value. Only 0 and 1 are valid:

~~~rust
pub struct ParamState {
    pub param_id: u32,
    pub revision: u32,
    pub value: ParamValue,
    pub persisted_value: Option<ParamValue>,
}

pub struct ParamCommitAck {
    pub canonical_crc32: u32,
    pub storage_generation: u32,
}
~~~

Decode must reject a marker above 1, trailing bytes, and a persisted value whose type differs from the RAM value. Implement `ParamValue::wire_eq` by comparing variant tags and f32 `to_bits()`; the other variants compare their exact payloads. This is a pre-release DCTP v1 schema correction: do not accept the old shorter payload as a fallback. Re-run the focused protocol test and require all cases green.

- [ ] **Step 4: Write failing simulator persistence tests**

Cover successful Commit, failed storage, failed verification, idempotent retry, and reconnect:

~~~rust
#[test]
fn commit_updates_flash_once_and_returns_generation() {
    let mut harness = WireHarness::new();
    let session = harness.hello(1).unwrap();
    let before = harness.read_parameter(session, 100).unwrap();
    let write = harness.write_f32(session, 100, before.revision, 2.5).unwrap();
    let sequence = 0x4242;
    let first = harness.commit_with_sequence(
        session,
        vec![(100, write.new_revision)],
        sequence,
    ).unwrap();
    let retry = harness.commit_with_sequence(
        session,
        vec![(100, write.new_revision)],
        sequence,
    ).unwrap();
    assert_eq!(first, retry);
    assert_eq!(first.storage_generation, 1);
    let after = harness.read_parameter(session, 100).unwrap();
    assert_eq!(after.value, ParamValue::F32(2.5));
    assert_eq!(after.persisted_value, Some(ParamValue::F32(2.5)));
}
~~~

StorageFailed and VerifyFailed responses must leave the old persisted value and generation unchanged while retaining the new RAM value.

- [ ] **Step 5: Implement simulator persistence and capabilities**

Add to SimDevice:

~~~rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitFailure {
    Storage,
    Verify,
}

struct Parameter {
    descriptor: ParamDescriptor,
    value: ParamValue,
    persisted_value: Option<ParamValue>,
    revision: u32,
}
~~~

Initialize persisted_value from default_value only when PERSISTENT is set. Implement MessageType::ParamCommit dispatch, validate strictly sorted IDs, current Revisions, PERSISTENT flags, and canonical_parameter_crc32 over the submitted current values. On success update every persisted value atomically, increment storage_generation once, and return ParamCommitAck. Advertise CapabilityFlags::PERSISTENCE. Let the existing reliable request cache replay a duplicate response before mutation.

- [ ] **Step 6: Write failing 16-channel dynamic telemetry tests**

~~~rust
#[test]
fn default_manifest_supports_eight_of_at_least_sixteen_dynamic_channels() {
    let mut harness = WireHarness::new();
    let session = harness.hello(2).unwrap();
    let manifest = harness.manifest(session).unwrap();
    assert!(manifest.telemetry.len() >= 16);
    let ids = manifest.telemetry.iter().take(8).map(|d| d.channel_id).collect();
    harness.subscribe_at(session, 500, ids).unwrap();
    harness.advance_ms(2);
    let first = harness.telemetry().unwrap();
    harness.advance_ms(2);
    let second = harness.telemetry().unwrap();
    assert_ne!(first.samples[0].values, second.samples[0].values);
}
~~~

Also assert presence of target speed, left/right raw delta, left/right total, left/right wheel speed, vehicle speed, speed error, left/right PWM, fault flags, loop jitter, battery voltage, and steering error.

- [ ] **Step 7: Implement sample-time-aware deterministic telemetry**

Generate each sample from its own timestamp, not one cloned value per batch:

~~~rust
fn telemetry_value(descriptor: &TelemetryDescriptor, timestamp_us: u64) -> u32 {
    let phase = (timestamp_us % 2_000_000) as f32 / 2_000_000.0;
    match descriptor.machine_name.as_str() {
        "drive.speed_mps" => (1.8 + (phase * std::f32::consts::TAU).sin() * 0.4).to_bits(),
        "encoder.left_delta" => (18 + ((timestamp_us / 2_000) % 5) as i32) as u32,
        "drive.fault_flags" => if timestamp_us % 5_000_000 < 10_000 { 1 } else { 0 },
        _ => deterministic_value_for_type(descriptor.telemetry_type, timestamp_us),
    }
}
~~~

Use Chinese display names/groups to exercise UTF-8. Keep exact values deterministic for the same descriptor and timestamp.

- [ ] **Step 8: Run full protocol/simulator gates**

Extend generate_vectors.rs with fixed ParamState and ParamCommitAck frames named `param-value.bin` and `param-commit-ack.bin`; update its exact filename unit test from four to six entries. Regenerate the committed files and manifest once, then use `--check` as the binding byte-compatibility gate.

Run:

~~~powershell
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu fmt --all -- --check
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu test --workspace --all-targets -- --test-threads=1
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu run -p dctp-sim --bin generate_vectors -- --check
~~~

Expected: all existing tests plus new persistence/telemetry tests pass and the vector checker prints DCTP v1 vectors match.

- [ ] **Step 9: Commit Task 1**

~~~powershell
git add crates/dctp-protocol crates/dctp-sim README.md
git commit -m 'feat(protocol): expose persisted parameter state'
~~~

---

### Task 2: Scaffold the browser-testable React application and quality gates

**Files:**
- Modify: .gitignore
- Create: .node-version
- Create: package.json
- Create: pnpm-workspace.yaml
- Create: apps/dicar-desktop/package.json
- Create: apps/dicar-desktop/index.html
- Create: apps/dicar-desktop/tsconfig.json
- Create: apps/dicar-desktop/tsconfig.app.json
- Create: apps/dicar-desktop/tsconfig.node.json
- Create: apps/dicar-desktop/vite.config.ts
- Create: apps/dicar-desktop/vitest.config.ts
- Create: apps/dicar-desktop/eslint.config.js
- Create: apps/dicar-desktop/components.json
- Create: apps/dicar-desktop/src/main.tsx
- Create: apps/dicar-desktop/src/app/App.tsx
- Create: apps/dicar-desktop/src/app/providers.tsx
- Create: apps/dicar-desktop/src/app/styles/tokens.css
- Create: apps/dicar-desktop/src/app/styles/global.css
- Create: apps/dicar-desktop/src/test/setup.ts
- Create: apps/dicar-desktop/src/app/App.test.tsx
- Create: pnpm-lock.yaml through pnpm

**Interfaces:**
- Produces: a Vite app and Vitest environment at apps/dicar-desktop.
- Produces: root scripts dev, lint, typecheck, test, build, and test:e2e.
- Consumed by: Tasks 7–11.

- [ ] **Step 1: Create package manifests and install locked dependencies**

The root package.json must be private, declare packageManager pnpm@11.16.0, and proxy scripts to @dicar/desktop. The app dependencies are React, React DOM, React Router, Zustand, @tauri-apps/api, @phosphor-icons/react, clsx, tailwind-merge, class-variance-authority, and the Radix primitives used by Card/Dialog/Select/Switch/Tabs/Tooltip. Dev dependencies are TypeScript, Vite, @vitejs/plugin-react, Tailwind CSS, @tailwindcss/vite, ESLint, Vitest, jsdom, Testing Library, Playwright, and @axe-core/playwright.

Run installation with the bundled Node directory prepended to PATH and commit the generated pnpm-lock.yaml. Add target, node_modules, dist, coverage, playwright-report, test-results, and local Tauri build output to .gitignore; do not ignore Cargo.lock.

- [ ] **Step 2: Write the failing application smoke test**

~~~tsx
import { render, screen } from "@testing-library/react";
import { App } from "./App";

it("shows the disconnected application shell and four menu destinations", () => {
  render(<App />);
  expect(screen.getByText("未连接")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /实时调参与波形/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /数据记录与回放/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /参数方案库/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /连接与链路诊断/ })).toBeInTheDocument();
});
~~~

- [ ] **Step 3: Run RED**

Run:

~~~powershell
pnpm --filter @dicar/desktop test --run src/app/App.test.tsx
~~~

Expected: FAIL because App and its route content are not implemented.

- [ ] **Step 4: Implement the minimal routed shell and semantic tokens**

Use BrowserRouter, an AppProviders wrapper, one skip link, and semantic CSS variables copied from design-system/dicar-tune/MASTER.md. Tailwind v4 is loaded with @tailwindcss/vite and global.css imports tailwindcss plus tokens.css. Do not add decorative animations or remote font loading.

- [ ] **Step 5: Add lint, typecheck, unit-test, and production-build gates**

Run:

~~~powershell
pnpm lint
pnpm typecheck
pnpm test
pnpm build
~~~

Expected: every command exits 0 and the app smoke test passes.

- [ ] **Step 6: Commit Task 2**

~~~powershell
git add .gitignore .node-version package.json pnpm-workspace.yaml pnpm-lock.yaml apps/dicar-desktop
git commit -m 'build(desktop): scaffold React quality gates'
~~~

---

### Task 3: Expose an ephemeral simulator server and implement the TCP Transport

**Files:**
- Create: crates/dctp-sim/src/server.rs
- Modify: crates/dctp-sim/src/lib.rs
- Modify: crates/dctp-sim/src/main.rs
- Create: crates/dctp-sim/tests/server.rs
- Create: crates/dicar-app-core/Cargo.toml
- Create: crates/dicar-app-core/src/lib.rs
- Create: crates/dicar-app-core/src/error.rs
- Create: crates/dicar-app-core/src/transport/mod.rs
- Create: crates/dicar-app-core/src/transport/tcp.rs
- Create: crates/dicar-app-core/tests/tcp_transport.rs

**Interfaces:**
- Produces: SimulatorServer::spawn(SocketAddr) -> SimulatorServer with local_addr() and shutdown().
- Produces: Endpoint::Simulator { address: SocketAddr }.
- Produces: TransportIdentity and blocking Transport trait.
- Produces: TcpTransport::connect(address).
- Consumed by: Tasks 4–6 and Rust E2E.

- [ ] **Step 1: Write the failing reusable-server test**

~~~rust
#[test]
fn spawned_server_reports_an_ephemeral_address_and_releases_it() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = server.local_addr();
    assert_ne!(address.port(), 0);
    TcpStream::connect(address).unwrap();
    server.shutdown().unwrap();
    assert!(TcpStream::connect(address).is_err());
}
~~~

Run cargo test -p dctp-sim --test server and record the missing type RED.

- [ ] **Step 2: Extract server lifetime from main.rs**

SimulatorServer owns the listener thread, shutdown flag, bound address, and JoinHandle. The CLI calls SimulatorServer::run_forever; tests call spawn on port 0. Preserve one-client rejection, disconnect reset, 1 ms device poll, queue behavior, and existing CLI help text.

- [ ] **Step 3: Write failing Transport tests**

~~~rust
#[test]
fn tcp_transport_reads_and_writes_the_simulator_byte_stream() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut transport = TcpTransport::connect(server.local_addr()).unwrap();
    let hello = hello_frame(77);
    transport.write_all(&encode_frame(&hello).unwrap()).unwrap();
    let mut bytes = [0; 1100];
    let count = transport.read(&mut bytes).unwrap();
    let frames = StreamDecoder::new().push(&bytes[..count]);
    assert_eq!(frames.into_iter().flatten().next().unwrap().header.message_type, MessageType::HelloAck);
}
~~~

Add two independent socket-boundary tests: a 10 ms read with no bytes returns `Ok(0)`, and peer shutdown/EOF returns `TransportError::Disconnected`. Each test owns and closes its real loopback listener so it does not assert framework internals.

- [ ] **Step 4: Implement the focused Transport API**

~~~rust
pub trait Transport: Send {
    fn identity(&self) -> TransportIdentity;
    fn read(&mut self, output: &mut [u8]) -> Result<usize, TransportError>;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    fn close(&mut self) -> Result<(), TransportError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Endpoint {
    Simulator { address: SocketAddr },
}
~~~

TcpTransport sets a 10 ms read timeout, a 1 s write timeout, TCP_NODELAY, and idempotent shutdown. WouldBlock or TimedOut becomes Ok(0); EOF is TransportError::Disconnected.

- [ ] **Step 5: Run focused and workspace regressions**

Run cargo test -p dctp-sim --test server, cargo test -p dicar-app-core --test tcp_transport, then cargo test --workspace --all-targets. Require all green.

- [ ] **Step 6: Commit Task 3**

~~~powershell
git add crates/dctp-sim crates/dicar-app-core
git commit -m 'feat(core): add simulator TCP transport'
~~~

---

### Task 4: Implement ProtocolSession handshake, Manifest loading, reads, heartbeat, and recovery

**Files:**
- Create: crates/dicar-app-core/src/clock.rs
- Create: crates/dicar-app-core/src/session.rs
- Create: crates/dicar-app-core/src/model.rs
- Modify: crates/dicar-app-core/src/lib.rs
- Create: crates/dicar-app-core/tests/session_integration.rs
- Create: crates/dicar-app-core/tests/session_faults.rs

**Interfaces:**
- Consumes: Transport, dctp-protocol public wire types, SimulatorServer.
- Produces: ProtocolSession<T: Transport>.
- Produces: ConnectedDevice, DeviceIdentity, ConnectionPhase, DiagnosticsSnapshot.
- Produces: connect_and_load(), poll(), request(), close().
- Consumed by: Tasks 5–7.

- [ ] **Step 1: Write the failing real-TCP session test**

~~~rust
#[test]
fn session_reaches_ready_with_manifest_and_all_parameter_states() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let transport = TcpTransport::connect(server.local_addr()).unwrap();
    let mut session = ProtocolSession::new(transport, FixedNonce(0x1020_3040), TestClock::new());
    let connected = session.connect_and_load().unwrap();
    assert_eq!(connected.phase, ConnectionPhase::Ready);
    assert!(connected.manifest.parameters.len() >= 10);
    assert_eq!(connected.parameter_states.len(), connected.manifest.parameters.len());
    assert!(connected.parameter_states.iter().any(|s| s.persisted_value.is_some()));
}
~~~

- [ ] **Step 2: Run RED**

Run cargo test -p dicar-app-core --test session_integration. Expected: missing ProtocolSession and model types.

- [ ] **Step 3: Implement request/response routing on the real byte stream**

ProtocolSession owns Transport, StreamDecoder, next_sequence, session_id, negotiated_max_payload, last_valid_frame_at, last_heartbeat_at, DiagnosticsSnapshot, and a VecDeque of unsolicited telemetry/log frames. request constructs Frame with ACK_REQUIRED, writes encode_frame output, reads arbitrary chunks, and accepts only a matching Sequence/Session response. Error frames decode ErrorPayload into CoreError::Device.

Manifest loading accepts ManifestChunk frames until ManifestDone, calls ManifestAssembler, decodes DeviceManifest, validates HelloAck.manifest_crc32, then ParamRead for every descriptor.

- [ ] **Step 4: Implement exact retry and heartbeat timing**

Use a Clock trait so 2999/3000 ms, 300/500/3000 ms deadlines, and Sequence reuse are deterministic. poll sends Heartbeat after 500 ms of idle time, dispatches TelemetryData without blocking requests, and transitions to Disconnected after 3000 ms without any valid frame.

- [ ] **Step 5: Add fault and stale-session tests**

Tests must prove:

- CRC noise increments decoder diagnostics and the next valid frame recovers.
- one ordinary request sends once plus exactly three retries (four total attempts) before failure and reuses Sequence for all four;
- one Manifest request permits four total attempts, while Commit permits three total attempts, with one Sequence per logical operation;
- a 3000 ms stale session stops writes;
- reconnect opens a new Session ID, reloads every parameter, and never replays old writes;
- Manifest CRC change replaces the description cache.

- [ ] **Step 6: Run focused, Clippy, and workspace tests**

Run cargo fmt, cargo clippy -p dicar-app-core --all-targets -- -D warnings, both session tests, and cargo test --workspace --all-targets. Require green.

- [ ] **Step 7: Commit Task 4**

~~~powershell
git add crates/dicar-app-core
git commit -m 'feat(core): establish DCTP application sessions'
~~~

---

### Task 5: Build the permission-aware parameter workspace and Commit workflow

**Files:**
- Create: crates/dicar-app-core/src/access.rs
- Create: crates/dicar-app-core/src/parameter_workspace.rs
- Modify: crates/dicar-app-core/src/session.rs
- Modify: crates/dicar-app-core/src/model.rs
- Modify: crates/dicar-app-core/src/lib.rs
- Create: crates/dicar-app-core/tests/parameter_workspace.rs
- Create: crates/dicar-app-core/tests/parameter_integration.rs

**Interfaces:**
- Produces: AccessProfile, AccessRole, LeaseState, PermissionDecision.
- Produces: ParameterRecord and ParameterWorkspace.
- Produces: write_parameter(), revert_all(), undo_last_confirmed_change(), commit_dirty().
- Consumed by: AppActor and Tasks 7–11.

- [ ] **Step 1: Write failing workspace-state tests**

~~~rust
#[test]
fn accepted_ram_write_becomes_dirty_without_changing_flash() {
    let mut workspace = fixture_workspace();
    workspace.accept_write(100, ParamValue::F32(2.0), 1).unwrap();
    let value = workspace.get(100).unwrap();
    assert_eq!(value.ram_value, ParamValue::F32(2.0));
    assert_eq!(value.persisted_value, Some(ParamValue::F32(1.0)));
    assert!(value.dirty);
}

#[test]
fn observer_and_tuner_without_flash_permission_receive_textual_denials() {
    assert_eq!(AccessProfile::observer().can_write(), PermissionDecision::Denied("仅观察者不能修改参数"));
    assert_eq!(AccessProfile::tuner().can_commit(), PermissionDecision::Denied("当前身份没有固化权限"));
}

#[test]
fn dirty_state_uses_wire_bits_for_signed_zero_and_nan_payloads() {
    let mut workspace = unconstrained_f32_workspace(100, ParamValue::F32(0.0));
    workspace.accept_write(100, ParamValue::F32(-0.0), 1).unwrap();
    assert!(workspace.get(100).unwrap().dirty);

    let bits = 0x7fc0_0001;
    let mut workspace = unconstrained_f32_workspace(100, ParamValue::F32(f32::from_bits(bits)));
    workspace.accept_write(100, ParamValue::F32(f32::from_bits(bits)), 1).unwrap();
    assert!(!workspace.get(100).unwrap().dirty);
    workspace.accept_write(100, ParamValue::F32(f32::from_bits(bits + 1)), 2).unwrap();
    assert!(workspace.get(100).unwrap().dirty);
}
~~~

Run cargo test -p dicar-app-core --test parameter_workspace and record missing types RED.

- [ ] **Step 2: Implement one authoritative ParameterRecord per param_id**

~~~rust
pub struct ParameterRecord {
    pub descriptor: ParamDescriptor,
    pub ram_value: ParamValue,
    pub persisted_value: Option<ParamValue>,
    pub revision: u32,
    pub dirty: bool,
    pub sync_state: DeviceSyncState,
    pub write_state: WriteState,
}
~~~

ParameterWorkspace validates descriptor type, Numeric bounds/step, Enum options, WRITABLE/PERSISTENT/DANGEROUS flags, and persisted-value type. Dirty is derived only through the shared `ParamValue::wire_eq` implementation, including f32 bits; do not use derived `PartialEq`, duplicate comparison logic, or compare displayed strings.

- [ ] **Step 3: Write failing real-session write/coalescing tests**

Test these observable behaviors:

- Owner with active lease writes RAM and receives the device-accepted value/new Revision.
- Observer cannot send a frame.
- one parameter has at most one in-flight write;
- while one write is in flight, later slider targets replace queued intermediate values;
- RevisionConflict refreshes current RAM/Revision from the structured conflict context and leaves the user target unresolved;
- a failed write restores the device-confirmed display value and preserves a nearby localized error.

Use a counting test Transport plus a real simulator integration case.

- [ ] **Step 4: Implement write scheduling**

ParameterWorkspace holds in_flight: HashMap<u32, PendingWrite> and queued_latest: HashMap<u32, ParamValue>. The actor starts only the latest queued value after the current ACK. ProtocolSession::write_parameter sends ParamWrite with expected_revision and returns ParamWriteAck or a typed DeviceError. Decode the lowercase ParamWriteAck hex in REVISION_CONFLICT context through dctp-protocol, not ad-hoc field parsing.

- [ ] **Step 5: Write failing Commit and revert tests**

~~~rust
#[test]
fn commit_sorts_dirty_persistent_values_and_updates_generation() {
    let mut connected = connected_owner_fixture();
    connected.write(101, ParamValue::F32(0.8)).unwrap();
    connected.write(100, ParamValue::F32(2.2)).unwrap();
    let result = connected.commit_dirty().unwrap();
    assert_eq!(result.storage_generation, 1);
    assert_eq!(connected.workspace().dirty_count(), 0);
}

#[test]
fn failed_commit_keeps_ram_dirty_and_old_flash_values() {
    let mut connected = connected_owner_with_failure(CommitFailure::Storage);
    connected.write(100, ParamValue::F32(2.2)).unwrap();
    assert!(connected.commit_dirty().is_err());
    assert_eq!(connected.workspace().get(100).unwrap().persisted_value, Some(ParamValue::F32(1.0)));
    assert!(connected.workspace().get(100).unwrap().dirty);
}
~~~

revert_all must send Revision-aware writes of persisted values to the device; it must not only edit local UI state. Non-persistent parameters are reported as not revertible. Keep a bounded history of accepted writes as (param_id, previous_device_value); undo_last_confirmed_change sends that value with the current Revision and records the inverse only after ACK.

- [ ] **Step 6: Implement Commit atomically from the App perspective**

Collect only dirty PERSISTENT records, sort by param_id, create ParamCommitEntry values with current Revisions, compute canonical_parameter_crc32 from the same RAM values, send ParamCommit, and validate the ACK CRC before accepting storage_generation. On success copy RAM to persisted for exactly those records. On any error leave every dirty/persisted value unchanged.

- [ ] **Step 7: Run focused and workspace gates**

Run both new tests, cargo fmt, Clippy warnings-as-errors, and full workspace tests. Require green and no changed golden vectors.

- [ ] **Step 8: Commit Task 5**

~~~powershell
git add crates/dicar-app-core
git commit -m 'feat(core): manage parameter workspaces and commits'
~~~

---

### Task 6: Implement telemetry processing, diagnostics, and the bounded AppActor

**Files:**
- Create: crates/dicar-app-core/src/telemetry_engine.rs
- Create: crates/dicar-app-core/src/actor.rs
- Create: crates/dicar-app-core/src/bridge_model.rs
- Modify: crates/dicar-app-core/src/session.rs
- Modify: crates/dicar-app-core/src/model.rs
- Modify: crates/dicar-app-core/src/lib.rs
- Create: crates/dicar-app-core/tests/telemetry_engine.rs
- Create: crates/dicar-app-core/tests/actor_integration.rs
- Create: crates/dicar-app-core/tests/actor_capacity.rs

**Interfaces:**
- Produces: CoreCommand and FIFO CoreEventPayload values; frontend-global indices are assigned only by Task 7's sequencer.
- Produces: AppActorHandle::spawn(), send(), snapshot(), subscribe().
- Produces: AppSnapshot, UiTelemetryBatch, TelemetryPoint, DiagnosticsSnapshot.
- Binding capacities: 64 commands, 64 reserved reliable events, one coalesced snapshot slot, four whole telemetry UI batches, eight channels x 30,000 points.
- Consumed by: Tauri bridge and frontend DTOs.

- [ ] **Step 1: Write failing timestamp/ring-buffer tests**

~~~rust
#[test]
fn engine_unwraps_u32_time_and_bounds_sixty_seconds_at_five_hundred_hz() {
    let mut engine = TelemetryEngine::new(Duration::from_secs(60), 8);
    engine.accept(batch_near_u32_wrap()).unwrap();
    engine.accept(batch_after_u32_wrap()).unwrap();
    assert!(engine.latest_timestamp_us() > u64::from(u32::MAX));
    assert!(engine.channel_len(200) <= 30_000);
}

#[test]
fn sequence_gap_and_device_drop_counters_are_distinct() {
    let mut engine = TelemetryEngine::default();
    engine.accept(batch_with_sequence(10, 0)).unwrap();
    engine.accept(batch_with_sequence(14, 2)).unwrap();
    assert_eq!(engine.diagnostics().sequence_gap_samples, 3);
    assert_eq!(engine.diagnostics().device_dropped_samples, 2);
}
~~~

- [ ] **Step 2: Implement type-safe raw-slot conversion and bounded storage**

~~~rust
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum TelemetryValue {
    F32(f32),
    I32(i32),
    U32(u32),
    Flags32(u32),
}
~~~

Resolve each raw u32 slot through the active TelemetryDescriptor type. Reject subscription-version or width mismatch. Use VecDeque per channel, evict oldest whole points, and retain raw batch metadata separately from future recorder hooks.

- [ ] **Step 3: Write the failing actor E2E test**

~~~rust
#[test]
fn actor_connects_writes_subscribes_and_streams_ordered_events() {
    let server = SimulatorServer::spawn("127.0.0.1:0".parse().unwrap()).unwrap();
    let actor = AppActorHandle::spawn(CoreConfig::simulator(server.local_addr()));
    actor.send(CoreCommand::Connect).unwrap();
    wait_for_ready(&actor);
    actor.send(CoreCommand::SetTelemetrySubscription {
        channel_ids: first_eight_channels(actor.snapshot()),
        sample_rate_hz: 500,
    }).unwrap();
    let events = collect_for(&actor, Duration::from_millis(80));
    assert_core_events_follow_emission_order(&events);
    assert!(events.iter().any(|event| matches!(event, CoreEvent::TelemetryBatch(_))));
}
~~~

- [ ] **Step 4: Implement actor commands and priority-aware event output**

CoreCommand variants are Connect, Disconnect, WriteParameter, CommitParameters, RevertAllPendingChanges, UndoLastConfirmedChange, SetTelemetrySubscription, SetPaused, SelectAccessProfile, AddMarker, GetSnapshot, and Shutdown.

CoreEventPayload variants use serde tag event/data and retain actor FIFO order without assigning a frontend-global index:

~~~rust
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum CoreEventPayload {
    SnapshotChanged(AppSnapshot),
    TelemetryBatch(UiTelemetryBatch),
    OperationCompleted(OperationResult),
    ConnectionLost(ConnectionLoss),
    FatalError(BridgeError),
}
~~~

Use bounded crossbeam channels. Reliable operation and connection events use a reserved channel; snapshots coalesce to the newest; telemetry drops oldest whole UI batch and increments ui_dropped_batches. No send may block the protocol loop longer than 2 ms.

- [ ] **Step 5: Implement subscription, pause, and 16–33 ms UI batching**

SetTelemetrySubscription validates 1–8 unique Manifest IDs and 1–500 Hz before sending. SetPaused(true) sends TELEMETRY_STOP but keeps the last Canvas buffer frozen; false re-sends the saved subscription with an incremented subscription_version. Aggregate received samples and emit a UiTelemetryBatch no faster than every 16 ms and no slower than 33 ms while data is flowing.

- [ ] **Step 6: Track diagnostics and disconnect truthfully**

AppSnapshot includes phase, transport identity, device identity, Manifest metadata, parameter records, selected subscription, access profile, storage generation, dirty count, and DiagnosticsSnapshot. On disconnect stop all new writes, mark dirty records DeviceSyncState::Unknown, freeze telemetry, and preserve last visible samples. Reconnect replaces state from the device and never replays queued writes.

- [ ] **Step 7: Run actor stress and full gates**

actor_capacity.rs advances a fake clock through 60 seconds of 8x500 Hz data while the UI consumer stalls. Assert at every sampled second: total retained points <=240,000, each channel <=30,000, at most four telemetry UI batches await delivery, at most one snapshot awaits delivery, and visual revisions <=30 per second. Fill the command queue and prove overload returns a typed error within 2 ms; fill the reliable-event reserve and prove the actor emits/retains an explicit frontend-overrun terminal state rather than blocking or silently dropping an operation result. Also assert ordered reliable events and a parameter write completes under continuous telemetry. Then run fmt, Clippy, and all workspace tests.

- [ ] **Step 8: Commit Task 6**

~~~powershell
git add crates/dicar-app-core
git commit -m 'feat(core): stream bounded telemetry and app state'
~~~

---

### Task 7: Add the Tauri shell, typed Channel bridge, and deterministic MockBridge

**Files:**
- Modify: Cargo.toml
- Create: apps/dicar-desktop/src-tauri/Cargo.toml
- Create: apps/dicar-desktop/src-tauri/build.rs
- Create: apps/dicar-desktop/src-tauri/tauri.conf.json
- Create: apps/dicar-desktop/src-tauri/capabilities/default.json
- Create: apps/dicar-desktop/src-tauri/src/main.rs
- Create: apps/dicar-desktop/src-tauri/src/lib.rs
- Create: apps/dicar-desktop/src-tauri/src/app_state.rs
- Create: apps/dicar-desktop/src-tauri/src/commands.rs
- Create: apps/dicar-desktop/src-tauri/src/channel_forwarder.rs
- Create: apps/dicar-desktop/src-tauri/src/window_guard.rs
- Create: apps/dicar-desktop/src-tauri/tests/commands.rs
- Create: apps/dicar-desktop/src-tauri/tests/window_close.rs
- Modify: apps/dicar-desktop/package.json
- Create: apps/dicar-desktop/src/domain/types.ts
- Create: apps/dicar-desktop/src/bridge/desktopBridge.ts
- Create: apps/dicar-desktop/src/bridge/tauriBridge.ts
- Create: apps/dicar-desktop/src/bridge/mockBridge.ts
- Create: apps/dicar-desktop/src/bridge/bridge.test.ts
- Modify: apps/dicar-desktop/src/app/providers.tsx
- Modify: Cargo.lock

**Interfaces:**
- Consumes: AppActorHandle, CoreCommand, CoreEvent.
- Produces: DesktopBridge and a shared TypeScript discriminated-union model.
- Produces: Tauri invoke commands and open_core_channel.
- Produces: a native close-request guard and resolve_window_close command.
- Consumed by: Tasks 8–11.

- [ ] **Step 1: Verify the native prerequisite gate**

Run:

~~~powershell
C:\Users\tluda\.cargo\bin\rustup.exe toolchain list
Get-Command link.exe -ErrorAction SilentlyContinue
Get-Command msbuild.exe -ErrorAction SilentlyContinue
~~~

If the linker/build tools are absent, request authorization for the official Microsoft C++ Build Tools Desktop development with C++ and Windows SDK installation. Continue browser/core work while approval or installation completes, but Task 7 cannot be marked complete until the MSVC cargo check succeeds.

- [ ] **Step 2: Write the failing bridge contract test**

~~~ts
it("delivers ordered snapshots and telemetry through one subscription", async () => {
  const bridge = new MockBridge(fixtures.ready);
  const events: BridgeEvent[] = [];
  const unsubscribe = await bridge.subscribe((event) => events.push(event));
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  bridge.advanceTelemetry(40);
  unsubscribe();
  expect(events.map((event) => event.eventIndex)).toEqual(
    [...events.map((event) => event.eventIndex)].sort((a, b) => a - b),
  );
});
~~~

Run the focused Vitest file. Expected: missing bridge types RED.

- [ ] **Step 3: Define one DesktopBridge contract**

~~~ts
export interface DesktopBridge {
  connect(endpoint: Endpoint): Promise<OperationResult>;
  disconnect(): Promise<OperationResult>;
  writeParameter(paramId: number, value: ParameterValue): Promise<OperationResult>;
  commitParameters(): Promise<OperationResult>;
  revertAll(): Promise<OperationResult>;
  undoLast(): Promise<OperationResult>;
  setTelemetrySubscription(request: TelemetrySubscriptionRequest): Promise<OperationResult>;
  setPaused(paused: boolean): Promise<OperationResult>;
  addMarker(label: string): Promise<OperationResult>;
  resolveWindowClose(requestId: number, decision: "cancel" | "disconnectKeepUnknown" | "revertThenClose"): Promise<OperationResult>;
  selectAccessProfile(profile: AccessProfileId): Promise<OperationResult>;
  getSnapshot(): Promise<AppSnapshot>;
  subscribe(listener: (event: BridgeEvent) => void): Promise<() => void>;
}
~~~

No React component imports @tauri-apps/api. MockBridge applies the same role, dirty, channel-count, and state-transition rules and generates deterministic time-varying samples for browser tests.

- [ ] **Step 4: Implement Tauri commands and a single ordered Channel**

~~~rust
#[tauri::command]
pub fn open_core_channel(
    state: tauri::State<'_, AppState>,
    on_event: tauri::ipc::Channel<CoreEvent>,
) -> Result<(), BridgeErrorDto> {
    state.replace_frontend_channel(on_event)
}
~~~

Register all commands in one generate_handler call. channel_forwarder drains the actor receiver and sends CoreEvent in order. Replacing or closing the frontend Channel stops the old forwarder. tauriBridge constructs Channel<BridgeEvent>, assigns onmessage before invoke, and returns an unsubscribe function that calls close_core_channel.

BridgeEvent is the frontend union of ordered CoreEvent DTOs plus `WindowCloseRequested { eventIndex, requestId, dirtyCount, canRevert }`, which is produced only by the Tauri window layer. AppState owns one `FrontendEventSequencer`: its mutex protects both allocation of the next eventIndex and Channel send, and both the actor forwarder and window-close handler must publish through this single serialization point. MockBridge uses the same one-publisher rule; the core actor remains window-independent. A focused concurrency test interleaves actor snapshots/telemetry with close requests and asserts strictly increasing, gap-free Channel indices in actual receive order.

- [ ] **Step 5: Keep Tauri capabilities minimal**

The default capability names only the main window and core permissions needed for invoke/Channel operation. Do not add shell, filesystem, arbitrary network, or process-execution plugins in this stage; TcpTransport stays Rust-side.

- [ ] **Step 6: Implement and test the native dirty-close guard**

Register a Tauri `CloseRequested` handler. If dirtyCount is zero, allow the close. Otherwise call `api.prevent_close()`, allocate one requestId, and send WindowCloseRequested on the existing ordered Channel. `resolve_window_close` rejects stale/duplicate IDs and implements exactly three decisions: cancel leaves the window/session untouched; disconnectKeepUnknown sends Disconnect, waits for the operation result that marks device sync Unknown, then destroys the window; revertThenClose requires READY, sends Revision-aware RevertAll, and only after every ACK sends Disconnect and destroys the window. Any error keeps the window open and returns a localized BridgeError.

window_close.rs tests clean close, cancel, stale request, disconnect/Unknown, successful revert/close, and failed revert remaining open. bridge.test.ts proves MockBridge forwards WindowCloseRequested and rejects stale decisions; Task 9 owns the visible confirmation dialog once the shared Dialog primitive exists.

- [ ] **Step 7: Run frontend, Rust, and native checks**

Run:

~~~powershell
pnpm --filter @dicar/desktop test --run src/bridge/bridge.test.ts
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu test -p dicar-app-core
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-msvc check -p dicar-desktop --all-targets
pnpm --filter @dicar/desktop build
~~~

Expected: all exit 0. The MSVC command is the binding native-shell gate.

Define the package script `tauri:build` as `tauri build --bundles nsis`; the binding package command used here and in Task 11 is `pnpm --filter @dicar/desktop run tauri:build`.

- [ ] **Step 8: Commit Task 7**

~~~powershell
git add Cargo.toml Cargo.lock apps/dicar-desktop
git commit -m 'feat(desktop): bridge Tauri to the app actor'
~~~

---

### Task 8: Implement the B-style home, app shell, and live diagnostics route

**Files:**
- Create: apps/dicar-desktop/src/app/routes.tsx
- Create: apps/dicar-desktop/src/stores/connectionStore.ts
- Create: apps/dicar-desktop/src/stores/workspaceStore.ts
- Create: apps/dicar-desktop/src/stores/collaborationStore.ts
- Create: apps/dicar-desktop/src/stores/settingsStore.ts
- Create: apps/dicar-desktop/src/hooks/useBridgeSubscription.ts
- Create: apps/dicar-desktop/src/components/ui/button.tsx
- Create: apps/dicar-desktop/src/components/ui/card.tsx
- Create: apps/dicar-desktop/src/components/ui/badge.tsx
- Create: apps/dicar-desktop/src/components/ui/skeleton.tsx
- Create: apps/dicar-desktop/src/components/ui/alert.tsx
- Create: apps/dicar-desktop/src/components/shell/AppShell.tsx
- Create: apps/dicar-desktop/src/components/shell/ConnectionStatusBar.tsx
- Create: apps/dicar-desktop/src/components/shell/VehicleSwitcher.tsx
- Create: apps/dicar-desktop/src/components/home/MenuCard.tsx
- Create: apps/dicar-desktop/src/components/home/ProjectSummary.tsx
- Create: apps/dicar-desktop/src/pages/HomePage.tsx
- Create: apps/dicar-desktop/src/pages/DiagnosticsPage.tsx
- Create: apps/dicar-desktop/src/pages/ComingSoonPage.tsx
- Create: apps/dicar-desktop/src/pages/NotFoundPage.tsx
- Create: apps/dicar-desktop/src/pages/HomePage.test.tsx
- Create: apps/dicar-desktop/src/pages/DiagnosticsPage.test.tsx
- Modify: apps/dicar-desktop/src/app/App.tsx
- Modify: apps/dicar-desktop/src/app/styles/global.css

**Interfaces:**
- Consumes: DesktopBridge and BridgeEvent.
- Produces: routes /, /diagnostics, /records, /parameter-sets, /live/:vehicleId.
- Produces: normalized Zustand stores updated by one bridge subscription.
- Consumed by: Tasks 9–11.

- [ ] **Step 1: Write failing home/navigation tests**

Tests assert the status strip, four labeled cards, real simulator Connect action, project metrics, exact development badges on deferred pages, diagnostic values, and NotFound return path. Do not accept a disabled card with no explanation.

- [ ] **Step 2: Run RED**

Run pnpm --filter @dicar/desktop test --run src/pages/HomePage.test.tsx. Expected: missing pages/components.

- [ ] **Step 3: Implement one event reducer and selector-driven stores**

useBridgeSubscription subscribes once in AppProviders, validates eventIndex monotonicity, and dispatches each BridgeEvent to store actions. Stores hold only authoritative or user-entered state; dirtyCount, permission decisions, filtered groups, and connection labels are selectors, not duplicated mutable fields.

- [ ] **Step 4: Implement the B menu and honest destination states**

HomePage uses the home design override: operational header, compact connection strip, 2x2 Card grid at desktop, one column below 768 px, and ProjectSummary. Real-time and diagnostics routes are live. Records and parameter sets route to ComingSoonPage with the exact deferred scope and no fabricated data.

- [ ] **Step 5: Implement diagnostics from AppSnapshot**

Show phase, endpoint, device/session, firmware/SDK, negotiated payload, RTT, inbound/outbound bytes, CRC/decode errors, sequence gaps, device drops, UI drops, reconnect reason, and last valid frame time. Status uses icon plus text and tabular numbers.

- [ ] **Step 6: Add accessibility and responsive assertions**

Test skip-link target, heading order, keyboard activation of cards, visible text for status, no color-only badge meaning, 768/1024 route rendering, and loading skeletons over 300 ms.

- [ ] **Step 7: Run frontend gates**

Run lint, typecheck, all Vitest tests, and production build. Require green.

- [ ] **Step 8: Commit Task 8**

~~~powershell
git add apps/dicar-desktop
git commit -m 'feat(desktop): add menu home and diagnostics'
~~~

---

### Task 9: Implement the A-style parameter and encoder workbench

**Files:**
- Create: apps/dicar-desktop/src/components/ui/input.tsx
- Create: apps/dicar-desktop/src/components/ui/label.tsx
- Create: apps/dicar-desktop/src/components/ui/select.tsx
- Create: apps/dicar-desktop/src/components/ui/switch.tsx
- Create: apps/dicar-desktop/src/components/ui/slider.tsx
- Create: apps/dicar-desktop/src/components/ui/dialog.tsx
- Create: apps/dicar-desktop/src/components/ui/alert-dialog.tsx
- Create: apps/dicar-desktop/src/components/ui/table.tsx
- Create: apps/dicar-desktop/src/components/ui/tabs.tsx
- Create: apps/dicar-desktop/src/components/ui/sheet.tsx
- Create: apps/dicar-desktop/src/components/ui/tooltip.tsx
- Create: apps/dicar-desktop/src/components/workbench/ParameterNav.tsx
- Create: apps/dicar-desktop/src/components/workbench/ParameterEditor.tsx
- Create: apps/dicar-desktop/src/components/workbench/TypedParameterControl.tsx
- Create: apps/dicar-desktop/src/components/workbench/EncoderCalibrationPanel.tsx
- Create: apps/dicar-desktop/src/components/workbench/PermissionGate.tsx
- Create: apps/dicar-desktop/src/components/workbench/LeasePanel.tsx
- Create: apps/dicar-desktop/src/components/workbench/ChangeBar.tsx
- Create: apps/dicar-desktop/src/components/workbench/CommitReviewDialog.tsx
- Create: apps/dicar-desktop/src/components/shell/WindowCloseDialog.tsx
- Create: apps/dicar-desktop/src/hooks/useAppShortcuts.ts
- Create: apps/dicar-desktop/src/hooks/useUnsavedChangesGuard.ts
- Create: apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx
- Create: apps/dicar-desktop/src/pages/LiveWorkbenchPage.test.tsx
- Create: apps/dicar-desktop/src/components/workbench/TypedParameterControl.test.tsx
- Create: apps/dicar-desktop/src/components/workbench/EncoderCalibrationPanel.test.tsx
- Modify: apps/dicar-desktop/src/app/routes.tsx
- Modify: apps/dicar-desktop/src/stores/workspaceStore.ts
- Modify: apps/dicar-desktop/src/stores/collaborationStore.ts

**Interfaces:**
- Consumes: parameter/access state from AppSnapshot and DesktopBridge operations.
- Produces: searchable Manifest-driven controls, encoder panel, dirty bar, Commit review, permission/lease UI, and safe navigation guard.
- Consumed by: Task 11 E2E.

- [ ] **Step 1: Write failing typed-control and encoder tests**

~~~tsx
it.each(["i32", "u32", "f32", "bool", "enum"] as const)(
  "renders and submits a %s manifest parameter",
  async (kind) => {
    const record = parameterFixture(kind);
    render(<TypedParameterControl record={record} />);
    await editToFixtureTarget(record);
    expect(mockBridge.writeParameter).toHaveBeenCalledWith(record.paramId, targetValue(record));
  },
);

it("never conflates PPR, multiplier, and read-only CPR", () => {
  render(<EncoderCalibrationPanel records={encoderRecords} />);
  expect(screen.getByLabelText("左编码器 PPR")).toBeEnabled();
  expect(screen.getByLabelText("右编码器 PPR")).toBeEnabled();
  expect(screen.getByLabelText("正交倍频")).toHaveValue("4");
  expect(screen.getByLabelText("左有效 CPR")).toHaveAttribute("aria-readonly", "true");
  expect(screen.getByLabelText("右有效 CPR")).toHaveAttribute("aria-readonly", "true");
  expect(screen.queryByLabelText("编码器线数")).not.toBeInTheDocument();
});
~~~

Also change left/right PPR and the multiplier through the real panel controls and assert the corresponding effective CPR output updates independently. Remove each required encoder machine_name in a table-driven fixture and assert the named compatibility warning appears while no misleading substitute field is rendered.

- [ ] **Step 2: Run RED**

Run the two focused Vitest files. Expected: missing controls and panel.

- [ ] **Step 3: Implement Manifest-driven navigation and exact controls**

ParameterNav supports Ctrl+K search, favorites, modified-only, recent adjustments, group counts, and keyboard list navigation. TypedParameterControl maps i32/u32/f32 to labeled number input plus optional Slider, bool to Switch plus on/off text, enum to Select, readonly to a semantic output, and dangerous parameters to a textual warning. Validate locally on blur, but always show device rejection beside the field.

- [ ] **Step 4: Implement the dedicated encoder section**

Match machine names, not display labels. Show left/right PPR, multiplier, left/right readonly CPR, inversion, wheel diameter, gear ratio, sample period, LPF, jump threshold, credible RPM, and missing-pulse switch. If a required descriptor is missing, show a named compatibility warning instead of hiding the field.

- [ ] **Step 5: Write failing permission and Commit-review tests**

Test:

- Observer can view every value/waveform but cannot edit.
- Tuner with active lease can write RAM but Commit is disabled with 当前身份没有固化权限.
- Owner without active lease cannot write or Commit.
- Owner with active lease reviews a before/RAM/Revision table and commits.
- dangerous rows are grouped and visually/textually marked.
- failed Commit keeps dirty rows and old Flash values.

- [ ] **Step 6: Implement ChangeBar, Commit dialog, and access profiles**

ChangeBar is fixed but reserves layout space. Revert All calls bridge.revertAll and never clears local values before ACK. CommitReviewDialog traps focus, lists every dirty persistent parameter, shows RAM/Flash values and units, and calls bridge.commitParameters once. LeasePanel clearly displays local fixture, role, active controller, observers, and why a gate is disabled.

- [ ] **Step 7: Guard dirty navigation and wire keyboard commands**

useUnsavedChangesGuard covers route navigation, browser beforeunload, simulator disconnect, and vehicle switch. WindowCloseDialog consumes the native WindowCloseRequested event and calls bridge.resolveWindowClose with the same choices. Choices are stay/cancel, disconnect while preserving unknown state, or revert on device before closing; there is no silent discard. Tests prove Escape/cancel returns focus, failed revert keeps the dialog/window open with the error, and each successful choice sends exactly one matching requestId. useAppShortcuts handles Ctrl+K, Ctrl+Shift+L, and Ctrl+Z through bridge.undoLast while ignoring normal editing shortcuts inside input, textarea, select, and contenteditable.

- [ ] **Step 8: Implement responsive workbench structure**

At >=1280 px render 264 px navigation, minmax(420 px, 1fr) editor, and the waveform slot minmax(440 px, 1.15fr). At 1024–1279 px move navigation into Sheet. Below 1024 px use accessible Parameters/Waveform Tabs. Keep ChangeBar reachable at every size and 200% zoom.

- [ ] **Step 9: Run focused, accessibility, and frontend gates**

Run all workbench tests, lint, typecheck, full Vitest, and build. Add jest-dom assertions for aria-invalid, descriptions, readonly semantics, focus return, and live error regions.

- [ ] **Step 10: Commit Task 9**

~~~powershell
git add apps/dicar-desktop
git commit -m 'feat(desktop): add parameter and encoder workbench'
~~~

---

### Task 10: Render bounded eight-channel Canvas waveforms and controls

**Files:**
- Create: apps/dicar-desktop/src/telemetry/ringBuffer.ts
- Create: apps/dicar-desktop/src/telemetry/downsample.ts
- Create: apps/dicar-desktop/src/telemetry/channelStyles.ts
- Create: apps/dicar-desktop/src/telemetry/ringBuffer.test.ts
- Create: apps/dicar-desktop/src/telemetry/downsample.test.ts
- Create: apps/dicar-desktop/src/components/workbench/WaveformPanel.tsx
- Create: apps/dicar-desktop/src/components/workbench/WaveformCanvas.tsx
- Create: apps/dicar-desktop/src/components/workbench/TelemetryToolbar.tsx
- Create: apps/dicar-desktop/src/components/workbench/TelemetryLegend.tsx
- Create: apps/dicar-desktop/src/components/workbench/TelemetryDataTable.tsx
- Create: apps/dicar-desktop/src/components/workbench/WaveformPanel.test.tsx
- Modify: apps/dicar-desktop/src/pages/LiveWorkbenchPage.tsx
- Modify: apps/dicar-desktop/src/hooks/useAppShortcuts.ts
- Modify: apps/dicar-desktop/src/stores/workspaceStore.ts

**Interfaces:**
- Consumes: ordered UiTelemetryBatch events and Manifest channel descriptors.
- Produces: bounded browser ring buffers, min/max pixel buckets, Canvas view, keyboard cursor, and accessible table/summary.
- Consumed by: Task 11 E2E and visual verification.

- [ ] **Step 1: Write failing bounded-buffer/downsampling tests**

~~~ts
it("retains only sixty seconds per channel at five hundred hertz", () => {
  const buffer = new TelemetryRingBuffer(8, 30_000);
  buffer.append(points(31_000));
  expect(buffer.length(200)).toBe(30_000);
  expect(buffer.first(200).timestampUs).toBe(points(31_000)[1_000].timestampUs);
});

it("preserves extrema when reducing many samples to one pixel column", () => {
  const buckets = minMaxBuckets([{ x: 0, y: -4 }, { x: 0, y: 9 }, { x: 1, y: 2 }], 2);
  expect(buckets[0]).toMatchObject({ min: -4, max: 9 });
});
~~~

- [ ] **Step 2: Implement allocation-stable telemetry buffers**

Use typed arrays or preallocated circular arrays for timestamp and numeric data. Append batches without one React setState per sample. Store flags/u32 without float coercion in the data-table path. Eviction is deterministic and bounded.

- [ ] **Step 3: Write the failing waveform interaction test**

Test that:

- Manifest offers at least 16 candidates;
- selecting a ninth channel is rejected with 最多同时显示 8 个通道;
- a 500 Hz request reaches bridge.setTelemetrySubscription;
- Pause sends setPaused(true) and freezes Canvas state;
- Space only toggles while the waveform region has focus;
- M adds a marker at the current expanded timestamp;
- 1/5/10/30/60 second windows change the visible range;
- arrow keys move the cursor and expose exact time/value text.
- the cursor summary is an aria-live text/table region containing the expanded time plus selected-channel values and units, and it updates after each arrow-key move.

- [ ] **Step 4: Implement Canvas with device-pixel-ratio scaling**

WaveformCanvas uses ResizeObserver, requestAnimationFrame capped by a 30 Hz deadline, devicePixelRatio backing size, and min/max-per-pixel downsampling. Draw grid, axes/units, series, cursor, and markers. Use the design-system palette plus distinct solid/dashed/dotted styles. Do not animate decorative transitions or create one DOM element per point.

- [ ] **Step 5: Implement toolbar, legend, and accessible fallback**

TelemetryToolbar contains channel selection, sample rate, time window, Pause/Resume, cursor toggle, and marker action. TelemetryLegend is keyboard operable and shows style, current value, and unit. TelemetryDataTable shows the latest timestamp and value per channel and a textual connection/paused/disconnected summary. When the cursor is active, an aria-live cursor summary exposes its expanded timestamp and every selected channel's value/unit so historical inspection is not Canvas-only.

- [ ] **Step 6: Prove React update and drawing budgets**

With fake timers and 8x500 Hz batches, assert the store publishes no more than 30 visual revisions per second and retains no more than 8 x 30,000 points. Instrument minMaxBuckets in a test-only adapter and prove output bucket count and draw-segment count are <=2 x canvas CSS pixel width for both 30,000 and 240,000 input points. These deterministic capacity/complexity assertions are the normal CI gate; Task 11 owns the named, machine-timed browser benchmark.

- [ ] **Step 7: Run focused and full frontend gates**

Run telemetry tests, waveform tests, lint, typecheck, Vitest, and build. Require green; the retained-point bound and width-bounded downsampling tests are the normal-CI memory/complexity gates.

- [ ] **Step 8: Commit Task 10**

~~~powershell
git add apps/dicar-desktop
git commit -m 'feat(desktop): render bounded live waveforms'
~~~

---

### Task 11: Lock the end-to-end product flow, accessibility, native build, and handoff

**Files:**
- Create: apps/dicar-desktop/playwright.config.ts
- Create: apps/dicar-desktop/e2e/fixtures.ts
- Create: apps/dicar-desktop/e2e/axe.ts
- Create: apps/dicar-desktop/e2e/home.spec.ts
- Create: apps/dicar-desktop/e2e/live-workbench.spec.ts
- Create: apps/dicar-desktop/e2e/permissions.spec.ts
- Create: apps/dicar-desktop/e2e/revision-conflict.spec.ts
- Create: apps/dicar-desktop/e2e/disconnect.spec.ts
- Create: apps/dicar-desktop/e2e/responsive.spec.ts
- Create: apps/dicar-desktop/e2e/visual.spec.ts
- Create: apps/dicar-desktop/e2e/performance.spec.ts
- Create: scripts/perf/run-waveform-perf.ps1
- Create: apps/dicar-desktop/src-tauri/tests/simulator_bridge.rs
- Modify: apps/dicar-desktop/package.json
- Modify: .github/workflows/ci.yml
- Modify: README.md
- Create: docs/screenshots/dicar-home-1280x720.png through Playwright
- Create: docs/screenshots/dicar-workbench-1280x720.png through Playwright

**Interfaces:**
- Consumes: complete simulator, AppActor, Tauri bridge, and UI.
- Produces: repeatable E2E, visual, axe, native-check, package, and operator instructions.
- This task closes the Windows live-workbench stage only; it does not close the full DiCar product goal.

- [ ] **Step 1: Write the failing complete-flow Playwright test**

~~~ts
test("B home to A workbench completes a real tuning workflow", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "连接模拟车辆" }).click();
  await expect(page.getByText("READY")).toBeVisible();
  await page.getByRole("link", { name: /实时调参与波形/ }).click();
  await page.getByRole("spinbutton", { name: "速度 Kp" }).fill("2.2");
  await expect(page.getByText("未固化 1 项")).toBeVisible();
  await expect(page.getByText("左编码器 PPR")).toBeVisible();
  await selectEightChannels(page);
  await expect(page.getByLabel("实时波形")).toBeVisible();
  await page.getByRole("button", { name: "审阅并固化" }).click();
  await page.getByRole("button", { name: "确认固化到 Flash" }).click();
  await expect(page.getByText(/Generation 1/)).toBeVisible();
});
~~~

Initially run it before wiring the full fixture flow and record the expected failing assertion.

- [ ] **Step 2: Complete deterministic browser fixtures without weakening assertions**

MockBridge uses the exact BridgeEvent shapes and state transitions from Rust, including ACK delay, Revision conflict, storage failure, disconnect, session replacement, 16 candidate channels, and dynamic samples. E2E tests may choose a scenario but cannot bypass permission, dirty, or channel-count logic.

permissions.spec.ts must exercise three page-to-bridge scenarios: Observer cannot write and emits no write/commit operation; Tuner with an active lease can write RAM but Commit remains disabled with 当前身份没有固化权限; Owner without the active lease can do neither. Every scenario asserts the persistent 本地演示权限 label, LeasePanel controller/observer state, and the exact textual denial reason.

revision-conflict.spec.ts injects a device-side write between edit and ACK, then proves the workbench shows the device's current RAM value and Revision, preserves the unresolved user target separately, and sends a retry only after the user explicitly accepts the refreshed base.

- [ ] **Step 3: Add Rust Tauri-to-simulator integration**

simulator_bridge.rs starts SimulatorServer on an ephemeral port, creates AppState, invokes the same command handlers used by Tauri, receives a CoreEvent channel, and proves HELLO -> Manifest -> ParamRead -> ParamWrite -> Telemetry -> Commit -> disconnect. It also selects Observer, Tuner, and Owner-without-lease fixtures and proves rejected paths emit no PARAM_WRITE/PARAM_COMMIT bytes before the allowed Owner path. This is the native bridge proof that browser fixtures cannot provide.

- [ ] **Step 4: Add axe and keyboard-only acceptance**

Use @axe-core/playwright with wcag2a, wcag2aa, wcag21a, and wcag21aa tags on home, diagnostics, workbench, Commit dialog, navigation Sheet, and telemetry table. No rule exclusions or disabled rules are permitted. Keyboard tests cover skip link, card navigation, Ctrl+K, workbench pane order, dialog trap/Escape/return focus, waveform Space/arrows/M, Ctrl+Z, and dirty navigation choices. After waveform arrow movement, assert the screen-reader cursor summary updates with time, selected-channel values, and units.

- [ ] **Step 5: Add responsive and visual snapshots**

Test 1280x720, 1024x768, and 768x1024. For the 200% case, emulate a 640 CSS-pixel content width with `document.documentElement.style.zoom = "2"`, then tab through and assert ChangeBar, Commit, parameter/waveform tabs, and Pause remain visible, focusable, and unobscured. Assert no unexpected horizontal overflow and no fixed bar covers content.

visual.spec.ts uses stable MockBridge fixtures, deterministic fonts/data, hidden caret, and prefers-reduced-motion. At each viewport plus the 200% case, assert Home and connected Workbench with Playwright `toHaveScreenshot` against version-controlled baselines; capture documentation PNGs separately so docs artifacts never substitute for a failing visual assertion. Record the platform-specific pixel-difference threshold in playwright.config.ts and fail on layout/token drift outside it.

performance.spec.ts accepts its dataset and output path from environment variables, warms 20 Canvas frames, then records three 100-frame trials over 240,000 retained points. `scripts/perf/run-waveform-perf.ps1` launches only this spec, writes machine/CPU/OS/Node/Chromium metadata plus all trial medians/p99 values to `artifacts/perf/waveform.json`, and exits nonzero after a confirming second trial violates median <=16 ms or p99 <=50 ms. CI defines a separately named `waveform-performance` job on the `[self-hosted, Windows, X64, dicar-perf-v1]` runner and uploads that JSON; generic hosted runners run the deterministic Task 10 gates but do not claim timing conformance. `artifacts/perf/` is ignored and upload-only; only the script and benchmark test are committed.

- [ ] **Step 6: Run the fresh full verification matrix**

Run from a clean dependency/build state:

~~~powershell
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu fmt --all -- --check
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu check --workspace --all-targets
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu test --workspace --all-targets -- --test-threads=1
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnu run -p dctp-sim --bin generate_vectors -- --check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm test:e2e
C:\Users\tluda\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-msvc check -p dicar-desktop --all-targets
pnpm --filter @dicar/desktop run tauri:build
git diff --check
~~~

Expected: every command exits 0, all Rust/frontend/E2E/axe tests pass, vector output says DCTP v1 vectors match, and an NSIS installer is produced.

The separately binding `waveform-performance` job runs only on `[self-hosted, Windows, X64, dicar-perf-v1]`:

~~~powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/perf/run-waveform-perf.ps1
~~~

It must execute without a skip, exit 0, and upload a non-empty `artifacts/perf/waveform.json`. Generic developer/hosted verification never runs or silently skips this hardware-timed gate.

- [ ] **Step 7: Perform manual visual and native smoke verification**

Launch dctp-sim on 127.0.0.1:7100 and the packaged desktop app. Verify actual WebView connection, B-to-A navigation, one PID write, one encoder write, eight moving channels, Tuner Commit denial, Owner successful Commit, disconnect freeze, reconnect truth, 1280x720 layout, keyboard focus, and Windows close dirty guard. Record exact versions, installer path, screenshots, and any hardware-independent limitations in README.

- [ ] **Step 8: Update CI and operator documentation**

CI runs Rust fmt/Clippy/tests/vectors and Node lint/typecheck/test/build on Windows, Linux, and macOS where supported; Windows additionally checks the Tauri crate. README includes prerequisite paths, bundled-runtime commands, simulator launch, browser mock mode, Tauri dev, tests, packaging, role-fixture warning, and a clear next-stage list: COM/C11 SDK, records/versioning, collaboration, flash, cross-platform/mobile/Web Serial, AI/PID, plugin market, and multi-vehicle.

- [ ] **Step 9: Request two-stage review and close the fixed finding set**

Generate one review package from the stage merge base through current HEAD and give that immutable diff plus the design spec and completed task ledger to two read-only reviewers: first spec compliance, then code quality/security/performance. Record every finding in the ledger. Critical/Important findings enter the bounded fix-and-scoped-re-review loop with a focused RED test and cannot remain open at stage close; Minor findings require an explicit ledger ruling and are re-triaged by the final reviewer. Rerun the full matrix after the single final fix wave.

- [ ] **Step 10: Commit Task 11**

~~~powershell
git add .github README.md apps/dicar-desktop docs/screenshots scripts/perf Cargo.lock pnpm-lock.yaml
git commit -m 'test(desktop): lock the live tuning workflow'
~~~

---

## Stage Completion Evidence

The Windows live-workbench stage is complete only when:

1. A fresh generic Rust + pnpm verification matrix passes, and the separate non-skipping `waveform-performance` job succeeds with its JSON artifact.
2. A packaged Windows executable connects to the real dctp-sim TCP service.
3. B home -> A workbench -> PID/encoder RAM write -> eight-channel waveform -> permission denial -> successful Commit -> disconnect/reconnect passes.
4. PARAM_VALUE proves distinct RAM/Flash values and Commit ACK proves Storage Generation.
5. Visual snapshots, axe, keyboard-only, responsive, 200% zoom, and bounded telemetry tests pass.
6. The branch is clean and independently reviewed.

After this evidence is recorded, continue the persistent full-product goal with the next written subproject plan: C11 SDK + Windows SerialTransport/hardware qualification. Do not mark the overall DiCar application goal complete at this stage.
