//! Bounds on a screen that will not go away, ported from brgr's `ScreenWatch`.
//!
//! A screen no rule answers may stand for [`SCREEN_DEADLINE`]; a rule may answer
//! the same standing screen [`PRESS_BUDGET`] times. Past either, the session
//! fails with the screen's text instead of waiting or pressing forever. Time is
//! passed in from the session's [`Clock`](crate::ports::clock::Clock).

use std::time::Duration;

use crate::core::rules::Classification;
use crate::domain::error::{ErrorKind, Result};
use crate::domain::id::PaneId;

/// How long a screen no rule answers may stand.
pub const SCREEN_DEADLINE: Duration = Duration::from_secs(3 * 60);

/// How many times one rule may answer before the screen counts as stuck: a
/// screen a key does not clear would otherwise be pressed forever.
pub const PRESS_BUDGET: u32 = 5;

#[derive(Debug, Clone)]
pub struct ScreenWatch {
    pane: PaneId,
    limit: Duration,
    /// When the current unknown screen was first seen.
    since: Option<Duration>,
    presses: Vec<(&'static str, u32)>,
}

impl ScreenWatch {
    pub fn new(pane: PaneId) -> Self {
        Self::with_limit(pane, SCREEN_DEADLINE)
    }

    pub fn with_limit(pane: PaneId, limit: Duration) -> Self {
        Self {
            pane,
            limit,
            since: None,
            presses: Vec::new(),
        }
    }

    /// Records a press for `rule`. Fails on the press past the budget.
    pub fn pressed(&mut self, rule: &'static str) -> Result<()> {
        let count = match self.presses.iter_mut().find(|(name, _)| *name == rule) {
            Some((_, count)) => {
                *count += 1;
                *count
            }
            None => {
                self.presses.push((rule, 1));
                1
            }
        };
        if count > PRESS_BUDGET {
            return Err(ErrorKind::PromptNotCleared {
                pane: self.pane.clone(),
                rule,
                presses: PRESS_BUDGET,
            }
            .into());
        }
        Ok(())
    }

    /// Records one poll's outcome at `now`. An unknown screen, or a prompt the
    /// caller did not enable, starts the clock; a changing unknown screen does
    /// not restart it. A clear screen is progress: clock and counts reset. An
    /// answered one stops the clock; count its press with [`Self::pressed`].
    pub fn observe(&mut self, outcome: &Classification, now: Duration) -> Result<()> {
        match outcome {
            Classification::Unknown { excerpt } | Classification::Prompt { excerpt, .. } => {
                self.stuck(excerpt, now)
            }
            Classification::Clear => {
                self.reset();
                Ok(())
            }
            Classification::Resolve { .. } => {
                self.restart_clock();
                Ok(())
            }
        }
    }

    /// Records a poll where the host reports the agent blocked but no rule
    /// answered: the same as an unknown screen.
    pub fn observe_blocked(&mut self, excerpt: &str, now: Duration) -> Result<()> {
        self.stuck(excerpt, now)
    }

    /// Forgets the clock and every press count.
    pub fn reset(&mut self) {
        self.since = None;
        self.presses.clear();
    }

    /// Stops the clock but keeps the press counts, for a screen that moved
    /// without the host saying the agent works.
    pub fn restart_clock(&mut self) {
        self.since = None;
    }

