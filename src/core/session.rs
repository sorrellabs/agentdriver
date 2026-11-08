//! A handle to one harness running in one host pane.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::core::rules::{Classification, RulePolicy, classify};
use crate::core::wait::Wait;
use crate::domain::error::{ErrorKind, Result};
use crate::domain::id::{PaneId, SessionId};
use crate::domain::key::Key;
use crate::domain::screen::Screen;
use crate::ports::clock::Clock;
use crate::ports::harness::Harness;
use crate::ports::host::{AgentState, Host, PaneRef, ReadOptions, TextMode};
use crate::ports::store::Store;

/// Time for a TUI to take in a paste before Enter submits it.
const PASTE_SETTLE: Duration = Duration::from_millis(150);
/// Time for the screen to redraw after a prompt is answered.
const REDRAW: Duration = Duration::from_millis(1500);

#[derive(Debug)]
pub struct Session {
    pub(crate) id: SessionId,
    pub(crate) host: Arc<dyn Host>,
    pub(crate) harness: Arc<dyn Harness>,
    pub(crate) store: Arc<dyn Store>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) pane: PaneRef,
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

    pub(crate) fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    pub(crate) fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
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
        self.clock.sleep(PASTE_SETTLE);
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
        let outcome = classify(
            &self.screen()?,
            &self.workspace,
            self.harness.rules(),
            &self.policy,
        );
        if let Classification::Resolve { rule, keys } = &outcome {
            self.press(keys)?;
            let line = format!(
                "{} {} {rule} {keys:?}",
                self.clock.unix_seconds(),
                self.pane()
            );
            self.store.append_rule_log(&self.id, &line)?;
            self.clock.sleep(REDRAW);
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
            self.store.remove_file(file);
        }
        Ok(())
    }
}
