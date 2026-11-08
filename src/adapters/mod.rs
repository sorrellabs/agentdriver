//! Connects the ports to real hosts, harnesses, disks and clocks.

use std::sync::Arc;

use crate::core::Driver;
use crate::core::rules::RulePolicy;
use crate::ports::host::Host;

pub mod clock;
pub mod exec;
pub mod harness;
pub mod host;
pub mod store;

/// A driver using the real clock and the on-disk store.
pub fn default_driver(host: Arc<dyn Host>, policy: RulePolicy) -> Driver {
    Driver::new(
        host,
        Arc::new(store::FsStore::new(store::FsStore::default_root())),
        Arc::new(clock::SystemClock::default()),
        policy,
    )
}
