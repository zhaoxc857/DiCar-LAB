# Simulator PID Closed Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This project must be executed inline; the user explicitly prohibited subagents.

**Goal:** Make Rust `dctp-sim` and frontend `MockBridge` expose a PID-responsive speed loop that the AI tuning wizard can run safely end to end without hardware.

**Architecture:** Rust and TypeScript each get a small, isolated `SpeedLoopModel` with identical constants and behavioral semantics. Existing DCTP messages carry aligned manifest metadata; device and Bridge layers bind parameters by stable machine name, drive the model with a fixed 2 ms internal step, and sample it at the requested telemetry rate. The wizard treats each experiment as an isolated transaction: it saves target/subscription state, supports prompt cancellation, and restores the experimental state on every exit path.

**Tech Stack:** Rust 2021 workspace, TypeScript 5.9, React 19, Vitest 3/jsdom, Tauri 2, C99 test shim, DCTP v1.

## Global Constraints

- Do not change the DCTP v1 wire format, message IDs, telemetry channel IDs, or committed golden vectors.
- Keep `control.target_speed_mps` writable, dangerous, and non-persistent; AI may write RAM but never Flash.
- Keep Kp/Ki/Kd persistent and manually commit-only.
- Do not add vehicle dynamics to `firmware/dctp-device`; only synchronize its test shim manifest.
- Use stable names `pid.kp`, `pid.speed.ki`, `pid.speed.kd`, and `control.target_speed_mps` in Rust, C shim, Mock, and YAML.
- Use fixed 2 ms internal integration, common model constants, and behavioral tolerance rather than bit-identical Rust/JS results.
- Keep unrelated telemetry channels deterministic and preserve existing sequence/drop/timestamp protocol semantics.
- Do not call a real DeepSeek endpoint in tests.
- Do not use subagents.

## File Structure

- Create `crates/dctp-sim/src/speed_loop.rs`: pure Rust PID/vehicle model and model unit tests.
- Modify `crates/dctp-sim/src/lib.rs`: register the internal model module.
- Modify `crates/dctp-sim/src/device.rs`: align the default manifest, read model parameters, advance the plant, and map dynamic telemetry.
- Modify `crates/dctp-sim/tests/session_flow.rs`: lock new descriptor metadata and dynamic response behavior.
- Modify `crates/dctp-sim/tests/final_review.rs` and `crates/dctp-sim/tests/e2e_wire.rs`: update value assertions while preserving timing/drop contracts.
- Modify `crates/dctp-device-c/shim/dctp_test_shim.c`: mirror the Rust default manifest exactly.
- Create `apps/dicar-desktop/src/tuning/speedLoopModel.ts`: pure TypeScript PID/vehicle model.
- Create `apps/dicar-desktop/src/tuning/speedLoopModel.test.ts`: behavior tests for the TypeScript model.
- Modify `apps/dicar-desktop/src/bridge/mockBridge.ts`: align fixtures, validate writes, drive dynamic telemetry, and manage real-time scheduling.
- Modify `apps/dicar-desktop/src/bridge/bridge.test.ts`: Mock lifecycle, sampling, validation, persistence, and dynamics tests.
- Modify `apps/dicar-desktop/src/bridge/desktopBridge.ts`: add explicit subscription clearing.
- Modify `apps/dicar-desktop/src/bridge/tauriBridge.ts` and its test: invoke the clear command.
- Modify `crates/dicar-app-core/src/actor.rs` and actor tests: add `ClearTelemetrySubscription` and clear desired/active state after existing `TELEMETRY_STOP`.
- Modify `apps/dicar-desktop/src-tauri/src/commands.rs` and `src-tauri/src/lib.rs`: expose `clear_telemetry_subscription`.
- Modify `apps/dicar-desktop/src/ai/aiClient.ts`; create `aiClient.test.ts`: external cancellation plus internal timeout distinction.
- Modify `apps/dicar-desktop/src/tuning/autoTune.ts` and test: classify an aborted null experiment correctly and pass cancellation to AI.
- Modify `apps/dicar-desktop/src/components/workbench/AutoTuneWizard.tsx` and test: validate step inputs, abort sleeps/network, and restore target/subscription state in `finally`.
- Modify `apps/dicar-desktop/src/vehicleProfiles/builtins/dicar-diff-drive.yaml` and store tests: enable the built-in loop with target/Kp/Ki/Kd.
- Modify `docs/development.md` and `HANDOFF.md`: document the closed-loop simulator and remove the completed roadmap item.

