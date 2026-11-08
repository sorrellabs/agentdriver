//! Clocks: the real one, and one tests can run instantly.

use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::ports::clock::Clock;

#[derive(Debug)]
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }

    fn unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    }
}

/// Sleeping advances time at once.
#[cfg(feature = "fake")]
#[derive(Debug, Default)]
pub struct FakeClock(std::sync::atomic::AtomicU64);

#[cfg(feature = "fake")]
impl Clock for FakeClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn sleep(&self, duration: Duration) {
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.0.fetch_add(nanos, std::sync::atomic::Ordering::SeqCst);
    }

    fn unix_seconds(&self) -> u64 {
        self.now().as_secs()
    }
}
