//! Session behaviour through the public API, on fakes only.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use agentdriver::adapters::clock::FakeClock;
use agentdriver::adapters::harness::recipe;
use agentdriver::adapters::host::fake::FakeHost;
use agentdriver::adapters::store::MemoryStore;
use agentdriver::core::rules::{Classification, RulePolicy};
use agentdriver::core::wait::IdleSource;
use agentdriver::domain::rule::Rule;
use agentdriver::ports::harness::{LaunchPlan, LaunchRequest, Signal, SignalSetup};
use agentdriver::ports::host::{
    AgentState, Capabilities, PaneInfo, PaneRef, Placement, ReadOptions, SpawnSpec, TextMode,
    unsupported,
};
use agentdriver::{
    Driver, ErrorKind, Harness, HarnessKind, Host, HostKind, Key, PaneId, Result, Screen, Session,
    SessionSpec,
};

struct Fixture {
    host: Arc<FakeHost>,
    store: Arc<MemoryStore>,
    session: Session,
}

fn launch() -> LaunchRequest {
    LaunchRequest {
        executable: PathBuf::from("/bin/agent"),
        model: None,
        full_permission: false,
        extra_args: Vec::new(),
    }
}

fn spawn_with(harness: Arc<dyn Harness>, policy: RulePolicy) -> Fixture {
    let host = Arc::new(FakeHost::default());
    let store = Arc::new(MemoryStore::default());
    let driver = Driver::new(
        host.clone(),
        store.clone(),
        Arc::new(FakeClock::default()),
        policy,
    );
    let session = driver
        .spawn(SessionSpec {
            harness,
            launch: launch(),
            placement: Placement::Tab { label: None },
            cwd: PathBuf::from("/repo/task"),
            env: Vec::new(),
        })
        .unwrap();
    Fixture {
        host,
        store,
        session,
    }
}

fn fixture(policy: RulePolicy) -> Fixture {
    spawn_with(Arc::new(recipe("codex").unwrap()), policy)
}

#[test]
fn a_prompt_is_pasted_then_submitted() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    session.send_prompt("hello").unwrap();
    assert_eq!(host.sent_text(session.pane()), ["hello"]);
    assert_eq!(host.sent_keys(session.pane()), [Key::Enter]);
}

#[test]
fn input_is_refused_when_the_pane_id_was_reused() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    host.replace_terminal(session.pane());
    let error = session.press(&[Key::Enter]).unwrap_err();
    assert!(matches!(error.kind(), ErrorKind::PaneReplaced(_)));
    assert!(host.sent_keys(session.pane()).is_empty());
}

#[test]
fn close_leaves_a_reused_pane_open() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    let pane = session.pane().clone();
    host.replace_terminal(&pane);
    assert!(session.close().is_err());
    assert!(host.exists(&pane));
}

#[test]
fn close_treats_a_missing_pane_as_done() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    let pane = session.pane().clone();
    host.close(&pane).unwrap();
    session.close().unwrap();
    assert!(!host.exists(&pane));
}

#[test]
fn a_three_minute_wait_times_out_with_the_last_screen_and_no_real_delay() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    host.set_screen(session.pane(), "still thinking");
    let error = session
        .wait()
        .at_most(Duration::from_secs(180))
        .for_text("done")
        .unwrap_err();
    assert!(error.is_timeout());
    assert!(error.to_string().contains("still thinking"));
}

#[test]
fn idle_comes_from_the_host_state_when_it_has_one() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    host.set_state(session.pane(), AgentState::Idle);
    assert_eq!(session.wait().for_idle().unwrap().source, IdleSource::Host);
}

#[test]
fn nothing_is_pressed_while_the_agent_works() {
    let Fixture { host, session, .. } = fixture(RulePolicy::answering(["continue-notice"]));
    host.set_state(session.pane(), AgentState::Working);
    host.set_screen(session.pane(), "Press Enter to continue");
    assert_eq!(session.resolve_prompt().unwrap(), Classification::Clear);
    assert!(host.sent_keys(session.pane()).is_empty());
}

#[test]
fn a_prompt_with_no_enabled_rule_is_reported_not_pressed() {
    let Fixture { host, session, .. } = fixture(RulePolicy::default());
    host.set_state(session.pane(), AgentState::Blocked);
    host.set_screen(session.pane(), "Press Enter to continue");
    assert!(matches!(
        session.resolve_prompt().unwrap(),
        Classification::Prompt { .. }
    ));
    assert!(host.sent_keys(session.pane()).is_empty());
}

