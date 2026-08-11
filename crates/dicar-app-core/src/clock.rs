use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub trait Clock: Send + 'static {
    fn now_ms(&self) -> u64;
    fn idle_until(&self, deadline_ms: u64);
}

#[derive(Debug)]
pub struct SystemClock {
    started_at: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn idle_until(&self, deadline_ms: u64) {
        let remaining = deadline_ms.saturating_sub(self.now_ms());
        if remaining != 0 {
            std::thread::sleep(Duration::from_millis(remaining.min(1)));
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestClock {
    now_ms: Arc<AtomicU64>,
}

impl TestClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance_ms(&self, elapsed_ms: u64) {
        self.now_ms.fetch_add(elapsed_ms, Ordering::SeqCst);
    }

    pub fn set_ms(&self, now_ms: u64) {
        self.now_ms.store(now_ms, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    fn idle_until(&self, _deadline_ms: u64) {
        std::thread::yield_now();
    }
}

pub trait NonceSource: Send + 'static {
    fn next_nonce(&mut self) -> u32;
}

#[derive(Clone, Copy, Debug)]
pub struct FixedNonce(pub u32);

impl NonceSource for FixedNonce {
    fn next_nonce(&mut self) -> u32 {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct SystemNonce {
    counter: u32,
}

impl NonceSource for SystemNonce {
    fn next_nonce(&mut self) -> u32 {
        self.counter = self.counter.wrapping_add(1);
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let nonce = time ^ self.counter.wrapping_mul(0x9e37_79b9);
        if nonce == 0 {
            1
        } else {
            nonce
        }
    }
}
