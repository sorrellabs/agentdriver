# agentdriver

**WebDriver for coding-agent TUIs.** One API to spawn Claude Code, Codex,
Cursor, Devin, OpenCode and friends inside whatever terminal host you already
use — Herdr, Orca, tmux, cmux — then read their screen, send them input, wait
until they are idle, and close them.

```text
your tool (orchestrator, workflow engine, test harness, bot)
        │  spawn · send · keys · read · wait · result · close
        ▼
   agentdriver ── host adapter ──► Herdr │ Orca │ tmux │ cmux │ …
               └─ harness adapter ─► Claude Code │ Codex │ Cursor │ Devin │ OpenCode │ …
```

agentdriver is also replacing [brgr]: the delegated-work layer on top —
bounded runs, worktrees, sealed results, owner decisions and messages, model
routing — moves here, and brgr retires once it is covered
([decision 17](docs/decisions.md#17-agentdriver-replaces-brgr)).

> Status: design stage. This repository holds the design and the research it
> is based on; code lands milestone by milestone (see [roadmap](docs/roadmap.md)).

## Why

Everyone building on top of coding agents rewrites the same layer:

- start an agent TUI in a pane of *this particular* terminal host,
- type into it without breaking its input box,
- answer or refuse the prompts it shows on start (folder trust, updates),
- know when a turn has finished — not "the screen stopped changing", but done,
- collect what it answered, then tear the pane down.

Each host speaks a different CLI, and each harness signals "turn finished" in a
different way. agentdriver owns that matrix so the tool above it does not have
to.

agentdriver keeps agents as **native, visible TUIs**. It does not run
`claude -p` or `codex exec` behind your back; the person can watch, scroll and
take over any pane at any time.

## What it is not

- Not an agentic development environment (ADE). It has no UI of its own and
  does not compete with the hosts it drives.
- The driving layer is not an orchestrator: it never decides what work to
  give which agent. That is the workflow layer's job, built on the driving
  layer's public API.
- Not a daemon or server. Every call is a short process or a bounded wait.

## Support at a glance

| | Tier 1 (release-blocking) | Tier 2 | Experimental |
|---|---|---|---|
| **Hosts** | Herdr, Orca, tmux | cmux, T3 Code, Ovis | Ghostty, Warp, plain-terminal attach, WSL |
| **Harnesses** | Claude Code, Codex, Cursor, Devin, OpenCode | OMP, Cline, Command Code, Pi | Copilot CLI, Gemini / Antigravity |

Only Tier 1 host × Tier 1 harness combinations are guaranteed, and they are
verified on every release on a fresh Linux VM and on macOS. Details:
[support tiers](docs/support-tiers.md).

## Planned surfaces

- **Rust crate** — `agentdriver`, the core API.
- **CLI** — `agentdriver spawn --host tmux --harness codex …`, JSON output.
- **MCP server** — the same verbs for agents that orchestrate other agents.

## Documentation

- [Concepts](docs/concepts.md) — hosts, harnesses, sessions, the verbs, idle
- [Architecture](docs/architecture.md) — crates, ports, state, no-daemon waits
- [Support tiers](docs/support-tiers.md) — what is guaranteed and how it is verified
- [Hosts](docs/hosts.md) — what each terminal host exposes, adapter notes
- [Harnesses](docs/harnesses.md) — how each agent signals a finished turn
- [Decisions](docs/decisions.md) — design decisions and their reasons
- [Prior art](docs/prior-art.md) — related tools and how agentdriver differs
- [brgr coverage](docs/brgr-coverage/README.md) — checklist of everything brgr does, all of it in scope
- [Roadmap](docs/roadmap.md)

## Origin

agentdriver started as the layer under [brgr], a cross-vendor agent workflow
tool, and now replaces it: everything brgr does is in scope, and brgr retires
once agentdriver covers it.

## License

MIT

[brgr]: https://github.com/justn-hyeok/brgr
