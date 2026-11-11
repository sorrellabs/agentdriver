# Architecture

This describes the code as it exists. The Herdr adapter is implemented; the
tmux adapter and most screen rules are filled in by the [roadmap](roadmap.md)
milestones.

## Layers

The crate is hexagonal. Dependencies point inward only.

```text
 driving adapters    src/bin/agentdriver/  main.rs (clap) · mcp.rs (rmcp, tokio)
                               │
 ┌─────────────────────────────▼─────────────────────────────┐
 │ core      Driver → Session → Wait · classify · RulePolicy │
 │                              │                            │
 │ ports     Host · Harness · Store · Clock                  │
 │                                                           │
 │ domain    Error · ids · Key · Screen · Rule · text        │
 └─────────────────────────────▲─────────────────────────────┘
                               │ implement the ports
 driven adapters     src/adapters/
                     host/{herdr,tmux,fake} · harness/{recipe,rules}
                     store (FsStore, MemoryStore) · clock (SystemClock, FakeClock)
                     exec (timeout + output cap for every external command)
```

| Layer | May use | Must not use |
|---|---|---|
| `domain` | the standard library's data types | any other layer |
| `ports` | `domain` | `core`, `adapters` |
| `core` | `ports`, `domain` | `adapters`, files, processes, threads, real time, environment variables |
| `adapters` | `ports`, `domain`, `core` only to build a `Driver` | clap, rmcp, tokio |
| `src/bin/` | everything | |

`tests/architecture.rs` fails the build when any of this is broken.

clap, rmcp and tokio are optional dependencies used only under `src/bin/`.
`cargo tree --no-default-features -e normal` must not list them.

## Module map

| Path | Responsibility |
|---|---|
| `src/core/driver.rs` | `Driver::spawn`: allocate, signal setup, launch, session record |
| `src/core/session.rs` | `Session`: `verify`, `screen`, `send_prompt`, `press`, `wait`, `resolve_prompt`, `close` |
| `src/core/wait.rs` | `Wait` builder: `at_most`, `every`, `forever`, `until`, `for_text`, `for_stable`, `for_idle` |
| `src/core/rules.rs` | `classify`, `RulePolicy`, `Classification` |
| `src/ports/host.rs` | `Host`, `Capabilities`, `PaneRef`, `SpawnSpec`, `Placement`, `StartMode` |
| `src/ports/harness.rs` | `Harness`, `LaunchPlan`, `SignalSetup`, `Signal` |
| `src/ports/store.rs` | `Store`, `SessionRecord` |
| `src/ports/clock.rs` | `Clock` |
| `src/domain/` | `Error`, `ErrorKind`, ids, `HostKind`, `HarnessKind`, `Key`, `Screen`, `Rule`, screen-text helpers |
| `src/adapters/host/` | one adapter per host, each behind a feature; `fake` for tests |
| `src/adapters/harness/` | launch recipes for eleven harnesses, their screen rules |
| `src/adapters/store.rs` | `FsStore`; `MemoryStore` for tests |
| `src/adapters/clock.rs` | `SystemClock`; `FakeClock` for tests |
| `src/adapters/exec.rs` | bounded command runner, for adapters only |

`adapters::default_driver(host, policy)` wires the real store and clock. The fakes
are behind the `fake` feature, which this crate's own tests turn on.

## The four ports

**`Host`** has eight required methods: `kind`, `capabilities`, `spawn`, `read`,
`send_text`, `send_keys`, `close` and `pane_info`. Optional abilities
(`agent_state`, `prompt`, `layout`, `resize`, `rename`, `locate_caller`) default
to `ErrorKind::Unsupported` and are announced through `Capabilities`.

`pane_info` returns `Ok(None)` only for a proven absence, such as Herdr's
`pane_not_found`. Any doubt is an error, so a transient failure can never look
like a closed pane.

**`Harness`** builds a `LaunchPlan` from a `LaunchRequest`, optionally installs a
turn-finished signal (`SignalSetup`) and parses its lines, and supplies its own
screen `rules`. The executable path always comes from the caller. Full-permission
flags are added only when the request asks for them.

**`Store`** allocates sessions and keeps their record, signal stream and rule log.
It also installs and removes harness signal files in the workspace. A consumer
can supply its own to choose the location.

**`Clock`** provides monotonic time, sleep and the wall clock, so deadlines are
tested without waiting.

`HostKind` and `HarnessKind` are open names rather than enums, and nothing in the
core lists hosts or harnesses. `tests/session.rs` plugs in a host and a harness
defined outside the crate to keep that true.

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

`FsStore` keeps each session in a directory, so any process can pick it up by id:

```text
$AGENTDRIVER_STATE/<session-id>/
  session.json   schema, id, host, pane ref, harness, cwd, created_at
  signals        append-only, one line per harness event
  rules.log      {unix_secs} {pane} {rule} {keys}
```

`$AGENTDRIVER_STATE` defaults to `~/Library/Application Support/agentdriver` on
macOS and `$XDG_STATE_HOME/agentdriver` elsewhere. A partial line in `signals`
is left until its newline arrives.

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
- **Session behaviour:** `tests/session.rs` uses `FakeHost`, `MemoryStore` and
  `FakeClock`, so deadlines run instantly.
- **Boundaries:** `tests/architecture.rs` checks the dependency direction.
- **Host adapters:** each adapter answers its unit tests with replies captured
  from the real host (`testdata/hosts/<host>-<version>/`), and has a live test
  (`tests/live_<host>.rs`, `--features live -- --ignored`).
- **Adapter contract:** one shared suite every host passes (M1).
- **Conformance:** the live Tier 1 matrix in
  [support tiers](support-tiers.md#release-verification).
