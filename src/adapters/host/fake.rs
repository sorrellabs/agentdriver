//! In-memory host for tests here and in consumers (`--features fake`).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::domain::error::{ErrorKind, Result};
use crate::domain::id::{HostKind, PaneId, TerminalId};
use crate::domain::key::Key;
use crate::domain::screen::Screen;
use crate::ports::host::{
    AgentState, Capabilities, Host, PaneInfo, PaneRef, ReadOptions, SpawnSpec, TextMode,
};

pub const KIND: HostKind = HostKind::from_static("fake");

#[derive(Debug, Default)]
struct Pane {
    terminal: u32,
    screen: String,
    state: Option<AgentState>,
    text: Vec<String>,
    keys: Vec<Key>,
}

#[derive(Debug, Default)]
struct Inner {
    next: u32,
    panes: HashMap<PaneId, Pane>,
}

#[derive(Debug, Default)]
pub struct FakeHost(Mutex<Inner>);

impl FakeHost {
    fn with<T>(&self, action: impl FnOnce(&mut Inner) -> T) -> T {
        action(&mut self.0.lock().unwrap_or_else(|poison| poison.into_inner()))
    }

    fn with_pane<T>(&self, pane: &PaneId, action: impl FnOnce(&mut Pane) -> T) -> Result<T> {
        self.with(|inner| {
            inner
                .panes
                .get_mut(pane)
                .map(action)
                .ok_or_else(|| ErrorKind::PaneNotFound(pane.clone()).into())
        })
    }

    pub fn set_screen(&self, pane: &PaneId, text: &str) {
        let _ = self.with_pane(pane, |state| state.screen = text.to_owned());
    }

    pub fn set_state(&self, pane: &PaneId, agent: AgentState) {
        let _ = self.with_pane(pane, |state| state.state = Some(agent));
    }

    /// Gives the pane id a new terminal, as a host does when it reuses an id.
    pub fn replace_terminal(&self, pane: &PaneId) {
        let _ = self.with_pane(pane, |state| state.terminal += 1000);
    }

    pub fn sent_text(&self, pane: &PaneId) -> Vec<String> {
        self.with_pane(pane, |state| state.text.clone())
            .unwrap_or_default()
    }

    pub fn sent_keys(&self, pane: &PaneId) -> Vec<Key> {
        self.with_pane(pane, |state| state.keys.clone())
            .unwrap_or_default()
    }

    pub fn exists(&self, pane: &PaneId) -> bool {
        self.with(|inner| inner.panes.contains_key(pane))
    }
}

fn terminal_id(terminal: u32) -> TerminalId {
    TerminalId::new(format!("t{terminal}"))
}

impl Host for FakeHost {
    fn kind(&self) -> HostKind {
        KIND
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            agent_state: true,
            ..Capabilities::default()
        }
    }

    fn spawn(&self, _spec: &SpawnSpec) -> Result<PaneRef> {
        Ok(self.with(|inner| {
            inner.next += 1;
            let pane = PaneId::new(format!("p{}", inner.next));
            let terminal = inner.next;
            inner.panes.insert(
                pane.clone(),
                Pane {
                    terminal,
                    ..Pane::default()
                },
            );
            PaneRef {
                pane,
                terminal: Some(terminal_id(terminal)),
            }
        }))
    }

    fn read(&self, pane: &PaneId, _options: ReadOptions) -> Result<Screen> {
        self.with_pane(pane, |state| Screen::new(state.screen.clone()))
    }

    fn send_text(&self, pane: &PaneId, text: &str, _mode: TextMode) -> Result<()> {
        self.with_pane(pane, |state| state.text.push(text.to_owned()))
    }

    fn send_keys(&self, pane: &PaneId, keys: &[Key]) -> Result<()> {
        self.with_pane(pane, |state| state.keys.extend_from_slice(keys))
    }

    fn close(&self, pane: &PaneId) -> Result<()> {
        self.with(|inner| inner.panes.remove(pane));
        Ok(())
    }

    fn pane_info(&self, pane: &PaneId) -> Result<Option<PaneInfo>> {
        Ok(self.with(|inner| {
            inner.panes.get(pane).map(|state| PaneInfo {
                pane: pane.clone(),
                terminal: Some(terminal_id(state.terminal)),
                pid: None,
            })
        }))
    }

    fn agent_state(&self, pane: &PaneId) -> Result<AgentState> {
        self.with_pane(pane, |state| state.state.unwrap_or(AgentState::Unknown))
    }
}
