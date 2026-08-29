import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(
    os.environ.get("DICAR_PACKAGE_TEST") == "1",
    "set DICAR_PACKAGE_TEST=1",
)
class PortableBuildTests(unittest.TestCase):
    def test_spec_builds_a_smoke_testable_app_with_runtime_resources(self):
        with tempfile.TemporaryDirectory() as temp:
            temp = Path(temp)
            build_env = os.environ.copy()
            system_root = Path(
                build_env.get("SystemRoot")
                or build_env.get("SYSTEMROOT")
                or build_env["WINDIR"]
            )
            build_env["PATH"] = os.pathsep.join(
                (
                    str(Path(sys.executable).parent),
                    str(system_root / "System32"),
                    str(system_root),
                    str(system_root / "System32" / "Wbem"),
                    str(system_root / "System32" / "WindowsPowerShell" / "v1.0"),
                )
            )
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "PyInstaller",
                    "dicar_lab.spec",
                    "--distpath",
                    str(temp / "dist"),
                    "--workpath",
                    str(temp / "work"),
                ],
                cwd=ROOT,
                env=build_env,
                capture_output=True,
                text=True,
                timeout=600,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            app = temp / "dist" / "DiCAR LAB"
            exe = app / "DiCAR LAB.exe"
            self.assertTrue(exe.is_file())
            runtime = app / "_internal"
            self.assertTrue(
                (runtime / "vehicles" / "stm32f103_line_car.yaml").is_file()
            )
            self.assertTrue((runtime / "docs").is_dir())
            env = build_env.copy()
            env.update(QT_QPA_PLATFORM="offscreen", DICAR_SMOKE_TEST="1")
            smoke = subprocess.run(
                [exe],
                env=env,
                capture_output=True,
                text=True,
                timeout=60,
            )
            self.assertEqual(0, smoke.returncode, smoke.stdout + smoke.stderr)


if __name__ == "__main__":
    unittest.main()
