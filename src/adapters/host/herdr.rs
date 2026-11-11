//! Herdr adapter. Every verb is one `herdr` CLI call through
//! [`crate::adapters::exec`]; replies are JSON on stdout, failures JSON on
//! stderr (`{"error":{"code","message"}}`). Checked against Herdr 0.9.0.

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::adapters::exec::{self, CommandOutput, CommandSpec};
use crate::domain::error::{ErrorKind, Result};
use crate::domain::id::{HostKind, PaneId, TerminalId};
use crate::domain::key::Key;
use crate::domain::screen::Screen;
use crate::ports::host::{
    AgentState, Capabilities, Direction, Host, Layout, PaneInfo, PaneRef, Placement, ReadOptions,
    ReadSource, Rect, SpawnSpec, StartMode, TextMode,
};

pub const KIND: HostKind = HostKind::from_static("herdr");

/// Limit for an ordinary call. Herdr answers from its server in milliseconds.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// `agent start` waits up to 30 s for the agent to be ready; leave it room.
const AGENT_START_TIMEOUT: Duration = Duration::from_secs(45);
/// Cap on a stderr excerpt carried in an error.
const MESSAGE_LIMIT: usize = 500;

const PASTE_START: &str = "\u{1b}[200~";
const PASTE_END: &str = "\u{1b}[201~";

/// Runs one command. Replaced in tests; the real one is [`exec::run`].
pub type Runner = dyn Fn(&CommandSpec) -> Result<CommandOutput> + Send + Sync;

pub struct Herdr {
    program: String,
    session: Option<String>,
    /// Workspace for [`Placement::Tab`]. Herdr would otherwise use the
    /// focused one, which may belong to the user or another client.
    workspace: Option<String>,
    run: Arc<Runner>,
}

impl fmt::Debug for Herdr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Herdr")
            .field("program", &self.program)
            .field("session", &self.session)
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

impl Default for Herdr {
    fn default() -> Self {
        Self::new()
    }
}

impl Herdr {
    /// `herdr` from `PATH`, the default session, no tab workspace.
    pub fn new() -> Self {
        Self {
            program: "herdr".to_owned(),
            session: None,
            workspace: None,
            run: Arc::new(exec::run),
        }
    }

