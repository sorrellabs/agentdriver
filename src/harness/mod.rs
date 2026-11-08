//! The port every agent harness implements, and the built-in recipes.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

mod recipe;

pub use recipe::{Recipe, recipes};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum HarnessKind {
    Claude,
    Codex,
    Cursor,
    Devin,
    OpenCode,
    Omp,
    CommandCode,
    Cline,
    Copilot,
    Gemini,
    Pi,
}

impl HarnessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Devin => "devin",
            Self::OpenCode => "opencode",
            Self::Omp => "omp",
            Self::CommandCode => "command-code",
            Self::Cline => "cline",
            Self::Copilot => "copilot",
            Self::Gemini => "gemini",
            Self::Pi => "pi",
        }
    }
}

impl fmt::Display for HarnessKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    /// Resolved by the caller; a recipe never guesses a binary name.
    pub executable: PathBuf,
    pub model: Option<String>,
    /// Caller's explicit choice to run with the harness's full-permission mode.
    pub full_permission: bool,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchPlan {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// A per-launch or per-workspace way to learn that a turn finished.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignalSetup {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Files written into the workspace and removed on close.
    pub files: Vec<(PathBuf, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    TurnFinished { answer: Option<String> },
}

pub trait Harness: Send + Sync + fmt::Debug {
    fn kind(&self) -> HarnessKind;
    fn launch(&self, request: &LaunchRequest) -> Result<LaunchPlan>;

    /// Defaults to no signal: idle then comes from the host or the screen.
    fn signal(&self, _state_dir: &Path) -> SignalSetup {
        SignalSetup::default()
    }

    fn parse_signal(&self, _line: &str) -> Option<Signal> {
        None
    }
}
