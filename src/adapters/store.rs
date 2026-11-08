//! Session stores: on disk, and in memory for tests.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::error::{ErrorKind, Result};
use crate::domain::id::SessionId;
use crate::ports::store::{SessionRecord, Store};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Each session is a directory under `root`, readable by any process:
/// `session.json`, an append-only `signals`, and `rules.log`.
#[derive(Debug, Clone)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$AGENTDRIVER_STATE`, else the platform's per-user state directory.
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

    fn session(&self, id: &SessionId) -> PathBuf {
        self.root.join(id.as_str())
    }

    fn signals(&self, id: &SessionId) -> PathBuf {
        self.session(id).join("signals")
    }
}

fn parse_error(error: &serde_json::Error) -> ErrorKind {
    ErrorKind::Parse {
        what: "session record",
        detail: error.to_string(),
    }
}

impl Store for FsStore {
    fn allocate(&self) -> Result<(SessionId, PathBuf)> {
        fs::create_dir_all(&self.root)?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let id = SessionId::new(format!("{}-{nanos}-{number}", process::id()));
        let path = self.session(&id);
        fs::create_dir(&path)?;
        Ok((id, path))
    }

    fn save_record(&self, record: &SessionRecord) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(record).map_err(|error| parse_error(&error))?;
        let directory = self.session(&record.id);
        let temporary = directory.join("session.json.tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(temporary, directory.join("session.json"))?;
        Ok(())
    }

    fn load_record(&self, id: &SessionId) -> Result<SessionRecord> {
        let bytes = fs::read(self.session(id).join("session.json"))?;
        serde_json::from_slice(&bytes).map_err(|error| parse_error(&error).into())
    }

    fn signal_cursor(&self, id: &SessionId) -> Result<u64> {
        Ok(fs::metadata(self.signals(id)).map_or(0, |metadata| metadata.len()))
    }

    fn signals_since(&self, id: &SessionId, cursor: u64) -> Result<(Vec<String>, u64)> {
        let mut file = match File::open(self.signals(id)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Vec::new(), cursor));
            }
            Err(error) => return Err(error.into()),
        };
        file.seek(SeekFrom::Start(cursor))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        // A partial line stays until its newline arrives.
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |at| at + 1);
        let lines = String::from_utf8_lossy(&bytes[..complete])
            .lines()
            .map(str::to_owned)
            .collect();
        Ok((lines, cursor + complete as u64))
    }

    fn append_rule_log(&self, id: &SessionId, line: &str) -> Result<()> {
        let path = self.session(id).join("rules.log");
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    fn write_file(&self, path: &Path, contents: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
        Ok(())
    }

    fn remove_file(&self, path: &Path) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(feature = "fake")]
pub use memory::MemoryStore;

#[cfg(feature = "fake")]
mod memory {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use crate::domain::error::{ErrorKind, Result};
    use crate::domain::id::SessionId;
    use crate::ports::store::{SessionRecord, Store};

    #[derive(Debug, Default)]
    struct Inner {
        next: u32,
        records: HashMap<SessionId, SessionRecord>,
        signals: HashMap<SessionId, Vec<String>>,
        rule_log: HashMap<SessionId, Vec<String>>,
        files: HashMap<PathBuf, String>,
    }

    #[derive(Debug, Default)]
    pub struct MemoryStore(Mutex<Inner>);

    impl MemoryStore {
        fn with<T>(&self, action: impl FnOnce(&mut Inner) -> T) -> T {
            action(&mut self.0.lock().unwrap_or_else(|poison| poison.into_inner()))
        }

        pub fn push_signal(&self, id: &SessionId, line: &str) {
            self.with(|inner| {
                inner
                    .signals
                    .entry(id.clone())
                    .or_default()
                    .push(line.to_owned())
            });
        }

        pub fn rule_log(&self, id: &SessionId) -> Vec<String> {
            self.with(|inner| inner.rule_log.get(id).cloned().unwrap_or_default())
        }

        pub fn has_file(&self, path: &Path) -> bool {
            self.with(|inner| inner.files.contains_key(path))
        }
    }

    impl Store for MemoryStore {
        fn allocate(&self) -> Result<(SessionId, PathBuf)> {
            Ok(self.with(|inner| {
                inner.next += 1;
                let id = SessionId::new(format!("s{}", inner.next));
                let path = PathBuf::from("/memory").join(id.as_str());
                (id, path)
            }))
        }

        fn save_record(&self, record: &SessionRecord) -> Result<()> {
            self.with(|inner| inner.records.insert(record.id.clone(), record.clone()));
            Ok(())
        }

        fn load_record(&self, id: &SessionId) -> Result<SessionRecord> {
            self.with(|inner| inner.records.get(id).cloned())
                .ok_or_else(|| {
                    ErrorKind::Parse {
                        what: "session record",
                        detail: format!("no session {id}"),
                    }
                    .into()
                })
        }

        fn signal_cursor(&self, id: &SessionId) -> Result<u64> {
            Ok(self.with(|inner| inner.signals.get(id).map_or(0, |lines| lines.len() as u64)))
        }

        fn signals_since(&self, id: &SessionId, cursor: u64) -> Result<(Vec<String>, u64)> {
            Ok(self.with(|inner| {
                let lines = inner.signals.get(id).map_or(&[][..], Vec::as_slice);
                let from = usize::try_from(cursor)
                    .unwrap_or(usize::MAX)
                    .min(lines.len());
                (lines[from..].to_vec(), lines.len() as u64)
            }))
        }

        fn append_rule_log(&self, id: &SessionId, line: &str) -> Result<()> {
            self.with(|inner| {
                inner
                    .rule_log
                    .entry(id.clone())
                    .or_default()
                    .push(line.to_owned())
            });
            Ok(())
        }

        fn write_file(&self, path: &Path, contents: &str) -> Result<()> {
            self.with(|inner| inner.files.insert(path.to_path_buf(), contents.to_owned()));
            Ok(())
        }

        fn remove_file(&self, path: &Path) {
            self.with(|inner| inner.files.remove(path));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_signal_line_waits_for_its_newline() {
        let root = tempfile::tempdir().unwrap();
        let store = FsStore::new(root.path());
        let (id, directory) = store.allocate().unwrap();
        fs::write(directory.join("signals"), "one\ntw").unwrap();

        let (lines, cursor) = store.signals_since(&id, 0).unwrap();
        assert_eq!(lines, ["one"]);

        fs::write(directory.join("signals"), "one\ntwo\n").unwrap();
        let (lines, _) = store.signals_since(&id, cursor).unwrap();
        assert_eq!(lines, ["two"]);
    }

    #[test]
    fn allocated_sessions_are_unique() {
        let root = tempfile::tempdir().unwrap();
        let store = FsStore::new(root.path());
        assert_ne!(store.allocate().unwrap().0, store.allocate().unwrap().0);
    }
}
