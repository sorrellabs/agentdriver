# brgr coverage checklist

Everything brgr (v2.15.21) does. agentdriver replaces brgr, so all of it is in
scope ([decision 17](../decisions.md#17-agentdriver-replaces-brgr)).

An item is checked when it is implemented in agentdriver **and** the brgr test
it was ported with passes (plus a live check where the item is about a live
host). Items tagged **[brgr]** belong to the workflow layer; the rest to the
driving layer. Every item
cites the brgr source it was taken from (`file:line`), so a change in brgr
can be traced back to the checklist.

| File | Covers | Items |
|---|---|---|
| [Spawn, layout, input, completion](spawn-and-run.md) | host plumbing, the 20 Herdr commands, launch argv per harness, env, native TUI host, placement and equal heights, names and receipts, prompt delivery, report and seal, drive loop, session server, headless boundary, platforms, timing constants | 159 |
| [Screen rules](screen-rules.md) | every rule in evaluation order, path guards, shell rule, resolve step, rule log, deadlines and press budget, where the loop runs, ready-for-input, the tests and fixtures to port | 172 |
| [Owner identity and delivery](owner-and-delivery.md) | caller-pane resolution and verification, the Codex pending marker, owner surfaces, worker identity, `FROM BRGR` formats and transport, dispatcher, Codex owner hook, Herdr plugin and bridge, model and usage observation | 128 |
| [Closing, supervision, head migration](closing-and-head.md) | close ownership proof, native and OMP close paths, process stop, liveness and recovery, head handoff / take / retire / exit panes, usage signal and handoff condition, workflow panes, registry recipes and probes | 114 |

## Corrections to earlier assumptions

Reading the code changed four things the first draft assumed:

- **Completion is a report file plus `__seal-report`, not a harness hook.**
  The Claude `Stop` and Cursor `afterAgentResponse` hooks and the
  `BRGR_REPORT_BEGIN/END` markers only run for read-only launches, and
  admission always clears the permission to full. They stay in scope for old
  receipts. agentdriver's hook-based idle is an additional signal, not a
  replacement brgr already uses.
- **Workers are not started with `herdr agent start`.** Every built-in recipe
  is native-hosted: brgr runs its own `__tui-host` process in the pane with
  `pane run`. `agent start` is used only for head handoff and old manifests.
  agentdriver needs both start paths.
- **brgr writes two global configs**: Codex `hooks.json` (owner hook) and the
  Claude / Cursor status-line command (usage signal).
  [Decision 5](../decisions.md#5-harness-configuration-is-per-launch-or-per-workspace-never-global)
  allows these as explicit owner integrations.
- **Most `herdr` calls have no timeout today.** agentdriver gives every host
  call a timeout; brgr keeps its current behaviour by passing none where it
  has none.

## Gaps agentdriver's current design does not yet cover

These come from the checklist and need design work before M1:

- [ ] a **caller / owner identity** verb: process-tree walk to the host, screen trace of Codex panes, bridge receipts, the `session:` pending marker with a bounded wait
- [ ] **identity tuples** on sessions (pane id + terminal id + agent session) and an `adopt` that verifies them
- [ ] **layout policy** hooks: split direction by caller width, stack under the last worker, tab fallback, equal-height rebalancing, one opener per caller
- [ ] a **native TUI host** process mode next to "host starts a recognised agent"
- [ ] **delivery** that reports why a notice was not delivered (not idle, identity changed, timeout)
- [ ] **usage observation** per harness (status-line chain, Codex rollout)
- [ ] **host plugin context** (Herdr plugin env, pane open with entrypoints)
