//! A screen rule: recognises one prompt and says which keys answer it.

use std::path::Path;

use crate::domain::key::Key;
use crate::domain::screen::Screen;

#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// Stable name callers use to enable it, such as `continue-notice`.
    pub name: &'static str,
    /// The keys that answer this prompt, when `screen` shows it.
    pub answer: fn(&Screen, &Path) -> Option<Vec<Key>>,
}
