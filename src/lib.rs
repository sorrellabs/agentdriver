//! Drive coding-agent TUIs inside terminal hosts.
//!
//! A [`Driver`] starts a harness (Claude Code, Codex, …) in a pane of a host
//! (Herdr, tmux, …) and returns a [`Session`]. A session reads the screen,
//! sends input, waits for a condition and closes the pane.
//!
//! The library is synchronous. Every external call is bounded by a timeout
//! and an output cap; see [`exec`].
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod driver;
pub mod error;
pub mod exec;
pub mod harness;
pub mod host;
pub mod id;
pub mod key;
pub mod rules;
pub mod screen;
pub mod session;
pub mod state;
pub mod wait;

pub use driver::{Config, Driver, SessionSpec};
pub use error::{Error, ErrorKind, Result};
pub use harness::{Harness, HarnessKind};
pub use host::{Host, HostKind};
pub use id::{PaneId, SessionId, TerminalId};
pub use key::Key;
pub use rules::RulePolicy;
pub use screen::Screen;
pub use session::Session;
