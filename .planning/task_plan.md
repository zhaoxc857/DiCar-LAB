# Task Plan

- [x] Acquire and inspect IKUN-CAR-LAB source.
- [ ] Back up selected old DiCAR_LAB content and replace it in place.
- [ ] Rebrand the application and launcher minimally.
- [ ] Add the STM32F103 line-car vehicle profile and tests.
- [ ] Replace the firmware transport with JSON Line over HC-05.
- [ ] Verify desktop tests, Keil build and final repository state.
- [ ] Design the distributable PySide6 desktop package and pressed-state UI refresh.
- [ ] Design the future unlimited-flashing workflow and safety boundaries.
- [ ] Write and review the approved design/spec before implementation.
- [ ] Package, test, document and publish the desktop release to GitHub.

## Current phase

- [x] Recover prior migration context and inspect the current app/package structure.
- [x] Confirm release shape and unlimited-flashing scope with the user.
- [x] Present approaches and recommended design for approval.
- [ ] Obtain user review of the committed design spec.
- [x] Obtain user review of the committed design spec.
- [x] Write and self-review the implementation plan.
- [x] Confirm inline execution in the current migration workspace.
- [x] Implement with test-first changes and verify the packaged app.
- [x] Commit and publish the verified release.

## Execution status

- [x] Task 1: Shared version and smoke-start behavior
- [x] Task 2: Safe flashing task-state model
- [x] Task 3: Firmware workspace and navigation
- [x] Task 4: Rendered button interaction states
- [x] Task 5: PyInstaller portable build
- [x] Task 6: GitHub Windows release workflow
- [x] Task 7: README and v1.7.0 documentation
- [x] Task 8: Full verification and publication

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Codex process creation failed with `setup refresh had errors` | Initial session and one retry | User restarted/resumed the task; terminal access recovered without changing files. |
| Smoke test failed to import PySide6 under system Python | First GREEN run | Root cause was interpreter mismatch; project `.venv` contains the declared dependencies, so verification now uses that interpreter. |
| Frozen EXE timed out before application startup | First packaging GREEN run | Bootloader debug build exposed a QtWidgets DLL load failure in flat contents mode; switched to PyInstaller's default `_internal` layout and retained one-folder portability. |
