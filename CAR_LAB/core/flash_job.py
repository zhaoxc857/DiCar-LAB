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
    FlashState.VALIDATING: {
        FlashState.FLASHING,
        FlashState.FAILED,
        FlashState.CANCELLED,
    },
    FlashState.FLASHING: {FlashState.VERIFYING, FlashState.FAILED, FlashState.CANCELLED},
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
            raise ValueError(
                f"invalid flash transition: {self.state.name} -> {target.name}"
            )
        return FlashJobState(target, message)
