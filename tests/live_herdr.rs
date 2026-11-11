//! Live checks of the Herdr adapter against the Herdr this test runs in.
//!
//! Opt-in: `cargo test --features live,herdr --test live_herdr -- --ignored`
//! from a Herdr pane (`HERDR_ENV=1`). Each test opens panes below the calling
//! pane without taking focus and closes every pane it opened.
#![cfg(all(feature = "live", feature = "herdr"))]
#![allow(clippy::unwrap_used, clippy::print_stdout)]

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use agentdriver::adapters::host::herdr::Herdr;
use agentdriver::ports::host::{
    AgentState, Direction, Host, PaneRef, Placement, ReadOptions, SpawnSpec, StartMode, TextMode,
};
use agentdriver::{Key, PaneId};

fn caller() -> PaneId {
    assert_eq!(
        std::env::var("HERDR_ENV").as_deref(),
        Ok("1"),
        "run inside a Herdr pane"
    );
    PaneId::new(std::env::var("HERDR_PANE_ID").expect("HERDR_PANE_ID"))
}

/// Closes the pane when the test ends, whether it passed or not.
struct Owned<'a> {
    host: &'a Herdr,
    pane: PaneRef,
}

impl Drop for Owned<'_> {
    fn drop(&mut self) {
        let _ = self.host.close(&self.pane.pane);
    }
}

fn spawn<'a>(host: &'a Herdr, cwd: PathBuf, argv: &[&str]) -> Owned<'a> {
    let pane = host
        .spawn(&SpawnSpec {
            placement: Placement::Split {
                target: caller(),
                direction: Direction::Down,
            },
            cwd,
            env: vec![("AGENTDRIVER_LIVE".to_owned(), "yes".to_owned())],
            start: StartMode::Run(argv.iter().map(|word| (*word).to_owned()).collect()),
        })
        .unwrap();
    Owned { host, pane }
}

fn until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn screen(host: &Herdr, pane: &PaneId) -> String {
    host.read(pane, ReadOptions::default()).unwrap().text
}

#[test]
#[ignore = "live: needs Herdr"]
fn a_command_runs_in_a_new_pane_takes_input_and_its_pane_closes() {
    let host = Herdr::from_env();
    let dir = tempfile::tempdir().unwrap();
    let owned = spawn(
        &host,
        dir.path().to_owned(),
        &[
            "sh",
            "-c",
            "printf 'env=%s\\n' \"$AGENTDRIVER_LIVE\"; read line; printf 'got=%s\\n' \"$line\"; sleep 60",
        ],
    );
    let pane = owned.pane.pane.clone();

    let info = host.pane_info(&pane).unwrap().unwrap();
    assert_eq!(
        info.terminal, owned.pane.terminal,
        "same terminal as spawned"
    );
    assert!(until(Duration::from_secs(10), || screen(&host, &pane)
        .contains("env=yes")));

    host.send_text(&pane, "it's -here", TextMode::Literal)
        .unwrap();
    host.send_keys(&pane, &[Key::Enter]).unwrap();
    assert!(
        until(Duration::from_secs(10), || screen(&host, &pane)
            .contains("got=it's -here")),
        "screen: {}",
        screen(&host, &pane)
    );

    host.send_keys(&pane, &[Key::Ctrl('c')]).unwrap();
    drop(owned);
    assert_eq!(host.pane_info(&pane).unwrap(), None, "pane is gone");
    host.close(&pane).unwrap();
}

#[test]
#[ignore = "live: needs Herdr and a logged-in Claude Code"]
fn claude_code_in_a_herdr_pane_goes_from_idle_through_work_back_to_idle() {
    let host = Herdr::from_env();
    let dir = tempfile::tempdir().unwrap();
    let owned = spawn(
        &host,
        dir.path().to_owned(),
        &["claude", "--model", "haiku"],
    );
    let pane = owned.pane.pane.clone();

    // A fresh folder asks for trust first. This is the session's own spawn
    // directory, the one folder agentdriver may answer that for. The menu
    // starts on "No, exit", so Enter alone would quit. A menu that has just
    // been drawn can still drop keys, so press only on a screen that has
    // held still, one step at a time, and read the selection again after.
    let mut answered_trust = false;
    assert!(
        until(Duration::from_secs(60), || {
            if matches!(
                host.agent_state(&pane).unwrap(),
                AgentState::Idle | AgentState::Done
            ) {
                return true;
            }
            let before = screen(&host, &pane);
            if !before.contains("Yes, I trust this folder") {
                return false;
            }
            thread::sleep(Duration::from_millis(500));
            if screen(&host, &pane) != before {
                return false;
            }
            if let Ok(dir) = std::env::var("AGENTDRIVER_CAPTURE_DIR") {
                std::fs::write(PathBuf::from(dir).join("claude-folder-trust.txt"), &before)
                    .unwrap();
            }
            let key = if before.contains("❯ Yes, I trust this folder") {
                answered_trust = true;
                Key::Enter
            } else {
                Key::Down
            };
            host.send_keys(&pane, &[key]).unwrap();
            false
        }),
        "never idle; screen: {}",
        screen(&host, &pane)
    );

    host.send_text(
        &pane,
        "Reply with exactly the word: pineapple",
        TextMode::Paste,
    )
    .unwrap();
    thread::sleep(Duration::from_millis(150));
    host.send_keys(&pane, &[Key::Enter]).unwrap();

    let mut saw_working = false;
    assert!(
        until(Duration::from_secs(120), || {
            let state = host.agent_state(&pane).unwrap();
            saw_working |= state == AgentState::Working;
            saw_working && matches!(state, AgentState::Idle | AgentState::Done)
        }),
        "turn did not finish; screen: {}",
        screen(&host, &pane)
    );
    assert!(screen(&host, &pane).contains("pineapple"));
    println!("answered trust prompt: {answered_trust}");
}
