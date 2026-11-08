//! Time as a port, so a three-minute deadline can be tested instantly.

use std::fmt;
use std::time::Duration;

pub trait Clock: Send + Sync + fmt::Debug {
    /// Monotonic time since an arbitrary start.
    fn now(&self) -> Duration;
    fn sleep(&self, duration: Duration);
    fn unix_seconds(&self) -> u64;
}
