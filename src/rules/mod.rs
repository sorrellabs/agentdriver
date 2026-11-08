//! Screen rules: recognise prompts a TUI shows, and answer only the ones the
//! caller enabled by name.

use std::collections::BTreeSet;
use std::path::Path;

use crate::key::Key;
use crate::screen::Screen;

pub mod text;

use text::{has_continue_notice, menu_options};

/// Which rules may answer. Empty by default: prompts are reported, not answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulePolicy {
    enabled: BTreeSet<&'static str>,
}

impl RulePolicy {
    pub fn answering(rules: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            enabled: rules.into_iter().collect(),
        }
    }

    pub fn allows(&self, rule: &str) -> bool {
        self.enabled.contains(rule)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// Nothing on screen needs an answer.
    Clear,
    /// An enabled rule matched; press these keys.
    Resolve { rule: &'static str, keys: Vec<Key> },
    /// A known prompt whose rule the caller did not enable.
    Prompt { rule: &'static str, excerpt: String },
    /// A menu no rule recognises. Never pressed.
    Unknown { excerpt: String },
}

/// One rule's verdict before the policy is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub rule: &'static str,
    pub keys: Vec<Key>,
}

pub type Rule = fn(&Screen, &Path) -> Option<Match>;

/// Rules in evaluation order; the first match wins.
const RULES: &[Rule] = &[continue_notice];

pub fn classify(screen: &Screen, workspace: &Path, policy: &RulePolicy) -> Classification {
    if let Some(found) = RULES.iter().find_map(|rule| rule(screen, workspace)) {
        return if policy.allows(found.rule) {
            Classification::Resolve {
                rule: found.rule,
                keys: found.keys,
            }
        } else {
            Classification::Prompt {
                rule: found.rule,
                excerpt: screen.excerpt(),
            }
        };
    }
    if menu_options(&screen.text)
        .iter()
        .any(|option| option.selected)
    {
        return Classification::Unknown {
            excerpt: screen.excerpt(),
        };
    }
    Classification::Clear
}

/// `continue-notice`: a "press Enter to continue" line, with no menu on screen.
fn continue_notice(screen: &Screen, _workspace: &Path) -> Option<Match> {
    let menu = menu_options(&screen.text)
        .iter()
        .any(|option| option.selected);
    (!menu && has_continue_notice(&screen.text)).then(|| Match {
        rule: "continue-notice",
        keys: vec![Key::Enter],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classified(text: &str, policy: &RulePolicy) -> Classification {
        classify(&Screen::new(text), Path::new("/repo/task"), policy)
    }

    #[test]
    fn a_prompt_is_reported_unless_its_rule_is_enabled() {
        let text = "Startup complete. Press Enter to continue";
        assert!(matches!(
            classified(text, &RulePolicy::default()),
            Classification::Prompt {
                rule: "continue-notice",
                ..
            }
        ));
        assert_eq!(
            classified(text, &RulePolicy::answering(["continue-notice"])),
            Classification::Resolve {
                rule: "continue-notice",
                keys: vec![Key::Enter]
            }
        );
    }

    #[test]
    fn a_menu_no_rule_covers_is_unknown_and_never_pressed() {
        let policy = RulePolicy::answering(["continue-notice"]);
        assert_eq!(
            classified("Choose a theme\n› 1. Dark\n  2. Light\n", &policy),
            Classification::Unknown {
                excerpt: "Choose a theme | › 1. Dark | 2. Light".to_owned()
            }
        );
    }

    #[test]
    fn an_ordinary_agent_screen_is_clear() {
        assert_eq!(
            classified("some output\n❯ \n", &RulePolicy::default()),
            Classification::Clear
        );
    }
}
