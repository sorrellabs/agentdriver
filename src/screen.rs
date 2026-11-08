//! A snapshot of a pane's visible text.
//!
//! agentdriver matches against snapshots, never a consumed output stream:
//! full-screen TUIs repaint, so a stream does not describe what is on screen.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Screen {
    pub text: String,
    /// Host content revision, when the host reports one.
    pub revision: Option<u64>,
}

impl Screen {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            revision: None,
        }
    }

    /// The last eight non-empty rows, trimmed and joined with ` | `.
    pub fn excerpt(&self) -> String {
        let rows: Vec<&str> = self
            .text
            .lines()
            .map(str::trim)
            .filter(|row| !row.is_empty())
            .collect();
        rows[rows.len().saturating_sub(8)..].join(" | ")
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.text.contains(needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_keeps_the_last_eight_non_empty_rows() {
        let text = (1..=10).map(|n| format!(" {n} \n\n")).collect::<String>();
        assert_eq!(
            Screen::new(text).excerpt(),
            "3 | 4 | 5 | 6 | 7 | 8 | 9 | 10"
        );
    }
}
