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
        failed = FlashJobState(FlashState.FLASHING).transition(
            FlashState.FAILED,
            "write failed",
        )
        cancelled = FlashJobState(FlashState.VALIDATING).transition(
            FlashState.CANCELLED
        )
        self.assertEqual("write failed", failed.message)
        self.assertEqual(FlashState.CANCELLED, cancelled.state)
        with self.assertRaises(ValueError):
            failed.transition(FlashState.SUCCEEDED)


if __name__ == "__main__":
    unittest.main()
