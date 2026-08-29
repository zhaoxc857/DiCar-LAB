# DiCAR_LAB IKUN Migration Implementation Plan

> **For Codex:** Execute this plan inline in order, with verification after each destructive or protocol-sensitive stage.

**Goal:** Replace the old DiCAR_LAB app with a minimally rebranded IKUN-CAR-LAB and connect the existing STM32F103 line car over HC-05 using newline-delimited JSON.

**Architecture:** Keep the upstream Python transport/protocol and add one vehicle YAML. Preserve the STM32 application's existing `dctp_port_*` API while replacing its implementation with a bounded JSON-Line adapter and non-blocking UART transmission.

**Tech Stack:** Python 3, PySide6, pyserial, pytest, STM32 HAL, Keil MDK-ARM/ArmClang.

---

### Task 1: Recoverable in-place replacement

- [ ] Create a selective backup containing the old Git bundle and valuable untracked project notes/configuration.
- [ ] Verify the Git bundle and copied backup inventory.
- [ ] Mirror the reviewed IKUN-CAR-LAB source into `C:\DiCar_LAB`, removing obsolete old files and generated caches.
- [ ] Initialize clean Git history for the replacement and retain the public upstream URL.

### Task 2: Minimal DiCAR_LAB branding

- [ ] Rename the launcher entry files to DiCAR names.
- [ ] Update user-visible application/window/settings names without rewriting upstream architecture.
- [ ] Update startup documentation and launcher build script references.
- [ ] Add focused tests for renamed entry points and application metadata where practical.

### Task 3: STM32F103 line-car vehicle definition

- [ ] Add a 9600-baud vehicle YAML containing only the four required tunable parameters.
- [ ] Configure core and detailed waveform channels for existing sensors, encoders and PWM values.
- [ ] Add tests that load the YAML and validate parameter/channel keys against the protocol contract.

### Task 4: Firmware JSON-Line adapter

- [ ] Add host-side tests/vectors for GET, SET, CMD, ACK and TEL framing before implementation.
- [ ] Replace `dctp_port.c/.h` internals while preserving the current public C API.
- [ ] Implement bounded RX line parsing, parameter range checks and ACK responses.
- [ ] Implement interrupt-driven TX with ACK priority and rate-limited telemetry suitable for HC-05 at 9600 baud.
- [ ] Remove obsolete external DCTP source references from the Keil project.

### Task 5: Verification and handoff

- [ ] Check Python, uv and Git availability, then install/sync dependencies only if required.
- [ ] Run focused protocol/config tests and a Python syntax/import check.
- [ ] Build the STM32 Keil target with zero errors and zero warnings.
- [ ] Inspect final diff/status for sensitive files and unrelated leftovers.
- [ ] Provide exact HC-05 wiring, APP selection and on-device connection test steps.
