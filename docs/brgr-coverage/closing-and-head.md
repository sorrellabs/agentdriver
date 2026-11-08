# brgr coverage — closing, supervision, head migration, usage, workflows, recipes

Source: brgr v2.15.21. Paths relative to `brgr/crates/` unless noted.
**[agentdriver]** = primitive agentdriver provides; **[brgr]** = stays in brgr, listed for the boundary.

## 0. Host transports in this area

| Transport | Binary | `--session` | Timeout | Failure parsing |
|---|---|---|---|---|
| [ ] `Herdr::call_owned` (`brgr-cli/src/pane_adapter.rs:1109-1144`) | `HERDR_BIN_PATH` if absolute, else `herdr` | if non-empty | none | blank stdout → null; stdout JSON; stderr `{"error":{"code","message"}}` → code, else text |
| [ ] `Herdr::screen` (`pane_adapter.rs:1092-1107`) | same | same | none | `pane read <p> --source visible --lines 300`, raw, none on failure |
| [ ] `caller_pane::herdr_output` (`brgr-cli/src/caller_pane.rs:320-359`) | `HERDR_BIN_PATH` as given, else `PATH`, canonical | always | 2000 ms / 200 ms in hooks, then kill | exit ≠ 0 or stdout > 1 MiB → none |
| [ ] raw `Command` in `pane_cleanup.rs`, `omp_adapter.rs` | `caller_pane::binary()` | **never** | none | absence = stderr `/error/code == "pane_not_found"` (`pane_cleanup.rs:293-306`) |
| [ ] `notification.rs` async | recorded binary | recorded session | 5 s | — |

**[agentdriver]** one host client with: binary + session pinned per session record, per-call timeout and output cap, typed error codes, and an explicit "absent" result only on a not-found code.

## 1. Closing native TUI panes (`runs/<task>-r<rev>.pane.json`, `brgr-cli/src/pane_adapter/lifecycle.rs`)

### 1.1 Ownership proof before any close — [agentdriver]
- [ ] Receipt task + revision match and not `closed`; store attempt maps back to task + revision; `terminal` recorded (`lifecycle.rs:210-243`)
- [ ] `pane get` returns the same `pane_id` **and** `terminal_id`; mismatch = pane id reused → `task terminal identity changed`, record `cleanup=pending`, **do not close** (test `close_checks_terminal_identity_and_retries_without_discarding_the_report`, `lifecycle/tests.rs:195`)
- [ ] Absent only on `pane_not_found`; any other error is "unknown, retry", never closed
- [ ] Receipt binary and session reused for every later call; receipts never deleted

### 1.2 `close_leftover_pane` (`lifecycle.rs:614-666`) — [agentdriver] `close(session)`
- [ ] skip if `keep_pane` or already closed
- [ ] absent → `closed`; identity error → `pending`
- [ ] **before closing**, SIGTERM every descendant of the native host pid (`ps -A -o pid=,ppid=` → `kill -TERM`), only while the host is alive (`phase=running` and `lstart` birth matches) (`tui_host.rs:174-217`) — after the pane is gone the descendants can't be found
- [ ] set `closing` → `pane close` → success: `closed` + rebalance the caller's column; failure: `pending` + `cleanup_error="Herdr close failed; retry retained"`
- [ ] no poll for absence after close on this path

### 1.3 When native panes close — [brgr] triggers, [agentdriver] `close`
- [ ] run ends without a candidate (failure, lost, cancel, error): supervisor `close_leftover_pane(…, keep_pane)` (`brgr-cli/src/supervision.rs:139-145`)
- [ ] runner fails with no report and no `--keep-pane` (`pane_adapter.rs:414-416`)
- [ ] accept / reject → `cleanup_settled` (`task_commands.rs:838-840`); `result --ack` (`:465`)
- [ ] session server `__session` (`native.rs:104-160`): cancellation → `stop_cancelled`; settled → `cleanup_settled` every 1 s until closed or kept; `reconcile_pending` every 5th tick
- [ ] `reconcile_pending` scans every `runs/*.pane.json` (`supervision.rs:320-342`)
- [ ] `cancel`: native nodes not just sent a cancel file → `close_leftover_pane`; nodes sent one are left to their supervisor (`task_commands.rs:617-629`); `cancel --force` on a terminal task → close (`:686-700`)
- [ ] `stop_external` (`lifecycle.rs:108-195`): deadline = first attempt clock + `deadline_seconds`, or cancellation; `keep_pane` → `kill -TERM <host pid>`, pane kept; else close; commit `Cancelled` ("external session cancelled") or `Failed` ("external session exceeded its original deadline")

