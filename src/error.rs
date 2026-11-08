//! Typed errors. Boxed so `Result` stays small; `ErrorKind` is non-exhaustive
//! so new failure classes are not breaking changes.

use std::io;
use std::time::Duration;

use crate::host::HostKind;
use crate::id::PaneId;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(Box<ErrorKind>);

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.0
    }

    pub fn is_timeout(&self) -> bool {
        matches!(*self.0, ErrorKind::Timeout { .. })
    }

    pub fn is_pane_not_found(&self) -> bool {
        matches!(*self.0, ErrorKind::PaneNotFound(_))
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(*self.0, ErrorKind::Unsupported { .. })
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self(Box::new(kind))
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        ErrorKind::Io(error).into()
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ErrorKind {
    #[error("{host} is unavailable: {detail}")]
    HostUnavailable { host: HostKind, detail: String },
    #[error("pane {0} does not exist")]
    PaneNotFound(PaneId),
    /// The pane id is live but now belongs to another terminal.
    #[error("pane {0} was replaced by another terminal")]
    PaneReplaced(PaneId),
    #[error("pane {0} cannot accept input yet")]
    PaneBusy(PaneId),
    /// A prompt is on screen that no enabled rule answers.
    #[error("agent in pane {pane} is waiting for input: {excerpt}")]
    AwaitingInput { pane: PaneId, excerpt: String },
    #[error("timed out after {elapsed:?} waiting for {waited_for}; last screen: {excerpt}")]
    Timeout {
        waited_for: String,
        elapsed: Duration,
        excerpt: String,
    },
    #[error("{host} does not support {operation}")]
    Unsupported {
        host: HostKind,
        operation: &'static str,
    },
    /// A failure reported by the host, with its code when it has one.
    #[error("{host} failed: {message}")]
    Host {
        host: HostKind,
        code: Option<String>,
        message: String,
        exit: Option<i32>,
    },
    #[error("could not run {program}: {source}")]
    Spawn { program: String, source: io::Error },
    #[error("{program} did not finish within {after:?}")]
    CommandTimeout { program: String, after: Duration },
    #[error("{program} wrote more than {limit} bytes")]
    OutputTooLarge { program: String, limit: usize },
    #[error("could not parse {what}: {detail}")]
    Parse { what: &'static str, detail: String },
    #[error(transparent)]
    Io(io::Error),
}