---

### Task 1: Rust Speed Loop Model

**Files:**
- Create: `crates/dctp-sim/src/speed_loop.rs`
- Modify: `crates/dctp-sim/src/lib.rs`

**Interfaces:**
- Consumes: finite `SpeedLoopInput { target_mps, kp, ki, kd }` and a monotonic `timestamp_us: u64`.
- Produces: `pub(crate) struct SpeedLoopModel`, `pub(crate) struct SpeedLoopSnapshot`, `SpeedLoopModel::reset()`, `SpeedLoopModel::advance_to(timestamp_us, input)`, and `SpeedLoopModel::snapshot(input)`.

- [ ] **Step 1: Add failing model behavior tests**

Add unit tests inside `speed_loop.rs` that exercise the intended public surface before its implementation:

```rust
#[test]
fn zero_target_stays_stopped_and_finite() {
    let mut model = SpeedLoopModel::default();
    let input = SpeedLoopInput { target_mps: 0.0, kp: 1.2, ki: 0.08, kd: 0.002 };
    model.advance_to(3_000_000, input);
    let state = model.snapshot(input);
    assert_eq!(state.speed_mps, 0.0);
    assert!(state.motor_output.is_finite());
}

#[test]
fn default_step_rises_and_reaches_a_stable_finite_response() {
    let mut model = SpeedLoopModel::default();
    let input = SpeedLoopInput { target_mps: 1.0, kp: 1.2, ki: 0.08, kd: 0.002 };
    model.advance_to(500_000, input);
    let early = model.snapshot(input).speed_mps;
    model.advance_to(3_000_000, input);
    let late = model.snapshot(input).speed_mps;
    assert!(early > 0.1 && early < late);
    assert!(late > 0.75 && late <= MAX_SPEED_MPS);
}

#[test]
fn zero_target_clears_integrator_between_repeated_steps() {
    let mut model = SpeedLoopModel::default();
    let run = SpeedLoopInput { target_mps: 1.0, kp: 0.6, ki: 0.5, kd: 0.01 };
    model.advance_to(3_000_000, run);
    let stop = SpeedLoopInput { target_mps: 0.0, ..run };
    model.advance_to(3_800_000, stop);
    assert!(model.snapshot(stop).speed_mps.abs() < 0.08);
    model.advance_to(6_800_000, run);
    assert!(model.snapshot(run).speed_mps > 0.7);
}
```

- [ ] **Step 2: Run the focused Rust test and verify red**

Run: `cargo test -p dctp-sim speed_loop --lib`

Expected: FAIL because `speed_loop` types and methods are not implemented/exported.

- [ ] **Step 3: Implement the minimal isolated model**

Use these exact constants and equations in `speed_loop.rs`:

```rust
const CONTROL_STEP_US: u64 = 2_000;
const CONTROL_DT_S: f32 = 0.002;
pub(crate) const MAX_SPEED_MPS: f32 = 4.0;
const PLANT_TAU_S: f32 = 0.25;
const DERIVATIVE_TAU_S: f32 = 0.03;
const INTEGRAL_LIMIT: f32 = 4.0;

pub(crate) struct SpeedLoopInput { pub target_mps: f32, pub kp: f32, pub ki: f32, pub kd: f32 }
pub(crate) struct SpeedLoopSnapshot { pub speed_mps: f32, pub error_mps: f32, pub motor_output: f32 }

impl SpeedLoopModel {
    pub(crate) fn advance_to(&mut self, timestamp_us: u64, input: SpeedLoopInput) {
        while self.timestamp_us.saturating_add(CONTROL_STEP_US) <= timestamp_us {
            self.step(input);
            self.timestamp_us = self.timestamp_us.saturating_add(CONTROL_STEP_US);
        }
    }
}
```

