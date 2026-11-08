//! A handle to one harness running in one host pane.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{ErrorKind, Result};
use crate::harness::Harness;
use crate::host::{AgentState, Host, PaneRef, ReadOptions, TextMode};
use crate::id::{PaneId, SessionId};
use crate::key::Key;
use crate::rules::{Classification, RulePolicy, classify};
use crate::screen::Screen;
use crate::state::SessionDir;
use crate::wait::Wait;

/// Time for a TUI to take in a paste before Enter submits it.
const PASTE_SETTLE: Duration = Duration::from_millis(150);
/// Time for the screen to redraw after a prompt is answered.
const REDRAW: Duration = Duration::from_millis(1500);

#[derive(Debug)]
pub struct Session {
    pub(crate) id: SessionId,
    pub(crate) host: Arc<dyn Host>,
    pub(crate) harness: Arc<dyn Harness>,
    pub(crate) pane: PaneRef,
    pub(crate) dir: SessionDir,
    pub(crate) workspace: PathBuf,
    pub(crate) policy: RulePolicy,
    /// Workspace files the signal setup wrote; removed on close.
    pub(crate) installed: Vec<PathBuf>,
}

impl Session {
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    pub fn pane(&self) -> &PaneId {
        &self.pane.pane
    }

    pub(crate) fn host(&self) -> &dyn Host {
        self.host.as_ref()
    }

    pub(crate) fn harness(&self) -> &dyn Harness {
        self.harness.as_ref()
    }

    pub(crate) fn dir(&self) -> &SessionDir {
        &self.dir
    }

    /// Fails unless the pane is still the one this session started in.
    pub fn verify(&self) -> Result<()> {
        let pane = self.pane();
        let info = self
            .host
            .pane_info(pane)?
            .ok_or_else(|| ErrorKind::PaneNotFound(pane.clone()))?;
        if self.pane.terminal.is_some() && info.terminal != self.pane.terminal {
            return Err(ErrorKind::PaneReplaced(pane.clone()).into());
        }
        Ok(())
    }

    pub fn screen(&self) -> Result<Screen> {
        self.host.read(self.pane(), ReadOptions::default())
    }

    /// Pastes `text` into the agent's input box and submits it.
    pub fn send_prompt(&self, text: &str) -> Result<()> {
        self.verify()?;
        self.host.send_text(self.pane(), text, TextMode::Paste)?;
        thread::sleep(PASTE_SETTLE);
        self.host.send_keys(self.pane(), &[Key::Enter])
    }

    pub fn press(&self, keys: &[Key]) -> Result<()> {
        self.verify()?;
        self.host.send_keys(self.pane(), keys)
    }

