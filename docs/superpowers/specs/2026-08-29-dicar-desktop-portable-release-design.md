# DiCAR LAB Desktop Portable Release Design

## Context

DiCAR LAB is being migrated from the previous Tauri/React/Rust application to the current Python 3 and PySide6 application under `CAR_LAB/`. The old desktop implementation is no longer part of the active product. Its history remains recoverable through Git history and the existing archive branch, but it will not be shipped or documented as a parallel application.

The current Windows launcher creates a virtual environment and starts the source application. That is useful for developers, but it is not a self-contained desktop release. This release turns the PySide6 application into the repository's primary desktop product, adds consistent button feedback, and reserves a safe extension boundary for future unlimited/repeated firmware flashing.

## Goals

- Publish DiCAR LAB v1.7.0 as a self-contained Windows x64 portable ZIP on `zhaoxc857/DiCar_Tune`.
- Require no preinstalled Python for users of the portable ZIP.
- Preserve the current PySide6 architecture and existing black/white themes.
- Give every Qt push button consistent hover, pressed, focus, checked, and disabled feedback.
- Add a visible but non-executing firmware-flashing workspace with a minimal task-state model.
- Replace the repository `main` state with the migrated application through a normal fast-forward push, without force-pushing.
- Keep future flashing safe by making validation and failure-stop behavior part of the public design before a hardware backend exists.

## Non-goals

- Restore or maintain the old Tauri desktop application.
- Add Electron, Tauri, a web frontend, or another UI framework.
- Build a Windows installer or a single-file executable.
- Invoke STM32CubeProgrammer, OpenOCD, pyOCD, vendor BSL tools, or any other real flashing command in v1.7.0.
- Claim that unlimited/repeated flashing is available in v1.7.0.
- Refactor unrelated tuning, telemetry, vehicle-profile, or protocol code.

## Chosen Architecture

### Desktop packaging

PyInstaller builds the existing `CAR_LAB/main.py` entry point in one-folder mode. A checked-in spec file is the single source of truth for bundled modules and data. The bundle includes all vehicle YAML files and user documentation needed at runtime. Qt, pyqtgraph, PyYAML, pyserial, bleak, and their required runtime files are collected by PyInstaller from the locked build environment.

The bundle uses a flat contents directory so the current `Path(__file__).resolve().parents[1]` resource lookups continue to resolve `vehicles/`, `data/`, `profiles/`, `reports/`, and `logs/` beside the executable. This is intentionally portable: user-generated data stays inside the extracted DiCAR LAB folder instead of being written to a machine-wide installation directory.

The Windows build script performs these steps in order:

1. Read `VERSION.txt` and extract `1.7.0`.
2. Build the one-folder application with the checked-in PyInstaller spec.
3. Run a frozen-application startup smoke check using Qt's offscreen platform.
4. Copy the current license and end-user README into the distribution folder.
5. Create `release/DiCAR-LAB-v1.7.0-Windows-x64.zip`.
6. Create `release/SHA256SUMS.txt` containing the ZIP's SHA-256 hash.

Generated executables, build directories, ZIP files, and local runtime data remain ignored by Git.

### Button interaction system

The existing Qt style sheet remains the only UI styling mechanism. No animation library or custom button subclass is added. Both black and white themes define the following stable states for all `QPushButton` controls:

- Default: readable surface, border, and label.
- Hover: a stronger surface and border without changing geometry.
- Pressed: darker surface, stronger border, and a one-pixel internal label shift achieved through padding while the button's outer bounds remain unchanged.
- Focus: a visible keyboard-focus border distinct from hover.
- Checked: a persistent selected surface for toggle buttons.
- Disabled: reduced emphasis with sufficient text contrast and no hover/press illusion.

`primary` and `danger` object names keep their existing semantic roles and gain explicit pressed, focus, and disabled states. Long-running buttons use their existing local control flow to switch to a progress label such as `扫描中…` and disable themselves until the operation ends. A new global helper is introduced only if at least two independent pages need identical busy-state code; otherwise the state stays local to avoid an unused abstraction.

### Future flashing boundary

A new `固件烧录` page appears under `工具`. In v1.7.0 it is an honest capability boundary, not a fake implementation. It presents:

- target-device summary;
- firmware-file selection area;
- single and continuous mode choices;
- task status and event log;
- a disabled execution button with the reason `烧录后端尚未配置`;
- safety copy explaining that the vehicle must remain powered safely and motors must not be armed by the application.

The core state model has these values:

