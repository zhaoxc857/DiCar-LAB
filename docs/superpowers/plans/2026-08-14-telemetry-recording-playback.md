# Telemetry Recording and Playback Implementation Plan

> **Execution:** Main agent only, sequential execution with test-driven-development. No subagents.

**Goal:** Add a bounded persistent telemetry recording library with lossless batch export/import and independent timeline playback.

**Architecture:** A recording controller consumes the existing Bridge event fan-out and serializes raw batches into an IndexedDB repository. Playback loads a selected recording into its own buffer and reuses the waveform renderer with an explicit viewport.

**Tech Stack:** TypeScript, IndexedDB, fake-indexeddb 6.2.5, Zustand, React, Canvas, Vitest, Playwright.

## Global Constraints

- Five minutes per recording, 20 records, 256 MiB logical library size.
- Auto-delete oldest complete records; protect active playback/export.
- Storage failure discards the entire current recording.
- Playback issues no DesktopBridge commands and never replaces live state.
- Preserve complete UiTelemetryBatch data; no downsampling in storage.

### Task 1: Recording domain and formats

**Files:** create `src/telemetry/recordings.ts` and `recordings.test.ts`.

- [ ] Write failing tests for metadata capture, name/note validation, stop reasons, stats recomputation, JSON v1 round-trip, duplicate-ID import, malicious input rejection, and CSV escaping.
- [ ] Implement the exact types, validators, JSON Blob parts, CSV wide rows, safe filenames, and constants.
- [ ] Run focused tests and typecheck; refactor while green.

### Task 2: IndexedDB repository

**Files:** create `src/telemetry/recordingRepository.ts` and tests; add fake-indexeddb dev dependency.

- [ ] Write failing tests for schema creation, ordered chunks, atomic import, incomplete cleanup, deletion, 20/256-MiB pruning, protected IDs, and quota/write failure rollback.
- [ ] Implement the native IndexedDB wrapper, transaction helpers, logical byte accounting, and startup cleanup.
- [ ] Run repository tests and typecheck.

### Task 3: Recording controller and event integration

**Files:** create `src/stores/recordingStore.ts` and tests; modify `useBridgeSubscription.ts`.

- [ ] Add failing tests for start eligibility, one-second/4096-point flushing, serial ordering, five-minute stop, manual stop, pause/disconnect/subscription-change sealing, markers, and full deletion on write error.
- [ ] Implement the queued controller/store and route every Bridge event to it before the live workspace consumer.
- [ ] Ensure start/stop reset between tests and app sessions; run focused tests.

### Task 4: Recording manager and toolbar

**Files:** create `RecordingManagerDialog.tsx` and tests; modify live workbench, waveform panel, and telemetry toolbar.

- [ ] Add failing UI tests for start form, denial messages, active status, auto-stop notice, newest-first list, delete, import, JSON/CSV download, and subscription-change stop-before-apply.
- [ ] Implement the header entry, toolbar controls, manager dialog, file input, download helpers, and accessible live regions.
- [ ] Run component tests, lint, and typecheck.

### Task 5: Independent playback

**Files:** create `RecordingPlaybackDialog.tsx` and tests; modify `WaveformCanvas.tsx` and reusable waveform helpers.

- [ ] Add failing tests for independent buffer loading, explicit viewport, play/pause, seek, step, all five speeds, end pause, cursor/data table reuse, and zero Bridge calls.
- [ ] Implement the playback session and renderer extension without changing live defaults.
- [ ] Run focused waveform/playback tests and typecheck.

### Task 6: Browser acceptance

**Files:** extend `e2e/initial-release.spec.ts`.

- [ ] Add a failing Playwright scenario that records Mock data, changes subscription to seal, opens the library, replays, and verifies JSON/CSV downloads.
- [ ] Implement only missing integration glue, then run the new scenario and the full seven-plus E2E suite.
- [ ] Run all frontend tests/build and commit `feat(app): record and replay telemetry sessions`.
