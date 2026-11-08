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
not make that choice for every user. (brgr accepts these prompts by contract;
it will opt in explicitly.)

## 5. Harness configuration is per launch or per workspace, never global

Signals are installed through launch flags or project-local files that the
adapter writes and removes on close.

**Why:** agentdriver must not change how the user's harness behaves outside
agentdriver sessions, and must leave nothing behind.

**Exception — owner integrations.** Some signals only exist globally: the
Codex `hooks.json` an owner session reads, and the Claude Code / Cursor
status-line command that reports usage. agentdriver installs these only
through an explicit `integrate` call, chains any existing entry instead of
replacing it, keeps a backup, and can remove exactly what it added.

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

## 9. Separate from brgr, and covers everything brgr needs

agentdriver is its own project: driving agent TUIs across hosts is a different
job from brgr's agent workflows. brgr runs on agentdriver, so every host and
harness capability brgr uses today is in scope — the Herdr commands it calls,
its screen rules, its turn signals and result capture.

**Why:** the two are different products, but brgr is the most demanding real
user of this layer. Covering it fully keeps the API honest. agentdriver is
considered stable once a second, independent consumer also runs on it.

## 10. Synchronous core

The library is blocking: `std::process`, bounded waits, one polling loop for
several sessions. Only the MCP server runs `tokio`, calling the core through
`spawn_blocking`.

**Why:** every operation is a short external command or a bounded wait on a
file or a screen, so async buys little. A blocking core does not force a
runtime on library users, starts fast enough to run inside a harness hook,
and matches brgr's pane layer, which is blocking today.

## 11. One package

The library, the `agentdriver` CLI and the MCP server (`agentdriver mcp`) ship
as one crates.io package with `cli` and `mcp` default features and one feature
per host.

**Why:** users get one name to install (`cargo install agentdriver`) and one
to depend on (`default-features = false`). More crates add release work and
are worth it only when a user needs one part without the rest.

## 12. Versioning

0.x SemVer from `0.1.0`; patch by default; minor only for a breaking change to
the Rust API, CLI JSON or MCP schemas; 1.0 only when the owner says so. Wire
formats carry their own schema version.

**Why:** Cargo treats a 0.x minor as breaking, so breaking changes must be
minors or every dependent breaks on `cargo update`. Keeping patch as the
default keeps the number honest about compatibility rather than progress.

## 13. Pinned toolchain

`rust-toolchain.toml` pins Rust 1.95.0, the same as brgr; `rust-version` is
the MSRV. Both move together in a dedicated change.

**Why:** every machine and CI job builds with the same compiler, so a new
stable release cannot surprise a build with new lints or behaviour. Matching
brgr means one toolchain installed for both projects.

## 14. Hosts are driven through their CLIs

Each host adapter shells out to the host's own CLI through `exec.rs`. No host
client crates, and no direct socket connections for now.

**Why:** Herdr's documentation recommends the CLI for most automation, and some
behaviour exists only there, such as retrying while a shell starts. Its protocol
number is at 22 and refuses mismatches, so a socket client would track a moving
schema. tmux needs about eight commands, less than the `tmux_interface` crate
costs. The transport stays private to each adapter, so Herdr's socket can be
added later for event subscriptions without changing the `Host` trait.

## 15. Match screens, not streams

Conditions run against a `Screen` snapshot, never an output stream that is
consumed as it is read.

**Why:** expect-style libraries assume output that scrolls by. Claude Code and
Codex repaint the whole screen, so a stream does not describe what is visible.

## 16. Hexagonal, enforced by a test

The crate is split into `domain`, `ports`, `core` and `adapters`, with dependencies
pointing inward. Hosts, harnesses, storage and time all sit behind ports, and
`HostKind` and `HarnessKind` are open names. `tests/architecture.rs` scans the
source and fails when a layer crosses the boundary.

**Why:** the point of agentdriver is that a new host or harness is a new adapter
and nothing else, including one written outside this crate. Time and storage are
ports because brgr's three-minute deadline must be testable without waiting, and
brgr must choose where its logs go. The layers are folders rather than crates
([decision 11](#11-one-package)), so a test does what the compiler would.
`exec.rs` is not a port: only adapters use it.