`step` must reset the integrator when `target_mps.abs() <= f32::EPSILON`, low-pass the measurement derivative, conditionally integrate only when output saturation is not being driven farther into saturation, clamp motor output to `[-1, 1]`, and update the first-order plant with `alpha = 1.0 - exp(-dt/tau)`.

- [ ] **Step 4: Run model tests and verify green**

Run: `cargo test -p dctp-sim speed_loop --lib`

Expected: PASS for zero target, default step, repeated-step reset, and bounded outputs.

- [ ] **Step 5: Commit the Rust model**

```powershell
git add -- crates/dctp-sim/src/speed_loop.rs crates/dctp-sim/src/lib.rs
git commit -m "feat(sim): add deterministic speed loop model"
```

### Task 2: Rust Device, Manifest, and C Shim Integration

**Files:**
- Modify: `crates/dctp-sim/src/device.rs`
- Modify: `crates/dctp-sim/tests/session_flow.rs`
- Modify: `crates/dctp-sim/tests/final_review.rs`
- Modify: `crates/dctp-sim/tests/e2e_wire.rs`
- Modify: `crates/dctp-device-c/shim/dctp_test_shim.c`

**Interfaces:**
- Consumes: Task 1 `SpeedLoopModel`, `SpeedLoopInput`, and `SpeedLoopSnapshot`.
- Produces: default manifest parameters IDs 1–4, parameter-driven dynamic channel values, and a C shim manifest byte-identical to Rust.

- [ ] **Step 1: Write failing manifest and response tests**

Extend `session_flow.rs` expected parameters with:

```rust
(1, "pid.kp", ParamType::F32, true, numeric_f32(0.0, 20.0, 0.01)),
(2, "pid.speed.ki", ParamType::F32, true, numeric_f32(0.0, 5.0, 0.001)),
(3, "pid.speed.kd", ParamType::F32, true, numeric_f32(0.0, 1.0, 0.0001)),
(4, "control.target_speed_mps", ParamType::F32, true, numeric_f32(0.0, 8.0, 0.05)),
```

Add assertions that ID 4 has `DANGEROUS`, lacks `PERSISTENT`, and returns `persisted_value == None`. Add an integration test that opens a session, subscribes to channels `[207, 200, 208, 209]`, writes target ID 4 to `1.0`, ticks through 3 seconds at 100 Hz, and asserts target is 1, speed rises above 0.7, error falls, and PWM stays at or below 1000.

- [ ] **Step 2: Run the focused tests and verify red**

Run: `cargo test -p dctp-sim --test session_flow default_manifest`

Expected: FAIL because IDs 2–4 and flags are absent.

Run: `cargo test -p dctp-sim --test session_flow speed_loop_telemetry_responds_to_ram_parameters`

Expected: FAIL because the test or dynamic values are absent.

- [ ] **Step 3: Add aligned descriptors and stateful device sampling**

In `device.rs`:

- add IDs 2–4 with the exact metadata above;
- set target flags to `ParamFlags::WRITABLE | ParamFlags::DANGEROUS` only;
- add `speed_loop: SpeedLoopModel` to `SimDevice` and reset it on a new session;
- read current numeric values by machine name with defaults that remain finite;
- advance the plant to every due sample timestamp, including skipped history before the first emitted sample;
- map dynamic channels from one `SpeedLoopSnapshot` and keep the existing fallback for all other names.

Use PWM conversion:

