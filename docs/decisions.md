# Decisions

Each entry records what was decided and why. Newer entries may supersede older
ones; superseded entries stay, marked as such.

## 1. A library, a CLI and an MCP server — no daemon, no HTTP API

The same verbs ship as a Rust crate, a CLI with JSON output, and an MCP server.
There is no long-running service.

**Why:** a daemon is one more thing to install, start, upgrade and debug on
every machine, and when it is missing or slow it blocks the agents that depend
on it. Session state on disk plus bounded waits give the same behaviour.
Other projects building on Herdr reached the same rule independently.

## 2. Agents stay native, visible TUIs

agentdriver drives the interactive TUI in a real pane. It does not switch the
harness to a headless mode (`-p`, `exec`) or redirect its output to a log.

**Why:** the person watching keeps the experience they chose the harness for —
they can read along, scroll, and take over. Headless modes also differ from
the TUI in features and behaviour per vendor.

## 3. Idle comes from the harness first, the screen last

Turn-finished signals come from harness hooks, notify commands or plugins;
then from the host's own agent state; screen heuristics are a fallback.

**Why:** a quiet screen is not a finished turn. Hooks fire exactly once at the
end of a turn (verified for all Tier 1 harnesses), while screen heuristics
misfire on long tool calls and pauses.

## 4. Prompt answers are opt-in

Start-up and mid-turn prompts (folder trust, hook trust, MCP trust, update
offers) are detected and reported as `waiting-for-input`. None are answered
unless the caller opts in by rule name. Folder trust can only ever be accepted
for the session's own spawn directory.

**Why:** accepting trust prompts is a security decision that belongs to the
caller. Tools that run unattended can opt in deliberately; a library should
not make that choice for every user.

## 5. Harness configuration is per launch or per workspace, never global

Signals are installed through launch flags or project-local files that the
adapter writes and removes on close.

**Why:** agentdriver must not change how the user's harness behaves outside
agentdriver sessions, and must leave nothing behind.

## 6. Guarantees cover Tier 1 × Tier 1 only

See [support tiers](support-tiers.md).

**Why:** the host × harness matrix grows multiplicatively. A small, fully
verified core is worth more than a large, partly working one.

## 7. Worktrees belong to the caller

agentdriver spawns in a directory it is given. Creating, listing and removing
git worktrees stays with the tool above it, even on hosts that offer worktree
commands.

**Why:** worktree policy (naming, cleanup, merge) is orchestration, and every
consumer has its own. Keeping it out keeps the port small.

## 8. Not an ADE

agentdriver has no UI and does not manage projects, threads or reviews.

**Why:** it is infrastructure for the ADEs, multiplexers and orchestrators
people already use. Competing with its own hosts would make it worse at being
driven into them.

## 9. A standalone project, not part of an orchestrator

agentdriver is its own project with its own users. It is not a component of
any orchestrator, and no orchestrator's needs define its API.

**Why:** driving agent TUIs across hosts and deciding what work agents do are
different problems. agentdriver is considered stable once independent
consumers run on it.
