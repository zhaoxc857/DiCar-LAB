# Task 5 report — parameter workspace and persistence workflow

## Scope and baseline

- Actual execution base: `d42dccb440eec4c14c71b4072c820a458f2b2186`.
- Changed only `crates/dicar-app-core` plus this report.
- The pre-existing untracked root `Cargo.lock` was neither edited intentionally nor staged.
- No protocol, simulator, actor, Tauri, or React implementation was changed.

## Delivered behavior

- `access.rs` defines the local/demo-only Owner, Tuner, Observer and active-lease gate. It explicitly documents that this is not a distributed security boundary. The required Chinese Observer-write and Tuner-commit denial strings are stable.
- `parameter_workspace.rs` owns one bounded record per Manifest ID, validates Manifest/state identity and persistence invariants, applies wire-bit dirty comparisons, validates local values before a request can be produced, and exposes a two-stage `queue_write` / `resolve_write` API.
- Per-ID coalescing holds at most one in-flight and one latest queued target. ACK value and revision are authoritative; ordinary errors keep confirmed state; revision conflicts refresh confirmed device state while preserving one unresolved latest user target without automatic retry.
- `CommitPlan` has private fields and read-only accessors, captures a sorted immutable parameter/value/revision set, and produces the exact protocol `ParamCommit`. CRC input contains values, not revisions. Commit application validates the complete plan and ACK before mutating any persisted record or generation.
- Revert emits revision-aware writes of persisted values. Undo uses the current revision, retains history on failure, records a reversible inverse on success, and keeps a 128-entry FIFO history.
- Disconnect marks every record Unknown and clears in-flight, queued, and undo history. Reconnect replaces the entire workspace from the new `ConnectedDevice`, so no stale operation can replay.
- `ProtocolSession` now exposes typed parameter write and commit calls. `REVISION_CONFLICT` context accepts only strict lowercase even-length hex and delegates the complete decoded byte slice to `ParamWriteAck::decode`; uppercase, odd, non-hex, or trailing data is rejected.

## TDD evidence

Each behavior group began with a focused failing test before its production implementation:

1. Construction RED: E0432, missing `ParameterWorkspace`; GREEN 1/1. Invariant RED then lacked duplicate/unknown/type/persistence errors; GREEN 2/2.
2. Access/dirty RED: missing access types; GREEN 4/4, including signed zero and NaN payload wire bits.
3. Validation RED: missing `queue_write` and validation errors; GREEN 7/7.
4. Coalescing/conflict RED: missing `resolve_write`, `WriteFailure`, and record outcome fields; GREEN 11/11.
5. Typed session RED: missing typed write/commit/conflict decoder APIs; GREEN integration 2/2.
6. Commit RED: missing plan/resolve/generation APIs. The first run exposed an incorrectly dirty test fixture (12/13); after making the fixture genuinely clean, GREEN 13/13.
7. Revert/undo/history RED: missing revert, undo, report, and history APIs; GREEN 18/18.
8. Disconnect/reconnect RED: missing boundary/replacement/Unknown gates; GREEN 19/19.
9. Counting-transport integration RED: missing immutable `CommitPlan::to_protocol_commit`; GREEN integration 3/3.
10. Self-review regression RED: malformed ACK left `WriteState::Queued` instead of Idle; targeted GREEN 1/1 after clearing pending state without changing confirmed RAM, Flash, or revision.

Final focused state after formatting: `parameter_integration` 3/3 and `parameter_workspace` 20/20.

## Validation semantics decision

The brief's mention of numeric `step` could be read as requiring alignment. The simulator is authoritative for this stage: `step` is metadata only. Local validation therefore uses inclusive bounds for I32/U32, finite plus inclusive bounds for constrained F32, enum membership, and no additional constraint for `ParamConstraints::None`. Tests explicitly prove an in-range off-step value is accepted and unconstrained F32 preserves NaN/Infinity wire values. This matches `dctp-sim` and avoids client/device disagreement.

## Fresh verification

- `cargo fmt --all -- --check` — exit 0.
- `cargo clippy --workspace --all-targets --target-dir C:\DiCar_LAB\task5-final-clippy-target -- -D warnings` — exit 0, zero warnings.
- `cargo test --workspace --all-targets --target-dir C:\DiCar_LAB\task5-final-test-target` — exit 0; 142 passed, 0 failed. The target directory was new and isolated.
- Focused post-format command: `cargo test -p dicar-app-core --test parameter_workspace --test parameter_integration --target-dir C:\DiCar_LAB\task5-cargo-target` — 23 passed, 0 failed.

## Review notes and non-goals

- The counting-transport real-simulator test proves denied Observer/inactive-lease writes and Tuner commit produce zero additional transport writes; active Tuner RAM write and active Owner write plus commit follow the real DCTP path.
- Partial revert reports confirmed and failed IDs separately, and only ACK-confirmed records change.
- All mutable maps are keyed by IDs already bounded by Manifest size; history is fixed at 128 with oldest eviction.
- AppActor/threading, Tauri, UI, distributed leases, Git history, serial/flashing, multi-car, and encoder algorithms remain intentionally outside Task 5.
