//! The port every terminal host implements.
//!
//! Required methods are the verbs every host has. Everything else defaults to
//! [`ErrorKind::Unsupported`] and is announced through [`Capabilities`], so the
//! core can refuse early instead of guessing.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{ErrorKind, Result};
use crate::id::{PaneId, TerminalId};
use crate::key::Key;
use crate::screen::Screen;

#[cfg(any(test, feature = "fake"))]
pub mod fake;
#[cfg(feature = "herdr")]
pub mod herdr;
#[cfg(feature = "tmux")]
pub mod tmux;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum HostKind {
    Herdr,
    Tmux,
    Orca,
    Cmux,
    T3Code,
    Fake,
}

impl HostKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Herdr => "herdr",
            Self::Tmux => "tmux",
            Self::Orca => "orca",
            Self::Cmux => "cmux",
            Self::T3Code => "t3-code",
            Self::Fake => "fake",
        }
    }
}

impl fmt::Display for HostKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// What a host can do beyond the required verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Capabilities {
    pub agent_state: bool,
    pub agent_start: bool,
    pub prompt: bool,
    pub layout: bool,
    pub locate_caller: bool,
}

/// A pane plus what proves it is still the same pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneRef {
    pub pane: PaneId,
    pub terminal: Option<TerminalId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub pane: PaneId,
    pub terminal: Option<TerminalId>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentState {
    Idle,
    Working,
    Blocked,
    Done,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Right,
    Down,
}

/// Where a new pane goes. Policies such as stacking workers beside their
/// caller are computed in the core from [`Host::layout`] and end up here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    Split {
        target: PaneId,
        direction: Direction,
    },
    Tab {
        label: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartMode {
    /// The host runs this argv in the new pane.
    Run(Vec<String>),
    /// The host recognises the agent and starts it itself.
    Agent {
        name: String,
        kind: String,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub placement: Placement,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub start: StartMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReadSource {
    #[default]
    Visible,
    RecentUnwrapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOptions {
    pub source: ReadSource,
    pub lines: u32,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            source: ReadSource::Visible,
            lines: 300,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextMode {
    /// Bracketed paste; embedded newlines do not submit.
    #[default]
    Paste,
    Literal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub panes: Vec<(PaneId, Rect)>,
}

pub trait Host: Send + Sync + fmt::Debug {
    fn kind(&self) -> HostKind;
    fn capabilities(&self) -> Capabilities;

    fn spawn(&self, spec: &SpawnSpec) -> Result<PaneRef>;
    fn read(&self, pane: &PaneId, options: ReadOptions) -> Result<Screen>;
    fn send_text(&self, pane: &PaneId, text: &str, mode: TextMode) -> Result<()>;
    fn send_keys(&self, pane: &PaneId, keys: &[Key]) -> Result<()>;
    fn close(&self, pane: &PaneId) -> Result<()>;

    /// `Ok(None)` is a proven absence; any doubt is an error.
    fn pane_info(&self, pane: &PaneId) -> Result<Option<PaneInfo>>;

    fn agent_state(&self, _pane: &PaneId) -> Result<AgentState> {
        Err(unsupported(self.kind(), "agent_state"))
    }

    /// Submits `text` through the host's own prompt path.
    fn prompt(&self, _pane: &PaneId, _text: &str) -> Result<()> {
        Err(unsupported(self.kind(), "prompt"))
    }

    fn layout(&self, _pane: &PaneId) -> Result<Layout> {
        Err(unsupported(self.kind(), "layout"))
    }

    fn resize(&self, _pane: &PaneId, _direction: Direction, _amount: f64) -> Result<()> {
        Err(unsupported(self.kind(), "resize"))
    }

    fn rename(&self, _pane: &PaneId, _label: &str) -> Result<()> {
        Err(unsupported(self.kind(), "rename"))
    }

    /// The pane the calling process runs in, proven rather than read from env.
    fn locate_caller(&self, _deadline: Duration) -> Result<Option<PaneId>> {
        Err(unsupported(self.kind(), "locate_caller"))
    }
}

pub(crate) fn unsupported(host: HostKind, operation: &'static str) -> crate::Error {
    ErrorKind::Unsupported { host, operation }.into()
}
