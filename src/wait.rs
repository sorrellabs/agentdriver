//! Bounded waiting on a session. Every wait has a limit unless the caller
//! explicitly asks for [`Wait::forever`].

use std::thread;
use std::time::{Duration, Instant};

use crate::error::{ErrorKind, Result};
use crate::harness::Signal;
use crate::host::{AgentState, ReadOptions};
use crate::screen::Screen;
use crate::session::Session;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_PERIOD: Duration = Duration::from_millis(250);

/// Where an idle verdict came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleSource {
    Harness,
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idle {
    pub source: IdleSource,
    pub answer: Option<String>,
}

#[must_use = "a Wait does nothing until a for_* or until method runs it"]
#[derive(Debug)]
pub struct Wait<'session> {
    session: &'session Session,
    timeout: Option<Duration>,
    period: Duration,
}

impl<'session> Wait<'session> {
    pub(crate) fn new(session: &'session Session) -> Self {
        Self {
            session,
            timeout: Some(DEFAULT_TIMEOUT),
            period: DEFAULT_PERIOD,
        }
    }

    pub fn at_most(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn forever(mut self) -> Self {
        self.timeout = None;
        self
    }

    pub fn every(mut self, period: Duration) -> Self {
        self.period = period;
        self
    }

    /// Polls `check`: `Ok(None)` retries, `Err` aborts, `Ok(Some)` returns.
    pub fn until<T>(
        self,
        waited_for: &str,
        mut check: impl FnMut(&Screen) -> Result<Option<T>>,
    ) -> Result<T> {
        let started = Instant::now();
        loop {
            let screen = self
                .session
                .host()
                .read(self.session.pane(), ReadOptions::default())?;
            if let Some(value) = check(&screen)? {
                return Ok(value);
            }
            if self.timeout.is_some_and(|limit| started.elapsed() >= limit) {
                return Err(ErrorKind::Timeout {
                    waited_for: waited_for.to_owned(),
                    elapsed: started.elapsed(),
                    excerpt: screen.excerpt(),
                }
                .into());
            }
            thread::sleep(self.period);
        }
    }

    pub fn for_text(self, needle: &str) -> Result<Screen> {
        let waited_for = format!("text {needle:?}");
        self.until(&waited_for, |screen| {
            Ok(screen.contains(needle).then(|| screen.clone()))
        })
    }

    /// The screen stays unchanged for `quiet`. Not the same as idle.
    pub fn for_stable(self, quiet: Duration) -> Result<Screen> {
        let mut last: Option<(Screen, Instant)> = None;
        self.until("a stable screen", |screen| {
            match &last {
                Some((previous, since)) if previous == screen => {
                    if since.elapsed() >= quiet {
                        return Ok(Some(screen.clone()));
                    }
                }
                _ => last = Some((screen.clone(), Instant::now())),
            }
            Ok(None)
        })
    }

    /// A finished turn, from the most reliable source available: a harness
    /// signal written after this call, then the host's agent state.
    pub fn for_idle(self) -> Result<Idle> {
        let session = self.session;
        let mut offset = session.dir().signal_offset();
        let host_state = session.host().capabilities().agent_state;
        self.until("the agent to finish its turn", |_| {
            let (lines, next) = session.dir().signals_since(offset)?;
            offset = next;
            let finished = lines
                .iter()
                .filter_map(|line| session.harness().parse_signal(line))
                .next();
            if let Some(Signal::TurnFinished { answer }) = finished {
                return Ok(Some(Idle {
                    source: IdleSource::Harness,
                    answer,
                }));
            }
            if host_state
                && matches!(
                    session.host().agent_state(session.pane())?,
                    AgentState::Idle | AgentState::Done
                )
            {
                return Ok(Some(Idle {
                    source: IdleSource::Host,
                    answer: None,
                }));
            }
            Ok(None)
        })
    }
}
