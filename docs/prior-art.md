# Prior art

Tools near agentdriver, and where the line is. Most of them are orchestrators
or interfaces built for one host; agentdriver is the layer they each rebuild
underneath.

## Orchestrators and workflow tools

| Tool | What it does | Relation |
|---|---|---|
| [brgr](https://github.com/justn-hyeok/brgr) | Cross-vendor agent workflows on Herdr: bounded runs, sealed results, durable inbox, owner decisions; debate, dynamic workflows, head migration | Being replaced: agentdriver takes over its driving and workflow layers ([decision 17](decisions.md#17-agentdriver-replaces-brgr)) |
| [mrrobot-ec/pod](https://github.com/mrrobot-ec/pod) | Herdr plugin delegating to Claude, Codex, Grok; durable acked messages; no daemon | Herdr-only driving layer of its own |
| [zqkra/cbds](https://github.com/zqkra/cbds) | Herdr dispatch with fs-watch `wait` and mandatory timeout | Same wait discipline as agentdriver |
| [husniadil/herdr-dispatch](https://github.com/husniadil/herdr-dispatch), [herdr-mail](https://github.com/husniadil/herdr-mail) | Worker pane per task; notify/ask/reply | Herdr-only |
| [junghan0611/entwurf](https://github.com/junghan0611/entwurf) | Cross-harness sibling sessions and messaging via ACP | Harness identity across vendors |
| T3 Code orchestrator MCP | `delegate_task`, `create_threads`, `schedule_task` inside T3 | T3-only; agentdriver drives T3 through its outside-agents MCP |
| Orca orchestration | Built-in agent orchestration in Orca | Orca-only; agentdriver uses only Orca's terminal CLI |

## Driving layers

| Tool | Status | Difference |
|---|---|---|
| [coder/agentapi](https://github.com/coder/agentapi) | archived | HTTP server wrapping one agent per process; agentdriver is daemon-less and drives agents inside existing hosts |
| ACP (Agent Client Protocol) | active | Structured protocol for harnesses that implement it; agentdriver drives the TUI itself, including harnesses without ACP |

## What agentdriver adds

- One API across several hosts, instead of one host per tool.
- Turn-finished signals per harness, verified, instead of screen heuristics.
- Prompt handling that is reported by default and answered only by opt-in.
- A published support matrix with release-blocking verification.
