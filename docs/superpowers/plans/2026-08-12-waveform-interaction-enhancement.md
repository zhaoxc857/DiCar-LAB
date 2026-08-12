# App Waveform Interaction Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing Windows App waveform with descriptor-derived workgroups, a live probe, timestamp-based A/B cursors, per-channel deltas, and local/global/fixed Y ranges.

**Architecture:** Keep the current DCTP, actor, bridge, ring-buffer, Canvas, and React data path. Add two pure telemetry modules, then make `WaveformPanel` the sole owner of interaction state while Canvas emits pointer timestamps and the existing legend/table provide equivalent text output.

**Tech Stack:** React 19, TypeScript 5.9, Vitest, Testing Library, Canvas 2D, Zustand, pnpm.

## Global Constraints

- Windows desktop App remains the primary product; browser DCTP is out of scope.
- Do not modify DCTP, Rust `AppActor`, the Manifest schema, or DesktopBridge commands.
- Keep at most 8 channels, 30,000 points per channel, pixel min/max downsampling, and no more than 30 visual revisions per second.
- Workgroup selection changes pending channels only; only the existing Apply action changes the device subscription.
- Cursor truth is device timestamp, never another channel's array index.
- Canvas output must have equivalent table and `aria-live` text.
- Follow strict RED -> GREEN -> REFACTOR and commit each task independently.

---

## File Structure

- Create `apps/dicar-desktop/src/telemetry/telemetryWorkgroups.ts`: deterministic descriptor classification and budget clipping.
- Create `apps/dicar-desktop/src/telemetry/telemetryWorkgroups.test.ts`: independent literal fixtures for classification and clipping.
- Create `apps/dicar-desktop/src/telemetry/waveformInteraction.ts`: timestamp/pixel conversion, binary nearest lookup, cursor transitions/steps, and Y ranges.
- Create `apps/dicar-desktop/src/telemetry/waveformInteraction.test.ts`: pure behavior and edge cases.
- Modify `apps/dicar-desktop/src/telemetry/ringBuffer.ts`: expose logarithmic timestamp lookup without copying snapshots.
- Modify `apps/dicar-desktop/src/components/workbench/WaveformPanel.tsx`: sole interaction-state owner and command priority.
- Modify `apps/dicar-desktop/src/components/workbench/TelemetryToolbar.tsx`: workgroups, Y mode, and cursor controls.
- Modify `apps/dicar-desktop/src/components/workbench/WaveformCanvas.tsx`: render/emit probe and A/B timestamps.
- Modify `apps/dicar-desktop/src/components/workbench/TelemetryLegend.tsx`: timestamp-derived priority readout.
- Modify `apps/dicar-desktop/src/components/workbench/TelemetryDataTable.tsx`: A/B/delta and accessible summary.
- Modify `apps/dicar-desktop/src/components/workbench/WaveformPanel.test.tsx`: integrated App interaction contract.

### Task 1: Descriptor-Derived Workgroups

**Interfaces:**

- Produces `buildTelemetryWorkgroups(descriptors): TelemetryWorkgroup[]`.
- Produces `clipWorkgroup(group, maxChannels): { channelIds; omittedCount }`.
- `TelemetryWorkgroup = { id: string; label: string; channelIds: number[] }`.

- [ ] **Step 1: Write failing classification and clipping tests** with literal descriptors proving speed/encoder overlap, Manifest order, omission count, empty-group removal, and All Channels.
- [ ] **Step 2: Run RED:** `pnpm --filter @dicar/desktop test -- src/telemetry/telemetryWorkgroups.test.ts --run`; expect missing-module failure.
- [ ] **Step 3: Implement minimal deterministic matching** using normalized `machineName`, `displayName`, and `unit`; always append non-empty `all`.
- [ ] **Step 4: Run GREEN** with the focused command, then `pnpm --filter @dicar/desktop typecheck`.
- [ ] **Step 5: Commit:** `git commit -m "feat(app): derive telemetry workgroups"`.

### Task 2: Timestamp Interaction Model

**Interfaces:**

