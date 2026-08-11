# Real Serial Connection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an honest Windows COM connection path that enumerates ports, opens the selected port, completes the existing DCTP HELLO/Manifest handshake, and only then reports the real device as ready.

**Architecture:** `dicar-app-core` gains a bounded `SerialTransport` behind the existing `Transport` contract and an `ActiveTransport` enum so the actor can switch between TCP simulator and serial endpoints without duplicating protocol code. Tauri exposes typed port discovery and dynamic connect commands. React keeps simulator and real-device controls separate; the ordinary browser build never claims a real serial connection.

**Tech Stack:** Rust 2021, `serialport`, existing DCTP protocol/session/actor, Tauri 2 commands, React/TypeScript, Vitest.

## Global Constraints

- Standard COM passthrough only; no vendor-specific wireless-DAP protocol.
- Baud rates exposed in the first serial slice are exactly 115200, 460800, and 921600; default 921600.
- A real endpoint becomes `ready` only after DCTP HELLO, Manifest, and parameter loading succeed.
- Simulator and real-device labels/states must never be conflated.
- One active physical session at a time; no automatic write replay after disconnect.
- Pure Web Serial remains the next independently testable client slice, not a simulated success path.

---

### Task 1: Serial transport and dynamic actor endpoint

**Files:**
- Modify: `crates/dicar-app-core/Cargo.toml`
- Modify: `crates/dicar-app-core/src/transport/mod.rs`
- Create: `crates/dicar-app-core/src/transport/serial.rs`
- Modify: `crates/dicar-app-core/src/actor.rs`
- Modify: `crates/dicar-app-core/src/lib.rs`
- Test: `crates/dicar-app-core/tests/serial_transport.rs`

**Interfaces:**
- Produces `Endpoint::Serial { port_name: String, baud_rate: u32 }`.
- Produces `SerialTransport::open(port_name, baud_rate)` implementing `Transport` with a 10 ms read timeout and bounded writes.
- Produces `SerialPortDescriptor` and `available_serial_ports()`.
- Produces `CoreCommand::ConnectTo { endpoint }`; legacy `Connect` continues to use `CoreConfig.endpoint`.

- [x] Write tests that fail because serial endpoint/transport/discovery types do not exist, including timeout-as-zero, EOF/error mapping, identity, and exact baud validation.
- [x] Run `cargo test -p dicar-app-core --test serial_transport` and verify the missing API is the failure.
- [x] Add the minimal serial transport, an enum transport wrapper, and actor endpoint selection.
- [x] Prove existing simulator tests and the new focused tests pass.

### Task 2: Tauri discovery and typed serial connect

**Files:**
- Modify: `apps/dicar-desktop/src-tauri/src/commands.rs`
- Modify: `apps/dicar-desktop/src-tauri/src/lib.rs`
- Modify: `apps/dicar-desktop/src-tauri/tests/commands.rs`

**Interfaces:**
- Extends `EndpointDto` with `{ kind: "serial", portName, baudRate }`.
- Produces `list_serial_ports() -> Result<Vec<SerialPortDescriptor>, BridgeErrorDto>`.
- `connect_core` dispatches `ConnectTo` and does not require the startup simulator endpoint to match.

- [x] Add failing endpoint mapping and discovery tests.
- [x] Run the focused native bridge tests and confirm the new variants/command are absent.
- [x] Implement DTO validation, discovery command, and dynamic actor dispatch.
- [x] Run native-check tests and Clippy on the GNU toolchain.

### Task 3: Honest connection chooser

**Files:**
- Modify: `apps/dicar-desktop/src/domain/types.ts`
- Modify: `apps/dicar-desktop/src/bridge/desktopBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/tauriBridge.ts`
- Modify: `apps/dicar-desktop/src/bridge/mockBridge.ts`
- Modify: `apps/dicar-desktop/src/components/shell/ConnectionStatusBar.tsx`
- Modify: corresponding unit and E2E tests.

**Interfaces:**
- Produces `SerialPortDescriptor` in the frontend domain.
- Produces `DesktopBridge.listSerialPorts()`.
- Connection UI has explicit `模拟器体验` and `真实串口` modes; web preview returns no real ports and explains that it cannot access hardware.

- [x] Add failing tests for Tauri command mapping, default-web refusal, serial selector, and ready-only-after-success behavior.
- [x] Implement the minimal bridge and UI changes.
- [x] Run lint, typecheck, focused Vitest, production build, and the critical browser flow.
- [x] Run the available Rust workspace tests and DCTP vector drift check; document the existing MSVC packaging prerequisite separately.

## Completion

- [x] A selected COM endpoint is serialized end-to-end with its port name and baud rate.
- [x] Failed open/HELLO never produces `ready`.
- [x] Successful DCTP load provides real device identity, Manifest parameters, and telemetry descriptors through the existing workspace.
- [x] The browser preview cannot falsely report a real serial connection.
