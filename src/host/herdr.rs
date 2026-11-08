//! Herdr adapter. Shells out to the `herdr` CLI through [`crate::exec`]; the verbs
//! are filled in by the roadmap milestone that ships this host.

use crate::error::Result;
use crate::host::{
    Capabilities, Host, HostKind, PaneInfo, PaneRef, ReadOptions, SpawnSpec, TextMode, unsupported,
};
use crate::id::PaneId;
use crate::key::Key;
use crate::screen::Screen;

#[derive(Debug, Default)]
pub struct Herdr;

impl Host for Herdr {
    fn kind(&self) -> HostKind {
        HostKind::Herdr
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn spawn(&self, _spec: &SpawnSpec) -> Result<PaneRef> {
        Err(unsupported(self.kind(), "spawn"))
    }

    fn read(&self, _pane: &PaneId, _options: ReadOptions) -> Result<Screen> {
        Err(unsupported(self.kind(), "read"))
    }

    fn send_text(&self, _pane: &PaneId, _text: &str, _mode: TextMode) -> Result<()> {
        Err(unsupported(self.kind(), "send_text"))
    }

    fn send_keys(&self, _pane: &PaneId, _keys: &[Key]) -> Result<()> {
        Err(unsupported(self.kind(), "send_keys"))
    }

    fn close(&self, _pane: &PaneId) -> Result<()> {
        Err(unsupported(self.kind(), "close"))
    }

    fn pane_info(&self, _pane: &PaneId) -> Result<Option<PaneInfo>> {
        Err(unsupported(self.kind(), "pane_info"))
    }
}
