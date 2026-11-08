//! Screen-text helpers shared by the rules.

use crate::domain::key::Key;

/// Removes all whitespace, so text wrapped by a narrow pane still matches.
pub fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuOption {
    pub selected: bool,
    pub text: String,
}

const BORDERS: [char; 3] = ['│', '┃', '║'];
const CURSORS: [&str; 4] = ["›", "❯", ">", "▸"];

/// Numbered menu rows on screen, with the row the cursor is on marked.
pub fn menu_options(screen: &str) -> Vec<MenuOption> {
    screen
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_matches(BORDERS).trim();
            let cursor = CURSORS.iter().find_map(|cursor| line.strip_prefix(cursor));
            let rest = cursor.map_or(line, str::trim_start);
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            if digits == 0 {
                return None;
            }
            let text = rest[digits..].strip_prefix(['.', ')'])?;
            Some(MenuOption {
                selected: cursor.is_some(),
                text: text.trim().to_owned(),
            })
        })
        .collect()
}

/// Arrow presses that move the cursor between two menu rows.
pub fn keys_between(from: usize, to: usize) -> Vec<Key> {
    let (key, count) = if to > from {
        (Key::Down, to - from)
    } else {
        (Key::Up, from - to)
    };
    vec![key; count]
}

const NOTICES: [&str; 3] = [
    "press enter to continue",
    "press return to continue",
    "press any key to continue",
];

/// A continue notice standing as its own sentence, not quoted in prose.
pub fn has_continue_notice(screen: &str) -> bool {
    screen.lines().any(|line| {
        let line = line
            .trim()
            .trim_matches(BORDERS)
            .trim()
            .trim_start_matches(['•', '·', '*', '-', '›', '❯'])
            .trim_start()
            .to_lowercase();
        NOTICES.iter().any(|notice| {
            line.match_indices(notice).any(|(at, _)| {
                let before = line[..at].trim_end();
                let after = line[at + notice.len()..].trim();
                (before.is_empty() || before.ends_with(['.', '!', '?']))
                    && after.chars().all(is_closing_punctuation)
            })
        })
    })
}

fn is_closing_punctuation(character: char) -> bool {
    character.is_ascii_punctuation() || "…⋯”’»。！？、".contains(character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_rows_are_read_through_borders_and_cursors() {
        let options = menu_options("│ ❯ 1. Yes │\n│   2) No │\nnot a row\n");
        assert_eq!(
            options,
            [
                MenuOption {
                    selected: true,
                    text: "Yes".to_owned()
                },
                MenuOption {
                    selected: false,
                    text: "No".to_owned()
                },
            ]
        );
    }

    #[test]
    fn a_shell_prompt_is_not_a_menu_row() {
        assert!(menu_options("> ./run.sh\n").is_empty());
    }

    #[test]
    fn keys_move_toward_the_target_row() {
        assert_eq!(keys_between(0, 2), [Key::Down, Key::Down]);
        assert_eq!(keys_between(1, 0), [Key::Up]);
        assert!(keys_between(1, 1).is_empty());
    }

    #[test]
    fn a_sentence_followed_by_a_continue_notice_counts() {
        assert!(has_continue_notice(
            "│ • Startup complete. Press Enter to continue… │"
        ));
    }

    #[test]
    fn prose_about_a_continue_notice_does_not_count() {
        for text in [
            "\"Press Enter to continue\" appeared",
            "I saw Press Enter to continue in the log.",
            "Then it says Press Enter to continue",
            "Press Enter to continue or Esc to cancel",
        ] {
            assert!(!has_continue_notice(text), "{text}");
        }
    }
}
