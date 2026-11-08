# Roadmap

Milestones are ordered; each one ships something usable.

## M0 — Design (now)

- Concepts, architecture, support tiers, host and harness research, decisions.
- Exit: the `Host` and `Harness` ports are reviewed against the Tier 1 hosts'
  real command surfaces.

## M1 — Core + Herdr

- `agentdriver` crate: session state, verbs, wait loop, signal store, prompt
  policy.
- Herdr host adapter.
- Tier 1 harness adapters with their signals and screen rules.
- Exit: the five Tier 1 harnesses pass the conformance scenario on Herdr.

## M2 — tmux and Linux

- tmux host adapter.
- Conformance scenario automated; runs on a fresh Linux VM and on macOS.
- Covers what the first verification did not: long and stalled turns,
  mid-turn questions, failed and interrupted turns.
- Exit: Herdr and tmux × the five Tier 1 harnesses pass on both platforms.

## M3 — Orca

- Orca host adapter, using its host-side idle where available.
- Exit: Orca joins the Tier 1 conformance matrix.

## M4 — CLI, MCP, first release

- `agentdriver` CLI with JSON output; MCP server with the same verbs.
- Publish to crates.io.
- Exit: an outside project runs on agentdriver.

## M5 — Tier 2 hosts and harnesses

- cmux, T3 Code (thread adapter), Ovis.
- Signals measured for OMP, Cline, Command Code, Pi.

## Later — experimental

- Ghostty (no screen read), Warp, plain-terminal attach through the PTY proxy,
  WSL.

## Not planned

- Zed (no external terminal control), native Windows.
