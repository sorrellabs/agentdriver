//! Entry point: starts harnesses in a host and hands back sessions.

use std::path::PathBuf;
use std::sync::Arc;

use crate::core::Session;
use crate::core::rules::RulePolicy;
use crate::domain::error::Result;
use crate::ports::clock::Clock;
use crate::ports::harness::{Harness, LaunchRequest};
use crate::ports::host::{Host, Placement, SpawnSpec, StartMode};
use crate::ports::store::{SESSION_SCHEMA, SessionRecord, Store};

#[derive(Debug)]
pub struct SessionSpec {
    pub harness: Arc<dyn Harness>,
    pub launch: LaunchRequest,
    pub placement: Placement,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// The primary checkout when `cwd` is a worktree of it. Worktrees belong to
    /// the caller, so the caller says; Codex's worktree trust prompt names it.
    pub primary_checkout: Option<PathBuf>,
}

/// Holds the ports. [`crate::adapters::default_driver`] wires the real ones.
#[derive(Debug)]
pub struct Driver {
    host: Arc<dyn Host>,
    store: Arc<dyn Store>,
    clock: Arc<dyn Clock>,
    /// Prompts to answer. Empty by default, so every prompt is reported.
    policy: RulePolicy,
}

impl Driver {
    pub fn new(
        host: Arc<dyn Host>,
        store: Arc<dyn Store>,
        clock: Arc<dyn Clock>,
        policy: RulePolicy,
    ) -> Self {
        Self {
            host,
            store,
            clock,
            policy,
        }
    }

    pub fn spawn(&self, spec: SessionSpec) -> Result<Session> {
        let (id, signal_dir) = self.store.allocate()?;

        let mut plan = spec.harness.launch(&spec.launch)?;
        let signal = spec.harness.signal(&signal_dir);
        plan.argv.extend(signal.args);
        let mut env = spec.env;
        env.extend(plan.env);
        env.extend(signal.env);

        let mut installed = Vec::new();
        for (relative, contents) in signal.files {
            let path = spec.cwd.join(relative);
            self.store.write_file(&path, &contents)?;
            installed.push(path);
        }

        let pane = self.host.spawn(&SpawnSpec {
            placement: spec.placement,
            cwd: spec.cwd.clone(),
            env,
            start: StartMode::Run(plan.argv),
        })?;

        self.store.save_record(&SessionRecord {
            schema: SESSION_SCHEMA.to_owned(),
            id: id.clone(),
            host: self.host.kind(),
            pane: pane.clone(),
            harness: spec.harness.kind(),
            cwd: spec.cwd.clone(),
            created_at: self.clock.unix_seconds(),
        })?;

        Ok(Session {
            id,
            host: self.host.clone(),
            harness: spec.harness,
            store: self.store.clone(),
            clock: self.clock.clone(),
            pane,
            workspace: spec.cwd,
            primary_checkout: spec.primary_checkout,
            policy: self.policy.clone(),
            installed,
        })
    }
}
