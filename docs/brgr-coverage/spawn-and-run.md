# brgr coverage — spawn, layout, input, completion

Source: brgr v2.15.21. `S` = `brgr/crates/brgr-cli/src`, `PA` = `S/pane_adapter`,
`R` = `brgr/crates/brgr-registry/src/recipe.rs`.

An item is done when agentdriver reproduces it (or exposes the primitive brgr
needs to keep doing it) and brgr's tests and live checks pass on it.

## 0. Facts that shape the scope

- [ ] **Every built-in recipe is native-hosted** (`native_host: true`, R:794). A normal worker is started as `pane run <pane> "exec '<brgr>' __tui-host '<config>'"`, not `herdr agent start`; status is read by pane id. `agent start` is only used for head handoff (`PA/head_pane.rs:59,76`) and old manifests → agentdriver must support **both** "host runs our process in a pane" and "host starts a recognised agent"
- [ ] **Permission is always full** — admission clears the task permission (`S/admission.rs:971`), `permission_arguments` always returns the full argv (`brgr-runner/src/manifest.rs:153-158`)
- [ ] **Read-only paths are dormant**: `BRGR_REPORT_BEGIN/END` markers and the Claude `Stop` / Cursor `afterAgentResponse` hooks only fire for a stored launch whose spec still says read-only (`PA.rs:855`, `PA/native.rs:604`). They stay in scope for old receipts
- [ ] **Main completion signal is the report file + `__seal-report`**, not a harness hook (§6)
- [ ] **`herdr` calls in the pane adapter have no timeout** (blocking `Command::output()`, `PA.rs:1097-1117`); only `caller_pane.rs` (2000 ms, 200 ms in a hook) and `notification.rs` bound them → agentdriver: every host call takes a timeout; brgr passes "none" where it has none today

## 1. Host invocation plumbing

- [ ] Binary: `HERDR_BIN_PATH` if absolute, else `herdr` (`PA.rs:1077-1080`); the caller resolver instead takes `HERDR_BIN_PATH` as given or searches `PATH` and canonicalises (`S/caller_pane.rs:369-383`)
- [ ] `--session <HERDR_SESSION>` prepended when non-blank (`PA.rs:1081-1113`, `caller_pane.rs:323-325`)
- [ ] Reply parsing (`PA.rs:1109-1144`): exit 0 + blank stdout → null; exit 0 → JSON (unparsable = error); non-zero → stderr `{"error":{"code","message"}}` → typed code, else trimmed stderr
- [ ] Error codes acted on: `agent_pane_busy`, `agent_not_ready`, `timeout`, `agent_not_found`, `pane_not_found` — agentdriver exposes them as typed errors
- [ ] Caller-resolver calls: stdin null, output to temp files, > 1 MiB rejected, child killed past its budget (`caller_pane.rs:320-359`)
- [ ] **Binary and session are pinned per task**: receipts store `binary` and `session`; every later call rebuilds the client from the receipt, not the current env (`PA/lifecycle.rs:273-276,378-381,625-628`; `PA/native.rs:183-186,284-287`) → agentdriver sessions persist their host connection details

## 2. Host commands brgr uses (Herdr adapter must cover)

