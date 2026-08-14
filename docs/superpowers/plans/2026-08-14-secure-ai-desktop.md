# Secure AI Desktop Channel Implementation Plan

> **Execution:** Main agent only, sequential execution with test-driven-development. No subagents.

**Goal:** Route AI tuning through a cancellable Rust/Tauri DeepSeek client and keep the API key only in Windows Credential Manager.

**Architecture:** A separately managed Rust AI service owns HTTP, credentials, limits, and cancellation. A frontend AiPlatform provider selects native or unavailable behavior while preserving the existing AiChatClient engine interface.

**Tech Stack:** Rust, Tauri 2, reqwest 0.12, keyring 3.6.3, tokio-util, React 19, Zustand, Vitest.

## Global Constraints

- Fixed official DeepSeek endpoint; no custom base URL.
- Retain editable model name with strict Rust validation.
- Never return, persist in frontend, or log the API key.
- Preserve RAM-only automation and all existing local tuning guards.
- No real DeepSeek calls in tests.

### Task 1: Rust AI domain and validation

**Files:** create `apps/dicar-desktop/src-tauri/src/ai_service.rs`; modify its Cargo manifest.

- [ ] Add tests for model/message/key validation, response parsing, response limit, stable error codes, and secret-free errors.
- [ ] Run `cargo test -p dicar-desktop --features native-check --lib` and confirm the missing service/API failures.
- [ ] Add exact dependencies and implement DTOs, credential trait, request validation, and response parsing.
- [ ] Re-run focused tests and Clippy; refactor only while green.
- [ ] Commit `feat(app): add secure native AI service` after the full milestone passes.

### Task 2: HTTP, credential, and cancellation behavior

**Files:** extend `ai_service.rs`; add unit-test-only in-memory credential store and local HTTP server helper.

- [ ] Add failing tests for success, 401/429/500, redirect rejection, 10/60-second timeout mapping, cancellation, duplicate IDs, 1 MiB limit, and cleanup of the active map.
- [ ] Implement the fixed reqwest client, keyring adapter, CancellationToken request map, chunked response reader, and error sanitization.
- [ ] Run the focused Rust suite until green, then `cargo clippy -p dicar-desktop --all-targets --features native-check -- -D warnings`.

### Task 3: Tauri commands

**Files:** modify `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, and native command tests.

- [ ] Add failing command tests for credential status/set/clear, complete, idempotent cancel, and command registration.
- [ ] Manage `AiServiceState` separately in the Tauri Builder and expose the five approved commands.
- [ ] Run native-check tests and verify no device Core interfaces changed.

### Task 4: Frontend AiPlatform and settings migration

**Files:** create `src/ai/aiPlatform.ts` and tests; modify app providers, `settingsStore`, and AI client tests.

- [ ] Add failing tests for native invoke mapping, AbortSignal cancellation, unavailable browser mode, and injected provider access.
- [ ] Add failing migration test starting from v2 localStorage and assert raw persisted JSON contains neither `aiApiKey` nor `aiBaseUrl` after hydration.
- [ ] Implement `AiPlatform`, native/unavailable implementations, provider injection, and settings v3 with only `aiModel`.
- [ ] Run focused Vitest and typecheck.

### Task 5: AutoTune UI integration

**Files:** modify `AutoTuneWizard.tsx` and its tests.

- [ ] Add failing tests for browser denial, desktop credential status, save/replace/delete, cleared password input, request cancellation, and unchanged local tuning guards.
- [ ] Replace direct DeepSeekClient construction with AiPlatform, remove base URL, and keep the model input.
- [ ] Run all AI/AutoTune tests, frontend lint/typecheck, Rust native-check, then commit the milestone.