    pub fn wait(&self) -> Wait<'_> {
        Wait::new(self)
    }

    pub fn agent_state(&self) -> Result<AgentState> {
        self.host.agent_state(self.pane())
    }

    /// Classifies the screen and, when an enabled rule matches, answers it.
    /// Nothing is read or pressed while the host reports the agent working.
    pub fn resolve_prompt(&self) -> Result<Classification> {
        if self.host.capabilities().agent_state && self.agent_state()? == AgentState::Working {
            return Ok(Classification::Clear);
        }
        let outcome = classify(&self.screen()?, &self.workspace, &self.policy);
        if let Classification::Resolve { rule, keys } = &outcome {
            self.press(keys)?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            self.dir
                .append_rule_log(&format!("{now} {} {rule} {keys:?}", self.pane()))?;
            thread::sleep(REDRAW);
        }
        Ok(outcome)
    }

    /// Closes the pane if it is provably still ours. A pane already gone is
    /// a success; a pane id now held by another terminal is left alone.
    pub fn close(self) -> Result<()> {
        match self.verify() {
            Ok(()) => self.host.close(self.pane())?,
            Err(error) if error.is_pane_not_found() => {}
            Err(error) => return Err(error),
        }
        for file in &self.installed {
            let _ = fs::remove_file(file);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;
    use crate::driver::{Config, Driver, SessionSpec};
    use crate::harness::{HarnessKind, LaunchRequest, recipes};
    use crate::host::Placement;
    use crate::host::fake::FakeHost;
    use crate::wait::IdleSource;

    struct Fixture {
        host: Arc<FakeHost>,
        session: Session,
        _state: TempDir,
    }

    fn fixture(policy: RulePolicy) -> Fixture {
        let state = tempfile::tempdir().unwrap();
        let host = Arc::new(FakeHost::default());
        let driver = Driver::new(
            host.clone(),
            Config {
                state_root: state.path().to_path_buf(),
                policy,
            },
        );
        let harness = *recipes()
            .iter()
            .find(|r| r.kind == HarnessKind::Codex)
            .unwrap();
        let session = driver
            .spawn(SessionSpec {
                harness: Arc::new(harness),
                launch: LaunchRequest {
                    executable: PathBuf::from("/bin/codex"),
                    model: None,
                    full_permission: false,
                    extra_args: Vec::new(),
                },
                placement: Placement::Tab { label: None },
                cwd: Path::new("/repo/task").to_path_buf(),
                env: Vec::new(),
            })
            .unwrap();
        Fixture {
            host,
            session,
            _state: state,
        }
    }

    fn quick(session: &Session) -> Wait<'_> {
        session
            .wait()
            .at_most(Duration::from_millis(100))
            .every(Duration::from_millis(5))
    }

    #[test]
    fn a_prompt_is_pasted_then_submitted() {
        let Fixture { host, session, .. } = &fixture(RulePolicy::default());
        session.send_prompt("hello").unwrap();
        assert_eq!(host.sent_text(session.pane()), ["hello"]);
        assert_eq!(host.sent_keys(session.pane()), [Key::Enter]);
    }

    #[test]
    fn input_is_refused_when_the_pane_id_was_reused() {
        let Fixture { host, session, .. } = &fixture(RulePolicy::default());
        host.replace_terminal(session.pane());
        let error = session.press(&[Key::Enter]).unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::PaneReplaced(_)));
        assert!(host.sent_keys(session.pane()).is_empty());
    }

    #[test]
    fn close_leaves_a_reused_pane_open() {
        let Fixture {
            host,
            session,
            _state,
        } = fixture(RulePolicy::default());
        let pane = session.pane().clone();
        host.replace_terminal(&pane);
        assert!(session.close().is_err());
        assert!(host.exists(&pane));
    }

    #[test]
    fn close_treats_a_missing_pane_as_done() {
        let Fixture {
            host,
            session,
            _state,
        } = fixture(RulePolicy::default());
        let pane = session.pane().clone();
        host.close(&pane).unwrap();
        session.close().unwrap();
        assert!(!host.exists(&pane));
    }

    #[test]
    fn a_wait_timeout_reports_the_last_screen() {
        let Fixture { host, session, .. } = &fixture(RulePolicy::default());
        host.set_screen(session.pane(), "still thinking");
        let error = quick(session).for_text("done").unwrap_err();
        assert!(error.is_timeout());
        assert!(error.to_string().contains("still thinking"));
    }

    #[test]
    fn idle_comes_from_the_host_state_when_it_has_one() {
        let Fixture { host, session, .. } = &fixture(RulePolicy::default());
        host.set_state(session.pane(), AgentState::Idle);
        assert_eq!(quick(session).for_idle().unwrap().source, IdleSource::Host);
    }

    #[test]
    fn nothing_is_pressed_while_the_agent_works() {
        let policy = RulePolicy::answering(["continue-notice"]);
        let Fixture { host, session, .. } = &fixture(policy);
        host.set_state(session.pane(), AgentState::Working);
        host.set_screen(session.pane(), "Press Enter to continue");
        assert_eq!(session.resolve_prompt().unwrap(), Classification::Clear);
        assert!(host.sent_keys(session.pane()).is_empty());
    }

    #[test]
    fn a_prompt_with_no_enabled_rule_is_reported_not_pressed() {
        let Fixture { host, session, .. } = &fixture(RulePolicy::default());
        host.set_state(session.pane(), AgentState::Blocked);
        host.set_screen(session.pane(), "Press Enter to continue");
        assert!(matches!(
            session.resolve_prompt().unwrap(),
            Classification::Prompt { .. }
        ));
        assert!(host.sent_keys(session.pane()).is_empty());
    }
}