### 1.4 `cleanup_settled` (`lifecycle.rs:353-423`) — [agentdriver] `close_when_idle`
- [ ] only for pane-mode launches without `keep_pane`, once the revision is not `Open`
- [ ] closed already → archive report, reclaim worktree; absent → mark closed, archive, reclaim
- [ ] present → close **only when ready/idle**: native host → `native_ready`; else `agent get <name>` idle/done (an `agent get` error counts as closable)
- [ ] while the agent works it returns Ok and leaves the pane
- [ ] **[brgr]** archive `.brgr/tasks/<task>-r<rev>/report.md` → `runs/<task>-r<rev>.report.md` after digest check; tolerant of a concurrent second cleaner (`lifecycle.rs:511-603`)

### 1.5 `brgr cleanup status|run TASK` (`brgr-cli/src/cli.rs:262-265,725-730`, `harness_commands.rs:597-621`)
- [ ] owner required; native receipt → `cleanup_settled`, print `{pane, terminal, attempt, phase, cleanup, cleanup_error}`; else OMP path (§2)

## 2. Closing legacy OMP panes (`runs/<task>.pane.json`, `brgr-cli/src/pane_cleanup.rs`)

- [ ] Receipt `task_id, attempt_id, owner_id, agent, pane_id, parent_pane_id, terminal_id, session_value, launcher_receipt, keep_pane, state, result_id, result_digest`; states `retained → cleanup_pending → eligible → closing → closed` (`:19-44`)
- [ ] Host calls: `agent get <name>` (`name`, `agent=="omp"`, `pane_id`, `terminal_id`, `agent_session.value`, `agent_status`); `pane get <p>` (`pane_id`, `terminal_id`, `agent=="omp"`, `tab_id`); `tab get <tab>` (`/result/tab/label`, `lobby` protected); non-zero → `Herdr cannot identify {kind} {id}` (`:350-378`)
- [ ] `record_spawn` (`:53-111`): launcher receipt agent == name, `codex_prompt_marked == false`; live name matches; one unfinished attempt; `agent_session.value` present; child ≠ parent; `HERDR_PANE_ID == parent_pane`
- [ ] `mark_pending` on accept / reject / ack with `result_id` + digest (`:113-132`, `task_commands.rs:458,853`)
- [ ] eligibility (`:134-172,308-344`): decision complete (owner, attempt, result, digest, inbox acknowledged; candidate needs a stored decision); live agent matches name, kind, pane, terminal, session; `pane get` matches; tab not `lobby`; status idle/done. Reasons: `cleanup_pending_decision_or_ack`, `retained_identity_changed`, `retained_pane_changed`, `retained_protected_lobby`, `retained_agent_<state>`
- [ ] `close_if_eligible` (`:178-236`): `closing` + absent → closed; only the **parent pane** may close (`HERDR_ENV=1`, `HERDR_PANE_ID == parent_pane_id`) else `cleanup_pending_parent_unavailable`; write `eligible` → `closing` → re-check (no conditional close in Herdr 0.9.0, `:174-177`); `pane close`; poll absence 10 × 50 ms; still there → `cleanup_pending` / `cleanup_pending_close_unconfirmed`
- [ ] `cancel_owned_pane` (`:241-291`): no decision or idle needed; same identity checks; close + 10 × 50 ms; failed close never records `closed` (test `:790-855`)
- [ ] **[agentdriver]** `adopt` must carry an identity tuple (pane, terminal, agent session) and a "protected tab" check

## 3. Process stop semantics

