# brgr coverage checklist

Everything brgr (v2.15.21) needs from the host and harness layer. agentdriver
covers all of it before brgr can drive Herdr only through agentdriver
([decision 9](decisions.md#9-separate-from-brgr-and-covers-everything-brgr-needs)).

Each item is checked when it is implemented in agentdriver **and** brgr's own
tests and live checks pass on it. brgr-only features (result contract,
messages, debate, head records, workflows, worktrees, harness registration)
are listed at the end for the boundary, not as agentdriver work.

## Spawning and layout

- [ ] Spawn a harness's native TUI in a new pane in a given directory, with argv and env
- [ ] First worker splits the owner's pane; later workers stack beneath it at equal heights, the owner keeps its half
- [ ] Re-balance the worker column on demand (brgr does it on every command)
- [ ] Name a worker pane from a template (`set-worker-placement`, pane name template)
- [ ] Headless one-shot mode stays in brgr (`--headless`); agentdriver only needs to not get in the way

## Owner identity

- [ ] Resolve the calling owner's pane and verify its session (`--source-pane`, `--as SESSION`)
- [ ] Find the calling Codex pane when Herdr has not yet reported its session id
- [ ] Look up a pane's process (`pane process-info`) to confirm what runs in it

## Screen and input

- [ ] Read a pane's visible screen
- [ ] Send text to a pane (`send-text`) and named keys (`send-keys`)
- [ ] Deliver a prompt to a live agent through the host's agent-prompt path (`agent prompt`)
- [ ] `brgr input TASK --key/--text`: pass caller input to a worker TUI unchanged

## Screen rules (opt-in, all enabled by brgr)

- [ ] Workspace / folder trust — Claude, Codex, Copilot, Cursor; only for the spawn directory
- [ ] Codex hooks trust (`codex-hooks-trust`)
- [ ] New MCP server trust (`mcp-trust`)
- [ ] Claude Code bypass-permissions warning
- [ ] "Press Enter to continue" notices
- [ ] Update offers, including shell update offers — skipped, never accepted
- [ ] Unrecognised screen: report its text; caller-defined deadline (brgr fails after three minutes)
- [ ] Rule log: which rule fired and which keys were sent, per pane

## Turn signals and results

- [ ] Codex lifecycle hooks (`SessionStart`, `UserPromptSubmit`, `Stop`) via a project `hooks.json`
- [ ] Claude Code `Stop` hook with `last_assistant_message`
- [ ] Cursor `afterAgentResponse` via `.cursor/hooks.json`
- [ ] Devin, OpenCode signals (Tier 1 in agentdriver; brgr gains them)
- [ ] Report capture between `BRGR_REPORT_BEGIN` / `BRGR_REPORT_END` markers — marker strings supplied by the caller
- [ ] Host agent state (`agent get`, `agent list`) as an idle source
- [ ] Signal files removed on close; nothing global changed

## Delivery to the owner

- [ ] Type a notice (`FROM BRGR …`) into the owner's pane without breaking its input box
- [ ] Report whether the notice was delivered, so the caller can retry or repeat it

## Closing

- [ ] Close a worker pane after a decision
- [ ] Retry a close that failed, for a pane recorded as owned (`brgr cleanup`)
- [ ] Never close a pane the caller does not own

## Head migration (pane parts only)

- [ ] Open a new head harness in a pane beside the current head (`head handoff`)
- [ ] Offer the incoming head its command through the agent-prompt path (`head take`)
- [ ] Close the outgoing head's pane, with retry (`head exit`)
- [ ] Read the harness's reported plan usage, for `set-handoff-condition` (Codex first)

## Herdr plugin

- [ ] Launch a Codex pane wired to a given binary and host bridge (`plugin codex`)
- [ ] Respect a configured Codex executable path when the Herdr server's `PATH` lacks it
- [ ] Task board and workspace actions stay in brgr's plugin

## Harness tiers

- [ ] Every harness brgr supports has an agentdriver adapter: Claude Code, Codex, Cursor, Devin (brgr Tier 1); Command Code, OMP, OpenCode, Cline (Tier 2); Copilot CLI, Gemini CLI, Pi (Tier 3)
- [ ] Launch flags for full-permission mode per harness (`--yolo`, `--dangerously-skip-permissions`, equivalents) — chosen by the caller

## Platforms

- [ ] macOS 15+ on Apple Silicon
- [ ] x86_64 Linux (Ubuntu 24.04)

## Stays in brgr (boundary, not agentdriver work)

- Result contract: `run` admission, sealing, `status`, `result`, `artifact`, `wait`, `watch`, `accept`, `reject`, `diff`, `apply`, `land`, `revise`, `bind`
- Failure notices and `errors` / issue reporting
- `message`, recursive workers, `debate`
- Head migration records: `head request/prepare/show/status/commit/abort/retire/claim`
- Dynamic Workflows: `workflow *`, the JS SDK
- Worktrees and `prune`
- Harness registration and certification: `harness *`, `doctor`, `config`
- `integrate claude/codex`, Claude Code mod and marketplace, plugin task board
