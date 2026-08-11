# Transparent Serial Hardware Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add usable nanoUART-wl, HC-05 Bluetooth SPP, and generic COM hardware profiles to the Windows App, including honest port metadata, safe baud probing, connection guidance, and link-aware telemetry limits.

**Architecture:** Extend the existing `SerialTransport` rather than adding a Bluetooth protocol. A typed serial hardware profile travels with the serial endpoint and transport identity; the React connection controller performs sequential, read-only DCTP handshake probes through the existing bridge, while the Rust actor enforces the selected profile's telemetry ceiling.

**Tech Stack:** Rust 2021, `serialport`, existing DCTP session/actor, Tauri 2, React, TypeScript, Zustand, Vitest, Playwright.

## Global Constraints

- DCTP v1 wire bytes and golden vectors do not change.
- A vehicle is `READY` only after COM open, DCTP HELLO, Manifest, and parameter loading.
- Automatic probing sends only the normal DCTP connection handshake; it never writes or commits parameters.
- Only one serial handle and one vehicle session may be active at a time.
- Failed or interrupted connections never replay an old parameter write.
- HC-05 is Windows Bluetooth Classic SPP through the outgoing virtual COM port, not Web Bluetooth.
- HC-05 defaults to a safe 4-channel, 50 Hz budget; 9600 baud defaults to 2 channels, 10 Hz.
- nanoUART-wl defaults to 460800 baud.
- Raw HC-05 UART IO is treated as 3.3 V logic.

---

### Task 1: Typed hardware profiles and serial port metadata

**Files:**
- Create: `crates/dicar-app-core/src/hardware_profile.rs`
- Modify: `crates/dicar-app-core/src/transport/mod.rs`
- Modify: `crates/dicar-app-core/src/transport/serial.rs`
- Modify: `crates/dicar-app-core/src/lib.rs`
- Test: `crates/dicar-app-core/tests/serial_transport.rs`

**Interfaces:**
- Produces `SerialHardwareProfile::{NanoUartWl,Hc05BluetoothSpp,GenericSerial}`.
- Produces `SerialHardwareProfile::recommended_baud_rate() -> u32`.
- Produces `SerialHardwareProfile::probe_baud_rates() -> &'static [u32]`.
- Produces `SerialHardwareProfile::telemetry_budget(baud_rate: u32) -> TelemetryBudget`.
- Produces `SerialPortKind::{Usb,Bluetooth,Pci,Unknown}` on `SerialPortDescriptor.port_kind`.
- Extends `Endpoint::Serial` with `hardware_profile: SerialHardwareProfile`.

- [ ] **Step 1: Write failing profile and metadata tests**

Add table-driven assertions to `serial_transport.rs`:

```rust
assert_eq!(
    SerialHardwareProfile::Hc05BluetoothSpp.probe_baud_rates(),
    &[115_200, 9_600, 38_400, 57_600, 230_400, 460_800],
);
assert_eq!(
    SerialHardwareProfile::Hc05BluetoothSpp.telemetry_budget(9_600),
    TelemetryBudget { max_channels: 2, max_sample_rate_hz: 10 },
);
assert_eq!(
    SerialHardwareProfile::Hc05BluetoothSpp.telemetry_budget(115_200),
    TelemetryBudget { max_channels: 4, max_sample_rate_hz: 50 },
);
```

Extend discovery tests so a scripted Bluetooth port becomes `SerialPortKind::Bluetooth`, while unknown ports remain selectable as `Unknown`.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
cargo +stable-x86_64-pc-windows-msvc test -p dicar-app-core --test serial_transport
```

Expected: compilation fails because the profile, budget, port kind, and endpoint field do not exist.

- [ ] **Step 3: Implement the minimal core types**

Create focused value types:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SerialHardwareProfile {
    NanoUartWl,
    Hc05BluetoothSpp,
    GenericSerial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryBudget {
    pub max_channels: u8,
    pub max_sample_rate_hz: u16,
}
```

Expand serial validation to exactly `9600, 38400, 57600, 115200, 230400, 460800, 921600`. Map `serialport::SerialPortType` to the explicit `SerialPortKind`, without guessing Bluetooth from a COM name.

- [ ] **Step 4: Run focused and affected actor/bridge tests**

Run:

```powershell
cargo +stable-x86_64-pc-windows-msvc test -p dicar-app-core --test serial_transport
cargo +stable-x86_64-pc-windows-msvc test -p dicar-app-core --test actor_integration
```

Expected: all tests pass; simulator behavior remains unchanged.

- [ ] **Step 5: Commit Task 1**

```powershell
git add crates/dicar-app-core
git commit -m "feat(core): model transparent serial hardware profiles"
```

---

### Task 2: Bridge types, HC-05 guidance, and safe baud probing

