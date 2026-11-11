//! A screen rule: recognises one prompt and says what to do about it.

use std::path::Path;

use crate::domain::key::Key;
use crate::domain::screen::Screen;

/// What a rule may look at besides the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleContext<'a> {
    /// The session's own spawn directory: the only folder a trust answer is for.
    pub workspace: &'a Path,
    /// The primary checkout of the repository `workspace` is a worktree of,
    /// when the caller knows it. Codex asks to trust that root for a worktree.
    pub primary_checkout: Option<&'a Path>,
}

impl<'a> RuleContext<'a> {
    pub fn new(workspace: &'a Path) -> Self {
        Self {
            workspace,
            primary_checkout: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// These keys answer the prompt, pressed in one call.
    Answer(Vec<Key>),
    /// The prompt is recognised but must not be answered, whatever the policy:
    /// a trust prompt for another folder, for one.
    Refuse,
}

#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// Stable name callers use to enable it, such as `continue-notice`.
    pub name: &'static str,
    /// `None` when `screen` does not show this rule's prompt.
    pub answer: fn(&Screen, &RuleContext<'_>) -> Option<Verdict>,
}