```rust
let pwm = (snapshot.motor_output.abs().clamp(0.0, 1.0) * 1_000.0).round() as u32;
```

Use left/right wheel factors `0.99` and `1.01` and clamp them to `MAX_SPEED_MPS`.

- [ ] **Step 4: Synchronize the C shim manifest**

In `dctp_test_shim.c`, insert the exact IDs 2–4 after ID 1, use the same values/ranges/steps/labels/groups/units, and define target flags without `DCTP_PARAM_PERSISTENT`:

```c
#define RAM_DANGEROUS (DCTP_PARAM_WRITABLE | DCTP_PARAM_DANGEROUS)
```

Do not change `read_channel_value` or production C library files.

- [ ] **Step 5: Update value-specific tests without weakening protocol assertions**

Keep timestamp, sequence, dropped-count, capacity, and type checks unchanged. Replace old hard-coded synthetic speed/encoder expectations only where the new model legitimately changes the sampled value; continue asserting finite f32 and exact deterministic fallback values for unrelated/custom channels.

- [ ] **Step 6: Run Rust and C cross-contract tests**

Run: `cargo test -p dctp-sim --all-targets`

Expected: PASS.

Run: `cargo test -p dctp-device-c --all-targets`

Expected: PASS including byte-identical manifest and CRC tests.

Run: `cargo run -p dctp-sim --bin generate_vectors -- --check`

Expected: PASS with no vector changes.

- [ ] **Step 7: Commit device and manifest integration**

```powershell
git add -- crates/dctp-sim/src/device.rs crates/dctp-sim/tests/session_flow.rs crates/dctp-sim/tests/final_review.rs crates/dctp-sim/tests/e2e_wire.rs crates/dctp-device-c/shim/dctp_test_shim.c
git commit -m "feat(sim): drive telemetry from PID parameters"
```

### Task 3: TypeScript Speed Loop Model

**Files:**
- Create: `apps/dicar-desktop/src/tuning/speedLoopModel.ts`
- Create: `apps/dicar-desktop/src/tuning/speedLoopModel.test.ts`

**Interfaces:**
- Consumes: `SpeedLoopInput { targetMps, kp, ki, kd }` and monotonic `timestampUs`.
- Produces: `SpeedLoopModel.reset()`, `advanceTo(timestampUs, input)`, and `snapshot(input): SpeedLoopSnapshot` with `speedMps`, `errorMps`, and `motorOutput`.

- [ ] **Step 1: Write failing parity-property tests**

Create tests matching Task 1 properties plus gain sensitivity:

```ts
it("changes the three-second response when PID gains change", () => {
  const low = response({ targetMps: 1, kp: 0.4, ki: 0.02, kd: 0 });
  const higher = response({ targetMps: 1, kp: 1.2, ki: 0.5, kd: 0.002 });
  expect(higher.speedMps).toBeGreaterThan(low.speedMps + 0.05);
  expect(Math.abs(higher.motorOutput)).toBeLessThanOrEqual(1);
});
```

- [ ] **Step 2: Run the focused test and verify red**

Run from `apps/dicar-desktop`: `pnpm exec vitest run src/tuning/speedLoopModel.test.ts`

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the TypeScript model with Task 1 constants**

Use exported types and class:

```ts
export interface SpeedLoopInput { targetMps: number; kp: number; ki: number; kd: number }
export interface SpeedLoopSnapshot { speedMps: number; errorMps: number; motorOutput: number }
export class SpeedLoopModel {
  reset(): void;
  advanceTo(timestampUs: number, input: SpeedLoopInput): void;
  snapshot(input: SpeedLoopInput): SpeedLoopSnapshot;
}
```

Mirror the Rust equations/constants, sanitize non-finite inputs to safe defaults, and never expose non-finite output.

- [ ] **Step 4: Run the focused tests and verify green**