**Files:**
- Modify: `apps/dicar-desktop/src/domain/types.ts`
- Create: `apps/dicar-desktop/src/domain/hardwareProfiles.ts`
- Create: `apps/dicar-desktop/src/domain/serialConnection.ts`
- Modify: `apps/dicar-desktop/src/bridge/mockBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/webSerialBridge.ts`
- Modify: `apps/dicar-desktop/src-tauri/src/commands.rs`
- Modify: `apps/dicar-desktop/src/components/shell/ConnectionStatusBar.tsx`
- Create: `apps/dicar-desktop/src/components/shell/HardwareConnectionGuide.tsx`
- Modify: `apps/dicar-desktop/src/stores/settingsStore.ts`
- Test: `apps/dicar-desktop/src/components/shell/ConnectionStatusBar.test.tsx`
- Test: `apps/dicar-desktop/src/domain/serialConnection.test.ts`
- Test: `apps/dicar-desktop/src-tauri/tests/commands.rs`

**Interfaces:**
- Produces TypeScript `SerialHardwareProfile` and `SerialPortKind` unions matching Rust serialization.
- Extends serial `Endpoint` with `hardwareProfile`.
- Produces `connectSerialWithProbe(bridge, request, onAttempt) -> Promise<SerialProbeResult>`.
- Produces `HardwareConnectionGuide` for nanoUART-wl, HC-05, and generic serial.

- [ ] **Step 1: Write failing DTO and connection-controller tests**

The controller test uses a fake bridge that fails two attempts and succeeds on the third:

```ts
const result = await connectSerialWithProbe(bridge, {
  hardwareProfile: "hc05BluetoothSpp",
  portName: "COM12",
  baudRate: "auto",
}, onAttempt);

expect(connect).toHaveBeenNthCalledWith(1, serialEndpoint("COM12", 115200, "hc05BluetoothSpp"));
expect(connect).toHaveBeenNthCalledWith(2, serialEndpoint("COM12", 9600, "hc05BluetoothSpp"));
expect(connect).toHaveBeenNthCalledWith(3, serialEndpoint("COM12", 38400, "hc05BluetoothSpp"));
expect(result.baudRate).toBe(38400);
```

Also assert that a successful attempt stops the sequence, all failures remain disconnected, and no write/commit bridge method is called.

- [ ] **Step 2: Run focused frontend and Tauri tests and verify RED**

Run:

```powershell
pnpm --filter @dicar/desktop test --run src/domain/serialConnection.test.ts src/components/shell/ConnectionStatusBar.test.tsx
cargo +stable-x86_64-pc-windows-msvc test -p dicar-desktop --test commands
```

Expected: missing profile fields, controller, guide, and DTO mapping cause failure.

- [ ] **Step 3: Implement shared frontend profile definitions**

Define exact presets:

```ts
export const HARDWARE_PROFILES = {
  nanoUartWl: { label: "nanoUART-wl", recommendedBaudRate: 460800, probeBaudRates: [460800, 230400, 115200] },
  hc05BluetoothSpp: { label: "HC-05 蓝牙串口", recommendedBaudRate: 115200, probeBaudRates: [115200, 9600, 38400, 57600, 230400, 460800] },
  genericSerial: { label: "通用串口", recommendedBaudRate: 115200, probeBaudRates: [115200] },
} as const;
```

`connectSerialWithProbe` calls only `bridge.connect(endpoint)`. It waits for each result, stops on `succeeded`, and records each attempted rate. It does not call parameter or telemetry operations.

- [ ] **Step 4: Implement the connection UI and saved settings**

Add a hardware selector before the COM selector. Mark descriptors whose `portKind` is `bluetooth`; keep unknown ports available. Show:

- HC-05 Windows pairing and outgoing-COM instructions.
- TX/RX crossover, common ground, and 3.3 V logic warning.
- nanoUART-wl 3V3/GND/TX/RX instructions.
- Current probe rate and final DCTP failure message.

Persist only the last successful `{ hardwareProfile, portName, baudRate }` in `settingsStore`; never auto-connect on launch.

- [ ] **Step 5: Run frontend, Tauri, and browser contract tests**

Run:

```powershell
pnpm --filter @dicar/desktop test --run src/domain/serialConnection.test.ts src/components/shell/ConnectionStatusBar.test.tsx src/bridge/bridge.test.ts src/bridge/webSerialBridge.test.ts
cargo +stable-x86_64-pc-windows-msvc test -p dicar-desktop --test commands
```

Expected: tests pass; Web Serial remains explicitly user-authorized and does not claim Bluetooth support.

- [ ] **Step 6: Commit Task 2**

```powershell
git add apps/dicar-desktop apps/dicar-desktop/src-tauri
git commit -m "feat(desktop): guide HC-05 and nanoUART connections"
```

---

### Task 3: Enforce telemetry budgets and expose link diagnostics

**Files:**
- Create: `crates/dicar-app-core/src/link_budget.rs`
- Modify: `crates/dicar-app-core/src/actor.rs`
- Modify: `crates/dicar-app-core/src/bridge_model.rs`
- Modify: `crates/dicar-app-core/src/lib.rs`
- Test: `crates/dicar-app-core/tests/actor_integration.rs`
- Modify: `apps/dicar-desktop/src/domain/types.ts`
- Modify: `apps/dicar-desktop/src/components/workbench/WaveformPanel.tsx`
- Modify: `apps/dicar-desktop/src/components/workbench/TelemetryToolbar.tsx`
- Test: `apps/dicar-desktop/src/components/workbench/WaveformPanel.test.tsx`
- Modify: `apps/dicar-desktop/src/pages/DiagnosticsPage.tsx`