- [ ] **[brgr]** headless runner: own process group; `kill -TERM -- -<pgid>`, ≤ 500 ms, `kill -KILL -- -<pgid>` even if the leader exited; pid file written and removed (`brgr-runner/src/lib.rs:250-266,558-598`)
- [ ] **[brgr]** workflow driver: `kill -TERM -- -<pid>`, 50 ms, `kill -KILL -- -<pid>` (`brgr-cli/src/workflow/driver.rs:384-397`)
- [ ] **[agentdriver]** native host: TERM descendants before pane close; `keep_pane` → TERM host pid only (`tui_host.rs:189-217`, `lifecycle.rs:144-154`)
- [ ] Codex daemon tools are unreachable (`tui_host.rs:186-188`) — documented limitation

## 4. Supervision and liveness

- [ ] **[brgr]** supervisor identity `ps -ww -p PID -o lstart=` + `comm=`, executable basename `brgr`; receipt written only after `reconcile_after_restart` (`supervision.rs:119-126,513-543`)
- [ ] **[brgr]** `observe_attempt` (`:450-471`): missing receipt → external observation; mismatch / bad pid → `Unknown`; identity equal → `SupervisorAlive`; differs → `NotObserved`; `ps` fails → external
- [ ] **[agentdriver]** `alive(session)` as used by `external_alive` (`lifecycle.rs:253-307`): matching receipt not closed; then sealed report digest matches → alive; native host alive → `pane get` identity; else `agent get <name>` pane + terminal match
- [ ] **[brgr]** recovery (`supervision.rs:252-344`): per unfinished attempt not alive, `try_lock runs/<task>.recover.lock`, `stop_external` or `collect_recovered` (digest check → `recover_external_report`); per-attempt errors printed, others continue
- [ ] **[brgr]** lost detection (`:296-401`): claim grace 5 s (supervisor) vs **1 min** (`claimant: worker_pane`) — a slow host otherwise records the task lost; stale if workspace missing, launch file missing/not regular, or receipt missing/mismatched/not live after grace; `Cancelled` or `Lost` ("task worktree is missing; refusing to recreate or retry automatically" / "supervisor did not claim the admitted task"), `unresolved_effects=["execution identity was not established"]`
- [ ] **[brgr]** `worker_may_be_running` (`:489-511`); wait reconcile at most every 5 s (`:25-55`)
- [ ] **[agentdriver]** runner forwards host context to workers only when `HERDR_ENV=1`, `HERDR_PLUGIN_ID=brgr`, `HERDR_PANE_ID` set, then sets `BRGR_WORKER_HERDR_CONTEXT=1` (`brgr-runner/src/lib.rs:438-456`)

## 5. Head migration panes

### 5.1 `head handoff [TASK] --kind K --goal G [--notes N] [--exit-old] [--agent-arg=X]…` (`brgr-cli/src/head.rs:77-96,749-841`)
- [ ] **[brgr]** preconditions: owner from env (bound if needed, revision 0 without TASK, `:677-687`); caller holds the head; no open takeover (`:759-765`)
- [ ] **[agentdriver]** caller pane must be `verified()`; Codex owners get the "run one short `brgr --as SESSION status --brief` first" hint (`:737-746`)
- [ ] **[brgr]** offer `$BRGR_HOME/handoffs/offer-<uuid>.json` (key `[A-Za-z0-9-]+`, `:653-658`) with `from_pane`, `from_terminal` (`pane get`), `exit_old`, `goal`, `notes`, `herdr_bin`, `herdr_session` (`:773-790`)
- [ ] **[agentdriver]** placement via `open_pane` (`pane_adapter.rs:1149-1235`, `layout.rs:131-150`): tab config → `tab create --workspace <caller prefix> --label head --cwd <cwd> --no-focus`; else layout: no workers → split caller right (down if < 100 cols); workers → split the last worker down; < 10 rows per pane → tab; unreadable layout → split caller right
- [ ] head pane writes **no worker receipt** → not counted in layout or balancing
- [ ] cwd = process cwd (`head.rs:797`)
- [ ] env: copy `BRGR_HOME BRGR_BIN BRGR_PARENT_TASK_ID BRGR_PARENT_ATTEMPT_ID`; override `BRGR_SESSION_ID=""`, `BRGR_OWNER_ID=<owner>`, `BRGR_HOME=<home>` (`head.rs:808-812`)
- [ ] `pane rename <p> head`, best effort (`pane_adapter.rs:1239-1245`)

