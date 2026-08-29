# DiCAR LAB Desktop Portable Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship DiCAR LAB v1.7.0 as a self-contained Windows x64 portable ZIP with consistent Qt button feedback and a safe, non-executing boundary for future unlimited firmware flashing.

**Architecture:** Keep the migrated Python/PySide6 application as the only desktop product. Add a small immutable flashing state model and one unavailable-state page, improve the existing QSS themes, then package the existing entry point with PyInstaller one-folder mode and publish tagged artifacts through GitHub Actions.

**Tech Stack:** Python 3.12+, PySide6, unittest, PyInstaller 6.20+, PowerShell, GitHub Actions.

## Global Constraints

- Release only a Windows x64 portable ZIP; do not add an installer or single-file executable.
- Do not restore Tauri, React, Rust desktop crates, Electron, or old release binaries.
- Do not invoke any real flashing tool or shell command from the v1.7.0 application.
- Preserve the current black/white themes and page structure.
- Keep button bounds stable during interaction and define pressed, focus, checked, and disabled states in both themes.
- Future continuous flashing must validate, flash, verify, stop on failure, and never arm motors.
- Use test-first changes for every behavior change.
- Update GitHub `main` by normal fast-forward only; never force-push.
- Preserve unrelated or user-owned workspace changes and never use stash, reset, restore, checkout, or clean.

---

## File Map

- Create `CAR_LAB/core/version.py`: one application/version constant used by Python UI code.
- Modify `CAR_LAB/main.py`: use the shared version and support the offscreen frozen-app smoke flag.
- Modify `CAR_LAB/core/startup_check.py`: distinguish source-only files from frozen runtime resources.
- Modify `CAR_LAB/ui/main_window.py`: use the shared version and register the firmware page.
- Create `CAR_LAB/core/flash_job.py`: immutable flash-state value and transition validation.
- Create `CAR_LAB/ui/firmware_flash.py`: unavailable-state firmware workspace with no backend execution.
- Modify `CAR_LAB/ui/theme.py`: complete button states in both QSS themes.
- Create `requirements-build.txt`: PyInstaller-only build dependency.
- Create `dicar_lab.spec`: one-folder PyInstaller graph and bundled data.
- Create `build_portable_windows.ps1`: versioned build, smoke check, ZIP, and SHA-256 generation.
- Create `build_portable_windows.bat`: double-click wrapper around the PowerShell build.
- Create `.github/workflows/windows-release.yml`: tests, portable build artifact, and tag release upload.
- Modify `VERSION.txt`, `CHANGELOG.md`, `README.md`, and `CAR_LAB/README.md`: v1.7.0 user and developer documentation.
- Create focused tests under `tests/` for smoke startup, versioning, flashing, UI rendering, packaging, and workflow behavior.

---

### Task 1: Shared version and real smoke-start behavior

**Files:**
- Create: `CAR_LAB/core/version.py`
- Modify: `CAR_LAB/main.py`
- Modify: `CAR_LAB/core/startup_check.py`
- Modify: `CAR_LAB/ui/main_window.py`
- Modify: `VERSION.txt`
- Test: `tests/test_application_release.py`

**Interfaces:**
- Produces: `APP_NAME: str`, `VERSION: str`, `DISPLAY_VERSION: str` from `core.version`.
- Produces: `DICAR_SMOKE_TEST=1` process contract: construct/show the main window offscreen, process one event cycle, close, return exit code 0.
- Consumes: the existing `load_vehicle_config`, `DataBus`, `JsonLineProtocol`, `TransportManager`, and `MainWindow` constructors.

- [ ] **Step 1: Write the failing release behavior tests**