**Interfaces:**
- Produces `LinkBudgetSnapshot { hardware_profile, max_channels, max_sample_rate_hz, reason }` in `AppSnapshot`.
- Produces `validate_subscription(endpoint, channel_count, sample_rate_hz) -> Result<TelemetryBudget, LinkBudgetError>`.
- Consumes the connected serial endpoint's typed hardware profile and baud rate.

- [ ] **Step 1: Write failing core budget tests**

Connect a scripted HC-05 endpoint and assert:

```rust
assert!(validate_subscription(&endpoint, 4, 50).is_ok());
assert_eq!(
    validate_subscription(&endpoint, 5, 50).unwrap_err().to_string(),
    "HC-05 当前链路最多 4 个通道",
);
assert_eq!(
    validate_subscription(&endpoint, 4, 100).unwrap_err().to_string(),
    "HC-05 当前链路最高 50 Hz",
);
```

At 9600 baud, assert 2 channels × 10 Hz. Simulator and generic 921600 retain 8 channels × 500 Hz.

- [ ] **Step 2: Run the focused actor test and verify RED**

Run:

```powershell
cargo +stable-x86_64-pc-windows-msvc test -p dicar-app-core --test actor_integration
```

Expected: over-budget subscriptions currently pass and no link budget exists in the snapshot.

- [ ] **Step 3: Add actor-side enforcement before protocol writes**

Call `validate_subscription` before allocating a subscription version or sending `TELEMETRY_SUBSCRIBE`. A rejected request must produce zero transport writes and a stable Chinese reason. Populate `link_budget` from the active transport identity; disconnected snapshots use `None`.

- [ ] **Step 4: Add UI limits and diagnostic presentation**

The waveform toolbar reads `snapshot.linkBudget`, offers only allowed rates, blocks channels above the limit, and shows the exact reason. The diagnostics page displays profile, baud rate, safe ceiling, CRC errors, retries, sequence gaps, and UI/device drops already present in `DiagnosticsSnapshot`.

- [ ] **Step 5: Run focused Rust and frontend tests**

Run:

```powershell
cargo +stable-x86_64-pc-windows-msvc test -p dicar-app-core --test actor_integration
pnpm --filter @dicar/desktop test --run src/components/workbench/WaveformPanel.test.tsx src/pages/DiagnosticsPage.test.tsx
```

Expected: HC-05 limits and generic/simulator ceilings pass without changing DCTP vectors.

- [ ] **Step 6: Commit Task 3**

```powershell
git add crates/dicar-app-core apps/dicar-desktop
git commit -m "feat(core): enforce serial telemetry budgets"
```

---

### Task 4: Package and qualify the Windows release

**Files:**
- Modify: `README.md`
- Modify: `apps/dicar-desktop/package.json`
- Modify: `apps/dicar-desktop/src-tauri/Cargo.toml`
- Modify: `apps/dicar-desktop/src-tauri/tauri.conf.json`
- Test: `apps/dicar-desktop/e2e/critical-flow.spec.ts`

**Interfaces:**
- Produces DiCar Tune version `0.1.2` Windows installer and portable executable.
- Documents exact nanoUART-wl and HC-05 connection steps and limitations.

- [ ] **Step 1: Add the critical UI flow**

Extend Playwright with deterministic mock cases for selecting HC-05, seeing Bluetooth/outgoing-COM guidance, auto-probe progress, a failed DCTP handshake that remains disconnected, and the 4×50 Hz ceiling.

- [ ] **Step 2: Run fresh full quality gates**

Run:

```powershell
cargo +stable-x86_64-pc-windows-msvc fmt --all -- --check
cargo +stable-x86_64-pc-windows-msvc clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-msvc test --workspace --all-targets
cargo +stable-x86_64-pc-windows-msvc run -p dctp-sim --bin generate_vectors -- --check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm --filter @dicar/desktop test:e2e
```

Expected: every command exits 0, no test failure, and vector check prints `DCTP v1 vectors match`.

- [ ] **Step 3: Build and smoke-test 0.1.2**

Build the Tauri NSIS bundle with the MSVC environment, copy installer and portable executable to `C:\DiCar_LAB\release`, launch the portable executable, verify its process stays alive, then stop only that exact test PID.

- [ ] **Step 4: Remove superseded stage artifacts**

After verifying 0.1.2 hashes and launch, delete the superseded 0.1.1 installer/portable files and task-specific isolated build directories. Keep source, tests, the current release, and reusable main build caches.

- [ ] **Step 5: Commit Task 4**

```powershell
git add README.md apps/dicar-desktop/package.json apps/dicar-desktop/src-tauri/Cargo.toml apps/dicar-desktop/src-tauri/tauri.conf.json apps/dicar-desktop/e2e/critical-flow.spec.ts
git commit -m "release: package HC-05 hardware compatibility"
```
