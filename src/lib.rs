//! Drive coding-agent TUIs inside terminal hosts.
//!
//! A [`Driver`] starts a harness (Claude Code, Codex, …) in a pane of a host
//! (Herdr, tmux, …) and returns a [`Session`]. A session reads the screen,
//! sends input, waits for a condition and closes the pane.
//!
//! The crate is hexagonal. [`domain`] is shared vocabulary, [`ports`] are the
//! traits the core needs, [`core`](crate::core) is the logic, and [`adapters`]
//! connect the ports to the outside world. `tests/architecture.rs` enforces
//! the direction of those dependencies.
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod adapters;
pub mod core;
pub mod domain;
pub mod ports;

pub use crate::core::{Driver, Session, SessionSpec, Wait};
pub use crate::domain::error::{Error, ErrorKind, Result};
pub use crate::domain::id::{HarnessKind, HostKind, PaneId, SessionId, TerminalId};
pub use crate::domain::key::Key;
pub use crate::domain::screen::Screen;
pub use crate::ports::clock::Clock;
pub use crate::ports::harness::Harness;
pub use crate::ports::host::Host;
pub use crate::ports::store::Store;