    /// What a process inside a Herdr pane is given: `HERDR_BIN_PATH` (when
    /// absolute), `HERDR_SESSION` and `HERDR_WORKSPACE_ID`.
    pub fn from_env() -> Self {
        let var = |name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        };
        let mut herdr = Self::new();
        if let Some(path) = var("HERDR_BIN_PATH").filter(|path| Path::new(path).is_absolute()) {
            herdr.program = path;
        }
        herdr.session = var("HERDR_SESSION");
        herdr.workspace = var("HERDR_WORKSPACE_ID");
        herdr
    }

    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = Some(session.into());
        self
    }

    pub fn with_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    /// Replaces command execution, for tests that answer with captured replies.
    pub fn with_runner(
        mut self,
        run: impl Fn(&CommandSpec) -> Result<CommandOutput> + Send + Sync + 'static,
    ) -> Self {
        self.run = Arc::new(run);
        self
    }

    fn call(&self, args: Vec<String>, timeout: Duration) -> Result<CommandOutput> {
        let mut argv = Vec::with_capacity(args.len() + 2);
        if let Some(session) = &self.session {
            argv.extend(["--session".to_owned(), session.clone()]);
        }
        argv.extend(args);
        let mut spec = CommandSpec::new(&self.program, argv);
        spec.timeout = Some(timeout);
        let output = (self.run)(&spec).map_err(|error| match error.kind() {
            ErrorKind::Spawn { source, .. } => ErrorKind::HostUnavailable {
                host: KIND,
                detail: format!("cannot run {}: {source}", self.program),
            }
            .into(),
            _ => error,
        })?;
        if output.success() {
            Ok(output)
        } else {
            Err(failure(&output))
        }
    }

    fn json<T: DeserializeOwned>(&self, args: Vec<String>, what: &'static str) -> Result<T> {
        let output = self.call(args, CALL_TIMEOUT)?;
        let envelope: Envelope<T> =
            serde_json::from_slice(&output.stdout).map_err(|error| ErrorKind::Parse {
                what,
                detail: error.to_string(),
            })?;
        Ok(envelope.result)
    }

    fn open(&self, spec: &SpawnSpec) -> Result<WirePane> {
        let cwd = utf8(&spec.cwd)?;
        let mut args = match &spec.placement {
            Placement::Split { target, direction } => words([
                "pane",
                "split",
                "--pane",
                target.as_str(),
                "--direction",
                direction_word(*direction),
                "--cwd",
                cwd,
            ]),
            Placement::Tab { label } => {
                let workspace = self.workspace.as_deref().ok_or_else(|| ErrorKind::Host {
                    host: KIND,
                    code: None,
                    message: "a tab needs a workspace: set HERDR_WORKSPACE_ID or call \
                              with_workspace"
                        .to_owned(),
                    exit: None,
                })?;
                let mut args = words(["tab", "create", "--workspace", workspace, "--cwd", cwd]);
                if let Some(label) = label {
                    args.extend(words(["--label", label]));
                }
                args
            }
        };
        for (key, value) in &spec.env {
            if key.is_empty() || key.contains('=') {
                return Err(ErrorKind::Host {
                    host: KIND,
                    code: None,
                    message: format!("invalid environment variable name {key:?}"),
                    exit: None,
                }
                .into());
            }
            args.extend(["--env".to_owned(), format!("{key}={value}")]);
        }
        args.push("--no-focus".to_owned());
        Ok(match spec.placement {
            Placement::Split { .. } => self.json::<SplitResult>(args, "herdr pane split")?.pane,
            Placement::Tab { .. } => self.json::<TabResult>(args, "herdr tab create")?.root_pane,
        })
    }

    fn start(&self, pane: &PaneId, start: &StartMode) -> Result<()> {
        match start {
            StartMode::Run(argv) => {
                self.call(
                    words(["pane", "run", pane.as_str(), &exec::shell_command(argv)]),
                    CALL_TIMEOUT,
                )?;
            }
            StartMode::Agent { name, kind, args } => {
                let mut call = words([
                    "agent",
                    "start",
                    name.as_str(),
                    "--kind",
                    kind.as_str(),
                    "--pane",
                    pane.as_str(),
                ]);
                if !args.is_empty() {
                    call.push("--".to_owned());
                    call.extend(args.iter().cloned());
                }
                self.call(call, AGENT_START_TIMEOUT)?;
            }
        }
        Ok(())
    }
}

