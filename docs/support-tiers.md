# Support tiers

## What the tiers mean

| Tier | Promise |
|---|---|
| **Tier 1** | Release-blocking. Every release is verified end to end on a fresh Linux VM and on macOS. A failure blocks the release. |
| **Tier 2** | Built and tested in CI. Live-verified when the adapter changes, not on every release. Regressions are fixed, but do not block a release. |
| **Experimental** | Exists behind a flag. May be incomplete (for example, no screen read) and may break. |
| **Unsupported** | Not planned. The reason is recorded below. |

The guarantee covers **Tier 1 host × Tier 1 harness** combinations only.
A Tier 1 harness on a Tier 2 host carries the host's tier.

## Hosts

| Tier | Host | Notes |
|---|---|---|
| 1 | **Herdr** | Reference host; agent state available from the host. |
| 1 | **Orca** | Full CLI: worktree spawn, screen read, send, `wait --for tui-idle`, close. |
| 1 | **tmux** | Everywhere, including servers and VMs. Idle comes from harness signals. |
| 2 | cmux | Full CLI with hooks and events; macOS only. |
| 2 | T3 Code | Driven through its MCP thread tools, not panes. |
| 2 | Ovis | Herdr client; adapter targets its server API for remote machines. |
| Exp. | Ghostty | 1.4 AppleScript can open, type and press keys, but cannot read the screen. |
| Exp. | Warp | Only undocumented `warpctrl` and `warp://` launch configs. |
| Exp. | Plain-terminal attach | Needs agentdriver's own PTY proxy; spike passed on macOS and Linux. |
| Exp. | WSL | Linux path through Windows; untested. |
| — | Zed | No external control of its terminal. |
| — | Native Windows | No ConPTY work planned. |

## Harnesses

| Tier | Harness | Turn-finished signal |
|---|---|---|
| 1 | **Claude Code** | `Stop` hook via `--settings` |
| 1 | **Codex** | `notify` via `-c`, or project `hooks.json` `Stop` |
| 1 | **Cursor** | project `.cursor/hooks.json` `stop` / `afterAgentResponse` |
| 1 | **Devin** | project `.devin/config.json` `Stop` |
| 1 | **OpenCode** | V2 plugin, `session.execution.succeeded` event |
| 2 | OMP | to be measured; screen rules meanwhile |
| 2 | Cline | to be measured |
| 2 | Command Code | to be measured |
| 2 | Pi | to be measured |
| Exp. | Copilot CLI | screen rules only |
| Exp. | Gemini / Antigravity | screen rules only |

See [harnesses](harnesses.md) for how each signal was verified.

## Release verification

For each release:

1. Create a fresh Linux VM and use a clean macOS user environment.
2. Install agentdriver and every Tier 1 harness from scratch, following only
   the published setup docs. The setup experience is part of what is tested.
3. For every Tier 1 host × Tier 1 harness pair, run the conformance scenario:
   - `spawn` in a fresh workspace and pass the start-up prompts the caller
     opted in to;
   - a plain answer turn → `wait` reports idle once, `result` returns the answer;
   - a tool-using turn (a shell command that runs for several seconds) → no
     idle reported mid-turn;
   - a turn interrupted with `keys(Esc)` / `Ctrl-C`;
   - `close` leaves no pane and no process behind.
4. Record the evidence (versions, transcript, timings) under `docs/evidence/`.

## Moving between tiers

A harness or host moves **up** when it has an adapter, a conformance run that
passes on both platforms, and someone who uses it day to day. It moves **down**
when it fails two consecutive releases or its vendor removes the interface the
adapter depends on.
