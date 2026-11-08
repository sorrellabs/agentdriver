# brgr coverage — owner identity, notice delivery, Herdr plugin

Source: brgr v2.15.21. Paths relative to `brgr/crates/brgr-cli/src/` unless noted.

Items marked **[agentdriver]** are host/harness primitives agentdriver must
provide. Items marked **[brgr]** stay in brgr but depend on those primitives;
they are listed so the boundary is checkable.

## 1. Host calls in this area

| # | Command | Ref | Fields | Failure |
|---|---|---|---|---|
| [ ] a | `agent list` | `caller_pane.rs:153` | `/result/agents[]` `.agent=="codex"`, `.pane_id` | → no proof; a missing `pane_id` on any entry aborts the trace |
| [ ] b | `pane read <p> --source recent-unwrapped --lines 1500` | `caller_pane.rs:160-168` | raw text | one failed read aborts the whole trace |
| [ ] c | `agent get <p>` (`live_kind`) | `caller_pane.rs:241` | `/result/agent/pane_id` must equal p; `/result/agent/agent` | → none |
| [ ] d | `pane process-info --pane <p>` | `caller_pane.rs:297` | recursive `pid` / `foreground_process_group` equal to the pid (`:308-318`) | skip receipt |
| [ ] e | `agent get <p>` (`recorded_agent`) | `notification.rs:373-387` | `/result/agent` → `.agent`, `.pane_id`, `/agent_session/value` | `Herdr Codex pane lookup timed out` / `recorded Codex pane is unavailable` / `Herdr did not return a Codex agent` |
| [ ] f | `pane report-agent-session <p> --source herdr:codex --agent codex --agent-session-id <s>` then re-check (e) | `notification.rs:277-293` | agent `codex`, same pane, `agent_session.value == s` | `…report timed out` / `…did not accept the Codex session report` / `…did not bind the exact Codex session to the pane`; existing different session → `recorded Herdr Codex session differs from the owner binding` (`:275`) |
| [ ] g | `agent get <p>` before each owner push | `notification.rs:1133-1137` | `/result/agent/{agent,pane_id,agent_status,agent_session/value}` | `…identity lookup timed out` / `recorded parent pane is unavailable` / `Herdr did not return an agent` / `recorded parent agent identity changed` / `parent agent is not idle` |
| [ ] h | `agent prompt <p> <body>` | `notification.rs:1171-1178` | exit status | `Herdr parent prompt timed out` / `Herdr did not accept the parent prompt` |
| [ ] i | `plugin pane open --plugin brgr --entrypoint codex\|board --placement tab --workspace <id> --focus\|--no-focus --cwd <p> --env BRGR_PLUGIN_WORKSPACE_CWD=<p>` | `herdr_plugin.rs:138-155` | stdout passthrough ≤ 64 KiB | `Herdr pane open timed out` / `Herdr could not open the brgr pane: …` / `Herdr pane response exceeds 64 KiB` |
| [ ] j | `pane get <p>` (`terminal_of`) | `PA/head_pane.rs:102-112` | `/result/pane/terminal_id` | none |
| [ ] k | `agent get` → `agent prompt` (`tell_head`) | `head_pane.rs:116-129` | status `idle`/`done` | false |