impl Host for Herdr {
    fn kind(&self) -> HostKind {
        KIND
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            agent_state: true,
            agent_start: true,
            prompt: true,
            layout: true,
            locate_caller: false,
        }
    }

    /// Opens the pane without taking focus, then starts the command in it. If
    /// the command cannot start, the pane just opened is closed again.
    fn spawn(&self, spec: &SpawnSpec) -> Result<PaneRef> {
        let opened = self.open(spec)?;
        let pane = PaneId::new(opened.pane_id);
        if let Err(error) = self.start(&pane, &spec.start) {
            let _ = self.call(words(["pane", "close", pane.as_str()]), CALL_TIMEOUT);
            return Err(error);
        }
        Ok(PaneRef {
            pane,
            terminal: opened.terminal_id.map(TerminalId::new),
        })
    }

    fn read(&self, pane: &PaneId, options: ReadOptions) -> Result<Screen> {
        let source = match options.source {
            ReadSource::Visible => "visible",
            ReadSource::RecentUnwrapped => "recent-unwrapped",
        };
        let lines = options.lines.to_string();
        let output = self
            .call(
                words([
                    "pane",
                    "read",
                    pane.as_str(),
                    "--source",
                    source,
                    "--lines",
                    &lines,
                ]),
                CALL_TIMEOUT,
            )
            .map_err(|error| missing_pane(error, pane))?;
        Ok(Screen::new(String::from_utf8_lossy(&output.stdout)))
    }

    /// `Paste` wraps the text in bracketed-paste markers, so embedded newlines
    /// do not submit. A closing marker inside the text is removed: it would end
    /// the paste early and turn the rest into keystrokes.
    fn send_text(&self, pane: &PaneId, text: &str, mode: TextMode) -> Result<()> {
        let text = match mode {
            TextMode::Paste => format!("{PASTE_START}{}{PASTE_END}", text.replace(PASTE_END, "")),
            TextMode::Literal => text.to_owned(),
        };
        self.call(
            words(["pane", "send-text", pane.as_str(), &text]),
            CALL_TIMEOUT,
        )
        .map_err(|error| missing_pane(error, pane))?;
        Ok(())
    }

    /// All keys go in one call; Herdr checks every key before writing any.
    fn send_keys(&self, pane: &PaneId, keys: &[Key]) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let mut args = words(["pane", "send-keys", pane.as_str()]);
        args.extend(keys.iter().map(key_name));
        self.call(args, CALL_TIMEOUT)
            .map_err(|error| missing_pane(error, pane))?;
        Ok(())
    }

    /// A pane that is already gone counts as closed.
    fn close(&self, pane: &PaneId) -> Result<()> {
        match self.call(words(["pane", "close", pane.as_str()]), CALL_TIMEOUT) {
            Ok(_) => Ok(()),
            Err(error) if host_code(&error) == Some("pane_not_found") => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn pane_info(&self, pane: &PaneId) -> Result<Option<PaneInfo>> {
        match self.json::<PaneResult>(words(["pane", "get", pane.as_str()]), "herdr pane get") {
            Ok(result) => Ok(Some(PaneInfo {
                pane: PaneId::new(result.pane.pane_id),
                terminal: result.pane.terminal_id.map(TerminalId::new),
                pid: None,
            })),
            Err(error) if host_code(&error) == Some("pane_not_found") => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// `unknown` when Herdr recognises no agent in the pane: that proves
    /// neither work nor completion.
    fn agent_state(&self, pane: &PaneId) -> Result<AgentState> {
        match self.json::<AgentResult>(words(["agent", "get", pane.as_str()]), "herdr agent get") {
            Ok(result) => Ok(agent_state(&result.agent.agent_status)),
            Err(error) if host_code(&error) == Some("agent_not_found") => Ok(AgentState::Unknown),
            Err(error) => Err(error),
        }
    }

    /// Herdr's own prompt path: it honours the pane's bracketed-paste mode and
    /// refuses an agent that is waiting at a question.
    fn prompt(&self, pane: &PaneId, text: &str) -> Result<()> {
        match self.call(
            words(["agent", "prompt", pane.as_str(), text]),
            CALL_TIMEOUT,
        ) {
            Ok(_) => Ok(()),
            Err(error) if host_code(&error) == Some("agent_blocked") => {
                Err(ErrorKind::AwaitingInput {
                    pane: pane.clone(),
                    excerpt: host_message(&error).unwrap_or_default().to_owned(),
                }
                .into())
            }
            Err(error) => Err(missing_pane(error, pane)),
        }
    }

    fn layout(&self, pane: &PaneId) -> Result<Layout> {
        let result: LayoutResult = self.json(
            words(["pane", "layout", "--pane", pane.as_str()]),
            "herdr pane layout",
        )?;
        Ok(Layout {
            panes: result
                .layout
                .panes
                .into_iter()
                .map(|entry| {
                    let WireRect {
                        x,
                        y,
                        width,
                        height,
                    } = entry.rect;
                    (
                        PaneId::new(entry.pane_id),
                        Rect {
                            x,
                            y,
                            width,
                            height,
                        },
                    )
                })
                .collect(),
        })
    }

    fn resize(&self, pane: &PaneId, direction: Direction, amount: f64) -> Result<()> {
        let amount = format!("{amount:.4}");
        self.call(
            words([
                "pane",
                "resize",
                "--pane",
                pane.as_str(),
                "--direction",
                direction_word(direction),
                "--amount",
                &amount,
            ]),
            CALL_TIMEOUT,
        )
        .map_err(|error| missing_pane(error, pane))?;
        Ok(())
    }

    fn rename(&self, pane: &PaneId, label: &str) -> Result<()> {
        self.call(
            words(["pane", "rename", pane.as_str(), label]),
            CALL_TIMEOUT,
        )
        .map_err(|error| missing_pane(error, pane))?;
        Ok(())
    }
}

fn words<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    items.into_iter().map(str::to_owned).collect()
}

fn utf8(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        ErrorKind::Host {
            host: KIND,
            code: None,
            message: format!("working directory is not UTF-8: {}", path.display()),
            exit: None,
        }
        .into()
    })
}

