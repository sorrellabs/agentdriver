//! Decides what a screen needs. Rules come from the harness; which of them
//! may answer comes from the caller.

use std::collections::BTreeSet;
use std::path::Path;

use crate::domain::key::Key;
use crate::domain::rule::Rule;
use crate::domain::screen::Screen;
use crate::domain::text::menu_options;

/// Which rules may answer. Empty by default: prompts are reported, not answered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulePolicy {
    enabled: BTreeSet<String>,
}

impl RulePolicy {
    pub fn answering<S: Into<String>>(rules: impl IntoIterator<Item = S>) -> Self {
        Self {
            enabled: rules.into_iter().map(Into::into).collect(),
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

/// The first matching rule wins.
pub fn classify(
    screen: &Screen,
    workspace: &Path,
    rules: &[Rule],
    policy: &RulePolicy,
) -> Classification {
    let matched = rules
        .iter()
        .find_map(|rule| (rule.answer)(screen, workspace).map(|keys| (rule.name, keys)));
    if let Some((rule, keys)) = matched {
        return if policy.allows(rule) {
            Classification::Resolve { rule, keys }
        } else {
            Classification::Prompt {
                rule,
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

#[cfg(test)]
mod tests {
    use super::*;

    const READY: Rule = Rule {
        name: "ready",
        answer: |screen, _| screen.contains("Ready?").then(|| vec![Key::Enter]),
    };

    fn classified(text: &str, policy: &RulePolicy) -> Classification {
        classify(
            &Screen::new(text),
            Path::new("/repo/task"),
            &[READY],
            policy,
        )
    }

    #[test]
    fn a_prompt_is_reported_unless_its_rule_is_enabled() {
        assert!(matches!(
            classified("Ready?", &RulePolicy::default()),
            Classification::Prompt { rule: "ready", .. }
        ));
        assert_eq!(
            classified("Ready?", &RulePolicy::answering(["ready"])),
            Classification::Resolve {
                rule: "ready",
                keys: vec![Key::Enter]
            }
        );
    }

    #[test]
    fn a_menu_no_rule_covers_is_unknown_and_never_pressed() {
        assert_eq!(
            classified(
                "Choose a theme\n› 1. Dark\n  2. Light\n",
                &RulePolicy::answering(["ready"])
            ),
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
