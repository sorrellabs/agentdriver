//! The port every agent harness implements.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::domain::error::Result;
use crate::domain::id::HarnessKind;
use crate::domain::rule::Rule;

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

    /// The prompts this harness can show, in evaluation order.
    fn rules(&self) -> &[Rule] {
        &[]
    }
}
