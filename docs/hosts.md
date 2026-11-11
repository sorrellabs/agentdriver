# Hosts

What each host exposes, and what its adapter has to do. Findings were checked
in November 2026 against the versions noted; re-check before relying on a
specific flag.

## Capability matrix

| Host | spawn | read | send | keys | close | host idle | adopt |
|---|---|---|---|---|---|---|---|
| Herdr | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Orca | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| tmux | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ |
| cmux | ✓ | ✓ | ✓ | ✓ | ✓ | events | ✓ |
| T3 Code | thread | thread | thread | interrupt | — | thread wait | — |
| Ovis | via Herdr | via Herdr | via Herdr | via Herdr | via Herdr | via Herdr | via Herdr |
| Ghostty 1.4 | ✓ | — | ✓ | ✓ | ? | — | via tty |
| Warp | launch config | — | — | — | — | — | — |
| Zed | — | — | — | — | — | — | — |

## Herdr (Tier 1)

The reference host, and the one brgr is built on today. brgr's current use of
the Herdr CLI is the minimum the Herdr adapter must cover:

| Herdr command | Uses in brgr |
|---|---|
| `agent get` | 11 |
| `agent prompt` | 7 |
| `pane get`, `pane close` | 5 each |
| `pane send-keys` | 4 |
| `pane read`, `pane run`, `pane layout`, `agent list` | 2 each |
| `pane send-text`, `pane rename`, `pane process-info` | 1 each |
| `worktree list/add/remove/create` | 9 in total |

Worktree commands are workspace management, not driving; they stay in the
consumer, not in agentdriver (see [decision 7](decisions.md#7-worktrees-belong-to-the-caller)).

### What the adapter does (Herdr 0.9.0)

| Verb | Herdr call | Notes |
|---|---|---|
| spawn (split) | `pane split --pane T --direction right\|down --cwd C --env K=V --no-focus`, then `pane run P <cmd>` | The argv is single-quoted into one command line by `exec::shell_command`. If `pane run` fails, the new pane is closed again. |
| spawn (tab) | `tab create --workspace W --cwd C [--label L] --env K=V --no-focus` | `W` comes from `HERDR_WORKSPACE_ID` or `with_workspace`; without it the call is refused, because Herdr would use the focused workspace. |
| spawn (agent) | `agent start NAME --kind K --pane P [-- args]` | 45 s limit; Herdr waits up to 30 s for the agent to be ready. |
| read | `pane read P --source visible\|recent-unwrapped --lines N` | Plain text on stdout. |
| send text | `pane send-text P TEXT` | Paste mode wraps the text in `ESC[200~ … ESC[201~` and strips a closing marker from the text. |
| keys | `pane send-keys P K…` | One call; Herdr checks every key before writing. Names: `enter esc tab shift+tab up down left right backspace space ctrl+x f1…f12` and single characters (`backtab` is rejected). |
| close | `pane close P` | `pane_not_found` counts as closed. |
| pane info | `pane get P` | `terminal_id` tells a reused pane id apart; `pane_not_found` is a proven absence, any other failure an error. |
| agent state | `agent get P` | `idle working blocked done unknown`; `agent_not_found` (no recognised agent) is `unknown`. |
| prompt | `agent prompt P TEXT` | `agent_blocked` becomes `AwaitingInput`. |
| layout, resize, rename | `pane layout --pane P`, `pane resize`, `pane rename` | |

Every call has a time limit and an output cap. `HERDR_SESSION` adds
`--session`; `HERDR_BIN_PATH` (absolute) replaces `herdr` from `PATH`.
Caller location (`locate_caller`) is not implemented yet: reading
`HERDR_PANE_ID` is not proof ([brgr coverage](brgr-coverage/README.md#gaps-agentdrivers-current-design-does-not-yet-cover)).

Found while checking it live:

- Claude Code 2.1.319's folder-trust menu starts on **No, exit**; Enter alone
  quits the agent. The answer has to move to *Yes* first.
- A menu that has just been drawn can drop keys. Pressing only on a screen
  that has held still for 500 ms, one key at a time and re-reading the
  selection after each, passed 5 of 5 runs; pressing at once failed 2 of 3.

## Orca (Tier 1)

Orca's CLI covers the whole verb set:

- `worktree create --agent` — spawn an agent in a new worktree;
- `terminal split`, `terminal read --screen`, `terminal send`, `terminal close`;
- `terminal wait --for tui-idle` — host-side idle;
- agent hooks.

Orca also has its own orchestration layer. agentdriver uses only the terminal
surface and leaves Orca's orchestration alone.

## tmux (Tier 1)

- spawn: `split-window` / `new-window` with `-c <cwd>` and `-P -F '#{pane_id}'`;
- read: `capture-pane -p`;
- send: `send-keys -l` for literal text (or `load-buffer` + `paste-buffer`
  for long text), then `send-keys Enter`;
- close: `kill-pane`.

tmux knows nothing about agents, so idle comes only from the harness signal.
That path was verified live for all five Tier 1 harnesses
([harnesses](harnesses.md#verification-2026-11-08-tmux-macos)).
Adapters should use a dedicated socket (`-L`) in tests to stay out of the
user's server.

## cmux (Tier 2)

CLI: `new-split`, `read-screen`, `send`, `send-key`, `close-surface`,
`rename-tab`, plus hooks and an event stream. macOS only, which is why it sits
at Tier 2 despite a complete surface.

## T3 Code (Tier 2)

T3 Code is not pane-shaped. Its outside-agents MCP server exposes threads:

- `t3_thread_launch` (with `workspaceStrategy: worktree`),
- `t3_thread_wait`, `t3_thread_read`, `t3_thread_send`, `t3_thread_interrupt`.

The adapter maps `spawn → launch`, `wait → thread_wait`, `result → thread_read`,
`send → thread_send`, `keys(interrupt) → thread_interrupt`. There is no screen,
so prompt rules do not apply. The diff of the worktree is the caller's to
collect.

## Ovis (Tier 2)

[devswha/ovis](https://github.com/devswha/ovis) is a browser, desktop and
phone client for Herdr (formerly the Herdr web UI): chat and live terminal for
every agent pane, prompt answers from a phone, remote PCs over SSH. It runs an
API and WebSocket server (port 7317 by default).

Ovis sits on top of Herdr rather than beside it, so sessions agentdriver
starts through the Herdr adapter already show up in Ovis. A separate adapter
earns its place for what Herdr alone does not reach — Ovis's remote machines —
through its server API. Its exact control surface is still to be confirmed.
Ovis is also a candidate *consumer* of agentdriver.

## Ghostty (Experimental)

Ghostty 1.4's AppleScript dictionary can create windows, tabs and splits, run a
command, input text, send keys and report a surface's tty. It cannot read the
screen. An agent can still be driven when its answer arrives through a harness
hook, but start-up prompts cannot be detected. The tty also lets a caller find
the terminal its own user is in.

## Warp (Experimental)

Only undocumented `warpctrl` and `warp://` URI launch configurations. Launch
works; running a command in an existing pane, submitting input and reading
the screen do not.

## Plain-terminal attach (Experimental)

For a user with no multiplexer, agentdriver can run the harness under its own
PTY proxy and let the terminal attach to it. A spike of that proxy passed on
macOS and Linux. It is effectively a tiny multiplexer, so it stays
experimental until the Tier 1 hosts are solid.

## Zed (Unsupported)

No external control of Zed's terminal; the upstream PR that would have added
it (zed-industries/zed#45558) was closed unmerged.