    fn stuck(&mut self, excerpt: &str, now: Duration) -> Result<()> {
        let since = *self.since.get_or_insert(now);
        let elapsed = now.saturating_sub(since);
        if elapsed >= self.limit {
            return Err(ErrorKind::StuckOnScreen {
                pane: self.pane.clone(),
                elapsed,
                excerpt: excerpt.to_owned(),
            }
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Ported from brgr's `screens.rs` `ScreenWatch` tests.

    use super::*;
    use crate::domain::key::Key;

    fn pane() -> PaneId {
        PaneId::new("w1:p2")
    }

    fn unknown(text: &str) -> Classification {
        Classification::Unknown {
            excerpt: text.to_owned(),
        }
    }

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn a_screen_that_a_key_does_not_clear_fails_after_the_budget() {
        let mut watch = ScreenWatch::new(pane());
        for _ in 0..PRESS_BUDGET {
            watch.pressed("continue-notice").unwrap();
        }
        let error = watch.pressed("continue-notice").unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::PromptNotCleared {
                rule: "continue-notice",
                presses: PRESS_BUDGET,
                ..
            }
        ));
        assert!(error.to_string().contains("did not clear"), "{error}");
        // A clear screen is progress: the count starts over.
        watch.observe(&Classification::Clear, MINUTE).unwrap();
        watch.pressed("continue-notice").unwrap();
    }

    #[test]
    fn each_rule_has_its_own_budget() {
        let mut watch = ScreenWatch::new(pane());
        for _ in 0..PRESS_BUDGET {
            watch.pressed("continue-notice").unwrap();
        }
        watch.pressed("workspace-trust").unwrap();
    }

    #[test]
    fn an_unknown_screen_fails_with_its_text_after_the_deadline() {
        let mut watch = ScreenWatch::with_limit(pane(), Duration::ZERO);
        let error = watch
            .observe(&unknown("Choose a theme"), Duration::ZERO)
            .unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::StuckOnScreen { .. }));
        let message = error.to_string();
        assert!(
            message.contains("w1:p2") && message.contains("last screen: Choose a theme"),
            "{message}"
        );
    }

    #[test]
    fn the_deadline_counts_from_the_first_sight_even_when_the_screen_changes() {
        let mut watch = ScreenWatch::new(pane());
        watch.observe(&unknown("a"), Duration::ZERO).unwrap();
        watch.observe(&unknown("b"), 2 * MINUTE).unwrap();
        assert!(watch.observe(&unknown("c"), 3 * MINUTE).is_err());
    }

    #[test]
    fn a_prompt_the_caller_did_not_enable_runs_the_same_clock() {
        let mut watch = ScreenWatch::with_limit(pane(), MINUTE);
        let prompt = Classification::Prompt {
            rule: "workspace-trust",
            excerpt: "trust?".to_owned(),
        };
        watch.observe(&prompt, Duration::ZERO).unwrap();
        assert!(watch.observe(&prompt, MINUTE).is_err());
    }

    #[test]
    fn a_clear_screen_resets_the_deadline() {
        let mut watch = ScreenWatch::new(pane());
        watch.observe(&unknown("x"), Duration::ZERO).unwrap();
        watch.observe(&Classification::Clear, MINUTE).unwrap();
        assert!(watch.since.is_none());
    }

    #[test]
    fn an_answered_screen_resets_the_deadline_but_keeps_the_count() {
        let mut watch = ScreenWatch::new(pane());
        watch.observe(&unknown("x"), Duration::ZERO).unwrap();
        watch.pressed("continue-notice").unwrap();
        watch
            .observe(
                &Classification::Resolve {
                    rule: "continue-notice",
                    keys: vec![Key::Enter],
                },
                MINUTE,
            )
            .unwrap();
        assert!(watch.since.is_none());
        assert_eq!(watch.presses, [("continue-notice", 1)]);
    }

    #[test]
    fn a_blocked_screen_no_rule_answers_runs_the_clock() {
        let mut watch = ScreenWatch::with_limit(pane(), MINUTE);
        watch
            .observe_blocked("Pick a region", Duration::ZERO)
            .unwrap();
        let error = watch.observe_blocked("Pick a region", MINUTE).unwrap_err();
        assert!(error.to_string().contains("Pick a region"));
    }

    #[test]
    fn restart_clock_keeps_counts_and_reset_clears_both() {
        let mut watch = ScreenWatch::new(pane());
        watch.observe(&unknown("x"), Duration::ZERO).unwrap();
        watch.pressed("continue-notice").unwrap();
        watch.restart_clock();
        assert!(watch.since.is_none());
        assert_eq!(watch.presses.len(), 1);
        watch.reset();
        assert!(watch.presses.is_empty());
    }

    #[test]
    fn the_unknown_screen_deadline_is_three_minutes() {
        assert_eq!(SCREEN_DEADLINE, Duration::from_secs(180));
        assert_eq!(PRESS_BUDGET, 5);
    }
}
