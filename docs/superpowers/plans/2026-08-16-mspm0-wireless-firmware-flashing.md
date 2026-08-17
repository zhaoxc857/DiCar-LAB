# MSPM0G3507 Wireless Firmware Flashing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan serially. Do not use subagents for this repository. Every production behavior follows RED -> GREEN -> REFACTOR.

**Goal:** Deliver signed wireless firmware updates for LCKFB Tianmengxing MSPM0G3507 over HC-05/nanoUART using TI ROM BSL, with per-device credentials and host-side recovery.

**Architecture:** Activate the reserved DCTP safe-transition messages, then hand the same COM port from the normal Core actor to an isolated Rust firmware service. Keep package trust, TI BSL framing and recovery in a reusable Rust crate; expose a separate native-only firmware platform to React.

**Tech Stack:** Rust 1.80, C99, DCTP v1, serialport 4.9, ring Ed25519/SHA-256, Tauri 2, React 19, TypeScript, Vitest, Playwright, TI MSPM0 SDK 2.11.00.07.

## Global Constraints

- Work serially on `codex/release-0.2.0`; no subagents, merge, push or force push.
- Never read, print or commit real credentials/signing keys.
- HC-05 and TI ROM BSL use 9600 8N1 in v1.
- STM32 F1/F4 and MSPM0G3519 are adapter-only future scope.
- Hardware validation is a separate gate and must never be inferred from simulator results.

---

### Task 1: Lock the design and baseline

**Files:**
- Create: `docs/superpowers/specs/2026-08-16-mspm0-wireless-firmware-flashing-design.md`
- Create: `docs/superpowers/plans/2026-08-16-mspm0-wireless-firmware-flashing.md`
- Modify untracked: `task_plan.md`, `findings.md`, `progress.md`

- [ ] Record the approved defaults, verified TI/Tianmengxing facts, package wire layout, state machine and explicit deferrals.
- [ ] Record baseline frontend and Rust/Tauri evidence, including Smart App Control 4551 and the equivalent profile rerun.
- [ ] Self-review for placeholders, contradictions and accidental STM32/G3519 scope.

### Task 2: Activate the DCTP safe-transition contract

**Files:**
- Modify: `crates/dctp-protocol/src/messages.rs`, `crates/dctp-protocol/src/lib.rs`
- Modify: `crates/dicar-app-core/src/session.rs`, `crates/dicar-app-core/src/actor.rs`
- Modify: `firmware/dctp-device/include/dctp_device.h`, `firmware/dctp-device/src/dctp_device.c`
- Test: protocol message tests, Core session tests, `crates/dctp-device-c/tests/behavior.rs`, golden vectors

**Interfaces:**
- Produce `PrepareFlash`, `PrepareFlashAck`, `FirmwareTargetId`, and `BootloaderProtocol` exact wire types from the design.
- Produce `CoreCommand::PrepareFirmwareFlash { operation_id, target_id, firmware_version, image_len, image_sha256 }`.
- Add optional C `prepare_flash` callback plus one-shot `dctp_device_take_flash_transition()`; NULL preserves UNKNOWN_MESSAGE.

- [ ] Add Rust message round-trip/invalid-length tests and verify RED.
- [ ] Implement minimal codec exports and verify GREEN.
- [ ] Add Core tests for permission/capability/dirty-state rejection and successful ACK-then-disconnect; verify RED then GREEN.
- [ ] Add C cross-language tests for capability advertisement, callback-once idempotency, ACK bytes and pending transition; verify RED then GREEN.
- [ ] Add named PREPARE_FLASH request/ACK vectors and regenerate/check Rust+C vectors.

### Task 3: Build signed package and TI ROM BSL core

**Files:**
- Create: `crates/dicar-firmware-flash/Cargo.toml`
- Create: `crates/dicar-firmware-flash/src/package.rs`, `src/bsl.rs`, `src/target.rs`, `src/lib.rs`
- Test: crate unit/integration tests with literal fixtures and fake serial I/O

**Interfaces:**
- `FirmwarePackage::inspect(bytes, trust_store) -> VerifiedFirmwarePackage` enforces the exact `.dicarfw` v1 format.
- `FirmwareTargetAdapter` exposes target ID, board name, image bounds, initial baud and BSL client construction.
- `Mspm0RomBsl<T: Read + Write>` exposes connect, device_info, unlock, erase_range, program, verify_crc and start_application.

- [ ] Add package parser tests for a valid signed fixture and every pre-device rejection path; verify RED.
- [ ] Implement bounded parser, SHA-256 and Ed25519 verification; verify GREEN.
- [ ] Add TI BSL literal packet/CRC/ACK tests from TI SLAU887 and verify RED.
- [ ] Implement packet codec and response parser; verify GREEN.
- [ ] Add fake-transport update tests for chunking, timeout, bad password, CRC mismatch and disconnect; implement only the tested orchestration.

### Task 4: Add offline signer and per-device provisioning