### Call budgets — [agentdriver] per-call timeouts and output caps
- [ ] Caller resolver: stdin null, output to temp files, `try_wait` every 5 ms, kill after **2000 ms** (200 ms in a hook), non-zero or stdout > 1 MiB → none, no retry (`caller_pane.rs:320-359`)
- [ ] Notifications: **5 s** per call (`HERDR_TIMEOUT`, `notification.rs:28`), `kill_on_drop`; `agent get` stdout > 65 536 bytes = failure (`:379,1138`)
- [ ] Plugin: **10 s** (`HERDR_CALL_DEADLINE`, `herdr_plugin.rs:33`)
- [ ] Pane adapter: none (see [spawn and run §0](spawn-and-run.md#0-facts-that-shape-the-scope))

### Binary and session selection — [agentdriver]
- [ ] Resolver binary: `HERDR_BIN_PATH` as given, else first regular-file `herdr` on `PATH`, canonicalised (`caller_pane.rs:369-383`); test override via thread-local
- [ ] Pane adapter binary: `HERDR_BIN_PATH` only if absolute, else `herdr` (`PA.rs:1077-1085`)
- [ ] `--session`: resolver adds it even when empty (`caller_pane.rs:323-325`); plugin and pane adapter only when non-blank (`herdr_plugin.rs:133-137`) — agentdriver: one rule (non-blank), brgr tests updated if they depend on the empty form
- [ ] Notifications use the **recorded** binary and session of the owner surface, not the current env; recorded binary must be absolute and a file, else `recorded Herdr executable is unavailable` (`notification.rs:1129-1132,1182-1188`)

## 2. Owner identity

### 2.1 Session and owner — [brgr], inputs from the harness env
- [ ] Flags `--owner-session` / `--as`, `--source-pane` stored once (`cli.rs:16-34`, `main.rs:214-220`, `invocation.rs:16-29`); a Codex hook replaces the context (`invocation.rs:31-37`, `hook.rs:46-47`)
- [ ] Current session (`main.rs:797-830`): `--as` (blank → `owner session must not be empty`), else first non-blank of `CODEX_THREAD_ID`, `BRGR_SESSION_ID`, `CLAUDE_CODE_SESSION_ID`, `CURSOR_CONVERSATION_ID`, `OPENCODE_SESSION_ID`; `CODEX_THREAD_ID` ≠ `BRGR_SESSION_ID` → `CODEX_THREAD_ID and BRGR_SESSION_ID disagree` (skipped when cwd identifies a worker)
- [ ] Owner kind (`main.rs:736-755`): opencode (OPENCODE set, CODEX not) → claude (CLAUDE set, CODEX and BRGR not) → cursor (live host kind `cursor`, or CURSOR the only one set) → codex
- [ ] Owner id (`main.rs:640-669`): `BRGR_OWNER_ID`; but `--as` ≠ `BRGR_SESSION_ID` → `<kind>:<--as>`; else `<kind>:<session>` or `<kind>:manual`
- [ ] `require_owner` messages (`main.rs:839-862`): no session; unbound owner; `BRGR_OWNER_ID` differs (allowed with note if session matches); session differs → points to `head request`, `head claim --abandoned`, `bind`

### 2.2 Which pane is the caller — [agentdriver] `caller()` primitive
- [ ] **`verified()`** (`caller_pane.rs:27-39`): `--source-pane P` → `named(P)` only; else `inherited()`; else `for_session(current, None)`
- [ ] **`named(P)`** (`:42-48`): `for_session(current, Some(P))`, or a workflow driver's recorded pane (`BRGR_WORKFLOW_RUN` set, `runs/<run>.workflow.pane == P`, P a live owner pane; `workflow/driver.rs:500-511`)
- [ ] **`inherited()`** (`:60-72`): `HERDR_ENV=="1"`, non-blank `HERDR_PANE_ID`, and `runs_inside(pane)`; debug-only `BRGR_TEST_TRUST_HERDR_PANE=1` skips checks
- [ ] **`runs_inside`** process-tree walk (`:386-433`)
  - [ ] one table from `/bin/ps -A -o pid=,ppid=,comm=`
  - [ ] walk up from own pid, ≤ 64 steps; true on a process whose `comm` basename is exactly `herdr`
  - [ ] false if an ancestor's env has a **different** `HERDR_PANE_ID`, ppid ≤ 1, or a pid is missing
  - [ ] env read: Linux `/proc/<pid>/environ` ≤ 1 MiB NUL-split; macOS `/bin/ps -E -ww -o command= -p <pid>`, last ` HERDR_PANE_ID=` (`:438-465`)
- [ ] **`for_session(session, explicit)`** (`:82-98`)
  - [ ] 1 screen trace (below); found but ≠ explicit → **none, no fall-through**
  - [ ] 2 explicit == inherited and live kind ∈ {codex, claude, cursor} → explicit
  - [ ] 3 bridge receipt: `<cwd>/.agent-progress/bridges/*.json`, then under the primary checkout; regular file (`symlink_metadata`) ≤ 65 536 bytes; `native_session == session`, `session == session`, `pane` (== explicit if given), `pid` proven by `pane process-info`
- [ ] **Screen trace** `traced` (`:141-198`)
  - [ ] skipped in hooks and for session ids < 13 chars (`TRACE_PREFIX`)
  - [ ] all Codex panes (`agent list`), 1500 recent lines each; exactly one match wins, zero or several → none
  - [ ] a matching line: after trimming whitespace and `• └ │`, starts with `Ran `, `Running `, `Run `; contains the 13-char session prefix; invokes brgr
  - [ ] `invokes_brgr` (`:204-217`): split on `&` `;` `|`; first word without `=` (quotes trimmed) is `brgr`, ends `/brgr`, or `$BRGR_BIN` / `${BRGR_BIN}`; not `grep brgr …`, `echo brgr --as …`, a bare id, or a user prompt `› please run brgr …` (tests `:503-535`)
  - [ ] only panes whose live agent is codex count
- [ ] **Host session records are never trusted** for locating the pane — test `a_poisoned_herdr_session_record_does_not_move_the_session` (`:629-648`)
- [ ] Inbox kinds are exactly codex, claude, cursor (`:53-58,101-103`)
- [ ] `codex_trace_status()` for `doctor`: `codex --version` vs `VERIFIED_CODEX = "codex-cli 0.166"` (`:183,221-238`) — informational

### 2.3 Pending caller marker for Codex — [agentdriver] wait-for-caller
- [ ] Codex ≥ 0.162 prints `Ran …` only after the command ends, so the starting call can't be traced yet
- [ ] Admission stores (`admission.rs:927-939`): worker pane → `verified()` → `"session:<id>"` if pending allowed, `HERDR_ENV=1`, and a host binary exists (`caller_pane.rs:113-121`)
- [ ] Pending allowed (`admission.rs:941-959`): `CODEX_THREAD_ID` set; or none of OpenCode/Claude/Cursor ids, `--as` non-blank, owner starts `codex:`
- [ ] No pane: pane mode → `a pane run needs an exact Herdr source pane; run it from a Herdr pane, or use --mode headless` (`admission.rs:922-925`); auto → headless, reason `no herdr pane in this environment` (`tests/orchestration.rs:102-140`)
- [ ] `__pane-run` resolves it: poll the trace every **700 ms** for **40 s** (`CALLER_WAIT`, debug `BRGR_TEST_CALLER_WAIT_MS`) (`PA.rs:94,317-329`, `caller_pane.rs:126-137`); failure text `could not find the calling Codex pane: no Codex pane shows this session's brgr call (start the command with --as SESSION; brgr doctor shows whether this Codex release was checked)`
- [ ] Other errors: `pane mode requires a Herdr caller`; `Herdr caller pane id is absent`
- [ ] Commands that require `verified()`: omp-role (`admission.rs:893-903`), `head handoff` (+ Codex hint "run one short brgr command with the same --as first", `head.rs:737-758`), `workflow run` (else `no verified Herdr pane: workflow steps must use mode headless`, `workflow/driver.rs:44,454-472`)

### 2.4 Registering the owner's surface
- [ ] **[brgr]** `register_current_surface` (`notification.rs:171-261`): `worker:` → worker surface; kind from owner prefix; pane from `for_session` / `verified()`; host binary absolute; binding exists (`Codex owner has no session binding`) and holds this session
- [ ] **[agentdriver]** live kind check: non-inbox kind → `brgr: Herdr agent <kind> is not an owner inbox; FROM BRGR will not arrive. Use brgr watch.` (`:219-225`); kind must equal expected unless a takeover head (`is_taken_head`, `:85-99`); live `pane_id` must equal → else `recorded Herdr pane is not the current owner agent`
- [ ] **[agentdriver]** Claude owner liveness: walk ≤ 32 ancestors with `ps` (`ppid`, `comm`) for `comm == claude`; record `{pane, pid, birth=lstart}` in `runs/claude-owner-<session>.json`; session must be ASCII alnum or `-` (`:936-977`); before each push pid + lstart unchanged and pane matches, else `the Claude Code session that owns this task no longer runs in pane P`; no record → skip (`:982-994`)
- [ ] **[agentdriver]** Cursor owner: no extra proof; Herdr keeps no session id for Claude or Cursor (`:231-233`)
- [ ] **[agentdriver]** Codex owner: `report-agent-session` binding (call f); never overwrite a different session
- [ ] **[brgr]** store upsert `register_owner_surface(owner, session, epoch, pane, HERDR_SESSION, bin)` keyed on owner (`brgr-store/src/notification.rs:187-227`)
- [ ] **[brgr]** registration points: admission (Claude/Cursor in-process, `admission.rs:1392-1408`; dispatcher `notification.rs:350-371`), `brgr bind` (`task_commands.rs:750-765`), Codex hook except Stop with 500 ms limit (`hook.rs:55-74`), `head take` via `register_offered_pane` (recorded bin/session, terminal id check, live kind inbox, pane equal → else `recorded Herdr pane is not the incoming head's pane`) then fallback (`notification.rs:60-81`)
- [ ] **[brgr]** OpenCode owners get no FROM BRGR (`codex_integration.rs:77`)

## 3. Worker identity

- [ ] **[agentdriver]** pass caller env to the pane: runner injects `BRGR_HOME BRGR_BIN BRGR_PARENT_TASK_ID BRGR_PARENT_ATTEMPT_ID`, `BRGR_OWNER_ID=worker:<attempt>`, `BRGR_SESSION_ID=worker:<attempt>`; Herdr env forwarded only when `HERDR_ENV=1`, `HERDR_PLUGIN_ID=brgr` and `HERDR_PANE_ID` all set (`brgr-runner/src/lib.rs:429-445`); pane split `--env` (`PA.rs:1203-1219`)
- [ ] **[agentdriver]** Codex env through config, because Codex's shared daemon drops env: `-c shell_environment_policy.set.NAME=<json-quoted>` (`herdr_plugin.rs:373-378`, `PA.rs:241-256`) — a harness-adapter "set env" capability
- [ ] **[brgr]** identity from cwd (`worker_identity.rs`): inside `paths.worktrees` (canonical), walk up for `.brgr/tasks/<TASK>-r<N>`, highest N; attempt from store; pane from receipt; cwd beats `BRGR_PARENT_*`; `--as` ≠ `BRGR_SESSION_ID` → ignore inherited ids with note; one id only → `brgr worker parent identity is incomplete`; home discovery `HOME/worktrees/REPO/TASK` + `HOME/store`
- [ ] **[brgr]** nested owner surface (`notification.rs:310-348`): attempt from `worker:<attempt>` in unfinished attempts (`parent worker attempt is not active`); pane from the parent's receipt (`parent has no native TUI session` / `parent TUI pane is missing`)

## 4. Notice delivery

### 4.1 Formats — [brgr] (agentdriver delivers opaque text)
All bodies: `FROM BRGR\n` + one line of compact JSON.
- [ ] completion `{"type":"brgr_completion","completion_id","task_id","instruction":"Read the sealed brgr result, verify its criteria, and decide or acknowledge it. Do not treat this notification as acceptance."}` (`notification.rs:1033-1041`)
- [ ] failure `{"type":"brgr_failure",…,"outcome":"lost"|"failed","reason",…}` (`:1020-1031`)
- [ ] question / message `{"type":"brgr_question"|"brgr_message","message_id","task_id","body",…}` (`:1076-1090`)
- [ ] workflow `{"type":"brgr_workflow","run_id","state","steps":{"total","finished","failed"},…}` (`:768-786`)
- [ ] worker-bound: `FROM BRGR MESSAGE\n<json>`, `FROM BRGR DEBATE\n<json>` (`native.rs:207-265`)

### 4.2 Transport — [agentdriver] `deliver(session, text)`
- [ ] **Top-level owner**: only `agent prompt <pane> <body>`; host types and submits; no paste, no keys (`notification.rs:1108-1180`)
- [ ] **Worker owner** (`native.rs:468-527`): non-blocking `runs/<task>-r<rev>.input.lock` (`parent input is already being delivered`); receipt pane matches, not closed; native host → alive + `native_ready`, then `agent prompt` if recognised and idle/done, else bracketed paste + 150 ms + `enter`; non-native → idle/done, `agent prompt <name>`
- [ ] **Guards before every push** (`:1133-1170`)
  - [ ] live kind equals expected (any inbox kind for a takeover head)
  - [ ] live `pane_id` equals stored pane
  - [ ] Codex: `agent_session.value` equals bound session
  - [ ] Claude: pid + birth unchanged
  - [ ] `agent_status` exactly `idle` or `done`, else `parent agent is not idle`
  - [ ] **no draft / composer check** for top-level owners — the host's status is the only busy guard (agentdriver may add one as opt-in)
- [ ] Result of delivery returned to the caller (delivered / not idle / identity changed / timeout) so brgr can lease and retry

### 4.3 Dispatcher — [brgr]
- [ ] detached `brgr --home H [--owner-session S] [--source-pane P] __notify <task>`; new process group; stdin/stdout null; stderr → `runs/<task>.notify.log` (0600); `BRGR_PLUGIN_*` removed; `--source-pane` only if not a `session:` marker (`notification.rs:479-517`)
- [ ] single instance: non-blocking flock `runs/<task>.notify.lock` (`tests/workflow_notice.rs:223-284`)
- [ ] exits if `brgr.sqlite3` dev/inode changes (`:30-49,544`), notifications off, workflow step task, or settled (`:864-891`); lifetime 24 h
- [ ] registration retry 0, 2 s, doubling to 60 s (`:540-553`)
- [ ] loop every 500 ms: questions, then completions by rowid; per notice a UUID claim with 20 s lease; delivered → mark (retry 15 s on contention; `NotificationClaimStale` = done); failed → release with `last_error` ≤ 512 chars, `attempts++`, backoff 500 ms doubling to 3 s
- [ ] workflow notices: `runs/<run>.workflow.notify.lock`, by `seq`, exit when empty (`:650-761`)
- [ ] store: claim joins binding and surface on session+epoch; delivered only if token and surface still match; rebinding clears delivered state so a new head is told again (`brgr-store/src/owner.rs:165-181`); accept/reject/ack resolve; questions dedupe by `question_notices(message_id, session_id)`, no lease

### 4.4 Repeated failure banner — [brgr]
- [ ] after every non-internal command (`main.rs:271-297`, `failure_banner.rs`): `brgr: N run(s) failed or were lost and are not acknowledged:` + ≤ 3 lines `<task> failed|lost: <reason ≤ 160 chars>` + `and N more` + Inspect / Retry / Dismiss hints; until acked

### 4.5 Codex owner hook — [agentdriver] harness hook install; [brgr] hook logic
- [ ] **[agentdriver]** install into `$CODEX_HOME/hooks.json` (default `~/.codex`): `SessionStart`, `UserPromptSubmit`, `Stop` → `'<brgr>' --home '<home>' __hook <event>`, timeout 1; backup `hooks.json.brgr-backup-<nanos>` on change, rollback on failure (`codex_integration.rs:92-195,447-474`) — this is the one **global** config write brgr makes; agentdriver's per-launch rule ([decision 5](../decisions.md#5-harness-configuration-is-per-launch-or-per-workspace-never-global)) needs an explicit "owner integration" exception
- [ ] **[brgr]** worker worktree → record rollout, print `{}`; SessionStart → bind owner; non-Stop → register surface (500 ms) and start dispatchers; output `hookSpecificOutput.additionalContext` with `brgr --as <sid>` guidance, pending list ≤ 10; Stop → `{"decision":"block","reason":…}` without `hookSpecificOutput` (`hook.rs:110-123`)
- [ ] **[agentdriver]** a host that never answers must not hang a hook — test `unavailable_herdr_does_not_hide_the_codex_inbox_hook`

## 5. Herdr plugin

### 5.1 Manifest (`brgr/herdr-plugin.toml`) — [brgr]
- [ ] `id="brgr"`, `min_herdr_version="0.9.0"`, macOS + Linux, build `cargo build --release --locked -p brgr-cli`
- [ ] actions `open`, `codex`, `doctor`; panes `board` (tab), `codex` (tab), `worker` (split)

### 5.2 Entrypoints
- [ ] **[agentdriver]** host-plugin context: `HERDR_ENV=1` + `HERDR_PLUGIN_ID=brgr` (`herdr_plugin.rs:59-66`); `HERDR_PLUGIN_CONTEXT_JSON` ≤ 64 KiB with `workspace_id`, `workspace_cwd`, `focused_pane_cwd`, `worktree.checkout_path`; id ASCII 1–128; path order checkout → workspace_cwd → focused_pane_cwd, absolute existing dir (`:48-57`)
- [ ] **[agentdriver]** `plugin open`: absolute existing `HERDR_BIN_PATH`, then call (i) (`:120-170`)
- [ ] **[brgr]** `plugin board [--once]`: read-only, latest 20 tasks, redraw 2 s with `\x1b[2J\x1b[H`, control chars → U+FFFD, error text `board refresh failed: … No tasks were mutated or reconciled.` (`:172-253`)
- [ ] **[brgr]** `plugin worker`: `BRGR_PLUGIN_WORKER_LAUNCH` canonicalises inside `paths.launches`, then supervise (`:267-279`)
- [ ] **`plugin codex`** (`:255-371`)
  - [ ] **[brgr]** workspace from `BRGR_PLUGIN_WORKSPACE_CWD` or context; install Codex integration
  - [ ] **[agentdriver]** Codex executable: configured absolute executable file (`config.rs:181-190`) else `codex`; `PATH` = brgr dir, Codex dir, old `PATH`
  - [ ] **[brgr]** bridge dir `brgr-plugin-bridge-*` mode 0700
  - [ ] **[agentdriver]** launch `codex -C <ws> --add-dir <bridge> -c shell_environment_policy.set.PATH=<json> -c …BRGR_PLUGIN_BRIDGE_DIR=<json> -c …BRGR_HOME=<json>`, env `PATH` + `BRGR_PLUGIN_BRIDGE_DIR`, removing `CODEX_THREAD_ID BRGR_SESSION_ID BRGR_OWNER_ID`, stdio inherited
  - [ ] **[brgr]** bridge served alongside; bridge stop → kill Codex (`brgr Herdr bridge stopped while Codex was running`); spawn error → hint `brgr config set-codex-executable PATH`; TTY error waits for Enter

### 5.3 Bridge protocol — [brgr]
- [ ] client (`plugin_bridge.rs:157-218`, `main.rs:235-255`): absolute real dir else `brgr Herdr bridge directory is unavailable`; deadline budget + 120 s (1 s–7 d; budget from run deadline / wait timeouts / 3600 default, `bridge_host.rs:14-36`); `<id>.request.json` exclusive, ≤ 64 KiB, id `<pid>-<nanos>-<seq>`; poll 50 ms; response regular ≤ 16 MiB, id must match; replay stdout/stderr; exit `code.clamp(1,125)`; client timeout + 10 s
- [ ] server (`:239-524`): poll 50 ms, ≤ 64 jobs; names `[0-9-]+`; claim via exclusive `<id>.claim` + rename to `.processing`; stale handling; request validation (regular, no symlink, ≤ 64 KiB, args non-empty, absolute cwd, timeout 1 s–7 d, args ≤ 64 KiB); run own exe with `BRGR_HOME`, `BRGR_PLUGIN_HOST_HOME`, `BRGR_PLUGIN_HOST_WORKSPACE`, only the client's three ids; own process group; output cap 8 MiB → `kill -KILL -<pgid>`; timeout → kill, wait 2 s, `brgr Herdr bridge command timed out`; atomic response, > 16 MiB → error
- [ ] refusals (`bridge_host.rs:38-163`): `--home` override; incomplete host context; `run --workflow`, `config set-issue-reporting|set-auto-recertify|set-handoff-condition`, `workflow` except `init`, `harness` except `status`, `integrate`, `prune`, `errors file`; internal commands; workspace-bound `run`, `revise --workspace`, `artifact export`, `apply`, `land`, `workflow init`

## 6. Model and usage observation — [agentdriver] harness adapter "observe"

- [ ] **Model** (`session_model.rs`), pane-mode Claude and Codex only (`:36-77`)
  - [ ] Claude: `<CLAUDE_CONFIG_DIR|~/.claude>/projects/<workspace with non-alnum → '-'>/<attempt>.jsonl`; last `type=="assistant"` with `sessionId==attempt` → `/message/model` (`:79-119`)
  - [ ] Codex: rollout from `runs/<task>-r<rev>.codex-session.jsonl` (hook-appended `{session_id, rollout, at}`); must be `rollout-*.jsonl` inside `$CODEX_HOME/sessions`, cwd matching exactly one of the latest 100 task workspaces; > 1 distinct rollout → unknown (`:151-236`); last `turn_context` with same `/payload/cwd` → `/payload/model` (`:123-141`)
  - [ ] limits: log 8 MiB, record 64 KiB; trimmed, control chars rejected, cut to `MAX_MODEL_CHARS` (`:255-261`)
- [ ] **Usage / quota** (`context_signal.rs`)
  - [ ] store `$BRGR_HOME/context/<session>.json`, dir 0700, ids `[A-Za-z0-9_-]{1,128}`
  - [ ] Claude, Cursor: `brgr __context` as the status-line command (stdin ≤ 1 MiB): `session_id`, `context_window.{used_percentage,context_window_size}`, `rate_limits.five_hour|seven_day.{used_percentage,resets_at}` (0–100); then run the user's chained status line with the same stdin, `~/` expanded (`:88-106,251-278`)
  - [ ] Codex: last 256 KiB of the rollout, last `event_msg`/`token_count` with `info`: context % = `last_token_usage.total_tokens / model_context_window` (basis points, ≤ 100 %); `rate_limits.{primary,secondary}.{used_percent,resets_at,window_minutes}`, `window_minutes > 1440` = seven-day (`:170-219`)
  - [ ] read-back `latest_quota(provider, max_age)`: newest fresh report, skip `*.note.*`, higher window; missing/stale = unknown, never over the limit

## 7. Edge cases to keep

- [ ] Codex runs tools and hooks in a **shared app-server daemon** whose env belongs to whoever started it → every trust decision avoids env: ancestry walk, screen trace, cwd identity, `--as` over env, `-c shell_environment_policy.set.*`
- [ ] pending `session:` marker resolved later — 40 s in `__pane-run`, up to 24 h of dispatcher retries; users told to run `brgr --as S status --brief` first (`codex_integration.rs:33,55`)
- [ ] 13-char session prefix match (narrow panes truncate); shorter ids never traceable
- [ ] trace counts only live Codex panes; two matches → none; one failed read → none
- [ ] in hooks: host calls 200 ms, no trace, registration 500 ms — a hung host never blocks a hook
- [ ] delivered state bound to session + epoch + pane; takeover heads may run another harness (`any_kind`)
- [ ] Claude owner liveness = pid + `ps lstart`
- [ ] `report-agent-session` only when the host has none; never overwrite
- [ ] bare `HERDR_PANE_ID` is never trusted outside debug tests
- [ ] dispatcher exits on store replacement; background internals don't recreate the home (`main.rs:259-270`)
- [ ] workflow steps get no per-task notices
- [ ] notifications can be off per calling harness
- [ ] busy owner never prompted (tests `idle_codex_parent_receives_completion_without_another_user_turn`, `a_busy_owner_pane_keeps_the_notice_pending_until_idle`)
- [ ] plugin Codex starts with session/owner ids removed; dispatchers start with bridge env removed (`notification.rs:506-508`)
