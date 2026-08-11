# Task 6 report — bounded telemetry and application actor

## Scope and baseline

- Actual execution base: `38c7bae` on `feature/windows-app-shell`.
- Changed only `crates/dicar-app-core` plus this report.
- The pre-existing root `Cargo.lock` remains untracked and was not staged; Task 7 owns the first committed workspace lockfile.
- No protocol wire schema, simulator behavior, Tauri shell, or React UI was changed.

## Delivered behavior

- `TelemetryEngine` validates one active 1–8-channel subscription, decodes Manifest-directed F32/I32/U32/FLAGS32 slots, unwraps the u32 microsecond clock, keeps sequence gaps separate from device-reported drops, and retains at most 30,000 points per channel / 240,000 total.
- `AppActorHandle` owns one joined worker thread and a bounded 64-command channel. Consecutive slider writes for the same parameter are latest-wins before a protocol barrier; all physical writes still use the opaque one-shot workspace operations from Task 5.
- The actor exposes all planned commands and one FIFO event stream. Reliable events reserve 64 slots, snapshots coalesce to one latest value, telemetry keeps four whole UI batches and drops the oldest whole batch, and reliable overflow becomes a sticky terminal `frontendOverrun` carrying the operation ID that could not be delivered.
- Telemetry is emitted through a deterministic 33,333,334 ns gate. Pause sends `TELEMETRY_STOP` and freezes retained samples; resume sends a new subscription version. Input validation occurs before the wire request.
- Snapshots contain connection/device/Manifest/parameter/subscription/access/storage/dirty state, bounded marker history, telemetry-buffer size, transport byte counts, RTT, decoder/retry counters, sequence/device/UI drop counters, and the last disconnect reason.
- Unexpected disconnect marks parameter truth Unknown, clears only the active subscription, preserves the desired subscription without automatic replay, freezes retained telemetry, retains the last protocol diagnostics, emits `ConnectionLost`, and prevents further writes until an explicit reconnect.

## TDD and debugging evidence

1. Telemetry engine RED failed on missing `TelemetryEngine`, `TelemetryError`, and tagged value exports. GREEN reached 5/5 for four raw types, timestamp/sequence wrap, independent drop counters, atomic rejection, and the 8 x 30,000 bound.
2. Actor E2E RED failed on missing Actor/command/event/config APIs. GREEN first covered real simulator connect -> write -> eight-channel 500 Hz subscribe -> pause/resume, then reached 3/3 with consecutive A/B/C coalescing and unexpected-disconnect truth.
3. Capacity tests were added after the bounded mailbox implementation and were immediately GREEN; this is regression evidence rather than a claimed product RED. They prove command overload returns within 2 ms, one snapshot/four telemetry batches remain under a stalled UI, and reliable overflow becomes ordered explicit terminal state.
4. The first wall-clock visual-rate mutation test was too loose and passed under a 16 ms fault; it was not counted as RED. Investigation showed the simulator's own 16-sample batching masks the internal cadence. A deterministic `UiFlushGate` test then had an honest missing-type RED, GREEN at 33,333,334 ns, mutation RED under 16 ms, and restored GREEN.
5. Self-review extended the real disconnect test and obtained RED because protocol byte diagnostics reset to zero after the session was dropped. A cached last diagnostics snapshot produced GREEN while preserving Unknown/no-replay behavior.
6. Clippy found `CoreEventPayload` was at least 376 bytes because `AppSnapshot` was inline. Boxing only the snapshot variant fixed the memory-layout warning without changing the serde event shape. The production-only test helper `pending_counts` was removed; capacity tests now count the real drained event stream after pausing.

## Fresh verification

- `cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check` — exit 0.
- `cargo +stable-x86_64-pc-windows-gnu clippy --offline --workspace --all-targets` with `CARGO_TARGET_DIR=C:\DiCar_LAB\task6-final-clippy-target` and `-D warnings` — exit 0, zero warnings, from a new target directory.
- `cargo +stable-x86_64-pc-windows-gnu test --offline --workspace --all-targets -- --test-threads=1` with `CARGO_TARGET_DIR=C:\DiCar_LAB\task6-final-test-target` — exit 0; 166 passed, 0 failed, from a second new target directory.
- `cargo +stable-x86_64-pc-windows-gnu run --offline -p dctp-sim --bin generate_vectors -- --check` — exit 0 with `DCTP v1 vectors match`.
- Final Task 6 focused inventory: actor unit 1/1, actor capacity 4/4, actor integration 3/3, telemetry engine 5/5.

## Review notes and compatibility

- The task diff is larger than the normal 800-line review target because it includes the bounded engine, event DTOs, actor runtime, and three real integration/stress suites. The smallest separable stage is the telemetry engine plus its tests; the remaining actor/event slice is still one coupled public boundary consumed by Task 7. The diff was reviewed file-by-file and all newly injected items are bounded.
- Adding four fields to the public pre-release `DiagnosticsSnapshot` is a source compatibility change for downstream exhaustive struct literals. There is no external app consumer yet; all workspace consumers compile and pass.
- `CoreEventPayload::SnapshotChanged` boxes the Rust payload for bounded enum size. Serde preserves the same externally tagged JSON shape, so Task 7's TypeScript contract is unaffected.
- Local roles/leases remain a demo policy, not a distributed security boundary. Serial, flashing, collaboration relay, Git history, Tauri Channels, UI rendering, recording, cross-platform clients, AI tuning, and multi-vehicle orchestration remain later planned stages.
