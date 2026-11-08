//! Entry point: starts harnesses in a host and hands back sessions.

use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::harness::{Harness, LaunchRequest};
use crate::host::{Host, Placement, SpawnSpec, StartMode};
use crate::id::SessionId;
use crate::rules::RulePolicy;
use crate::session::Session;
use crate::state::{SESSION_SCHEMA, SessionDir, SessionRecord, default_root};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct Config {
    pub state_root: PathBuf,
    /// Prompts to answer. Empty by default, so every prompt is reported.
    pub policy: RulePolicy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            state_root: default_root(),
            policy: RulePolicy::default(),
        }
    }
}

#[derive(Debug)]
pub struct SessionSpec {
    pub harness: Arc<dyn Harness>,
    pub launch: LaunchRequest,
    pub placement: Placement,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct Driver {
    host: Arc<dyn Host>,
    config: Config,
}

impl Driver {
    pub fn new(host: Arc<dyn Host>, config: Config) -> Self {
        Self { host, config }
    }

    pub fn spawn(&self, spec: SessionSpec) -> Result<Session> {
        let id = new_session_id();
        let dir = SessionDir::create(&self.config.state_root, &id)?;

        let mut plan = spec.harness.launch(&spec.launch)?;
        let signal = spec.harness.signal(dir.path());
        plan.argv.extend(signal.args);
        let mut env = spec.env;
        env.extend(plan.env);
        env.extend(signal.env);

        let mut installed = Vec::new();
        for (relative, contents) in signal.files {
            let path = spec.cwd.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, contents)?;
            installed.push(path);
        }

        let pane = self.host.spawn(&SpawnSpec {
            placement: spec.placement,
            cwd: spec.cwd.clone(),
            env,
            start: StartMode::Run(plan.argv),
        })?;

        dir.write_record(&SessionRecord {
            schema: SESSION_SCHEMA.to_owned(),
            id: id.clone(),
            host: self.host.kind(),
            pane: pane.clone(),
            harness: spec.harness.kind(),
            cwd: spec.cwd.clone(),
            created_at: unix_seconds(),
        })?;

        Ok(Session {
            id,
            host: self.host.clone(),
            harness: spec.harness,
            pane,
            dir,
            workspace: spec.cwd,
            policy: self.config.policy.clone(),
            installed,
        })
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn new_session_id() -> SessionId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    SessionId::new(format!(
        "{}-{nanos}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