**Files:**
- Create: `crates/dicar-firmware-flash/src/bin/dicar-firmware-tool.rs`
- Create: `crates/dicar-firmware-flash/src/credential_store.rs`, `src/recovery_store.rs`
- Test: CLI argument/format tests and in-memory credential/recovery backends

**Interfaces:**
- `package` command accepts manifest values, image path, external key path and output path.
- `provision-record` reads the 32-byte BSL password from stdin, validates device/board IDs, stores it in Windows Credential Manager, imports the allowed release public key, and seeds a signed recovery package.
- No command accepts a password directly as a command-line argument.

- [ ] Add tests proving deterministic packages verify and a changed image/signature fails.
- [ ] Add credential-name/zeroization/error-redaction tests before implementing the Windows backend.
- [ ] Add recovery-store tests for atomic replace, per-device bound and corrupt-file rejection.
- [ ] Implement CLI and document that NONMAIN password programming remains a hardware-gated procedure until Tianmengxing validation.

### Task 5: Coordinate AppState, Tauri and the serial handoff

**Files:**
- Create: `apps/dicar-desktop/src-tauri/src/firmware_service.rs`
- Modify: Tauri `app_state.rs`, `commands.rs`, `lib.rs`, `Cargo.toml`
- Test: new native-check firmware service integration tests

**Interfaces:**
- `firmware_inspect(bytes) -> FirmwarePackageSummary` returns no secret or raw image.
- `firmware_start(request, Channel<FirmwareFlashEvent>) -> FirmwareFlashResult` owns one operation through reconnect.
- `firmware_retry(operation_id)` and `firmware_rollback(operation_id)` are valid only from `recoveryRequired`.
- `firmware_cancel(operation_id)` succeeds only before DCTP PREPARE_FLASH is accepted.

- [ ] Add tests that simulator/browser, missing capability, non-owner, dirty parameters, missing credential and target mismatch send zero flash bytes.
- [ ] Add an AppState upgrade-lock test proving connect/write/close cannot race a flash operation.
- [ ] Implement RAII lock and command registration; keep errors generic and secrets backend-only.
- [ ] Add a fake dual-protocol serial integration test for DCTP ACK -> port handoff -> BSL -> DCTP reconnect.

### Task 6: Replace the reserved frontend entry with a real wizard

**Files:**
- Create: `apps/dicar-desktop/src/firmware/firmwarePlatform.ts`, `firmwareTypes.ts`
- Create: `apps/dicar-desktop/src/components/firmware/FirmwareFlashWizard.tsx`
- Modify: providers and `FirmwareFlashEntry`/`ConnectionDrawer`
- Test: platform, entry, wizard and drawer Vitest; one Mock Playwright flow

**Interfaces:**
- `FirmwareFlashPlatform` mirrors the Tauri commands and exposes an explicit unavailable implementation outside native Tauri.
- `FirmwareFlashUiState` uses the full design state machine; critical phases cannot render a normal cancel action.

- [ ] Add provider/platform tests proving non-Tauri remains unavailable and Tauri invokes exact commands.
- [ ] Add wizard tests for package rejection, downgrade warning, explicit confirmation, progress ordering and recovery instructions.
- [ ] Implement one component tree with accessible dialog/focus/progress semantics.
- [ ] Add Playwright Mock coverage for successful flash and recovery-package rollback without touching real serial hardware.

### Task 7: Add the Tianmengxing firmware integration seam

**Files:**
- Create: `firmware/targets/lckfb-tmx-mspm0g3507/README.md`
- Create: focused C adapter/header and host-buildable tests under the target directory
- Modify: `firmware/dctp-device/README.md`

**Interfaces:**
- Adapter maps target ID 1 to PA10/PA11 UART0, 9600 8N1, 250 ms transition delay and TI `BOOTLOADERENTRY` reset sequence.
- Application provides `safe_stop`, `uart_tx_complete` and `enter_rom_bsl`; the generic DCTP library remains vendor-neutral.

- [ ] Write host tests for callback order and one-shot transition before target code.
- [ ] Implement adapter using MSPM0 SDK 2.11 APIs without vendoring the SDK.
- [ ] Add TI Arm Clang/GCC build instructions and verify whichever installed compiler is available.

### Task 8: Full verification, documentation and hardware gate

**Files:**
- Modify: README, user guide, development guide and HANDOFF
- Update untracked planning records

- [ ] Run fmt, Clippy `-D warnings`, all Rust/C tests, native-check tests and all golden vectors.
- [ ] Run frontend lint, typecheck, full Vitest, build and Playwright.
- [ ] Run package-tool known-answer tests and secret/path scan; review tracked diff for real credentials or private key material.
- [ ] If hardware is available, run normal update, interrupted erase/program, BSL+RST recovery and rollback on Tianmengxing+HC-05; otherwise record all four as unverified.
- [ ] Report what changed, what remains, and any design deviation; do not rebuild release artifacts unless separately requested.