| # | Command | Use | Ref |
|---|---|---|---|
| [ ] 1 | `pane get <pane>` → `/result/pane/{terminal_id,pane_id}` | identity check; `pane_not_found` = gone | `PA.rs:384-388`, `lifecycle.rs:229-241`, `head_pane.rs:104-146` |
| [ ] 2 | `pane layout --pane <caller>` → `panes[]{pane_id,rect}`, `splits[]{direction,ratio,rect}` | placement and balancing; failure → split caller right / no-op | `PA.rs:1164-1174,1266-1272`, `layout.rs:65-95` |
| [ ] 3 | `tab create --workspace <ws> --label <l\|brgr-work> --cwd <dir> --no-focus [--env K=V]…` → `/result/pane/pane_id` or `/result/root_pane/pane_id` | tab placement | `PA.rs:1177-1191,1224-1232` |
| [ ] 4 | `pane split --pane <p> --direction right\|down --cwd <dir> --no-focus [--env K=V]…` | worker pane | `PA.rs:1192-1202` |
| [ ] 5 | `pane rename <pane> <label>` | label; failure = one stderr line | `PA.rs:1239-1245` |
| [ ] 6 | `pane resize --pane <p> --direction down\|up --amount <f64 {:.4}>` | equal heights; errors ignored | `PA.rs:1274-1285` |
| [ ] 7 | `agent get <name\|pane>` → `agent_status` (default `unknown`), `agent` (kind, non-null = recognised), `pane_id`, `terminal_id`; OMP also `name`, `agent_session{kind,value}`, `state_change_seq` | status, liveness | `PA.rs:1288-1297`, `native.rs:330-341,507-514,781-784`, `omp_adapter.rs:192-201` |
| [ ] 8 | `agent start <name> --kind <k> --pane <p> --timeout 60000 -- <args…> [--session-id <attempt>]` | non-native start; `agent_pane_busy` → retry 250 ms up to 20 s; `agent_not_ready` or `timeout` with `agent get` OK = started | `PA.rs:429-469` |
| [ ] 9 | `pane read <pane> --source visible --lines 300` | screen; non-zero = unreadable | `PA.rs:1092-1107` |
| [ ] 10 | `pane read <pane> --source recent-unwrapped --lines 1500` | Codex caller trace | `caller_pane.rs:160-168` |
| [ ] 11 | `agent prompt <name\|pane> <text>` | prompt submission | `native.rs:342,359,517,534`, `head_pane.rs:88,128` |
| [ ] 12 | `pane send-text <pane> "\x1b[200~<text>\x1b[201~"`, 150 ms, `pane send-keys <pane> enter` | bracketed paste + submit | `native.rs:383-393` |
| [ ] 13 | `pane send-keys <pane> <k>…` | rule answers (all keys in one call), oh-my-zsh `n`, `brgr input --key` | `screens.rs:394-398,431`, `native.rs:301-303` |
| [ ] 14 | `pane run <pane> "exec '<brgr>' __tui-host '<config>'"` (single quote → `'\''`) | native host bootstrap | `native.rs:730-740` |
| [ ] 15 | `pane close <pane>` | cleanup; failure → `cleanup=pending` | `lifecycle.rs:651-665`, `head_pane.rs:94,153-155` |
| [ ] 16 | `worktree create --cwd <primary> --branch brgr/task-<slug> --base <rev> --path <target> --label brgr-<slug> --no-focus --trust-repository` | omp-role only; git-lock retry 25 ms × n — **stays in brgr** ([decision 7](../decisions.md#7-worktrees-belong-to-the-caller)) | `workspace.rs:655-662,837-882` |
| [ ] 17 | `agent list` → `agents[]{agent,pane_id}` (codex only) | caller lookup | `caller_pane.rs:153-159` |
| [ ] 18 | `pane process-info --pane <p>` → `pid` / `foreground_process_group` (recursive search) | caller verification | `caller_pane.rs:297-318` |
| [ ] 19 | `pane report-agent-session <pane> --source herdr:codex --agent codex --agent-session-id <id>` | owner notification surface | `notification.rs:277-295` |
| [ ] 20 | `herdr agent get <agent>` for OMP — bare binary, no `--session`; recovery retries 20 × 100 ms until `agent_session` appears | OMP adapter | `omp_adapter.rs:192-201,384-400` |

## 3. Launching a worker

### 3.1 Pane runner
- [ ] Supervisor runs `brgr --home <h> __pane-run --prompt-file --workspace --task --revision --kind <k> [--caller] [--native-executable] [--opencode-model=…] --agent-arg=<a>… --keep-pane` (`PA.rs:266-299`, `supervision.rs:102-107`)
- [ ] Env allowlist: harness `env_allow` + `HOME PATH LANG TMPDIR USER HERDR_ENV HERDR_PANE_ID HERDR_WORKSPACE_ID HERDR_SESSION HERDR_SOCKET_PATH HERDR_BIN_PATH` + debug-only test vars (`PA.rs:182-209`)
- [ ] Refuses unless `HERDR_ENV=1` or `--caller` given (`PA.rs:317-319`)

### 3.2 Argv, in order (`PA.rs:240-264`)
- [ ] 1 interactive argv → 2 config calling-option argv → 3 full-permission argv → 4 Codex only, at the **front**: `-c shell_environment_policy.set.BRGR_HOME="<home>"` (`PA.rs:245-259`, `herdr_plugin.rs:373-378`) → 5 model argv (unless OpenCode session-model case) → 6 effort argv
- [ ] Claude: append `--session-id <BRGR_PARENT_ATTEMPT_ID>` (`native.rs:597-603`, `PA.rs:444-448`)

### 3.3 Per-harness launch recipe

| Harness | Kind | Interactive argv | Full permission | Model | Effort | R |
|---|---|---|---|---|---|---|
| [ ] Claude Code | `claude` | — | `--permission-mode bypassPermissions` | `--model` | `--effort` | 363-412 |
| [ ] Codex | `codex` | — | `--dangerously-bypass-approvals-and-sandbox` | `--model` | `-c model_reasoning_effort=…` | 539-590 |
| [ ] Cursor | `cursor` | `--trust` | `--force` | `--model` | — | 210-247 |
| [ ] Devin | `devin` | `--respect-workspace-trust false` | `--permission-mode dangerous` | `--model=<m>` (joined) | — | 291-334 |
| [ ] OpenCode | `opencode` | `--standalone` if documented | `--auto` | see 3.4 | refused in pane mode | 447-483 |
| [ ] OMP | `omp` | — | `--approval-mode=yolo` | `--model` | `--thinking` | 171-209 |
| [ ] Command Code | `command-code` | `--trust --no-auto-update --skip-onboarding` | `--permission-mode yolo` | `--model` | `--effort` if help lists it | 248-290 |
| [ ] Cline | `cline` | — | `--auto-approve true` | unsupported | `--thinking` | 413-446 |
| [ ] Copilot | `copilot` | `--no-auto-update --no-ask-user` | `--yolo` | `--model` | `--reasoning-effort` | 484-538 |
| [ ] Gemini | `gemini` | `--skip-trust` | `--approval-mode yolo` | `--model` | — | 591-635 |
| [ ] Pi | `pi` | — | none needed | `--model` | `--thinking` | 636-686 |
| [ ] Fixture (tests) | `fixture` | — | — | `--model` | `--thinking` | 143-170 |

### 3.4 OpenCode session model (`S/tui_host/opencode.rs:44-115`, `PA.rs:127-144`)
- [ ] Effort is print-only → refused in pane mode
- [ ] Model is print-only when `--standalone` exists; exception when kind opencode, native host, argv exactly `["--standalone"]`: set the model through OpenCode's API
  - [ ] `opencode serve --stdio --hostname 127.0.0.1 --port 0` with `OPENCODE_PASSWORD`
  - [ ] `opencode api --server <url> model.list --param location.directory=<ws>`
  - [ ] `POST /api/session`, `GET /api/session/<id>`; append `--session ses_…`
  - [ ] refuse conflicting args: `--session --continue --server --model -s… -c… -m…`

### 3.5 Launch environment
- [ ] Self-update off: `OPENCODE_DISABLE_AUTOUPDATE=1` (OpenCode), `DISABLE_AUTOUPDATER=1` (Claude) (`native.rs:545-551`)
- [ ] Pane `--env`: `BRGR_HOME BRGR_BIN BRGR_PARENT_TASK_ID BRGR_PARENT_ATTEMPT_ID BRGR_OWNER_ID BRGR_SESSION_ID` when set + extra env (`PA.rs:1204-1223`) → agentdriver: caller-supplied env passed to the pane
- [ ] Native host env: `env_allow` + those `BRGR_*` + `HERDR_ENV HERDR_PANE_ID HERDR_WORKSPACE_ID HERDR_BIN_PATH HERDR_SESSION HERDR_SOCKET_PATH` (`native.rs:575-596`)
- [ ] Host overrides from the **pane's own env**: the `HERDR_*` set plus `HERDR_TAB_ID` (`tui_host.rs:90-102`); "pane variables" (`env_allow` + `TERM COLORTERM NO_COLOR`) taken from the pane, **removed if the pane lacks them** (`tui_host.rs:37-70,103-106`)
- [ ] Final exec: `env_clear` + envs + `current_dir(workspace)` + inherited stdio (`tui_host.rs:138-147`)
- [ ] `USER` kept for Claude and Copilot so the macOS keychain login is found (R:383-390, 510-516)

### 3.6 Native TUI host process (`S/tui_host.rs:82-236`)
- [ ] Config file: regular file ≤ 64 KiB
- [ ] Single host per state: `try_lock` on `<state>.lock` → "already has a host"
- [ ] Executable sha256 must equal `launch.digest`
- [ ] State file `{pid, birth (ps lstart), phase running|exited|failed, exit_code, startup_error}`
- [ ] `alive()` = phase `running` and `ps -o lstart=` still equals `birth` (pid reuse guard) (`:174-184`)
- [ ] Startup wait: poll state every 250 ms up to 20 s; `failed` + startup error fails the run (`native.rs:741-755`)
- [ ] SIGTERM every descendant of the host pid before closing — tools in their own process groups survive a pane close otherwise (`:186-236`); Codex daemon tools are out of reach

## 4. Placement and layout

- [ ] Config `herdr.worker_placement` = `adjacent` (default) | `tab` (`config.rs:27-33,261-265`)
- [ ] Owned workers = receipts `runs/*.pane.json` with `caller == caller` and `cleanup != closed` (`layout.rs:192-204`)
- [ ] Column = those panes in the caller's layout whose rect starts right of or below the caller, sorted by y (`layout.rs:118-129`)
- [ ] **First worker**: split the caller `right`, or `down` if the caller is narrower than 100 columns (`layout.rs:25,133-141`)
- [ ] **Later workers**: split the lowest worker `down`; the caller keeps its half (`layout.rs:146-149`)
- [ ] **Tab fallback**: column rows / (n+1) < 10 → open a tab (`layout.rs:27,142-145`)
- [ ] Layout unreadable → split caller right (`PA.rs:1168-1172`)
- [ ] **One opener per caller**: file lock `runs/layout-<caller ':'→'_'>.lock` held across open, receipt write and balance (`PA.rs:351-393`)
- [ ] **Equal heights** (`layout.rs:152-189`): for worker i of n find the smallest taller `down` split with the same x, y, width; target ratio 1/(n−i); change > 0.02 → resize that pane `down`; < −0.02 → resize the next pane `up`; never a negative amount
- [ ] Rebalance after open (`PA.rs:392`), after close (`lifecycle.rs:653-655`), and after **every non-internal brgr command for all callers** (`main.rs:294-298`, `PA.rs:1042-1069`)
- [ ] Cwd = task workspace (git worktree `<worktrees>/<repo>/<task8>[-r<rev>]`, no trailing separator) (`workspace.rs:631-639,679-687,753-760`)

## 5. Names and receipts

- [ ] Agent name `brgr-<task8>` or `brgr-<task8>-r<rev>` (`PA.rs:336-341`); native receipt name is the pane id (`:376-380`); head handoff `head-<task8>` (`head_pane.rs:69`)
- [ ] Label template default `{harness}/{provider}/{model}: {objective}`; placeholders harness, provider, model, effort, objective, task; ≤ 200 bytes, closed braces, known placeholders only (`config.rs:16-24,340-360`)
- [ ] Rendering (`PA/name.rs`): strip `local.`; task 8 chars; objective = first line, control chars dropped, whitespace collapsed, ≤ 48 chars + `…`; whole ≤ 120 chars, only the objective shrinks; a missing field eats one adjacent `/`; provider from a `provider/` model prefix or table (claude-code→anthropic, codex→openai, gemini→google, copilot→github, cursor-cli→cursor); empty template → no rename
- [ ] **Re-apply the label** 12 × 150 ms (debug 3 × 1 ms) because Herdr renames the pane when the agent registers (`PA.rs:1247-1260,583-584`)
- [ ] Receipt `runs/<task>-r<rev>.pane.json` (`lifecycle.rs:9-50`): pane, caller, binary, session, task, revision, attempt, name, label, phase (`starting|working|awaiting_input|finished`), report, terminal, cleanup (`""|closing|pending|closed`), cleanup_error, report_digest, archived_report, archive_refused, finished_at; updated under `<receipt>.lock`, written atomically (`:52-63`)

## 6. Input delivery

- [ ] **Shell settle before typing** — see [screen rules §4](screen-rules.md#4-shell-rule-shell-update-offer-oh-my-zsh)
- [ ] **First prompt**: retry up to 20 s every 250 ms; resolve dialogs between tries; only an `Unsent` error retries, any other error or a screen-watch failure stops (`native.rs:695-720`, tests `:858-925`)
- [ ] **`send_prompt`** (`native.rs:312-363`)
  - [ ] verify receipt identity (`pane_id` + `terminal_id`), then `native_ready`; not ready → `Unsent`
  - [ ] native: host must be alive, else `Unsent`
  - [ ] host recognises the agent and it is idle/done → `agent prompt <pane> <text>`; on `agent_not_ready`/`agent_not_found` fall back to paste (host refused before sending, so no duplicate); other failure = error
  - [ ] otherwise bracketed paste + 150 ms + `enter`
  - [ ] non-native → `agent prompt <name> <text>`
- [ ] **Readiness** (`native_ready`, `editor_ready`, Codex override, 1 s settle) — see [screen rules §9](screen-rules.md#9-ready-for-input-on-screen); owner notified when blocked "while starting" or after 20 s (`native.rs:756-813`)
- [ ] **`brgr input`** (`native.rs:267-310`): blocking lock `runs/<task>-r<rev>.input.lock`; owner required; receipt not closed and identity verified; native host alive if a state file exists; `--text` 1..8192 bytes → paste + Enter; `--key` → `send-keys <key>`
- [ ] **Worker messages** (`native.rs:207-265`): `try_lock` the input lock (skip if busy); prompt-ready; latest attempt Running/Blocked; payload `FROM BRGR MESSAGE\n<json>` or `FROM BRGR DEBATE\n<json>`; dedupe via store claim → finish; `Unsent` releases the claim, other errors finish it as "native transport outcome uncertain"; delivered from the drive loop (native: every poll, else when idle/done, `PA.rs:631-633`) and from the `__session` server every 1 s (`native.rs:187-202`)
- [ ] **Owner notice into a parent TUI** (`native.rs:468-538`): `try_lock` input lock; pane equals receipt pane; verify; native → alive + ready, then `agent prompt` if recognised and idle, else paste; non-native → idle, then `agent prompt <name>`
- [ ] No length check on `agent prompt`; no escaping beyond paste framing

## 7. Completion

### 7.1 Report file
- [ ] Path `<workspace>/.brgr/tasks/<task>-r<rev>/report.md`; directory created and old file removed before start (`PA.rs:342-347`)
- [ ] Read with `O_NOFOLLOW|O_NONBLOCK`, regular file ≤ `max_bytes`, bounded (`PA.rs:1005-1021`); a FIFO does not hang (`lifecycle/tests.rs:486-501`)
- [ ] Digest `sha256:<hex>` (`native.rs:3-11`)
- [ ] Prompt suffix (`PA.rs:848-870`): write final Markdown to the report only when done, then run `"$BRGR_BIN" __seal-report <task> <rev>`; read-only variant: `brgr report <task> --body …` or `BRGR_REPORT_BEGIN_<attempt>` / `BRGR_REPORT_END_<attempt>` lines

### 7.2 Sealing (`lifecycle.rs:65-99`)
- [ ] Worker identity (cwd worktree, else `BRGR_PARENT_TASK_ID`/`ATTEMPT_ID`, `worker_identity.rs:70-81`) matches the receipt task, attempt, revision; receipt not closed
- [ ] Store revision equals the revision; digest equals any existing one (immutable)
- [ ] Sets `report_digest`, `phase=finished`, `finished_at`; spawns the session server

### 7.3 Drive loop, every 1 s (`PA.rs:588-704`)
- [ ] 1 Receipt has `report_digest` → re-read; changed digest → "completed native report changed"; else done
- [ ] 2 Status: native `agent get <pane>` (error → `unknown`); non-native `agent get <name>` (error → fail)
- [ ] 3 `blocked` → screen resolve (see screen rules §8)
- [ ] 4 Non-native idle/done with report present → done (`PA.rs:677`)
- [ ] 5 `working` → mark worked, reset watch
- [ ] 6 idle/done → `IdleReport::settle` (`PA.rs:779-823`)
  - [ ] not yet seen working and < 60 s since prompt → wait (`UNSEEN_WORK_GRACE`)
  - [ ] idle < 15 s → wait (`REPORT_GRACE`)
  - [ ] report exists → seal on the agent's behalf
  - [ ] already reminded → fail `AgentGaveNoReport` with last 8 rows; runner exits `AGENT_GAVE_NO_REPORT_EXIT` (`PA.rs:305-314`)
  - [ ] else one reminder: seen working → "Write … now … __seal-report"; never seen working → full instruction prefixed "Your earlier prompt may not have reached you…" (`PA.rs:830-838`)
- [ ] 7 Unclassified (Fixture, Command Code — always `unknown`): native `seal_stable_report` once (len, mtime) unchanged 15 s (`PA.rs:752-772`), then `poll_unclassified`

### 7.4 Harness-specific completion
- [ ] **Claude (read-only)** (`native_result.rs:57-164`): merge into any existing `--settings` (inline JSON or path, removed from argv) `hooks.Stop += {"hooks":[{"type":"command","command":"'<exe>' --home '<home>' __native-result <task> <rev> claude","timeout":20}]}`; write `runs/<task>-r<rev>.claude-settings.json`; pass `--settings`. Hook stdin ≤ 2 × max_bytes; requires `hook_event_name=="Stop"`, `session_id == attempt` (hence `--session-id`), `cwd` canonicalises to the workspace; reads `last_assistant_message`
- [ ] **Cursor (read-only)** (`native_result.rs:87-247`): project `<ws>/.cursor/hooks.json`, symlinked dir/file refused; merge `{"version":1,"hooks":{}}`, append `afterAgentResponse`; save original + generated bytes to `runs/<task>-r<rev>.cursor-hook.json`; input needs `hook_event_name=="afterAgentResponse"` and a `workspace_roots[]` entry canonicalising to the workspace; reads `text`; **restore** on submit and archive only if the file still equals the generated bytes (put original back, or remove file and dir)
- [ ] **Marker extraction** (`native_result.rs:33-48,107-112`): last BEGIN (`rsplit_once`), first END after it, trim `\r\n`; body non-empty, ≤ max_bytes; same body again = no-op, different = "immutable"; restore Cursor hook, write atomically, seal; `__native-result` always prints `{}` and swallows errors (`main.rs:384-393`)
- [ ] **Codex**: no worker completion hook; Codex hooks are **owner-side** (see [owner and delivery](owner-and-delivery.md)); inside a worker worktree the hook prints `{}` and only records the rollout path (`hook.rs:29-37`); readiness uses the screen override because Herdr reports a fresh Codex as `working`
- [ ] **Command Code / Fixture**: status always `unknown` → stable report or `__seal-report`; Command Code skips the start status gate (`native.rs:797`)
- [ ] **Devin, OpenCode, Copilot, Gemini, Pi, Cline**: generic report → seal → idle reminder path
- [ ] **OMP via omp-role** (`omp_adapter.rs:97-452`)
  - [ ] report `runs/<task>-r<rev>.omp-report.md`, must not pre-exist
  - [ ] launch `python3 <launcher> default <agent> --cwd <ws> --task <slug> --reuse-worktree-objective <slug> --reuse-worktree-owner <agent> --expected-report <report> [--model] [--effort]`; stdout = receipt path; receipt `codex_prompt_marked == false`
  - [ ] prompt `omp-prompt --expected-report <report> <agent> "<prompt>\n\nWrite the final result as Markdown to <report> before finishing."` → `{"status":"prompted","target":agent}`
  - [ ] done when `state_change_seq` > initial, status idle/done, report exists; poll 200 ms
  - [ ] fail on `blocked`, on any change of name/kind/pane/terminal/session, or a decreasing sequence (`:223-281`)
  - [ ] report read: regular, non-empty, dev/ino/len unchanged between stat and open (`:283-307`)
  - [ ] recovery contract `~/.omp/agent/callbacks/contracts/<agent>.json` (`:373-452`)

### 7.5 agentdriver additions for the same scope
- [ ] Hook-based idle for Claude (`Stop`), Codex (`notify`, de-duplicated), Cursor (`stop`/`afterAgentResponse`), Devin (`Stop`), OpenCode (`session.execution.succeeded`) as an extra signal source next to brgr's report contract ([harnesses](../harnesses.md))

## 8. Session server, ownership, cancel

- [ ] **`__session <task>`** (`native.rs:73-205`): detached, `process_group(0)`, cwd = home (not the worktree), stdout/stderr → `runs/<task>.session.log`; single instance via `try_lock runs/<task>.session.lock`; ≤ 24 h; exits if the store is gone; each 1 s tick: cancellation → stop descendants + close pane; recovery every 5th tick; settled → `cleanup_settled`; else deliver messages
- [ ] **Identity on every action** (`lifecycle.rs:197-243`): task, revision, attempt, `cleanup != closed`, store attempt→task, host `pane_id` + `terminal_id` — catches pane-id reuse
- [ ] **`external_alive`** (`lifecycle.rs:253-307`): digest matches; or native host alive + verified; or `agent get <name>` matches pane and terminal
- [ ] **`collect_recovered`**: recover a result with no live pane when the digest is set and the report matches (`lifecycle.rs:309-351`, test `tests.rs:163-192`)
- [ ] **Cancel / deadline** `stop_external` (`lifecycle.rs:108-195`): triggered by store cancellation, `runs/<task>.cancel` file, or the deadline passing; `keep_pane` → SIGTERM host pid, else close pane; commit Cancelled/Failed with the observed session model
- [ ] Failed pane run without a report closes its pane unless `--keep-pane` (`PA.rs:412-416`)
- [ ] Owner notes via `"$BRGR_BIN" --json message send <task> --to owner --kind note --body …` reading `message_id`, stderr fallback (`PA.rs:949-974`); withdrawn when the agent is ready again (`PA.rs:913-934`)

## 9. Headless vs native

- [ ] Mode request Pane (default) / Headless / Auto; Auto → pane if a caller pane exists, else headless with the process adapter, else error (`S/execution_mode.rs:4-50`, `admission.rs:611-619`); `--headless` conflicts with `--mode` (`admission.rs:595-609`)
- [ ] A revision of a headless run stays headless (`admission.rs:534-537,589-593`)
- [ ] Pane mode requires an interactive recipe and no print-only effort/model (OpenCode exception) (`PA.rs:136-154`), and an exact source pane — "a pane run needs an exact Herdr source pane" (`admission.rs:909-925`); false for omp-role (`:982`)
- [ ] Pane mode: calling-option argv goes into agent args, not the headless manifest (`supervision.rs:86-107`); a pane run without a Candidate gets `close_leftover_pane` (`:139-145`); claim grace 1 min (worker pane) vs 5 s (`:347-366`)
- [ ] Headless execution itself stays in brgr

## 10. Platforms

- [ ] Ancestor `HERDR_PANE_ID`: Linux `/proc/<pid>/environ` (≤ 1 MiB); macOS `/bin/ps -E -ww -o command= -p <pid>`, last ` HERDR_PANE_ID=` (`caller_pane.rs:438-465`)
- [ ] macOS home has a space (`~/Library/Application Support/brgr`) → trust path matching survives wrapping at spaces
- [ ] macOS: `/usr/bin/caffeinate -i -w <pid>` during start; no-op elsewhere (`sleep_guard.rs:16-31`)
- [ ] Linux: concurrent `git worktree add` "failed to read .git/worktrees/" retried as a lock error (`workspace.rs:904-918`) — brgr side
- [ ] Both: `/bin/ps -A -o pid=,ppid=[,comm=]`, `/bin/ps -ww -p <pid> -o lstart=`, `/bin/kill -TERM`, `O_NOFOLLOW`

## 11. Timing constants

| Constant | Value | Ref |
|---|---|---|
| [ ] `POLL` | 1 s | `PA.rs:59` |
| [ ] `REPORT_GRACE` | 15 s | `PA.rs:62` |
| [ ] `UNSEEN_WORK_GRACE` | 60 s | `PA.rs:65` |
| [ ] `STILL_POLLS` | 10 | `PA.rs:70` |
| [ ] `SHELL_READY` / `SHELL_POLL` | 20 s / 250 ms | `PA.rs:121-122` |
| [ ] `READY_SETTLE` | 1 s | `native.rs:640` |
| [ ] `SCREEN_DEADLINE` | 3 min | `screens.rs:28` |
| [ ] `PRESS_BUDGET` | 5 per rule | `screens.rs:481` |
| [ ] `REDRAW` | 1500 ms | `screens.rs:25` |
| [ ] agent start busy retry | 250 ms up to 20 s | `PA.rs:429-469` |
| [ ] native host start | 250 ms up to 20 s | `native.rs:741-755` |
| [ ] Codex caller wait | 700 ms up to 40 s | `caller_pane.rs:115-177` |

Debug-only overrides: `BRGR_TEST_REPORT_GRACE_MS`, `BRGR_TEST_CALLER_WAIT_MS`, `BRGR_TEST_SCREEN_DEADLINE_MS`.
