//! Session state on disk, so any process can pick a session up by id.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ErrorKind, Result};
use crate::harness::HarnessKind;
use crate::host::{HostKind, PaneRef};
use crate::id::SessionId;

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

/// `$AGENTDRIVER_STATE`, else the platform's per-user data directory.
pub fn default_root() -> PathBuf {
    if let Some(root) = std::env::var_os("AGENTDRIVER_STATE") {
        return PathBuf::from(root);
    }
    let home = std::env::var_os("HOME").map_or_else(std::env::temp_dir, PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.join("Library/Application Support/agentdriver");
    }
    std::env::var_os("XDG_STATE_HOME")
        .map_or_else(|| home.join(".local/state"), PathBuf::from)
        .join("agentdriver")
}

#[derive(Debug, Clone)]
pub struct SessionDir {
    path: PathBuf,
}

impl SessionDir {
    pub fn create(root: &Path, id: &SessionId) -> Result<Self> {
        let path = root.join(id.as_str());
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append-only; harness signals write one line per event.
    pub fn signals(&self) -> PathBuf {
        self.path.join("signals")
    }

    /// One line per answered prompt: `{unix_secs} {pane} {rule} {keys}`.
    pub fn rules_log(&self) -> PathBuf {
        self.path.join("rules.log")
    }

    pub fn write_record(&self, record: &SessionRecord) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(record).map_err(|error| ErrorKind::Parse {
            what: "session record",
            detail: error.to_string(),
        })?;
        let temporary = self.path.join("session.json.tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, self.path.join("session.json"))?;
        Ok(())
    }

    pub fn read_record(&self) -> Result<SessionRecord> {
        let bytes = fs::read(self.path.join("session.json"))?;
        serde_json::from_slice(&bytes).map_err(|error| {
            ErrorKind::Parse {
                what: "session record",
                detail: error.to_string(),
            }
            .into()
        })
    }

    pub fn signal_offset(&self) -> u64 {
        fs::metadata(self.signals()).map_or(0, |metadata| metadata.len())
    }

    /// Complete lines written since `offset`, and the offset to resume from.
    pub fn signals_since(&self, offset: u64) -> Result<(Vec<String>, u64)> {
        let mut file = match File::open(self.signals()) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Vec::new(), offset));
            }
            Err(error) => return Err(error.into()),
        };
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |at| at + 1);
        let lines = String::from_utf8_lossy(&bytes[..complete])
            .lines()
            .map(str::to_owned)
            .collect();
        Ok((lines, offset + complete as u64))
    }

    pub fn append_rule_log(&self, line: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.rules_log())?;
        writeln!(file, "{line}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_signal_line_waits_for_its_newline() {
        let root = tempfile::tempdir().unwrap();
        let dir = SessionDir::create(root.path(), &SessionId::new("s1")).unwrap();
        fs::write(dir.signals(), "one\ntw").unwrap();

        let (lines, offset) = dir.signals_since(0).unwrap();
        assert_eq!(lines, ["one"]);

        fs::write(dir.signals(), "one\ntwo\n").unwrap();
        let (lines, _) = dir.signals_since(offset).unwrap();
        assert_eq!(lines, ["two"]);
    }
}