#[test]
fn an_answered_prompt_is_pressed_and_logged() {
    let Fixture {
        host,
        store,
        session,
    } = fixture(RulePolicy::answering(["continue-notice"]));
    host.set_state(session.pane(), AgentState::Blocked);
    host.set_screen(session.pane(), "Press Enter to continue");
    assert!(matches!(
        session.resolve_prompt().unwrap(),
        Classification::Resolve { .. }
    ));
    assert_eq!(host.sent_keys(session.pane()), [Key::Enter]);
    assert!(store.rule_log(session.id())[0].contains("continue-notice"));
}

// A harness and a host defined outside the crate, using only the public API.

#[derive(Debug)]
struct OutsideHarness;

const OUTSIDE_RULE: Rule = Rule {
    name: "outside-prompt",
    answer: |screen, _| screen.contains("Outside?").then(|| vec![Key::Char('y')]),
};

impl Harness for OutsideHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::new("outside")
    }

    fn launch(&self, _request: &LaunchRequest) -> Result<LaunchPlan> {
        Ok(LaunchPlan::default())
    }

    fn signal(&self, _state_dir: &Path) -> SignalSetup {
        SignalSetup {
            files: vec![(PathBuf::from(".outside/hook"), "hook".to_owned())],
            ..SignalSetup::default()
        }
    }

    fn parse_signal(&self, line: &str) -> Option<Signal> {
        (line == "finished").then_some(Signal::TurnFinished {
            answer: Some("answer".to_owned()),
        })
    }

    fn rules(&self) -> &[Rule] {
        &[OUTSIDE_RULE]
    }
}

#[test]
fn a_harness_outside_the_crate_brings_its_own_rules() {
    let policy = RulePolicy::answering(["outside-prompt"]);
    let Fixture { host, session, .. } = spawn_with(Arc::new(OutsideHarness), policy);
    host.set_screen(session.pane(), "Outside?");
    session.resolve_prompt().unwrap();
    assert_eq!(host.sent_keys(session.pane()), [Key::Char('y')]);
}

#[test]
fn a_signal_written_before_the_wait_does_not_finish_it() {
    let Fixture {
        host,
        store,
        session,
    } = spawn_with(Arc::new(OutsideHarness), RulePolicy::default());
    host.set_state(session.pane(), AgentState::Working);
    store.push_signal(session.id(), "finished");

    let wait = session.wait().at_most(Duration::from_secs(1));
    assert!(wait.for_idle().unwrap_err().is_timeout());
}

#[test]
fn a_harness_signal_finishes_the_wait_with_its_answer() {
    let Fixture {
        host,
        store,
        session,
    } = spawn_with(Arc::new(OutsideHarness), RulePolicy::default());
    host.set_state(session.pane(), AgentState::Working);

    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (store, id, stop) = (store.clone(), session.id().clone(), stop.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                store.push_signal(&id, "finished");
                thread::yield_now();
            }
        })
    };
    let idle = session.wait().forever().for_idle().unwrap();
    stop.store(true, Ordering::SeqCst);
    writer.join().unwrap();

    assert_eq!(idle.source, IdleSource::Harness);
    assert_eq!(idle.answer.as_deref(), Some("answer"));
}

#[test]
fn signal_files_are_installed_and_removed_with_the_session() {
    let Fixture { store, session, .. } =
        spawn_with(Arc::new(OutsideHarness), RulePolicy::default());
    let hook = Path::new("/repo/task/.outside/hook");
    assert!(store.has_file(hook));
    session.close().unwrap();
    assert!(!store.has_file(hook));
}

#[derive(Debug)]
struct OutsideHost;

impl Host for OutsideHost {
    fn kind(&self) -> HostKind {
        HostKind::new("outside")
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
        Ok(None)
    }
}

#[test]
fn a_host_outside_the_crate_plugs_into_the_driver() {
    let driver = Driver::new(
        Arc::new(OutsideHost),
        Arc::new(MemoryStore::default()),
        Arc::new(FakeClock::default()),
        RulePolicy::default(),
    );
    let error = driver
        .spawn(SessionSpec {
            harness: Arc::new(OutsideHarness),
            launch: launch(),
            placement: Placement::Tab { label: None },
            cwd: PathBuf::from("/repo/task"),
            env: Vec::new(),
        })
        .unwrap_err();
    assert!(error.is_unsupported());
    assert!(error.to_string().starts_with("outside "));
}