- `UNAVAILABLE`: no verified flashing backend is configured;
- `IDLE`: backend is ready and no job is active;
- `VALIDATING`: firmware and target are being checked;
- `FLASHING`: bytes are being written;
- `VERIFYING`: device contents or firmware identity are being verified;
- `SUCCEEDED`: write and verification completed;
- `FAILED`: the job stopped after an error;
- `CANCELLED`: the user cancelled at a safe boundary.

The only v1.7.0 transition is initial construction into `UNAVAILABLE`. Tests define the complete allowed transition table for the future backend so later implementation cannot skip validation or verification. The UI consumes this state and never shells out to a programmer tool.

The allowed future transition table is exact:

- `UNAVAILABLE -> IDLE` after a backend is detected and validated;
- `IDLE -> VALIDATING` when the user starts an explicitly selected job;
- `VALIDATING -> FLASHING | FAILED | CANCELLED`;
- `FLASHING -> VERIFYING | FAILED`;
- `VERIFYING -> SUCCEEDED | FAILED`;
- `SUCCEEDED | FAILED | CANCELLED -> IDLE` after the result is acknowledged;
- no other transition is valid.

Future continuous/unlimited flashing must follow this per-device sequence:

`IDLE -> VALIDATING -> FLASHING -> VERIFYING -> SUCCEEDED`

Any validation, write, or verification error transitions to `FAILED` and stops the sequence. Cancellation transitions to `CANCELLED` only at a backend-declared safe boundary. Starting another device requires an explicit target detection event; a successful job never automatically arms motors or starts the vehicle control loop.

## Data and Error Flow

At application startup, the existing startup checks verify bundled modules and required resources. Missing runtime files produce the existing blocking error dialog and a local error log. A build-only `DICAR_SMOKE_TEST=1` environment flag constructs and shows the main window offscreen, processes one Qt event cycle, and exits with status 0 without opening a transport. The portable smoke test fails the build on any other exit status.

The flashing page obtains a read-only `FlashJobState` value from the core state model. Because no backend is present, selecting a firmware file changes only the displayed path; it cannot enable execution. Any future backend error is represented as a failed state and a user-readable message, while detailed output belongs in the page log and application log.

## Testing Strategy

Implementation follows test-first development for behavior changes:

- Resource tests prove the bundled spec includes vehicle YAML and required documents.
- Theme tests prove normal, primary, and danger buttons define pressed, focus, and disabled states in both themes.
- Flash-state tests prove valid transitions and reject unsafe skips such as `IDLE -> FLASHING` or `FLASHING -> SUCCEEDED`.
- Flash-page tests prove the execution control is disabled and explains why no backend is available.
- Navigation tests prove the new page is reachable under `工具` without changing existing page ordering unexpectedly.
- Existing Python tests and static startup checks continue to pass.
- A local PyInstaller build and offscreen frozen-app smoke test validate the actual portable artifact.
- The release workflow builds the same artifact on `windows-latest` with Python 3.12, uploads the ZIP and checksum as workflow artifacts, and attaches them to a GitHub Release for tags matching `v*`.

## Documentation and Publication

The root README is rewritten around the packaged desktop product. It includes:

- a prominent GitHub Release download path;
- ZIP extraction and first-run instructions;
- simulator-first quick start;
- STM32F103 plus HC-05 connection notes;
- current feature list and safety warning;
- a clearly labelled `无限烧录路线图` section stating that v1.7.0 only reserves the safe boundary;
- developer build and test commands;
- release artifact naming and SHA-256 verification instructions.

`CHANGELOG.md` records v1.7.0. `VERSION.txt`, application metadata, window titles, and release filenames use the same version.

After all tests and the packaged smoke check pass, the complete migrated repository state is committed. `main` is updated with a normal fast-forward push. Tag `v1.7.0` is pushed, the GitHub release workflow builds the portable ZIP, and the resulting Release is checked for the ZIP and `SHA256SUMS.txt` attachments.

## Success Criteria

- A Windows user can download the v1.7.0 ZIP, extract it, and start DiCAR LAB without installing Python.
- The application loads bundled vehicle profiles and can run the simulator.
- Buttons visibly respond to mouse press and keyboard focus in both themes without layout movement.
- The firmware page clearly reports that flashing is unavailable and cannot execute a command.
- All existing and new tests pass, and the packaged executable passes the offscreen smoke check.
- GitHub `main` represents the new PySide6 application, and GitHub Release v1.7.0 contains the portable ZIP and checksum.