```python
# tests/test_application_release.py
import os
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "CAR_LAB"


class ApplicationReleaseTests(unittest.TestCase):
    def test_python_metadata_matches_release_file(self):
        sys.path.insert(0, str(APP))
        from core.version import DISPLAY_VERSION

        self.assertEqual("DiCAR LAB v1.7.0", DISPLAY_VERSION)
        self.assertEqual(DISPLAY_VERSION, (ROOT / "VERSION.txt").read_text(encoding="utf-8").strip())

    def test_offscreen_smoke_mode_constructs_and_exits(self):
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM="offscreen", DICAR_SMOKE_TEST="1")
        result = subprocess.run(
            [sys.executable, "main.py"],
            cwd=APP,
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
        )
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `python -m unittest tests.test_application_release -v`

Expected: import failure for `core.version`; the smoke behavior is not yet available.

- [ ] **Step 3: Add the minimum version and smoke implementation**

```python
# CAR_LAB/core/version.py
APP_NAME = "DiCAR LAB"
VERSION = "1.7.0"
DISPLAY_VERSION = f"{APP_NAME} v{VERSION}"
```

Update `main.py` to import `os` and `DISPLAY_VERSION`, call `app.setApplicationName(DISPLAY_VERSION)`, and place this branch immediately after `win.show()`:

```python
if os.environ.get("DICAR_SMOKE_TEST") == "1":
    app.processEvents()
    win.close()
    return 0
```

Update `MainWindow` title and subtitle from `DISPLAY_VERSION` and `VERSION`. Update frozen startup checks so `main.py` and `requirements.txt` are source-only requirements:

```python
frozen = bool(getattr(sys, "frozen", False))
if not frozen:
    checks.append(("main.py", (ROOT / "main.py").exists(), ""))
    checks.append(("requirements.txt", (ROOT / "requirements.txt").exists(), ""))
checks.append(("vehicles", (ROOT / "vehicles").exists(), ""))
```

Set `VERSION.txt` to the exact line `DiCAR LAB v1.7.0`.

- [ ] **Step 4: Run the focused and branding tests and verify GREEN**

Run: `python -m unittest tests.test_application_release tests.test_branding tests.test_branding_docs -v`

Expected: all tests pass and smoke mode exits without connecting a transport.

- [ ] **Step 5: Commit the task**

```powershell
git add -- CAR_LAB/core/version.py CAR_LAB/main.py CAR_LAB/core/startup_check.py CAR_LAB/ui/main_window.py VERSION.txt tests/test_application_release.py
git commit -m "feat: prepare desktop release startup"
```

---

### Task 2: Safe flashing task-state model

**Files:**
- Create: `CAR_LAB/core/flash_job.py`
- Test: `tests/test_flash_job.py`

**Interfaces:**
- Produces: `FlashState(str, Enum)` with `UNAVAILABLE`, `IDLE`, `VALIDATING`, `FLASHING`, `VERIFYING`, `SUCCEEDED`, `FAILED`, `CANCELLED`.
- Produces: immutable `FlashJobState.transition(target: FlashState, message: str = "") -> FlashJobState`.
- Failure: invalid transitions raise `ValueError` and preserve the original value.

- [ ] **Step 1: Write the failing transition tests**

```python
# tests/test_flash_job.py
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from core.flash_job import FlashJobState, FlashState


class FlashJobStateTests(unittest.TestCase):
    def test_default_state_is_unavailable_with_reason(self):
        job = FlashJobState()
        self.assertEqual(FlashState.UNAVAILABLE, job.state)
        self.assertEqual("烧录后端尚未配置", job.message)

    def test_safe_path_requires_validation_and_verification(self):
        job = FlashJobState().transition(FlashState.IDLE)
        for state in (
            FlashState.VALIDATING,
            FlashState.FLASHING,
            FlashState.VERIFYING,
            FlashState.SUCCEEDED,
            FlashState.IDLE,
        ):
            job = job.transition(state)
        self.assertEqual(FlashState.IDLE, job.state)

    def test_unsafe_skip_is_rejected_without_mutating_job(self):
        job = FlashJobState(FlashState.IDLE, "ready")
        with self.assertRaisesRegex(ValueError, "IDLE -> FLASHING"):
            job.transition(FlashState.FLASHING)
        self.assertEqual(FlashState.IDLE, job.state)

    def test_failure_and_cancel_paths_stop_before_success(self):
        failed = FlashJobState(FlashState.FLASHING).transition(FlashState.FAILED, "write failed")
        cancelled = FlashJobState(FlashState.VALIDATING).transition(FlashState.CANCELLED)
        self.assertEqual("write failed", failed.message)
        self.assertEqual(FlashState.CANCELLED, cancelled.state)
        with self.assertRaises(ValueError):
            failed.transition(FlashState.SUCCEEDED)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the test and verify RED**