fn direction_word(direction: Direction) -> &'static str {
    match direction {
        Direction::Right => "right",
        Direction::Down => "down",
    }
}

/// Herdr's key names, as `herdr pane send-keys` accepts them.
fn key_name(key: &Key) -> String {
    match key {
        Key::Enter => "enter".to_owned(),
        Key::Esc => "esc".to_owned(),
        Key::Tab => "tab".to_owned(),
        Key::ShiftTab => "shift+tab".to_owned(),
        Key::Up => "up".to_owned(),
        Key::Down => "down".to_owned(),
        Key::Left => "left".to_owned(),
        Key::Right => "right".to_owned(),
        Key::Backspace => "backspace".to_owned(),
        Key::Char(' ') => "space".to_owned(),
        Key::Char(character) => character.to_string(),
        Key::Ctrl(character) => format!("ctrl+{}", character.to_ascii_lowercase()),
        Key::F(number) => format!("f{number}"),
    }
}

fn agent_state(status: &str) -> AgentState {
    match status {
        "idle" => AgentState::Idle,
        "working" => AgentState::Working,
        "blocked" => AgentState::Blocked,
        "done" => AgentState::Done,
        _ => AgentState::Unknown,
    }
}

/// The error for a failed call: Herdr's code and message when stderr carries
/// them, otherwise a bounded excerpt of stderr.
fn failure(output: &CommandOutput) -> crate::Error {
    let reply = serde_json::from_slice::<ErrorReply>(&output.stderr).ok();
    let (code, message) = match reply {
        Some(reply) => (Some(reply.error.code), reply.error.message),
        None => (None, excerpt(&output.stderr)),
    };
    ErrorKind::Host {
        host: KIND,
        code,
        message,
        exit: output.exit,
    }
    .into()
}

fn excerpt(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    match text.char_indices().nth(MESSAGE_LIMIT) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

fn host_code(error: &crate::Error) -> Option<&str> {
    match error.kind() {
        ErrorKind::Host { code, .. } => code.as_deref(),
        _ => None,
    }
}

fn host_message(error: &crate::Error) -> Option<&str> {
    match error.kind() {
        ErrorKind::Host { message, .. } => Some(message),
        _ => None,
    }
}

fn missing_pane(error: crate::Error, pane: &PaneId) -> crate::Error {
    if host_code(&error) == Some("pane_not_found") {
        ErrorKind::PaneNotFound(pane.clone()).into()
    } else {
        error
    }
}

// Wire types. Unknown fields are ignored so a newer Herdr keeps working.

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    result: T,
}

#[derive(Debug, Deserialize)]
struct ErrorReply {
    error: WireError,
}

