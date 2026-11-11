//! The logic. Talks to the outside only through [`crate::ports`].

mod driver;
pub mod rules;
mod session;
pub mod wait;
pub mod watch;

pub use driver::{Driver, SessionSpec};
pub use session::Session;
pub use wait::Wait;
pub use watch::ScreenWatch;
