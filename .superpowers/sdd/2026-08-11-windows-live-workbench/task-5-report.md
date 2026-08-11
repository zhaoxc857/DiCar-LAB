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

## Formal review fix round 1

- Write completion is now bound to an opaque, monotonically increasing operation token and a workspace generation. `resolve_write` verifies generation, token, parameter ID, expected revision, and bit-exact value before removing an in-flight operation or changing state. Reconnect increments the generation without resetting the token counter; stale, duplicate, wrong-ID, and old-generation completions are typed zero-mutation errors.
- A commit plan is registered as one active immutable operation with its own token and generation. An active commit blocks writes and a second commit. Only the exact active plan can settle it; success applies the complete captured value set atomically, while timeout/device/verify/storage, CRC, and plan errors clear the active operation without changing Flash truth, dirty flags, or storage generation.
- `revert_all` performs a read-only preflight of every target before registering any write. Its immutable batch token binds the complete write set. Resolution validates exact one-to-one coverage, unique tokens, matching operations, and response types before applying any result. A malformed/missing/duplicate/extra result therefore changes nothing; a structurally valid partial failure applies ACK-confirmed results and reports failed IDs.
- Raw `ParamWrite` and `ParamCommit` use through the public generic session request is rejected before encoding or transport. Raw typed sends are private; the public typed execution path accepts only an active opaque `PendingWrite` or `CommitPlan` minted by the workspace after access and value validation. The access policy remains a local/demo gate, not a distributed security boundary.
- The scripted wire regression injects uppercase, odd-length, non-hex, and trailing-byte revision-conflict contexts. Each is rejected as a protocol error without changing RAM, Flash, revision, or dirty truth. The caller then explicitly settles the exact pending operation as an ordinary failure: only operation bookkeeping/error presentation changes, pending becomes zero, and all four confirmed truth fields remain unchanged. A strict lowercase real-simulator conflict still refreshes current device truth and does not auto-retry.
- A two-record commit regression models a batch verify failure surfaced after the second entry. Both complete records remain byte/field-equivalent to their snapshots, both remain dirty, and storage generation remains unchanged; the cleared active operation permits a retry with a new token.

### Fix-round TDD and compatibility notes

- A/B/C/D each began with focused compile/test REDs for the missing token, generation, active-operation, coverage, and sealed-execution APIs, followed by focused GREENs. The final workspace regression suite contains 25 tests.
- The malformed-context integration first had a harness-only compile RED while the scripted ERROR-frame helper was absent. Once the harness compiled, the product behavior was immediately GREEN; this is recorded honestly as added regression evidence rather than a newly fixed product defect.
- The two-record commit-failure test was also immediately GREEN after the active commit redesign; it strengthens all-or-nothing evidence rather than claiming a separate production change.
- Three pre-existing `session_faults` tests were adapted from generic raw parameter requests to opaque workspace plans. Their retry count, sequence reuse, disconnect deadline, reconnect, and frame-count assertions were unchanged; this was an API adaptation, not a behavior change.
- Adding `CoreError::Workspace` and `CoreError::UnauthorizedParameterOperation` can break downstream exhaustive matches on the public enum. This is recorded as a Minor source-compatibility change.

### Fix-round fresh verification

- `cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check` — exit 0.
- Focused affected suites in the isolated `target-task5-fix` directory — `parameter_integration` 3/3, `parameter_workspace` 25/25, and `session_faults` 19/19; 47 passed, 0 failed.
- `cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets --target-dir C:\DiCar_LAB\task5-fix-clippy-round1 -- -D warnings` — exit 0, zero warnings, from a new isolated target directory.
- `cargo +stable-x86_64-pc-windows-gnu test --workspace --all-targets --target-dir C:\DiCar_LAB\task5-fix-test-round1` — exit 0; 148 passed, 0 failed, from a second new isolated target directory.
- `git diff --check` — exit 0; only Git's existing LF-to-CRLF notices were emitted.

## Formal review fix round 2

- `PendingWrite` now records private revert-batch membership. Public single-write resolution rejects a batch-owned operation with `BatchWriteRequiresBatchResolution` before changing any record, pending entry, or batch state. Only the exact active `RevertPlan` can resolve those entries through the private matched resolver.
- Public session execution is one-shot per active operation. `ParameterWorkspace` records dispatched write tokens and the dispatched commit token before any transport write. Repeating `execute_write` or `execute_commit` for the same opaque handle returns a typed `AlreadyDispatched` error with zero additional frames.
- Exact write resolution removes its dispatch token; exact commit resolution clears both active and dispatch state on success or error; disconnect clears every dispatch set. A failed commit request must therefore be explicitly settled before a new plan can be dispatched, preventing both duplicate Flash writes and permanently ambiguous reuse.
- Revert batch entries use the same one-shot send gate, while their results remain bound to the immutable batch plan. The resolver intentionally accepts caller-injected associated results so deterministic model tests and future transport adapters can settle operations; the public `ProtocolSession` boundary is what enforces one physical send per token.

### Round-2 TDD evidence

- Batch bypass RED: missing `BatchWriteRequiresBatchResolution`; GREEN proves a single batch entry cannot resolve independently, leaves two pending writes and both records unchanged, and the exact batch can subsequently settle.
- Duplicate commit RED: missing `CommitAlreadyDispatched`; GREEN uses a real simulator plus counting transport. The first commit sends one frame, the second sends zero, exact ACK resolution leaves device and local storage generation at 1.
- Duplicate write RED: missing `WriteAlreadyDispatched`; GREEN proves first send +1 frame, duplicate/stale/wrong handle +0 frames, and the exact replacement operation can still send and settle.
- Additional regressions prove a device-error commit remains one-shot until explicit error settlement, after which a new plan may dispatch; a real-session revert entry also sends once and can only be applied through exact batch resolution.

### Round-2 fresh verification

- `cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check` — exit 0.
- Focused affected suites — `parameter_integration` 6/6, `parameter_workspace` 26/26, `session_faults` 20/20; 52 passed, 0 failed.
- `cargo +stable-x86_64-pc-windows-gnu clippy --offline --target-dir C:\DiCar_LAB\controller-task5-fix2-target --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
- `cargo +stable-x86_64-pc-windows-gnu test --offline --target-dir C:\DiCar_LAB\controller-task5-fix2-target --workspace --all-targets -- --test-threads=1` — exit 0; 153 passed, 0 failed.
- `git diff --check` — exit 0; only LF-to-CRLF notices were emitted.