Run: `python -m unittest tests.test_flash_job -v`

Expected: import failure for `core.flash_job`.

- [ ] **Step 3: Implement the immutable state model**

```python
# CAR_LAB/core/flash_job.py
from dataclasses import dataclass
from enum import Enum


class FlashState(str, Enum):
    UNAVAILABLE = "unavailable"
    IDLE = "idle"
    VALIDATING = "validating"
    FLASHING = "flashing"
    VERIFYING = "verifying"
    SUCCEEDED = "succeeded"
    FAILED = "failed"
    CANCELLED = "cancelled"


ALLOWED_TRANSITIONS = {
    FlashState.UNAVAILABLE: {FlashState.IDLE},
    FlashState.IDLE: {FlashState.VALIDATING},
    FlashState.VALIDATING: {FlashState.FLASHING, FlashState.FAILED, FlashState.CANCELLED},
    FlashState.FLASHING: {FlashState.VERIFYING, FlashState.FAILED},
    FlashState.VERIFYING: {FlashState.SUCCEEDED, FlashState.FAILED},
    FlashState.SUCCEEDED: {FlashState.IDLE},
    FlashState.FAILED: {FlashState.IDLE},
    FlashState.CANCELLED: {FlashState.IDLE},
}


@dataclass(frozen=True)
class FlashJobState:
    state: FlashState = FlashState.UNAVAILABLE
    message: str = "烧录后端尚未配置"

    def transition(self, target: FlashState, message: str = "") -> "FlashJobState":
        if target not in ALLOWED_TRANSITIONS[self.state]:
            raise ValueError(f"invalid flash transition: {self.state.name} -> {target.name}")
        return FlashJobState(target, message)
```

- [ ] **Step 4: Run the transition tests and verify GREEN**

Run: `python -m unittest tests.test_flash_job -v`

Expected: four tests pass.

- [ ] **Step 5: Commit the task**

```powershell
git add -- CAR_LAB/core/flash_job.py tests/test_flash_job.py
git commit -m "feat: define safe firmware flash states"
```

---

### Task 3: Non-executing firmware workspace and navigation

**Files:**
- Create: `CAR_LAB/ui/firmware_flash.py`
- Modify: `CAR_LAB/ui/main_window.py`
- Test: `tests/test_firmware_flash_page.py`

**Interfaces:**
- Produces: `FirmwareFlashPage(config: dict)`.
- Exposes for UI tests: `state`, `target_label`, `firmware_path`, `single_mode`, `continuous_mode`, `run_button`, `reason_label`, `log`.
- Consumes: `FlashJobState()` only; no transport, subprocess, or programmer backend.

- [ ] **Step 1: Write the failing real-widget test**