### 5.2 Starting the incoming agent (`brgr-cli/src/pane_adapter/head_pane.rs:46-98`)
- [ ] **[brgr]** `on_open` writes `incoming_pane` + `incoming_terminal` into the offer **before** the prompt (`:38-41,56`)
- [ ] **[agentdriver]** shell settle (oh-my-zsh `n`, 5 s / 100 ms, 2 unchanged polls)
- [ ] **[agentdriver]** `agent start head-<task8> --kind <K> --pane <p> --timeout 60000 -- <agent-args verbatim>` — no recipe argv, no permission flags; Claude gets `--session-id $BRGR_PARENT_ATTEMPT_ID` if set (`pane_adapter.rs:444-448`); `agent_pane_busy` retry 250 ms / 20 s; `agent_not_ready` or `timeout` with `agent get` OK → settle
- [ ] **[agentdriver]** settle every 1 s with screen rules (unknown screen → notice once, fail at 3 min) until idle/done (`pane_adapter.rs:478-516`)
- [ ] **[agentdriver]** wait `native_ready(…, alive=false)` every 1 s up to **2 min** (`READY`, `head_pane.rs:21,77-86`): no numbered menu row, and idle/done or the Codex screen override
- [ ] **[agentdriver]** `agent prompt head-<task8> <prompt>`
- [ ] any failure in start → prompt: close the pane (result ignored) and delete the offer (`head_pane.rs:92-96`, `head.rs:827-833`)
- [ ] **[brgr]** prompt text (`head.rs:689-731`): every command prefixed `env -u BRGR_SESSION_ID -u BRGR_PARENT_TASK_ID -u BRGR_PARENT_ATTEMPT_ID BRGR_OWNER_ID='<owner>' BRGR_HOME='<home>' <brgr>` (Codex daemon env); steps `head take --offer`, `status`, goal, `head retire`, `head exit` when the old head is idle; quoting `'` → `'\''`
- [ ] **[brgr]** receipt `{offer, incoming_pane, incoming_terminal, exit_old, next}`; the binding does not move yet (test `head_handoff.rs:216-238`)

