# Architecture

This describes the code as it exists. The skeleton compiles and its tests pass;
the Herdr and tmux adapters and most screen rules are filled in by the
[roadmap](roadmap.md) milestones.

## Layers

```text
 src/bin/agentdriver/   main.rs (clap)        mcp.rs (rmcp, tokio)
                               │                    │ spawn_blocking
 ──────────────────────────────┴────────────────────┴──────────────
 library                  Driver → Session → Wait
                               │
              ┌────────────────┼─────────────────┐
              ▼                ▼                 ▼
         trait Host       trait Harness     rules (classify)
              │                │
   herdr · tmux · fake     Recipe table          state/ (disk)
              │
           exec.rs  (every external command: timeout + output cap)
```

The library never sees clap, rmcp or tokio: they are optional dependencies used
only under `src/bin/`. `cargo tree --no-default-features -e normal` must not list
them.

## Module map

| Path | Responsibility |
|---|---|
| `src/driver.rs` | `Driver::spawn`: session id, signal setup, launch, session record |
| `src/session.rs` | `Session`: `verify`, `screen`, `send_prompt`, `press`, `wait`, `resolve_prompt`, `close` |
| `src/wait.rs` | `Wait` builder: `at_most`, `every`, `forever`, `until`, `for_text`, `for_stable`, `for_idle` |
| `src/host/mod.rs` | `Host` trait, `Capabilities`, `PaneRef`, `SpawnSpec`, `Placement`, `StartMode` |
| `src/host/{herdr,tmux}.rs` | one adapter per host, each behind a feature |
| `src/host/fake.rs` | in-memory host for tests (`--features fake`) |
| `src/harness/mod.rs` | `Harness` trait, `LaunchPlan`, `SignalSetup`, `Signal` |
| `src/harness/recipe.rs` | launch data for the eleven built-in harnesses |
| `src/rules/mod.rs` | `classify`, `RulePolicy`, the ordered rule list |
| `src/rules/text.rs` | `squash`, `menu_options`, `keys_between`, `has_continue_notice` |
| `src/state.rs` | `SessionRecord`, session directory, signal file reading |
| `src/exec.rs` | bounded command runner |
| `src/error.rs` | `Error`, `ErrorKind` |
| `src/id.rs`, `key.rs`, `screen.rs` | `PaneId` and friends, `Key`, `Screen` |

## The two ports

**`Host`** has eight required methods: `kind`,
`capabilities`, `spawn`, `read`, `send_text`, `send_keys`, `close` and
`pane_info`. Optional abilities (`agent_state`, `prompt`, `layout`, `resize`,
`rename`, `locate_caller`) default to `ErrorKind::Unsupported` and are announced
through `Capabilities`, so a host only implements what it has. The trait is
object-safe: the driver holds an `Arc<dyn Host>`.

`pane_info` returns `Ok(None)` only for a proven absence, such as Herdr's
`pane_not_found`. Any doubt is an error, so a transient failure can never look
like a closed pane.

**`Harness`** builds a `LaunchPlan` from a `LaunchRequest`, and optionally
installs a turn-finished signal (`SignalSetup`) and parses its lines. The
executable path always comes from the caller. Full-permission flags are added
only when the request asks for them.

## Decisions taken from prior art

| From | Taken | Not taken |
|---|---|---|
| bat, mdBook | One package; library at `src/lib.rs`, binary in `src/bin/`; `dep:` optional dependencies; `required-features` on the binary | mdBook's later split into many crates, which served plugin authors we do not have |
| fantoccini | Cheap session handle; `wait().at_most().every().for_x()` builder; `Ok(None)` retries, `Err` aborts | Its async API |
| thirtyfour | Boxed, non-exhaustive error, so `Result` stays small and new failures are not breaking | |
| rexpect | Timeout errors that carry what was awaited and what was seen | A stream of consumed output |
| expectrl | Open predicates (`until`) | Matching a byte stream: full-screen TUIs repaint, so agentdriver matches screen snapshots |
| Herdr | `pane_id` + `terminal_id` identity; `Idle/Working/Blocked/Done/Unknown` states | Its socket, for now (see [decision 14](decisions.md#14-hosts-are-driven-through-their-clis)) |
| tmux_interface | Nothing | The dependency: compile-time tmux versions and untyped errors, for eight commands we can wrap directly |
| rmcp SDK | `#[tool_router]`, stdio, `spawn_blocking` around blocking work | |

## Session state

```text
$AGENTDRIVER_STATE/<session-id>/
  session.json   schema, id, host, pane ref, harness, cwd, created_at
  signals        append-only, one line per harness event
  rules.log      {unix_secs} {pane} {rule} {keys}
```

`$AGENTDRIVER_STATE` defaults to `~/Library/Application Support/agentdriver` on
macOS and `$XDG_STATE_HOME/agentdriver` elsewhere. Nothing is held in memory
between calls. A partial line in `signals` is left until its newline arrives.

## Waiting

`Wait` is a bounded loop in the caller's process with no daemon. It reads the
screen, then runs the condition: `Ok(None)` retries, `Err` aborts, `Ok(Some)`
returns. `for_idle` checks for a harness signal written since the call began,
then the host's agent state. A timeout is `ErrorKind::Timeout`, which names what
was awaited and quotes the last screen.

## Safety properties the skeleton already enforces

- Input and close call `Session::verify`; a pane id now held by another terminal
  fails with `PaneReplaced`, and is not closed.
- `close` on a pane that is already gone succeeds.
- Nothing is read or pressed while the host reports the agent working.
- A prompt whose rule is not enabled is reported as `Classification::Prompt`.
- A menu no rule recognises is `Classification::Unknown` and never pressed.
- External commands run with stdin closed, a timeout and an output cap, and a
  background process holding a pipe open cannot block the return.

## Testing

- **Unit:** beside the code. Screen rules take captured screens.
- **Session behaviour:** `FakeHost` scripts screens, states and reused terminals.
- **Adapter contract:** one shared suite every host passes (M1).
- **Conformance:** the live Tier 1 matrix in
  [support tiers](support-tiers.md#release-verification).
