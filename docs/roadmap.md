# Roadmap

Milestones are ordered; each one ships something usable.

## M0 — Design (now)

- Concepts, architecture, support tiers, host and harness research, decisions.
- Exit: the `Host` and `Harness` ports are reviewed against brgr's current
  Herdr usage and against one outside consumer's needs.

## M1 — Core + Herdr

- `agentdriver` crate: session state, verbs, wait loop, signal store, prompt
  policy.
- Herdr host adapter, extracted from brgr.
- Tier 1 harness adapters with their signals and brgr's screen rules.
- Exit: every driving-layer item (not tagged **[brgr]**) in the
  [brgr coverage checklist](brgr-coverage/README.md) is checked.

## M1.5 — Workflow layer, brgr retired

- brgr's delegated runs on agentdriver: worktrees, sealed results and owner
  decisions, owner messages, model routing, head migration
  ([decision 17](decisions.md#17-agentdriver-replaces-brgr)).
- Exit: every **[brgr]** item is checked, and the owner's daily brgr work runs
  on agentdriver instead.

## M2 — tmux and Linux

- tmux host adapter.
- Conformance scenario automated; runs on a fresh Linux VM and on macOS.
- Covers what the first verification did not: long and stalled turns,
  mid-turn questions, failed and interrupted turns.
- Exit: Herdr and tmux × the five Tier 1 harnesses pass on both platforms.

## M3 — Orca

- Orca host adapter, using its host-side idle where available.
- Exit: Orca joins the Tier 1 conformance matrix.

## M4 — CLI, MCP, second consumer

- `agentdriver` CLI with JSON output; MCP server with the same verbs.
- Publish to crates.io.
- Exit: one consumer besides agentdriver's own workflow layer runs on it.

## M5 — Tier 2 hosts and harnesses

- cmux, T3 Code (thread adapter), Ovis.
- Signals measured for OMP, Cline, Command Code, Pi.

## Later — experimental

- Ghostty (no screen read), Warp, plain-terminal attach through the PTY proxy,
  WSL.

## Not planned

- Zed (no external terminal control), native Windows.
