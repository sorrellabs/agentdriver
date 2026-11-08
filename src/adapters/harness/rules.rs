//! Screen rules for the built-in harnesses.

use std::path::Path;

use crate::domain::key::Key;
use crate::domain::rule::Rule;
use crate::domain::screen::Screen;
use crate::domain::text::{has_continue_notice, menu_options};

/// A "press Enter to continue" line, with no menu on screen.
pub const CONTINUE_NOTICE: Rule = Rule {
    name: "continue-notice",
    answer: continue_notice,
};

/// Rules every harness shares, in evaluation order.
pub const COMMON: &[Rule] = &[CONTINUE_NOTICE];

fn continue_notice(screen: &Screen, _workspace: &Path) -> Option<Vec<Key>> {
    let menu = menu_options(&screen.text)
        .iter()
        .any(|option| option.selected);
    (!menu && has_continue_notice(&screen.text)).then(|| vec![Key::Enter])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answered(text: &str) -> Option<Vec<Key>> {
        (CONTINUE_NOTICE.answer)(&Screen::new(text), Path::new("/repo/task"))
    }

    #[test]
    fn a_continue_notice_is_answered_with_enter() {
        assert_eq!(
            answered("Startup complete. Press Enter to continue"),
            Some(vec![Key::Enter])
        );
    }

    #[test]
    fn a_continue_notice_beside_a_menu_is_left_to_the_menu_rules() {
        assert_eq!(
            answered("Press Enter to continue\n› 1. Update now\n  2. Skip\n"),
            None
        );
    }
}