- Adds `TelemetryRingBuffer.nearest(channelId, timestampUs)` and `indexAtOrNearest(channelId, timestampUs)` using binary search over logical ring indices.
- Produces `timestampForX`, `xForTimestamp`, `advanceCursor`, `clickCursor`, `nearestReading`, and `computeChannelRange` from `waveformInteraction.ts`.
- Cursor state is `{ cursorAUs: number | null; cursorBUs: number | null; activeCursor: "A" | "B" }`.

- [ ] **Step 1: Write failing tests** proving timestamp/pixel boundaries, wrapped-buffer binary lookup, A/B click cycle, normal/10-sample stepping, missing-neighbor rejection, constant ranges, and fixed-range snapshots.
- [ ] **Step 2: Run RED:** `pnpm --filter @dicar/desktop test -- src/telemetry/waveformInteraction.test.ts src/telemetry/ringBuffer.test.ts --run`; expect missing exports.
- [ ] **Step 3: Implement minimal pure functions and ring lookup** without calling `snapshot()` from pointer lookup.
- [ ] **Step 4: Run GREEN**, then existing ring-buffer/downsample/store tests.
- [ ] **Step 5: Commit:** `git commit -m "feat(app): model timestamp waveform cursors"`.

### Task 3: Workgroup and Cursor Controls

**Interfaces:**

- `TelemetryToolbar` receives `workgroups`, `selectedWorkgroup`, `yScaleMode`, `hasCursors`, `onWorkgroup`, `onYScaleMode`, `onClearCursors`, and `onResetFixedRanges`.
- `WaveformPanel` stores timestamp cursor state and retains the existing Bridge calls.

- [ ] **Step 1: Extend `WaveformPanel.test.tsx` first** to prove workgroup choice does not call the Bridge until Apply, budget clipping is announced, manual toggle becomes Custom, click cycle/clear/keyboard state is visible, and Pause retains cursors.
- [ ] **Step 2: Run RED:** focused WaveformPanel test; expect missing controls and timestamp behavior.
- [ ] **Step 3: Implement toolbar and panel state wiring**, retaining at least one channel and current link-budget enforcement.
- [ ] **Step 4: Run GREEN**, lint, and typecheck.
- [ ] **Step 5: Commit:** `git commit -m "feat(app): add waveform workgroup and cursor controls"`.

### Task 4: Canvas Probe and Accessible A/B Readouts

**Interfaces:**

- `WaveformCanvas` receives `probeTimestampUs`, `cursorAUs`, `cursorBUs`, `yScaleMode`, fixed ranges, and `onProbe`/`onLockCursor` callbacks.
- `TelemetryDataTable` receives target timestamps and renders A/B/Delta columns when both cursors exist.
- `TelemetryLegend` reads the priority timestamp: probe, active cursor, then latest.

- [ ] **Step 1: Write failing component tests** for mouse move/leave/click conversion, A/B Delta t, signed per-channel delta, missing-neighbor text, and marker priority.
- [ ] **Step 2: Run RED:** focused Canvas/Panel tests; expect absent probe/A-B output.
- [ ] **Step 3: Implement pointer bounds, three distinct vertical-line styles, range modes, timestamp readings, and matching screen-reader summary.**
- [ ] **Step 4: Run GREEN**, then all frontend unit tests, lint, typecheck, and production build.
- [ ] **Step 5: Commit:** `git commit -m "feat(app): render waveform probes and AB deltas"`.

### Task 5: App-Level Verification and Documentation

**Interfaces:** No new production API; validates the complete slice.

- [ ] **Step 1: Add or update Playwright coverage** for keyboard-only A/B operation at 1280 x 720 and the readable table at 200% zoom; first run must fail on the missing acceptance behavior if any gap remains.
- [ ] **Step 2: Fix only acceptance gaps**, preserving existing layout and semantics.
- [ ] **Step 3: Run the fresh final matrix:** `pnpm lint`, `pnpm typecheck`, `pnpm test -- --run`, `pnpm build`, and `pnpm test:e2e` from the workspace root with the bundled Node/pnpm paths.
- [ ] **Step 4: Run `git diff --check` and inspect the complete BASE..HEAD diff for scope, placeholders, unbounded work, and regressions.
- [ ] **Step 5: Commit:** `git commit -m "test(app): verify advanced waveform interaction"` if acceptance files changed; otherwise record verification without an empty commit.