Run: `pnpm exec vitest run src/tuning/speedLoopModel.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit the TypeScript model**

```powershell
git add -- apps/dicar-desktop/src/tuning/speedLoopModel.ts apps/dicar-desktop/src/tuning/speedLoopModel.test.ts
git commit -m "feat(app): add deterministic speed loop model"
```

### Task 4: MockBridge Dynamics, Validation, and Real-Time Sampling

**Files:**
- Modify: `apps/dicar-desktop/src/bridge/mockBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/bridge.test.ts`
- Modify: `apps/dicar-desktop/src/pages/HomePage.test.tsx`
- Modify: `apps/dicar-desktop/src/vehicleProfiles/builtins/dicar-diff-drive.yaml`
- Modify: `apps/dicar-desktop/src/stores/vehicleProfileStore.test.ts`

**Interfaces:**
- Consumes: Task 3 `SpeedLoopModel`.
- Produces: aligned Mock fixtures, rate-correct manual batches, listener-scoped real-time scheduling, and an eligible built-in speed loop.

- [ ] **Step 1: Add failing Mock contract tests**

In `bridge.test.ts`, use fake timers and collect telemetry batches to assert:

```ts
it("uses the requested 100 Hz period and produces telemetry while observed", async () => {
  vi.useFakeTimers();
  const bridge = new MockBridge();
  const batches: BridgeEvent[] = [];
  const unsubscribe = await bridge.subscribe((event) => { if (event.event === "telemetryBatch") batches.push(event); });
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  await bridge.setTelemetrySubscription({ channelIds: [207, 200], sampleRateHz: 100 });
  const before = batches.length;
  await vi.advanceTimersByTimeAsync(100);
  expect(batches.length).toBeGreaterThan(before);
  const points = batches.at(-1)!.event === "telemetryBatch" ? batches.at(-1)!.data.points : [];
  expect(points.at(-2)!.timestampUs - points.at(-4)!.timestampUs).toBe(10_000);
  unsubscribe();
  vi.useRealTimers();
});
```

Add tests for target write not changing `dirtyCount`, NaN/Infinity/out-of-range rejection, target/PID changing speed/error/PWM, and scheduler stopping on pause/disconnect/unsubscribe.

- [ ] **Step 2: Add failing built-in profile eligibility test**

Update `simulatorParameters()` to include IDs 2–4, then assert the resolved loop has `targetParamId == 4`, writable target, and gains `[1,2,3]` mapped to Kp/Ki/Kd.

- [ ] **Step 3: Run focused tests and verify red**

Run: `pnpm exec vitest run src/bridge/bridge.test.ts src/stores/vehicleProfileStore.test.ts`

Expected: FAIL for missing scheduling, metadata, validation, and YAML mappings.

- [ ] **Step 4: Align fixtures and write semantics**

Refactor `numericParameter` to accept `{ persistent?: boolean; dangerous?: boolean }`. Create target ID 4 with `persistent: false`, default `0`, and `dangerous: true`; set its `persistedValue` to `null`. Keep IDs 1–3 persistent.

Before accepting numeric Mock writes, reject non-finite values and values outside `record.numeric.min/max`. Set `dirty` only when `persistedValue !== null` and the value differs. Commit/revert must continue to touch only dirty persistent records.

- [ ] **Step 5: Drive channel values from the model**

Add a `SpeedLoopModel` field. Reset only vehicle state on successful simulator connect; preserve accepted RAM parameters. In `advanceTelemetry`, calculate period from `activeSubscription.sampleRateHz`, advance timestamp and model per sample, take one snapshot, and map the seven dynamic channel names. Preserve deterministic fallback for the remaining channels.

- [ ] **Step 6: Add the lifecycle-controlled scheduler**

Use one private timer handle and a monotonic wall-clock anchor. Reconcile it after subscribe/connect/subscription/pause/disconnect. A 20 ms timer computes due samples from elapsed time and requested period, caps each burst to avoid blocking, then calls the same manual advance path. The unsubscribe closure must reconcile after deleting the listener.

- [ ] **Step 7: Update built-in YAML**

Add exactly:

```yaml
    target_parameter: control.target_speed_mps
    gains:
      Kp: pid.kp
      Ki: pid.speed.ki
      Kd: pid.speed.kd
