# brgr coverage — screen rules

Source: `brgr/crates/brgr-cli/src/pane_adapter/screens.rs` (rules, loop, log)
and `pane_adapter/native.rs` (path matching, readiness). brgr v2.15.21.
Paths below are relative to `brgr/crates/brgr-cli/src/`.

Every rule is opt-in in agentdriver ([decision 4](../decisions.md#4-prompt-answers-are-opt-in));
brgr enables all of them. An item is done when the behaviour matches exactly and
the ported test (§10) passes.

## 1. Text helpers

- [ ] **`squash(text)`** removes every `char::is_whitespace()` char, newlines included; case kept (`native.rs:13`)
- [ ] **Menu parsing** `menu_options(screen)` (`screens.rs:561-585`)
  - [ ] per line: `trim` → strip `│ ┃ ║` → `trim`
  - [ ] cursor glyphs `›` `❯` `>` `▸` at line start mark the option `selected`, rest `trim_start`ed
  - [ ] the rest must be one or more ASCII digits then `.` or `)`; text after it, trimmed, is the option text
  - [ ] unnumbered lines are not options (so the unnumbered Claude trust form yields none)
- [ ] **`keys_between(from, to)`**: `down` × (to−from) or `up` × (from−to), empty when equal; callers append `enter` (`:179-182`)
- [ ] **`is_skip(option)`** (`:587-601`), lowercased:
  - [ ] false if it contains `update now` or `install`
  - [ ] true if it contains `skip`, `not now`, `later`, `no thanks`, `dismiss` or `remind me`
  - [ ] the **first** matching option wins
- [ ] **`is_update_offer(lower)`** on the whole lowercased screen (`:161-167`): (`update` or `upgrade`) and (`available` or `new version` or `newer version` or a version number)
- [ ] **`has_version_number`** (`:170-176`): split on chars that are neither digit nor `.`; a token, `.`-trimmed, with ≥ 3 non-empty dot parts. `0.2.0`, `v2.0.15` yes; `2.0`, `1.2` no
- [ ] **`has_continue_notice(screen)`** (`:209-232`)
  - [ ] per line: `trim` → strip `│ ┃ ║` → `trim` → strip leading `• · * - › ❯` → `trim_start` → lowercase
  - [ ] notices: `press enter to continue`, `press return to continue`, `press any key to continue`
  - [ ] text before the notice is empty or ends with `.` `!` `?`
  - [ ] text after it is empty or only closing punctuation: ASCII punctuation or `… ⋯ ” ’ » 。 ！ ？ 、` (`:234-240`)
- [ ] **Screen excerpt** `head(screen)` returns the **last 8** non-empty lines, trimmed, joined with ` | ` (`:613-620`; `tail` is an alias)
- [ ] **Redaction markers**: error text is cut at `last screen:` and at `no brgr rule answers`, so quoted screens never leave the machine (`screens.rs:604`, `error_memo.rs:30,35`) — agentdriver must keep these strings stable or expose them

## 2. Spawn-path matching (folder trust guard)

- [ ] **`trust_workspace_matches`** requires `squash(screen)` to contain `Yes,Itrustthisfolder` (case-sensitive), then rows **or** narrow-pane match (`native.rs:18-24`)
- [ ] **Row match** (`native.rs:29-57`)
  - [ ] split at the first `Accessing workspace:`; missing → no match
  - [ ] expected = workspace path with trailing `/` removed
  - [ ] walk following lines, trimmed, skipping leading empty rows; stop at an empty row
  - [ ] after the first row, drop one leading space from the remainder (wrap at a space)
  - [ ] each row must be a prefix of the remainder; a row ending in `/` may match only as the final piece
  - [ ] match only if ≥ 1 row and the remainder is empty — longer or different paths fail
- [ ] **Narrow-pane match** (`native.rs:62-70`): squashed text after `Accessingworkspace:` must start with the squashed path, then (after stripping `/`) with `Quicksafetycheck`
- [ ] Paths containing spaces, wrapped at a space, and in very narrow panes all match; prefixes of the path never do (`native.rs` tests)

## 3. Rules, in evaluation order

`classify(screen, workspace)` → `Clear` | `Resolve{rule, keys}` | `Unknown(excerpt)`; first hit wins (`screens.rs:56-157`).

### 3.1 `workspace-trust` — Claude Code (`:57-84`)
- [ ] Detect with `trust_workspace_matches` (§2)
- [ ] Numbered menu present with a selected option and one starting `Yes, I trust this folder` → `keys_between(selected, yes)` + `enter`
- [ ] Unnumbered form: if squashed screen contains `❯Yes,Itrustthisfolder`, `›Yes,…` or `>Yes,…` → `enter`; otherwise `down enter` (cursor assumed on `No, exit` above)
- [ ] Prompt for another folder does not match here and is refused by 3.5

### 3.2 `codex-trust` — Codex (`:85-89`, `:246-285`)
- [ ] Detect: squashed screen contains `Trustthisfolder?` and `Trustandcontinue`
- [ ] Path guard: after `Folderaccess`, strip the squashed workspace (trailing `/` removed), then leading `/`; the rest must start with
  - [ ] `Trustthisfolder`, or
  - [ ] `Note:` — then the text between `repositoryroot:` and `Trustthisfolder` (trailing `/` removed) must equal the **primary checkout** of the workspace: the first `worktree` line of `git worktree list --porcelain`, squashed (`workspace.rs:563`). Primary unknown → refuse
- [ ] `…repo-evil`, `…repo/sub` fail
- [ ] Accept via `trust_choice` (§4) the option starting `Trust and continue`; guard failure → `Unknown`

### 3.3 `copilot-trust` — Copilot CLI (`:90-92`, `:292-310`)
- [ ] Flatten: drop whitespace and `│ ╭ ╮ ╰ ╯ ─`; must contain `Confirmfoldertrust` and `Doyoutrustthefilesinthisfolder?`
- [ ] Path = text between `Confirmfoldertrust` and `Copilotcanread`, trailing `/` removed, must equal the squashed workspace exactly
- [ ] Accept the option whose text is exactly `Yes` — never `Yes, and remember this folder`
- [ ] Mismatch → `Unknown`

### 3.4 `cursor-trust` — Cursor Agent (`:93-95`, `:317-349`)
- [ ] Same flattening as 3.3; must contain `WorkspaceTrustRequired` and `[a]Trustthisworkspace`
- [ ] **Already answered**: also contains `Trustingworkspace` → no match (pressing again would type into the agent's input)
- [ ] Path = text between `Doyoutrustthecontentsofthisdirectory?` and `[a]Trustthisworkspace`, trailing `▶ ▸ ❯ › >` then `/` removed, equals the squashed workspace
- [ ] Match → key `a`; mismatch → `Unknown`

### 3.5 Trust refusal — any harness (`:96-100`)
- [ ] `squash(lowercased screen)` contains `trustthisfolder` and no rule above matched → `Unknown`. A trust prompt for another folder is never accepted

### 3.6 `update-applied` — OpenCode and any self-updated harness (`:101-113`)
- [ ] Lowercased screen contains `update complete` and `restart`, and some line, trimmed and lowercased, is exactly `ok` or `esc`
- [ ] Press `esc` (Enter on `ok` quits OpenCode)
- [ ] Prose such as "Update complete, please restart" without a bare `ok`/`esc` line → `Clear`

### 3.7 Accepting menus (`:114-120`, `accepting_option` `:187-207`)
Needs a selected numbered option. Checked in order; a keyword hit without the option falls through.
- [ ] `bypass-warning` — Claude: screen has `bypass permissions mode` → option starting `yes, i accept`
- [ ] `codex-hooks-trust` — Codex: screen has `hooks need review` → option starting `trust all`
- [ ] `mcp-trust` — Claude: screen has `mcp server` → option starting `use this mcp server`, else `use this` (single-server choice preferred)
- [ ] Keys: `keys_between(selected, target)` + `enter`

### 3.8 `update-offer-skip` — any (`:121-133`)
- [ ] Selected option exists, `is_update_offer`, and an `is_skip` option → move to it + `enter`
- [ ] Runs **before** the continue-notice rule, so a stray "Press Enter" never confirms "Update now"
- [ ] Update menu without a skip option → `Unknown`, never guessed

### 3.9 `renderer-offer-skip` — Claude Code 2.1.318 first run (`:134-145`)
- [ ] Selected option exists, screen has `try the new fullscreen renderer?`, an `is_skip` option (`Not now`) → move + `enter`

### 3.10 `continue-notice` — any (`:146-155`)
- [ ] Only when **no** selected numbered option is on screen
- [ ] `has_continue_notice` → `enter`; otherwise `Clear`

### 3.11 Unknown menu (`:156`)
- [ ] A selected numbered option remains and nothing matched → `Unknown(excerpt)`; nothing is pressed

### 3.12 `trust_choice(screen, shown, rule, accept)` (`:354-372`)
- [ ] target = first option where `accept(text)`; `!shown` or no target → `Unknown`
- [ ] selected = selected option's index, or target when none is selected
- [ ] keys = `keys_between(selected, target)` + `enter`

## 4. Shell rule: `shell-update-offer` (oh-my-zsh)

- [ ] Detect: lowercased screen contains `[oh-my-zsh]` and `would you like to update?` (`:410-413`)
- [ ] Runs on every new pane **before anything is typed** — worker panes (`pane_adapter.rs:577`) and head panes (`pane_adapter/head_pane.rs:75`)
- [ ] Loop up to 5 s, polling every 100 ms (`SHELL_DRAW`, `SHELL_DRAW_POLL`)
  - [ ] unreadable screen → stop
  - [ ] offer shown and not answered → `send-keys n` once; log `shell-update-offer n`; on failure print `brgr pane mode · could not skip a shell update offer: …`
  - [ ] non-blank screen equal to the previous read → count; stop at 2 unchanged reads; any change resets the count

## 5. One resolve step

`resolve(herdr, pane, run, paths, status)` (`screens.rs:378-404`):
- [ ] Host status `working` → `Clear`; nothing is read or pressed
- [ ] Read the screen: `herdr pane read <pane> --source visible --lines 300` (`pane_adapter.rs:1092`); read failure → `Clear`
- [ ] `classify`; on `Resolve` send **all keys in one** `pane send-keys <pane> k1 k2 …` call
- [ ] Send failure → `brgr could not answer the {rule} screen: {failure}`
- [ ] Print `brgr pane mode · {rule}: pressed {keys}`; append to the rule log (§6)
- [ ] Sleep 1500 ms (`REDRAW`) after a press so the same prompt is not answered twice

## 6. Rule log

- [ ] Path: `$BRGR_HOME/runs/<task>-r<revision>.screens.log` (`:451-469`) — agentdriver takes the path from the caller
- [ ] One appended line per press: `{unix_secs} {pane} {rule} {keys joined by space}`, e.g. `1731100000 w1:p2 update-applied esc`
- [ ] Create + append; write failure prints `brgr pane mode · could not log the {rule} key press: {error}` and is not fatal
- [ ] Prompt submission is not logged

## 7. Deadlines and press budget (`ScreenWatch`, `:472-559`)

- [ ] Unknown-screen deadline 3 min (`SCREEN_DEADLINE`); debug builds read `BRGR_TEST_SCREEN_DEADLINE_MS` (`:31-40`) and pass it through the pane env allowlist (`pane_adapter.rs:206`) — agentdriver: caller-set deadline, test override
- [ ] Press budget 5 per rule name; the 6th press fails: `the {kind} agent kept showing a screen brgr answers ({rule}); 5 presses did not clear it`
- [ ] `observe(Unknown)` starts the clock on first sight and **does not restart it when the unknown text changes**
- [ ] `observe(Clear)` clears the clock and all press counts; `observe(Resolve)` clears the clock only
- [ ] `observe_blocked` treats a host-`blocked` screen with no rule as Unknown
- [ ] `reset()` clears both; `restart_clock()` clears the clock, keeps counts
- [ ] Deadline failure: `the {kind} agent stayed on a screen no brgr rule answers for {secs}s: {excerpt}`

## 8. Where the loop runs

Constants: `POLL` 1 s, `STILL_POLLS` 10, `SHELL_READY` 20 s, `SHELL_POLL` 250 ms, `READY_SETTLE` 1 s. Host status from `herdr agent get <name>` → `/result/agent/agent_status`, default `unknown` (`pane_adapter.rs:1288`).

- [ ] **Startup wait for a host-launched agent** `settle` (`pane_adapter.rs:478-516`), each poll:
  - [ ] Resolve → count the press
  - [ ] Unknown → notify the owner once (text below), then observe
  - [ ] Clear + `idle`/`done` → withdraw notices, return
  - [ ] Clear + `working` → reset
  - [ ] Clear + other → observe_blocked
  - [ ] Owner text: `The {kind} agent in Herdr pane {pane} is on a screen no brgr rule answers: {excerpt}. Use brgr input if it needs a key.` — sent as an owner note, or printed as `brgr pane mode · the {kind} agent in pane {pane} needs attention: …` (`:951`)
- [ ] **Main run loop** `drive` (`pane_adapter.rs:~590-690`)
  - [ ] `blocked` → resolve; Resolve → withdraw, phase `working`, count, sleep, continue; Unknown/Clear → announce once, phase `awaiting_input`, observe / observe_blocked
  - [ ] Announce text (`:937`): `The {kind} agent in Herdr pane {pane} is waiting for an approval while working (for example a folder trust or MCP prompt). This is native input, not a mailbox question. Read the pane and use brgr input TASK --key KEY or --text TEXT. The task continues once unblocked.`
  - [ ] `working` → reset; `idle`/`done` → report handling then reset; other → unclassified path
- [ ] **Unclassified agents** `poll_unclassified` (`:524-563`)
  - [ ] count consecutive identical reads; under 10 (~10 s) → restart clock, withdraw blocked notice, press nothing
  - [ ] once still → resolve; Resolve → count, reset stillness; Unknown → announce, observe; Clear → restart clock
  - [ ] menu-like output from an agent that is still moving is never pressed
- [ ] **First prompt** `send_first_prompt` (`native.rs:696-720`): retry sending for up to 20 s every 250 ms; between tries resolve with current status (Resolve → count, Unknown → observe, Clear → reset)
- [ ] **Native start loop** `start_native` (`native.rs:~757-800`): Resolve → count, reset settle, sleep, continue; Unknown → reset settle, notify once (`The {kind} agent is on a screen no brgr rule answers: {excerpt}. Use brgr input if it needs a key.`), observe; Clear → reset watch
- [ ] **Report missing** failure: `the agent finished without writing its report to {path}; last screen: {excerpt}`

## 9. Ready-for-input on screen

- [ ] **`menu_on_screen`** (`pane_adapter.rs:876-899`): any line, `trim_start` only (borders **not** stripped), starting with `› ❯ > ▸`, optional space, then digits + `.`/`)`. Blocks readiness
- [ ] **`editor_ready(screen, kind)`** (`native.rs:442-462`)
  - [ ] false if the screen contains `trust this folder`
  - [ ] brand required: `Claude Code` for claude, `Command` for command-code, none otherwise
  - [ ] a line, after trim + strip `│ ┃` + trim, starts with `❯`, `›` or `>`; its rest must not start with `exec `, `No,`, `Yes,` and must not be a numbered option
- [ ] **Codex override** `screen_overrides_working` (`native.rs:412-417`): status `working`, kind codex, `editor_ready`, no `esc to interrupt` → treated as ready
- [ ] **`native_ready`** (`native.rs:419-440`): false on a menu; true on `idle`/`done` or the Codex override; other known status false; `unknown`/error → process alive and `editor_ready`
- [ ] **Start readiness** (`native.rs:~780-800`): status idle/done/none/unknown (or working-but-idle, or kind command-code) **and** (`editor_ready` or host agent of this kind idle/done) **and** no menu; must hold continuously for 1 s
- [ ] **Self-update off at launch** (`native.rs:545`): `OPENCODE_DISABLE_AUTOUPDATE=1` for OpenCode, `DISABLE_AUTOUPDATER=1` for Claude; others rely on `update-offer-skip`

## 10. Tests to port

Fixtures: `brgr/testdata/fixtures/screens/`
- [ ] `claude-2.1.318-renderer-offer.txt`
- [ ] `codex-0.160-worktree-trust.txt` (path and root wrapped)
- [ ] `codex-0.166-hooks-review.txt`
- [ ] `codex-0.166-worktree-trust.txt` (path with a space, root on two lines)
- [ ] `copilot-1.0.86-trust.txt` (nested box, wrapped path)
- [ ] `copilot-1.0.86-idle.txt` (→ Clear)
- [ ] `cursor-2026.10.22-trust-wide.txt`
- [ ] `cursor-2026.10.22-trust.txt` (already answered → Clear)
- [ ] inline `OPENCODE_UPDATED` (`screens.rs:628-649`) and `CODEX_TRUST` (`:699-709`)

Unit tests in `screens.rs` (workspace `/repo/task`):
- [ ] applied self-update dismissed with `esc`, not `enter`
- [ ] MCP auth toast alone does not block
- [ ] Claude trust for the task workspace: `down enter`; cursor on Yes: `enter`
- [ ] numbered Claude trust menu: `enter`; cursor on option 2: `up enter`
- [ ] Codex trust for the workspace: `enter`; cursor on option 2: `up enter`
- [ ] Codex trust for another folder / longer path → Unknown
- [ ] Codex trust in a task worktree naming its repository (0.160 and 0.166) → `enter`
- [ ] Codex trust whose root is not the worktree's repository (primary none, another repo, the worktree itself) → Unknown
- [ ] trust prompt for another folder → Unknown
- [ ] Cursor trust for the workspace → `a`; another folder (prefix, parent, unrelated) → Unknown; answered box still drawn → Clear
- [ ] Copilot trust for the workspace → `enter` once; another folder → Unknown; idle screen → Clear
- [ ] update offer skipped by moving to skip (`down enter`); cursor already on skip (`enter`); skip as a later option
- [ ] update menu with a continue line is skipped, not confirmed
- [ ] update menu without skip → Unknown
- [ ] update-offer wording: true for "a new version of codex is ready, update?", "upgrade to v2.0.15"; false for "update the 2.0 schema", "update readme.md and 1.2 notes"
- [ ] agent questions about updating ("Update the schema now?", "Upgrade the dependencies too?", "> 1. update the config, then skip later steps") → Unknown
- [ ] renderer offer declined (`down enter`); a renderer question without the offer → Unknown
- [ ] Codex hook review → `down enter`
- [ ] bypass warning accepted → `down enter`
- [ ] new MCP server trusted → `down enter`; single-server choice preferred
- [ ] prose about an update is not the applied-update dialog → Clear
- [ ] `│ • Startup complete. Press Enter to continue… │` → `enter`
- [ ] quoted / discussed / "Then it says …" / "Press Enter to continue or Esc to cancel" → Clear
- [ ] continue notice dismissed with `enter`
- [ ] menu no rule covers → `Unknown("Choose a theme | › 1. Dark | 2. Light")`, never pressed
- [ ] ordinary screen `some output\n❯ \n` → Clear
- [ ] shell update offer declined with `n` before typing; log ends `w1:p2 shell-update-offer n`
- [ ] plain shell prompt `❯ update the docs? [Y/n]` → nothing pressed
- [ ] resolve sends `pane send-keys w1:p2 esc`; log ends `w1:p2 update-applied esc`
- [ ] resolve presses nothing while working
- [ ] 6th press of the same rule fails; Clear resets the count
- [ ] unknown screen fails after the deadline with its text; deadline is 3 min; Resolve and Clear reset it

Unit tests in `native.rs`:
- [ ] Codex folder prompt is not a ready composer
- [ ] OpenCode and Claude launch with self-update disabled
- [ ] trust path keeps spaces and rejects prefixes
- [ ] trust prompt in a very narrow pane matches
- [ ] trust path wrapped at a space matches

Integration (`brgr/crates/brgr-cli/tests/cli_contract.rs`, fake `herdr` ~`:1265-1305`):
- [ ] menu-like output from an unclassified agent is not pressed mid-run — only the prompt's own Enter (`:1894`)
- [ ] unclassified agent stuck on "Pick a region" fails at the deadline (500 ms override) with `no brgr rule answers` and the menu text (`:1914`)
- [ ] native trust for the owned workspace resolved with `pane send-keys w9:p2 down enter`, no owner messages (`:2234`)
- [ ] update offer skipped (`down enter`) before `agent prompt`, no bare `enter` before it (`:2743`)

## Exact behaviours easy to miss

- [ ] the excerpt is the **last** 8 rows, not the first
- [ ] the unknown-screen clock is not restarted by a changing unknown screen
- [ ] `continue-notice` only fires with no selected numbered option on screen
- [ ] unnumbered Claude trust without a glyph before Yes assumes the cursor is one row above
- [ ] `menu_on_screen` keeps box borders; `menu_options` strips them
- [ ] Claude, Codex, Copilot trust matching is case-sensitive; the other-folder refusal is lowercased