### 5.3 `head take [TASK] --offer KEY` (`head.rs:898-962`)
- [ ] **[brgr]** owner match; refuse a moved head; `claim_core` idempotent by key (same session → same commit; aborted needs a new key; outgoing can't take) (`:406-485`); departure record `$BRGR_HOME/handoffs/<takeover>.json` `{from_pane, from_terminal, exit_old, closed:false}` once
- [ ] **[agentdriver]** `tell_head(from_pane, text)`: prompt only if idle/done, else report not delivered (`head_pane.rs:116-129`)
- [ ] **[agentdriver]** register the **offered** pane as the new head's surface without screen tracing: terminal id matches the offer, live kind is an inbox kind, pane matches; Codex `report-agent-session` if none (`notification.rs:60-169,373-376,277-288`; test `head_handoff.rs:535-557`)

### 5.4 `head retire` / `head exit` (`head.rs:518-557,966-1021`)
- [ ] **[brgr]** retire: incoming only; from `Committed` try `resume_idle_takeover` (`TakeoverHasOpenWork` refuses); success → `close_if_requested`
- [ ] **[brgr]** `close_if_requested`: none unless `exit_old`; closed already → `closed:true`; else `close_outgoing`; error → `retry with brgr head exit <id>`, `{closed:false, pane, reason}`
- [ ] **[agentdriver]** `close_outgoing(from_pane, from_terminal, keep=current pane)` (`head_pane.rs:133-157`): refuse if it is the incoming pane, `pane get` fails, terminal differs, or status not idle/done; then `pane close`; no absence poll
- [ ] **[brgr]** exit: incoming only, state `Retired`, `exit_old` set → re-run `close_if_requested` (test `head_handoff.rs:373-417`)

### 5.5 Auto handoff (`head.rs:845-879`)
- [ ] **[brgr]** same `open_handoff` with the first owner task in `tasks(100)`; goal "Continue this owner's open work. The handoff started automatically because the previous head's <reasons>."

## 6. Usage signal and handoff condition

### 6.1 Sources — [agentdriver] harness "usage" observation
- [ ] **Claude Code, Cursor — status line**: install `brgr --home H __context [--source cursor] [-- <chained>]` into `~/.claude/settings.json` or `$CURSOR_CONFIG_DIR|~/.cursor/cli-config.json` (`brgr-cli/src/statusline_install.rs:25-58,422-431`) — another **global** config write
- [ ] parse stdin ≤ 1 MiB: `session_id`, `context_window.{used_percentage,context_window_size}`, `rate_limits.five_hour|seven_day.{used_percentage,resets_at}`; percents finite 0–100 else unknown (`context_signal.rs:88-106`)
- [ ] pipe the same stdin to the chained status line (`~/` expanded); without a chain print `ctx N% · 5h N% · 7d N%` (`:251-278`); the chain keeps working even if recording fails
- [ ] **Codex — rollout**: on each owner hook with `transcript_path` (`hook.rs:42-44`), last 256 KiB; last `event_msg`/`token_count` with object `info`: context = `last_token_usage.total_tokens / model_context_window` (basis points, ≤ 100); `rate_limits.primary|secondary.{used_percent,resets_at}`, `window_minutes > 1440` = seven-day (`context_signal.rs:162-237`)
- [ ] store `$BRGR_HOME/context/<session>.json` `{session, source, at, …}`, dir 0700, ids `[A-Za-z0-9_-]{1,128}` (`:65-71,108-114`)

### 6.2 Condition — [brgr] (`brgr-cli/src/handoff_condition.rs`)
- [ ] `MAX_AGE` 30 min, `REPEAT_AFTER` 30 min, `REARM_GAP` 15 points (`:26-30`)
- [ ] armed only with `kind` and `quota_percent` (`config.rs:150-152`); `context_percent` read and dropped (`:131-134`)
- [ ] value = higher of five-hour / seven-day, named by window (`context_signal.rs:47-55`); over when ≥ limit; re-arm below limit − 15; **unknown never re-arms or triggers** (`:53-78`)
- [ ] no note if target kind == head's own harness or signal stale (`:121-123`); target at its own limit → note without command (`:127-146`)
- [ ] evaluated at the end of every non-internal command, after the failure banner, before rebalancing (`main.rs:280-298`); only if config loads, session exists and is bound, no open handoff, fresh signal
- [ ] note state `context/<session>.note.json` written **before** an auto handoff (no double start); `auto` failure prints the note + error (`:189-272`)

## 7. Workflow-driven panes (`brgr-cli/src/workflow/driver.rs`)

- [ ] **[brgr]** driver never opens panes; `workflow run|resume` → `bind_source_pane` (`:454-464`): `verified()` or the previously recorded pane if still a live owner pane → `runs/<run>.workflow.pane` (0600, atomic); none → remove file, print `no verified Herdr pane: workflow steps must use mode headless` (`:44`), run continues
- [ ] **[brgr]** detached driver + node with cleared env; allowed `PATH HOME BRGR_HOME BRGR_OWNER_ID BRGR_SESSION_ID BRGR_WORKFLOW_SOURCE_PANE CODEX_THREAD_ID CLAUDE_CODE_SESSION_ID CURSOR_CONVERSATION_ID HERDR_*`; set `BRGR_HOME BRGR_WORKFLOW_RUN BRGR_BIN` owner, session, source pane (`:32-42,299-335`)
- [ ] **[brgr]** SDK prepends `--source-pane=<pane>` unless `mode === "headless"` or no pane (`brgr-cli/assets/brgr-workflow.mjs:189-260`; test `workflow_pane.rs:567-614`)
- [ ] **[agentdriver]** `caller(named=P)` accepts a recorded pane only inside a driver (`BRGR_WORKFLOW_RUN`, matching record, live inbox pane); an alive pane outside a driver is refused (test `workflow_pane.rs:640-661`)
- [ ] **[brgr]** no silent fallback from pane failure to headless
- [ ] **[brgr]** driver liveness: `runs/<run>.workflow.pid` `{pid, birth}`; birth matches, `kill -0`, cmdline contains `workflow __drive`; legacy bare-pid files honoured; cancel stops the group then cancels unfinished steps (`:144-219,337-362`)

## 8. Registry recipes and probes

- [ ] **[agentdriver]** per-harness launch data (table in [spawn and run §3.3](spawn-and-run.md#33-per-harness-launch-recipe)) plus:
  - [ ] `${input.prompt}` is one argv value; a leading space is added if it starts with `-` or `@` (`brgr-runner/src/argv.rs:37-47`)
  - [ ] a model / effort flag is offered only if the help documents it as a whole token; `own_process` appended if documented (`brgr-registry/src/recipe.rs:851-858`)
  - [ ] `interactive.native_host = true` for all (`:794`); result `max_bytes` 1 MiB (`:803`)
  - [ ] env extras: Claude `USER ANTHROPIC_API_KEY`; Codex `OPENAI_API_KEY CODEX_API_KEY CODEX_HOME USER`; Command Code `COMMAND_CODE_API_KEY`; Copilot `GH_TOKEN GITHUB_TOKEN COPILOT_GITHUB_TOKEN USER`; Gemini Google vars; Pi provider keys
- [ ] **[brgr]** one-shot (headless) argv per harness, e.g. Claude `-p --output-format text --no-session-persistence`, Codex `exec --skip-git-repo-check --ephemeral --color never`, Cursor `--print --output-format text --trust --workspace <ws>`, Devin `--prompt-file <f> -p`, OpenCode `run` (help read from stderr when stdout empty, `brgr-registry/src/lib.rs:1152-1158`)
- [ ] **[brgr]** tier table `brgr-cli/src/config_edit.rs:8-21` (test forces a tier for every recipe, `:467-484`):
  - Tier 1 Claude Code, Codex, Cursor CLI, Devin
  - Tier 2 Command Code, OMP, OMP-role, OpenCode, Cline
  - Tier 3 Copilot, Gemini, Pi
- [ ] **[brgr]** omp-role: adapter `OMP_ROLE_ADAPTER_V1`, env with `HERDR_ENV HERDR_PANE_ID`, registered without a scratch run as `presentation-probe/v1` (`lib.rs:227-236,827-832`)
- [ ] **[brgr]** probes: `--version`/help 15 s, 64 KiB cap, group KILL on overflow (`lib.rs:87`); catalog 2 tries 2 s apart, 8 s after sleep (≥ 20 s wall vs monotonic gap) (`:877-960`); scratch run 3 min, non-empty result, help re-probed unchanged (`:91,404-512`); custom recipes reject reserved ids and limit env to `HOME PATH LANG TMPDIR` (`:962-1092`)
- [ ] none of the probes touch a terminal; real-pane verification is the manual Tier 1 release round (`lib.rs:1671-1672`) → agentdriver's conformance suite replaces it

## 9. Edge cases to keep

- [ ] compare `terminal_id` (and agent session for OMP) before every close — pane ids are reused
- [ ] "absent" only on `pane_not_found`
- [ ] failed close keeps the receipt with an error; closed receipts are never deleted
- [ ] OMP close only from the parent pane, never in a `lobby` tab, only after decision **and** ack
- [ ] native close after settlement waits for idle; cancel and deadline close regardless, TERM descendants first
- [ ] `keep_pane` + deadline → TERM host, keep pane
- [ ] offer updated with the incoming pane **before** the prompt
- [ ] handoff prompt carries `env -u …` with explicit owner and home
- [ ] Codex caller invisible during its first call → hint / `session:` marker + 40 s wait
- [ ] `tell_head` and notes only when idle/done; non-delivery is a note, not an error
- [ ] usage: unknown or stale never triggers; note before auto handoff; target at its limit blocks; same-harness handoff suppressed
- [ ] workflow pane file re-bound on resume, removed when stale, trusted only in a driver
- [ ] head pane is not a worker (no receipt, no balancing)
- [ ] shell settles before `agent start`; `agent_pane_busy` = shell prompt not drawn yet; Herdr can say idle with a numbered menu on screen → check the screen
- [ ] worker-pane claim grace 1 min
- [ ] two concurrent cleaners: receipt file locks, archive "already done" proven by digest; session process cwd = home so it doesn't hold the worktree
