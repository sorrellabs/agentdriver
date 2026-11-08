# Architecture

## Layers

```text
┌──────────────────────────────────────────────────────────┐
│ surfaces      agentdriver-cli          agentdriver-mcp   │
├──────────────────────────────────────────────────────────┤
│ core          agentdriver   (Session, verbs, wait loop,  │
│               signal store, prompt policy)               │
├───────────────────────────────┬──────────────────────────┤
│ host port     trait Host      │ harness port  trait Harness
├───────────────────────────────┼──────────────────────────┤
│ host adapters                 │ harness adapters         │
│ herdr · orca · tmux · cmux ·  │ claude · codex · cursor ·│
│ t3 · ovis · ghostty · …       │ devin · opencode · …     │
└───────────────────────────────┴──────────────────────────┘
```

The core never calls a host CLI or reads a harness's config format. Every
host-specific and harness-specific detail lives behind one of the two ports.

## Ports

```rust
pub trait Host {
    fn capabilities(&self) -> Capabilities;
    fn spawn(&self, spec: &SpawnSpec) -> Result<PaneRef>;
    fn read(&self, pane: &PaneRef) -> Result<Screen>;
    fn send_text(&self, pane: &PaneRef, text: &str) -> Result<()>;
    fn send_keys(&self, pane: &PaneRef, keys: &[Key]) -> Result<()>;
    fn close(&self, pane: &PaneRef) -> Result<()>;
    fn agent_state(&self, pane: &PaneRef) -> Result<Option<HostAgentState>>; // hosts with idle
    fn process(&self, pane: &PaneRef) -> Result<Option<ProcessInfo>>;
}

pub trait Harness {
    fn launch(&self, req: &LaunchRequest) -> Result<LaunchPlan>;   // argv, env, files to write
    fn signal(&self, state_dir: &Path) -> SignalSetup;               // hook/notify/plugin
    fn parse_signal(&self, line: &str) -> Option<Signal>;            // turn-finished, answer
    fn screen_rules(&self) -> &[ScreenRule];                         // prompts, ready state
    fn submit(&self) -> SubmitStyle;                                 // how Enter is delivered
}
```

The signatures are a sketch and will change while the first adapters are
built. The split is the part that is settled.

`Capabilities` lets the core refuse early: `read` on a host without screen
access fails with `Unsupported`, it does not guess.

## Session state

Each session gets a directory:

```text
$AGENTDRIVER_STATE/<session-id>/
  session.json     host, pane ref, harness, cwd, created_at, owner pid
  signals          append-only, one line per signal (written by hooks)
  rules.log        which screen rule fired, which keys were sent
  files/           per-workspace hook files the adapter wrote (removed on close)
```

`$AGENTDRIVER_STATE` defaults to the platform state directory. Nothing is
kept in memory between calls, so any process — the CLI, the MCP server, a
library user — can pick up a session by id.

## Waiting without a daemon

`wait` is a bounded loop in the caller's process:

1. watch `signals` for a new turn-finished line (file watch, with polling as
   a fallback);
2. when the host has `idle`, poll its agent state;
3. read the screen at a slow interval and run the screen rules — to detect
   prompts and to cover harnesses without a signal;
4. stop on the condition, on process exit, or on the mandatory timeout.

Hooks only append a line and exit. Nothing waits on a background process, and
a missing agentdriver process never delays the agent.

## Sending text safely

TUIs treat pasted text, typed text and Enter differently. Each harness
declares its `SubmitStyle` (for example: paste, settle, then Enter as a
separate key), and the host adapter picks the most faithful transport it has
(bracketed paste, `paste-buffer`, `send-text`). The core waits for the screen
to show the input before submitting.

## Errors

Every error names the layer (`host`, `harness`, `core`), the verb, and the
external command and exit status when there is one. A timeout returns the last
screen and last signal instead of an error.

## Testing

- **Unit:** screen rules against recorded screens; signal parsers against
  recorded hook payloads.
- **Adapter contract:** one shared test suite every host adapter must pass,
  run against a fake harness (a small TUI that behaves like an agent).
- **Conformance:** the live Tier 1 matrix described in
  [support tiers](support-tiers.md#release-verification).