#[derive(Debug, Deserialize)]
struct WireError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct WirePane {
    pane_id: String,
    terminal_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SplitResult {
    pane: WirePane,
}

#[derive(Debug, Deserialize)]
struct PaneResult {
    pane: WirePane,
}

#[derive(Debug, Deserialize)]
struct TabResult {
    root_pane: WirePane,
}

#[derive(Debug, Deserialize)]
struct AgentResult {
    agent: WireAgent,
}

#[derive(Debug, Deserialize)]
struct WireAgent {
    agent_status: String,
}

#[derive(Debug, Deserialize)]
struct LayoutResult {
    layout: WireLayout,
}

#[derive(Debug, Deserialize)]
struct WireLayout {
    panes: Vec<WireLayoutPane>,
}

#[derive(Debug, Deserialize)]
struct WireLayoutPane {
    pane_id: String,
    rect: WireRect,
}

#[derive(Debug, Deserialize)]
struct WireRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/testdata/hosts/herdr-0.9.0/",
                $name
            ))
        };
    }

    /// A reply Herdr gave: exit status, stdout, stderr.
    type Reply = (i32, &'static str, &'static str);

    const OK: Reply = (0, "", "");
    const PANE_NOT_FOUND: Reply = (1, "", fixture!("error-pane-not-found.json"));

    /// Answers each call with the next reply and records the argv it saw.
    #[derive(Clone, Default)]
    struct Script {
        replies: Arc<Mutex<Vec<Reply>>>,
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl Script {
        fn new(replies: &[Reply]) -> Self {
            let script = Self::default();
            *script.replies.lock().unwrap() = replies.iter().rev().copied().collect();
            script
        }

        fn host(&self) -> Herdr {
            let script = self.clone();
            Herdr::new().with_runner(move |spec| {
                script.calls.lock().unwrap().push(spec.args.clone());
                let (exit, stdout, stderr) = script
                    .replies
                    .lock()
                    .unwrap()
                    .pop()
                    .expect("an unexpected extra herdr call");
                Ok(CommandOutput {
                    exit: Some(exit),
                    stdout: stdout.as_bytes().to_vec(),
                    stderr: stderr.as_bytes().to_vec(),
                })
            })
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    fn pane() -> PaneId {
        PaneId::new("wKV:p3")
    }

    fn split_spec(start: StartMode) -> SpawnSpec {
        SpawnSpec {
            placement: Placement::Split {
                target: PaneId::new("wKV:p1"),
                direction: Direction::Down,
            },
            cwd: PathBuf::from("/work/task"),
            env: vec![("AGENTDRIVER_PROBE".to_owned(), "1".to_owned())],
            start,
        }
    }

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn spawn_splits_beside_the_target_without_focus_then_runs_the_quoted_argv() {
        let script = Script::new(&[(0, fixture!("pane-split.json"), ""), OK]);
        let opened = script
            .host()
            .spawn(&split_spec(StartMode::Run(argv(&["codex", "it's here"]))))
            .unwrap();
        assert_eq!(opened.pane, pane());
        assert_eq!(
            opened.terminal,
            Some(TerminalId::new("term_65ffb1a40d953278"))
        );
        assert_eq!(
            script.calls(),
            vec![
                argv(&[
                    "pane",
                    "split",
                    "--pane",
                    "wKV:p1",
                    "--direction",
                    "down",
                    "--cwd",
                    "/work/task",
                    "--env",
                    "AGENTDRIVER_PROBE=1",
                    "--no-focus",
                ]),
                argv(&["pane", "run", "wKV:p3", r"'codex' 'it'\''s here'"]),
            ]
        );
    }

    #[test]
    fn a_command_that_cannot_start_closes_the_pane_it_opened() {
        let script = Script::new(&[
            (0, fixture!("pane-split.json"), ""),
            (
                1,
                "",
                r#"{"error":{"code":"server_error","message":"boom"}}"#,
            ),
            (0, r#"{"result":{"type":"ok"}}"#, ""),
        ]);
        let error = script
            .host()
            .spawn(&split_spec(StartMode::Run(argv(&["codex"]))))
            .unwrap_err();
        assert_eq!(host_code(&error), Some("server_error"));
        assert_eq!(
            script.calls().last().unwrap(),
            &argv(&["pane", "close", "wKV:p3"])
        );
    }

    #[test]
    fn a_recognised_agent_is_started_by_herdr_with_its_own_arguments() {
        let script = Script::new(&[(0, fixture!("pane-split.json"), ""), OK]);
        script
            .host()
            .spawn(&split_spec(StartMode::Agent {
                name: "reviewer".to_owned(),
                kind: "codex".to_owned(),
                args: argv(&["--yolo"]),
            }))
            .unwrap();
        assert_eq!(
            script.calls()[1],
            argv(&[
                "agent", "start", "reviewer", "--kind", "codex", "--pane", "wKV:p3", "--",
                "--yolo",
            ])
        );
    }

    #[test]
    fn a_tab_opens_in_the_configured_workspace_and_reports_its_root_pane() {
        let script = Script::new(&[(0, fixture!("tab-create.json"), ""), OK]);
        let mut spec = split_spec(StartMode::Run(argv(&["codex"])));
        spec.placement = Placement::Tab {
            label: Some("worker".to_owned()),
        };
        let opened = script.host().with_workspace("wKV").spawn(&spec).unwrap();
        assert!(opened.pane.as_str().starts_with("wKV:"));
        assert_eq!(
            script.calls()[0],
            argv(&[
                "tab",
                "create",
                "--workspace",
                "wKV",
                "--cwd",
                "/work/task",
                "--label",
                "worker",
                "--env",
                "AGENTDRIVER_PROBE=1",
                "--no-focus",
            ])
        );
    }

    #[test]
    fn a_tab_without_a_known_workspace_is_refused_before_any_call() {
        let script = Script::new(&[]);
        let mut spec = split_spec(StartMode::Run(argv(&["codex"])));
        spec.placement = Placement::Tab { label: None };
        assert!(script.host().spawn(&spec).is_err());
        assert!(script.calls().is_empty());
    }

    #[test]
    fn an_environment_name_with_an_equals_sign_is_refused_before_any_call() {
        let script = Script::new(&[]);
        let mut spec = split_spec(StartMode::Run(argv(&["codex"])));
        spec.env = vec![("A=B".to_owned(), "c".to_owned())];
        assert!(script.host().spawn(&spec).is_err());
        assert!(script.calls().is_empty());
    }

    #[test]
    fn pane_info_reports_the_terminal_behind_the_pane() {
        let script = Script::new(&[(0, fixture!("pane-get.json"), "")]);
        let info = script.host().pane_info(&pane()).unwrap().unwrap();
        assert_eq!(
            info.terminal,
            Some(TerminalId::new("term_65ffb1a40d953278"))
        );
    }

    #[test]
    fn a_missing_pane_is_a_proven_absence() {
        let script = Script::new(&[PANE_NOT_FOUND]);
        assert_eq!(script.host().pane_info(&pane()).unwrap(), None);
    }

    #[test]
    fn a_host_failure_is_not_mistaken_for_absence() {
        let script = Script::new(&[(1, "", "connection refused")]);
        let error = script.host().pane_info(&pane()).unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::Host { code: None, message, exit: Some(1), .. } if message == "connection refused"
        ));
    }

    #[test]
    fn a_garbled_reply_is_a_parse_error() {
        let script = Script::new(&[(0, "{not json", "")]);
        let error = script.host().pane_info(&pane()).unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::Parse { .. }));
    }

    #[test]
    fn closing_a_pane_that_is_already_gone_succeeds() {
        let script = Script::new(&[PANE_NOT_FOUND]);
        script.host().close(&pane()).unwrap();
    }

    #[test]
    fn reading_a_missing_pane_is_pane_not_found() {
        let script = Script::new(&[PANE_NOT_FOUND]);
        assert!(
            script
                .host()
                .read(&pane(), ReadOptions::default())
                .unwrap_err()
                .is_pane_not_found()
        );
    }

    #[test]
    fn a_read_asks_for_the_source_and_line_count() {
        let script = Script::new(&[(0, "❯ ready\n", "")]);
        let screen = script
            .host()
            .read(
                &pane(),
                ReadOptions {
                    source: ReadSource::RecentUnwrapped,
                    lines: 120,
                },
            )
            .unwrap();
        assert_eq!(screen.text, "❯ ready\n");
        assert_eq!(
            script.calls()[0],
            argv(&[
                "pane",
                "read",
                "wKV:p3",
                "--source",
                "recent-unwrapped",
                "--lines",
                "120"
            ])
        );
    }

    #[test]
    fn a_paste_is_bracketed_and_cannot_be_ended_early_by_its_text() {
        let script = Script::new(&[OK]);
        script
            .host()
            .send_text(&pane(), "a\n\u{1b}[201~b", TextMode::Paste)
            .unwrap();
        assert_eq!(
            script.calls()[0][3],
            "\u{1b}[200~a\nb\u{1b}[201~".to_owned()
        );
    }

    #[test]
    fn keys_go_in_one_call_under_herdr_names() {
        let script = Script::new(&[OK]);
        script
            .host()
            .send_keys(
                &pane(),
                &[
                    Key::ShiftTab,
                    Key::Ctrl('C'),
                    Key::Char(' '),
                    Key::Char('y'),
                    Key::F(5),
                    Key::Esc,
                    Key::Enter,
                ],
            )
            .unwrap();
        assert_eq!(
            script.calls()[0],
            argv(&[
                "pane",
                "send-keys",
                "wKV:p3",
                "shift+tab",
                "ctrl+c",
                "space",
                "y",
                "f5",
                "esc",
                "enter"
            ])
        );
    }

    #[test]
    fn a_rejected_key_carries_herdr_code() {
        let script = Script::new(&[(1, "", fixture!("error-invalid-key.json"))]);
        let error = script.host().send_keys(&pane(), &[Key::Enter]).unwrap_err();
        assert_eq!(host_code(&error), Some("invalid_key"));
    }

    #[test]
    fn agent_state_comes_from_herdr_lifecycle() {
        let script = Script::new(&[(0, fixture!("agent-get-working.json"), "")]);
        assert_eq!(
            script.host().agent_state(&pane()).unwrap(),
            AgentState::Working
        );
        for (status, state) in [
            ("idle", AgentState::Idle),
            ("done", AgentState::Done),
            ("blocked", AgentState::Blocked),
            ("unknown", AgentState::Unknown),
            ("something-new", AgentState::Unknown),
        ] {
            assert_eq!(agent_state(status), state, "{status}");
        }
    }

    #[test]
    fn a_pane_with_no_recognised_agent_is_unknown_not_idle() {
        let script = Script::new(&[(1, "", fixture!("error-agent-not-found.json"))]);
        assert_eq!(
            script.host().agent_state(&pane()).unwrap(),
            AgentState::Unknown
        );
    }

    #[test]
    fn a_prompt_to_a_blocked_agent_is_reported_as_awaiting_input() {
        let script = Script::new(&[(
            1,
            "",
            r#"{"error":{"code":"agent_blocked","message":"agent is waiting at a question"}}"#,
        )]);
        let error = script.host().prompt(&pane(), "go").unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::AwaitingInput { .. }));
    }

    #[test]
    fn layout_lists_every_pane_with_its_rectangle() {
        let script = Script::new(&[(0, fixture!("pane-layout.json"), "")]);
        let layout = script.host().layout(&pane()).unwrap();
        assert_eq!(
            layout.panes,
            vec![
                (
                    PaneId::new("wKV:p1"),
                    Rect {
                        x: 0,
                        y: 0,
                        width: 96,
                        height: 60
                    }
                ),
                (
                    PaneId::new("wKV:p2"),
                    Rect {
                        x: 96,
                        y: 0,
                        width: 96,
                        height: 60
                    }
                ),
            ]
        );
    }

    #[test]
    fn a_named_session_is_passed_before_the_command() {
        let script = Script::new(&[PANE_NOT_FOUND]);
        let _ = script.host().with_session("test").pane_info(&pane());
        assert_eq!(
            script.calls()[0][..3],
            argv(&["--session", "test", "pane"])[..]
        );
    }

    #[test]
    fn a_missing_herdr_binary_makes_the_host_unavailable() {
        let host = Herdr::new().with_program("/no/such/herdr");
        let error = host.pane_info(&pane()).unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::HostUnavailable { .. }));
    }
}
