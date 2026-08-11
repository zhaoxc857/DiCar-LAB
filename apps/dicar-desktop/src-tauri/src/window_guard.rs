use dicar_app_core::{AppSnapshot, SnapshotPhase};
use serde::Deserialize;

use crate::BridgeErrorDto;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseDecision {
    Cancel,
    DisconnectKeepUnknown,
    RevertThenClose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseRequestOutcome {
    Allow,
    Prevented {
        request_id: u64,
        dirty_count: usize,
        can_revert: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseResolution {
    KeepOpen,
    CloseWindow,
}

#[derive(Clone, Copy)]
struct ActiveCloseRequest {
    request_id: u64,
    dirty_count: usize,
    can_revert: bool,
    resolving: bool,
}

#[derive(Default)]
pub(crate) struct WindowCloseCoordinator {
    next_request_id: u64,
    active: Option<ActiveCloseRequest>,
}

impl WindowCloseCoordinator {
    pub(crate) fn request(
        &mut self,
        snapshot: &AppSnapshot,
    ) -> Result<(CloseRequestOutcome, bool), BridgeErrorDto> {
        if snapshot.dirty_count == 0 {
            return Ok((CloseRequestOutcome::Allow, false));
        }
        if let Some(active) = self.active {
            return Ok((outcome(active), false));
        }
        self.next_request_id = self.next_request_id.checked_add(1).ok_or_else(|| {
            BridgeErrorDto::new("closeRequestExhausted", "窗口关闭请求序号已耗尽")
        })?;
        let active = ActiveCloseRequest {
            request_id: self.next_request_id,
            dirty_count: snapshot.dirty_count,
            can_revert: snapshot.phase == SnapshotPhase::Ready,
            resolving: false,
        };
        self.active = Some(active);
        Ok((outcome(active), true))
    }

    pub(crate) fn begin_resolution(&mut self, request_id: u64) -> Result<(), BridgeErrorDto> {
        let active = self.active.as_mut().ok_or_else(stale_close_request)?;
        if active.request_id != request_id || active.resolving {
            return Err(stale_close_request());
        }
        active.resolving = true;
        Ok(())
    }

    pub(crate) fn complete(&mut self, request_id: u64) -> Result<(), BridgeErrorDto> {
        let active = self.active.ok_or_else(stale_close_request)?;
        if active.request_id != request_id {
            return Err(stale_close_request());
        }
        self.active = None;
        Ok(())
    }

    pub(crate) fn retryable_failure(&mut self, request_id: u64) {
        if let Some(active) = self.active.as_mut() {
            if active.request_id == request_id {
                active.resolving = false;
            }
        }
    }

    pub(crate) fn discard(&mut self, request_id: u64) {
        if self
            .active
            .is_some_and(|active| active.request_id == request_id)
        {
            self.active = None;
        }
    }
}

fn outcome(active: ActiveCloseRequest) -> CloseRequestOutcome {
    CloseRequestOutcome::Prevented {
        request_id: active.request_id,
        dirty_count: active.dirty_count,
        can_revert: active.can_revert,
    }
}

fn stale_close_request() -> BridgeErrorDto {
    BridgeErrorDto::new("staleCloseRequest", "关闭请求已失效")
}