```

- [ ] **Step 8: Run focused frontend tests**

Run: `pnpm exec vitest run src/tuning/speedLoopModel.test.ts src/bridge/bridge.test.ts src/stores/vehicleProfileStore.test.ts src/pages/HomePage.test.tsx`

Expected: PASS; update the Home parameter-count assertion from 19 to 19 only if the Mock count remains 19, otherwise to the exact resulting count after confirming the fixture list.

- [ ] **Step 9: Commit Mock and profile integration**

```powershell
git add -- apps/dicar-desktop/src/bridge/mockBridge.ts apps/dicar-desktop/src/bridge/bridge.test.ts apps/dicar-desktop/src/pages/HomePage.test.tsx apps/dicar-desktop/src/vehicleProfiles/builtins/dicar-diff-drive.yaml apps/dicar-desktop/src/stores/vehicleProfileStore.test.ts
git commit -m "feat(app): run a live PID simulator in MockBridge"
```

### Task 5: Explicitly Clear Telemetry Subscriptions

**Files:**
- Modify: `crates/dicar-app-core/src/actor.rs`
- Modify: `crates/dicar-app-core/tests/actor_integration.rs`
- Modify: `apps/dicar-desktop/src-tauri/src/commands.rs`
- Modify: `apps/dicar-desktop/src-tauri/src/lib.rs`
- Modify: `apps/dicar-desktop/src/bridge/desktopBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/mockBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/tauriBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/tauriBridge.test.ts`
- Modify: `apps/dicar-desktop/src/bridge/bridge.test.ts`

**Interfaces:**
- Produces: `DesktopBridge.clearTelemetrySubscription(): Promise<OperationResult>` and `CoreCommand::ClearTelemetrySubscription`.
- Semantics: send existing `TELEMETRY_STOP` if connected, flush telemetry, set desired/active to `None`/`null`, and set paused true.

- [ ] **Step 1: Add failing Core state test**

In `actor_integration.rs`, connect, set a subscription, dispatch `ClearTelemetrySubscription`, wait for completion, then assert:

```rust
assert!(actor.snapshot().desired_subscription.is_none());
assert!(actor.snapshot().active_subscription.is_none());
assert!(actor.snapshot().paused);
```

- [ ] **Step 2: Add failing Bridge invocation tests**

In `tauriBridge.test.ts`, call `clearTelemetrySubscription()` and expect `{ command: "clear_telemetry_subscription" }` in the exact invoke sequence. In `bridge.test.ts`, clear a Mock subscription and assert both snapshot fields are null and timers stop producing points.

- [ ] **Step 3: Run focused tests and verify red**

Run: `cargo test -p dicar-app-core --test actor_integration clear_telemetry_subscription`

Run: `pnpm exec vitest run src/bridge/tauriBridge.test.ts src/bridge/bridge.test.ts`

Expected: FAIL because the command/methods do not exist.

- [ ] **Step 4: Implement Core clear semantics**

Add the enum variant and handler. Factor a private stop helper only if it removes duplication between pause and clear. The clear path must call `TELEMETRY_STOP`, flush buffered telemetry, then set:

```rust
self.desired_subscription = None;
self.active_subscription = None;
self.paused = true;
self.accumulator = None;
```

- [ ] **Step 5: Wire Tauri and frontend Bridge implementations**

Expose a zero-argument Tauri command, register it in `command_handler`, invoke it from `TauriBridge`, and implement the same state transition plus scheduler reconciliation in `MockBridge`.

- [ ] **Step 6: Run focused Core/Tauri/frontend tests**

Run: `cargo test -p dicar-app-core --test actor_integration clear_telemetry_subscription`

Run: `cargo test -p dicar-desktop --features native-check --lib`

Run: `pnpm exec vitest run src/bridge/tauriBridge.test.ts src/bridge/bridge.test.ts`

Expected: PASS.

- [ ] **Step 7: Commit the clear-subscription API**

```powershell
git add -- crates/dicar-app-core/src/actor.rs crates/dicar-app-core/tests/actor_integration.rs apps/dicar-desktop/src-tauri/src/commands.rs apps/dicar-desktop/src-tauri/src/lib.rs apps/dicar-desktop/src/bridge/desktopBridge.ts apps/dicar-desktop/src/bridge/mockBridge.ts apps/dicar-desktop/src/bridge/tauriBridge.ts apps/dicar-desktop/src/bridge/tauriBridge.test.ts apps/dicar-desktop/src/bridge/bridge.test.ts
git commit -m "feat(core): clear telemetry subscriptions explicitly"
```

### Task 6: AI Cancellation and Experiment-State Restoration

**Files:**
- Modify: `apps/dicar-desktop/src/ai/aiClient.ts`
- Create: `apps/dicar-desktop/src/ai/aiClient.test.ts`
- Modify: `apps/dicar-desktop/src/tuning/autoTune.ts`
- Modify: `apps/dicar-desktop/src/tuning/autoTune.test.ts`
- Modify: `apps/dicar-desktop/src/components/workbench/AutoTuneWizard.tsx`
- Modify: `apps/dicar-desktop/src/components/workbench/AutoTuneWizard.test.tsx`

**Interfaces:**
- Changes: `AiChatClient.complete(messages, signal?): Promise<string>`.
- Changes: `AutoTuneDeps` carries an `AbortSignal`; `runAutoTune` passes it to AI completion and classifies cancellation as `aborted`.
- Consumes: Task 5 `clearTelemetrySubscription()`.

- [ ] **Step 1: Add failing AI client cancellation tests**

Mock fetch so it rejects with `AbortError` when the external controller aborts, and assert user cancellation reports `AI 请求已取消`. Use fake timers with a never-resolving fetch for `timeoutMs: 50`, then assert it reports `AI 请求超时`. No network request may leave the test process.

- [ ] **Step 2: Add failing engine abort-after-experiment test**

In `autoTune.test.ts`, make `runExperiment` set `harness.aborted = true` and return `null`; assert result status is `aborted`, not `failed`. Add a scripted AI that observes the passed signal and rejects after abort.

- [ ] **Step 3: Add failing wizard cleanup tests**

Use a MockBridge test subclass/spies to record calls. Start from an original target value and original subscription, make the AI request fail immediately, and assert the wizard restores both. Add a second case with no initial subscription and assert `clearTelemetrySubscription` is called. Add validation cases for non-finite/out-of-range rest and step values.

- [ ] **Step 4: Run focused tests and verify red**

Run: `pnpm exec vitest run src/ai/aiClient.test.ts src/tuning/autoTune.test.ts src/components/workbench/AutoTuneWizard.test.tsx`

Expected: FAIL for missing signal, wrong abort classification, and missing cleanup.

- [ ] **Step 5: Thread external cancellation through the AI stack**

Change the client interface to:

```ts
complete(messages: AiChatMessage[], signal?: AbortSignal): Promise<string>;
```

In `DeepSeekClient`, create an internal timeout controller, combine internal/external signals using `AbortSignal.any`, and distinguish which controller aborted in the catch path. Pass `deps.signal` from `runAutoTune` to `ai.complete`.

- [ ] **Step 6: Make experiments promptly abortable**

Replace long sleeps with a helper that waits in at most 50 ms slices and returns false when the signal aborts. Check abort before and after every target write and wait. If aborted, return `null`; the engine must check `isAborted()` immediately after `runExperiment()` before treating null as invalid.

- [ ] **Step 7: Restore target and telemetry in `finally`**

Capture the pre-run snapshot before changing subscription. After `runAutoTune`, or after setup/run throws, restore the original target, then:

- call `setTelemetrySubscription` with the original desired request and re-pause if it was paused; or
- call `clearTelemetrySubscription` when no desired request existed.

Preserve the engine's best gain values. If cleanup fails, convert the final result to failed with a message naming the cleanup failure.

- [ ] **Step 8: Validate experiment targets before starting**

Denial must reject non-finite values, equal rest/step values, and values outside `targetRecord.numeric.min/max`. Keep the dangerous target eligible as an experiment stimulus.

- [ ] **Step 9: Run focused AI/wizard tests**

Run: `pnpm exec vitest run src/ai/aiClient.test.ts src/tuning/autoTune.test.ts src/components/workbench/AutoTuneWizard.test.tsx`

Expected: PASS.

- [ ] **Step 10: Commit AI isolation and cancellation**

```powershell
git add -- apps/dicar-desktop/src/ai/aiClient.ts apps/dicar-desktop/src/ai/aiClient.test.ts apps/dicar-desktop/src/tuning/autoTune.ts apps/dicar-desktop/src/tuning/autoTune.test.ts apps/dicar-desktop/src/components/workbench/AutoTuneWizard.tsx apps/dicar-desktop/src/components/workbench/AutoTuneWizard.test.tsx
git commit -m "fix(app): isolate and cancel auto-tune experiments"
```

### Task 7: Documentation and Full Verification

**Files:**
- Modify: `docs/development.md`
- Modify: `HANDOFF.md`
- Modify: `docs/superpowers/specs/2026-08-14-simulator-pid-closed-loop-design.md`
- Create/Modify: `task_plan.md`, `findings.md`, `progress.md` as uncommitted session records unless explicitly requested otherwise.

**Interfaces:**
- Produces: documented simulator behavior, completed roadmap status, and fresh verification evidence.

- [ ] **Step 1: Update development and handoff docs**

Document the aligned parameter names, RAM-only target, model scope, and the fact that Web Mock and Rust simulator now generate live PID-responsive speed telemetry. Move HANDOFF priority 1 from unfinished to completed and renumber remaining work without changing the user's roadmap exclusions.

- [ ] **Step 2: Run frontend quality gates**

From `apps/dicar-desktop` run:

```powershell
pnpm lint
pnpm typecheck
pnpm test -- --run
pnpm build
pnpm test:e2e
```

Expected: all pass; record exact test counts in `progress.md`.

- [ ] **Step 3: Run Rust/C quality gates**

From repository root run:

```powershell
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p dctp-sim --bin generate_vectors -- --check
```

Expected: all pass, including C cross-validation and unchanged vectors.

- [ ] **Step 4: Review safety and compatibility diff**

Run:

```powershell
git diff --check
git status --short
git diff --stat HEAD~6..HEAD
rg -n "control.target_speed_mps|pid.speed.ki|pid.speed.kd|DANGEROUS|PERSISTENT" crates/dctp-sim/src/device.rs crates/dctp-device-c/shim/dctp_test_shim.c apps/dicar-desktop/src/bridge/mockBridge.ts apps/dicar-desktop/src/vehicleProfiles/builtins/dicar-diff-drive.yaml
```

Confirm target lacks persistent flags in Rust/C/Mock, AI has no commit call, and golden vectors are untouched.

- [ ] **Step 5: Invoke verification-before-completion and commit docs**

After the verification skill confirms current evidence, commit documentation and any uncommitted implementation fixes:

```powershell
git add -- HANDOFF.md docs/development.md docs/superpowers/specs/2026-08-14-simulator-pid-closed-loop-design.md docs/superpowers/plans/2026-08-14-simulator-pid-closed-loop.md
git commit -m "docs: document closed-loop simulator"
```

