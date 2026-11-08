//! Where sessions are kept, so a consumer can choose the location or medium.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::error::Result;
use crate::domain::id::{HarnessKind, HostKind, SessionId};
use crate::ports::host::PaneRef;

pub const SESSION_SCHEMA: &str = "agentdriver.session/1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub schema: String,
    pub id: SessionId,
    pub host: HostKind,
    pub pane: PaneRef,
    pub harness: HarnessKind,
    pub cwd: PathBuf,
    pub created_at: u64,
}

pub trait Store: Send + Sync + fmt::Debug {
    /// A new unique session and the directory its harness signals write into.
    fn allocate(&self) -> Result<(SessionId, PathBuf)>;
    fn save_record(&self, record: &SessionRecord) -> Result<()>;
    fn load_record(&self, id: &SessionId) -> Result<SessionRecord>;

    /// Where the signal stream ends now.
    fn signal_cursor(&self, id: &SessionId) -> Result<u64>;
    /// Complete signal lines after `cursor`, and the cursor to resume from.
    fn signals_since(&self, id: &SessionId, cursor: u64) -> Result<(Vec<String>, u64)>;
    fn append_rule_log(&self, id: &SessionId, line: &str) -> Result<()>;

    /// Installs a harness signal file into the workspace.
    fn write_file(&self, path: &Path, contents: &str) -> Result<()>;
    /// Best effort: a file already gone is not a failure.
    fn remove_file(&self, path: &Path);
}