```python
# tests/test_firmware_flash_page.py
import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtWidgets import QApplication
from core.flash_job import FlashState
from ui.firmware_flash import FirmwareFlashPage
from ui.main_window import PAGE_DEFS


class FirmwareFlashPageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def test_unconfigured_page_cannot_start_a_flash(self):
        page = FirmwareFlashPage({"vehicle": {"display_name": "STM32 巡线车"}})
        self.assertEqual(FlashState.UNAVAILABLE, page.state.state)
        self.assertFalse(page.run_button.isEnabled())
        self.assertEqual("烧录后端尚未配置", page.reason_label.text())
        self.assertIn("STM32 巡线车", page.target_label.text())
        self.assertTrue(page.single_mode.isChecked())

    def test_tools_navigation_exposes_firmware_page_last(self):
        group, pages = PAGE_DEFS[-1]
        self.assertEqual("工具", group)
        self.assertEqual("固件烧录", pages[-1][0])
        self.assertIs(FirmwareFlashPage, pages[-1][2])


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the page test and verify RED**

Run: `python -m unittest tests.test_firmware_flash_page -v`

Expected: import failure for `ui.firmware_flash`.

- [ ] **Step 3: Implement the unavailable page and append it to Tools**

Build the page with standard Qt widgets only. The file picker writes the selected path into `firmware_path`; it never changes `run_button` to enabled. Use this safety-critical core:

```python
self.state = FlashJobState()
self.run_button = QPushButton("开始烧录")
self.run_button.setObjectName("primary")
self.run_button.setEnabled(False)
self.run_button.setToolTip(self.state.message)
self.reason_label = QLabel(self.state.message)
self.reason_label.setObjectName("statusBad")
self.log = QPlainTextEdit()
self.log.setReadOnly(True)
self.log.setPlainText("本版本仅预留安全烧录边界，未加载任何烧录后端。")
```

Append `("固件烧录", "安全校验、烧录与写后验证", FirmwareFlashPage)` to the existing Tools page list. Add an explicit `_instantiate_page` branch returning `FirmwareFlashPage(self.config)` so no transport is passed to this boundary page.

- [ ] **Step 4: Run the UI and lifecycle tests and verify GREEN**

Run: `python -m unittest tests.test_firmware_flash_page tests.test_protocol_monitor_lifecycle -v`

Expected: page and existing lifecycle tests pass; existing page indexes remain unchanged because the page is appended.

- [ ] **Step 5: Commit the task**

```powershell
git add -- CAR_LAB/ui/firmware_flash.py CAR_LAB/ui/main_window.py tests/test_firmware_flash_page.py
git commit -m "feat: add safe firmware workspace boundary"
```

---

### Task 4: Rendered button press, focus, and disabled feedback

**Files:**
- Modify: `CAR_LAB/ui/theme.py`
- Test: `tests/test_button_interaction.py`

**Interfaces:**
- Consumes: `DARK_STYLE` and `LIGHT_STYLE` applied to real `QPushButton` widgets.
- Produces: visibly distinct normal/pressed/focused/disabled rendering for normal, `primary`, and `danger` buttons without changing widget geometry.

- [ ] **Step 1: Write a failing rendered-widget test**

```python
# tests/test_button_interaction.py
import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "CAR_LAB"))

from PySide6.QtCore import QPoint, Qt
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QPushButton
from ui.theme import DARK_STYLE, LIGHT_STYLE


class ButtonInteractionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def render_signature(self, style, object_name="", action=None):
        button = QPushButton("")
        button.setObjectName(object_name)
        button.setStyleSheet(style)
        button.resize(120, 44)
        button.show()
        self.app.processEvents()
        before_geometry = button.geometry()
        if action == "pressed":
            QTest.mousePress(button, Qt.MouseButton.LeftButton, pos=QPoint(60, 22))
        elif action == "focused":
            button.setFocus(Qt.FocusReason.TabFocusReason)
        elif action == "disabled":
            button.setEnabled(False)
        self.app.processEvents()
        image = button.grab().toImage()
        signature = (
            image.pixelColor(12, 12).name(),
            image.pixelColor(1, 22).name(),
        )
        self.assertEqual(before_geometry, button.geometry())
        if action == "pressed":
            QTest.mouseRelease(button, Qt.MouseButton.LeftButton, pos=QPoint(60, 22))
        button.close()
        return signature

    def test_every_theme_and_semantic_button_changes_when_pressed(self):
        for style in (DARK_STYLE, LIGHT_STYLE):
            for name in ("", "primary", "danger"):
                with self.subTest(style=style[:20], name=name):
                    self.assertNotEqual(
                        self.render_signature(style, name),
                        self.render_signature(style, name, "pressed"),
                    )

    def test_focus_and_disabled_states_render_distinctly(self):
        for style in (DARK_STYLE, LIGHT_STYLE):
            normal = self.render_signature(style, "primary")
            self.assertNotEqual(normal, self.render_signature(style, "primary", "focused"))
            self.assertNotEqual(normal, self.render_signature(style, "primary", "disabled"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the rendering test and verify RED**

Run: `python -m unittest tests.test_button_interaction -v`

Expected: primary/danger pressed, focus, or disabled comparisons fail because those states are incomplete.

- [ ] **Step 3: Complete both theme state tables**

Add these explicit selectors to the dark theme:

```css
QPushButton:pressed { background:#0f1924; border-color:#5d7591; padding-top:8px; padding-bottom:6px; }
QPushButton:focus { border-width:2px; border-color:#65a9df; }
QPushButton:disabled { color:#66778a; background:#111923; border-color:#253244; }
QPushButton#primary:pressed { background:#0f5b8d; border-color:#69b8e6; }
QPushButton#primary:focus { border-width:2px; border-color:#7bc7f4; }
QPushButton#primary:disabled { color:#87a7ba; background:#18384d; border-color:#28516a; }
QPushButton#danger:pressed { background:#521b21; border-color:#c4505b; }
QPushButton#danger:focus { border-width:2px; border-color:#ed7a83; }
QPushButton#danger:disabled { color:#a77b7e; background:#331b1e; border-color:#5a3035; }
```

Add these explicit selectors to the light theme:

```css
QPushButton:pressed { background:#dce4eb; border-color:#8997a6; padding-top:8px; padding-bottom:6px; }
QPushButton:focus { border-width:2px; border-color:#1674ae; }
QPushButton:disabled { color:#939ca6; background:#eef1f4; border-color:#d7dce2; }
QPushButton#primary:pressed { background:#0b5684; border-color:#0b5684; }
QPushButton#primary:focus { border-width:2px; border-color:#07517e; }
QPushButton#primary:disabled { color:#eef6fa; background:#a9c9dc; border-color:#a9c9dc; }
QPushButton#danger:pressed { background:#92242d; border-color:#7d1d25; }
QPushButton#danger:focus { border-width:2px; border-color:#8e1f28; }
QPushButton#danger:disabled { color:#ffffff; background:#e1a5aa; border-color:#d29aa0; }
```

Choose literal colors derived from the existing palette, keeping text readable in both themes. Do not add transforms, timers, or a custom widget subclass.

- [ ] **Step 4: Run the rendered interaction and firmware-page tests**

Run: `python -m unittest tests.test_button_interaction tests.test_firmware_flash_page -v`

Expected: all rendered states are distinct and widget geometry remains stable.

- [ ] **Step 5: Commit the task**

```powershell
git add -- CAR_LAB/ui/theme.py tests/test_button_interaction.py
git commit -m "style: add tactile button feedback"
```

---

### Task 5: Real PyInstaller one-folder portable build

**Files:**
- Create: `requirements-build.txt`
- Create: `dicar_lab.spec`
- Create: `build_portable_windows.ps1`
- Create: `build_portable_windows.bat`
- Test: `tests/test_portable_build.py`

**Interfaces:**
- Produces: `dist/DiCAR LAB/DiCAR LAB.exe` with `vehicles/` and `docs/` beside it.
- Produces: `release/DiCAR-LAB-v1.7.0-Windows-x64.zip` and `release/SHA256SUMS.txt`.
- Consumes: `VERSION.txt`, `LICENSE`, `README.md`, `CAR_LAB/main.py`, runtime requirements, and PyInstaller.

- [ ] **Step 1: Install the isolated build dependency and write the failing integration test**

`requirements-build.txt` will contain `PyInstaller>=6.20,<7` after the RED check so both the CI Python 3.12 build and the current local Python 3.14 build are supported. Use the ignored `CAR_LAB/.venv` locally instead of installing build packages into the system interpreter:

```powershell
if (!(Test-Path 'CAR_LAB/.venv/Scripts/python.exe')) {
    python -m venv CAR_LAB/.venv
}
$buildPython = (Resolve-Path 'CAR_LAB/.venv/Scripts/python.exe').Path
& $buildPython -m pip install -r CAR_LAB/requirements.txt "PyInstaller>=6.20,<7"
```

Then create this test:

```python
# tests/test_portable_build.py
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.environ.get("DICAR_PACKAGE_TEST") == "1", "set DICAR_PACKAGE_TEST=1")
class PortableBuildTests(unittest.TestCase):
    def test_spec_builds_a_smoke_testable_app_with_runtime_resources(self):
        with tempfile.TemporaryDirectory() as temp:
            temp = Path(temp)
            result = subprocess.run(
                [
                    sys.executable, "-m", "PyInstaller", "dicar_lab.spec",
                    "--distpath", str(temp / "dist"),
                    "--workpath", str(temp / "work"),
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=600,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            app = temp / "dist" / "DiCAR LAB"
            exe = app / "DiCAR LAB.exe"
            self.assertTrue(exe.is_file())
            self.assertTrue((app / "vehicles" / "stm32f103_line_car.yaml").is_file())
            self.assertTrue((app / "docs").is_dir())
            env = os.environ.copy()
            env.update(QT_QPA_PLATFORM="offscreen", DICAR_SMOKE_TEST="1")
            smoke = subprocess.run([exe], env=env, capture_output=True, text=True, timeout=60)
            self.assertEqual(0, smoke.returncode, smoke.stdout + smoke.stderr)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Verify RED against the missing spec**

Run: `$env:DICAR_PACKAGE_TEST='1'; & $buildPython -m unittest tests.test_portable_build -v`

Expected: PyInstaller cannot open `dicar_lab.spec`.

- [ ] **Step 3: Add the minimum spec and build scripts**

`dicar_lab.spec` must use `CAR_LAB/main.py`, `pathex=["CAR_LAB"]`, one-folder `COLLECT`, `console=False`, `contents_directory="."`, and these data mappings:

```python
datas = [
    ("CAR_LAB/vehicles", "vehicles"),
    ("CAR_LAB/docs", "docs"),
    ("CAR_LAB/examples", "examples"),
]
hiddenimports = collect_submodules("bleak.backends")
```

`build_portable_windows.ps1` selects `CAR_LAB/.venv/Scripts/python.exe` when it exists and otherwise uses the `python` found on `PATH`. It must fail if an intended release output already exists rather than deleting or overwriting it. It builds in a fresh timestamped directory, runs the frozen smoke mode, copies `LICENSE` and `README.md` into the app folder, archives the top-level `DiCAR LAB` folder, and writes this checksum format:

```text
<64 lowercase hex characters>  DiCAR-LAB-v1.7.0-Windows-x64.zip
```

`build_portable_windows.bat` calls:

```bat
@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_portable_windows.ps1"
```

- [ ] **Step 4: Verify the real bundle and release script**

Run:

```powershell
& $buildPython -m pip install -r requirements-build.txt
$env:DICAR_PACKAGE_TEST='1'
& $buildPython -m unittest tests.test_portable_build -v
Remove-Item Env:DICAR_PACKAGE_TEST
powershell -NoProfile -ExecutionPolicy Bypass -File .\build_portable_windows.ps1
```

Expected: integration test passes; the script creates the versioned ZIP and checksum without modifying tracked source files.

- [ ] **Step 5: Commit the packaging sources, not generated artifacts**

```powershell
git add -- requirements-build.txt dicar_lab.spec build_portable_windows.ps1 build_portable_windows.bat tests/test_portable_build.py
git commit -m "build: add portable Windows package"
```

---

### Task 6: GitHub test and release workflow

**Files:**
- Create: `.github/workflows/windows-release.yml`
- Test: `tests/test_windows_release_workflow.py`

**Interfaces:**
- Pull requests and pushes to `main`: run Python tests and build the portable artifact.
- Tags matching `v*`: upload the ZIP and checksum to a GitHub Release.
- Manual dispatch: run the same test/build path without publishing a release.

- [ ] **Step 1: Write the failing workflow-structure test**

```python
# tests/test_windows_release_workflow.py
import unittest
from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[1]


class WindowsReleaseWorkflowTests(unittest.TestCase):
    def test_workflow_tests_builds_and_publishes_tagged_artifacts(self):
        path = ROOT / ".github" / "workflows" / "windows-release.yml"
        workflow = yaml.load(path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)
        self.assertEqual("write", workflow["permissions"]["contents"])
        steps = workflow["jobs"]["windows-portable"]["steps"]
        commands = "\n".join(step.get("run", "") for step in steps)
        actions = "\n".join(step.get("uses", "") for step in steps)
        self.assertIn("python -m unittest discover -s tests -v", commands)
        self.assertIn("build_portable_windows.ps1", commands)
        self.assertIn("actions/upload-artifact@v4", actions)
        self.assertIn("softprops/action-gh-release@v2", actions)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the workflow test and verify RED**

Run: `python -m unittest tests.test_windows_release_workflow -v`

Expected: file-not-found error for `windows-release.yml`.

- [ ] **Step 3: Add the single Windows workflow**

Create one `windows-portable` job on `windows-latest` with:

```yaml
permissions:
  contents: write

steps:
  - uses: actions/checkout@v4
  - uses: actions/setup-python@v5
    with:
      python-version: "3.12"
      cache: pip
  - run: python -m pip install -r CAR_LAB/requirements.txt -r requirements-build.txt
  - run: python -m unittest discover -s tests -v
  - shell: powershell
    run: .\build_portable_windows.ps1
  - uses: actions/upload-artifact@v4
    with:
      name: dicar-lab-windows-x64
      path: |
        release/*.zip
        release/SHA256SUMS.txt
  - if: startsWith(github.ref, 'refs/tags/v')
    uses: softprops/action-gh-release@v2
    with:
      files: |
        release/*.zip
        release/SHA256SUMS.txt
```

Trigger on pull requests to `main`, pushes to `main`, `v*` tags, and `workflow_dispatch`.

- [ ] **Step 4: Run workflow and full fast tests**

Run: `python -m unittest tests.test_windows_release_workflow -v`

Run: `python -m unittest discover -s tests -v`

Expected: workflow behavior test and all non-packaging tests pass; packaging integration remains skipped unless explicitly enabled.

- [ ] **Step 5: Commit the workflow**

```powershell
git add -- .github/workflows/windows-release.yml tests/test_windows_release_workflow.py
git commit -m "ci: publish Windows portable release"
```

---

### Task 7: End-user README and v1.7.0 release notes

**Files:**
- Modify: `README.md`
- Modify: `CAR_LAB/README.md`
- Modify: `CHANGELOG.md`
- Modify: `README_小白用户.txt`
- Modify: `README_开发者.txt`
- Test: `tests/test_release_documentation.py`

**Interfaces:**
- README download path: GitHub Releases for `zhaoxc857/DiCar_Tune`.
- Artifact name: `DiCAR-LAB-v1.7.0-Windows-x64.zip`.
- Current capability copy: firmware page is a non-executing boundary; unlimited flashing is a roadmap item.

- [ ] **Step 1: Write the failing user-visible consistency test**

```python
# tests/test_release_documentation.py
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class ReleaseDocumentationTests(unittest.TestCase):
    def test_readme_names_download_and_does_not_claim_flashing_support(self):
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("DiCAR-LAB-v1.7.0-Windows-x64.zip", readme)
        self.assertIn("https://github.com/zhaoxc857/DiCar_Tune/releases", readme)
        self.assertIn("无限烧录路线图", readme)
        self.assertIn("本版本不会执行任何烧录命令", readme)

    def test_changelog_starts_with_v170_release(self):
        changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertLess(changelog.index("## v1.7.0"), changelog.index("## v1.6.1"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the documentation test and verify RED**

Run: `python -m unittest tests.test_release_documentation -v`

Expected: the portable artifact and v1.7.0 entries are missing.

- [ ] **Step 3: Rewrite the root README around the desktop release**

Keep the existing product description and protocol summary, but lead with:

1. `下载桌面版` linking to GitHub Releases.
2. Extract ZIP, run `DiCAR LAB.exe`, and try the simulator first.
3. Windows x64 and no-Python requirement statement.
4. SHA-256 verification command using `Get-FileHash`.
5. STM32F103 + HC-05 quick connection instructions.
6. `无限烧录路线图` with the exact sentence `本版本不会执行任何烧录命令`.
7. Developer test and `build_portable_windows.bat` commands.

Add v1.7.0 to `CHANGELOG.md`. Update both text guides and `CAR_LAB/README.md` so source users and packaged users receive the correct entry points.

- [ ] **Step 4: Run documentation, branding, and version tests**

Run:

```powershell
python -m unittest tests.test_release_documentation tests.test_branding tests.test_branding_docs tests.test_application_release -v
```

Expected: all release naming and branding checks pass.

- [ ] **Step 5: Commit documentation**

```powershell
git add -- README.md CAR_LAB/README.md CHANGELOG.md README_小白用户.txt README_开发者.txt tests/test_release_documentation.py
git commit -m "docs: publish DiCAR LAB desktop guide"
```

---

### Task 8: Full verification, repository replacement, and GitHub publication

**Files:**
- Verify all intended migration, desktop, firmware, test, and documentation files.
- Exclude local planning state and generated runtime/build artifacts from commits.

**Interfaces:**
- Local verification: Python unit suite, static startup check, focused firmware contract tests, package integration, frozen smoke, ZIP checksum.
- Remote publication: normal fast-forward update of `origin/main`, annotated tag `v1.7.0`, and GitHub Release artifacts.

- [ ] **Step 1: Re-read the approved spec and run all source verification**

Run:

```powershell
python -m unittest discover -s tests -v
python CAR_LAB/tools/static_startup_check.py
python -m compileall -q CAR_LAB tests
```

Expected: zero failures, zero errors, and no unexpected warnings.

- [ ] **Step 2: Run the package integration and verify checksum content**

Run:

```powershell
$env:DICAR_PACKAGE_TEST='1'
python -m unittest tests.test_portable_build -v
Remove-Item Env:DICAR_PACKAGE_TEST
Get-Content release/SHA256SUMS.txt
Get-FileHash release/DiCAR-LAB-v1.7.0-Windows-x64.zip -Algorithm SHA256
```

Expected: packaged executable exits 0 in smoke mode; checksum text matches `Get-FileHash` case-insensitively.

- [ ] **Step 3: Audit the final repository without cleaning or restoring anything**

Run:

```powershell
git diff --check
git status --short
git diff --stat
git diff --name-only
git ls-files --others --exclude-standard
```

Inspect every intended path. Do not stage `.planning/`, generated `release/` artifacts, build directories, virtual environments, logs, databases, or any path matching `*env*`, `credentials.*`, `*private*key*`, `*token*`, or `*secret*`. If a sensitive-looking path appears, stop before staging and report it.

- [ ] **Step 4: Commit the remaining approved migration state**

Stage tracked replacements plus the explicit new project paths only after the audit. Do not include `.planning/`, `AGENTS.md`, or `AGENT_GUARDRAILS.md`:

```powershell
git add -u
git add -- .gitattributes .github/workflows/windows-release.yml CAR_LAB CHANGELOG.md CONTRIBUTING.md DiCAR_Launcher.bat DiCAR_Launcher.py LICENSE README.md README_小白用户.txt README_开发者.txt VERSION.txt build_launcher_windows.bat build_portable_windows.bat build_portable_windows.ps1 dicar_lab.spec requirements-build.txt docs firmware/stm32f103_line_car tests
```

Confirm the staged file list, then commit:

```powershell
git diff --cached --name-only
git diff --cached --check
git commit -m "release: replace desktop app with DiCAR LAB v1.7.0"
```

Expected: old Tauri/Rust product paths are recorded as deletions, the PySide6 application is recorded as the replacement, and local/generated files remain unstaged.

- [ ] **Step 5: Explain the exact remote updates, then publish without force**

Run read-only preflight first:

```powershell
gh auth status
git fetch origin
git merge-base --is-ancestor origin/main HEAD
git status --short
```

Expected: GitHub CLI is authenticated, `origin/main` is an ancestor of `HEAD`, and only intentionally uncommitted local planning/generated files remain.

Then publish:

```powershell
git push origin HEAD:main
git tag -a v1.7.0 -m "DiCAR LAB v1.7.0"
git push origin v1.7.0
gh run list --workflow windows-release.yml --limit 3
```

Wait for the tag workflow. Verify the release:

```powershell
gh release view v1.7.0 --json url,assets,tagName
```

Expected: Release v1.7.0 lists `DiCAR-LAB-v1.7.0-Windows-x64.zip` and `SHA256SUMS.txt`.

---

## Plan Self-Review Checklist

- Every design requirement maps to one task: packaging (5), buttons (4), flashing boundary (2-3), docs (7), GitHub publication (6 and 8).
- Runtime version interfaces are defined once in Task 1 and consumed consistently later.
- Flash state names and transitions match the approved design exactly.
- No task invokes a real flashing backend.
- Tests exercise subprocess startup, real Qt widgets, real state transitions, real PyInstaller output, and parsed workflow behavior.
- The publication path contains no force-push, stash, reset, restore, checkout, or clean command.
