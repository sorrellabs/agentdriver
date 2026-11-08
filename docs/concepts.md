# Concepts

## Host

A **host** is the program that owns terminal panes: Herdr, Orca, tmux, cmux,
and so on. agentdriver never draws a terminal itself; it asks the host to
create a pane, run a command in it, show its screen and type into it.

Hosts differ in what they can do. agentdriver describes each one with a
capability set:

| Capability | Meaning |
|---|---|
| `spawn` | create a pane / split / tab running a given command in a given directory |
| `read` | return the visible screen text of a pane |
| `send` | paste text into a pane |
| `keys` | send named keys (Enter, Esc, arrows, Ctrl-C) |
| `close` | close a pane and its process |
| `idle` | the host itself reports agent state (busy / idle / waiting) |
| `adopt` | attach to a pane agentdriver did not create |

A host that lacks `idle` relies on the harness's own signal (below). A host
that lacks `read` can still run agents whose answer arrives through a harness
hook, but cannot resolve on-screen prompts.

## Harness

A **harness** is the agent program: Claude Code, Codex, Cursor CLI, Devin,
OpenCode, OMP, Cline, and others. A harness adapter knows how to:

- build the launch command line and environment,
- install a turn-finished signal (a hook, notify command or plugin) without
  touching the user's global configuration,
- recognise the prompts the TUI can show (folder trust, hook trust, update
  offers) and which answer, if any, is allowed,
- extract the final answer of a turn.

## Session

A **session** is one harness running in one host pane, created by `spawn` or
taken over by `adopt`. It has a stable id, the host's pane handle, the harness
kind, the working directory, and a small state directory where signals land.

## Verbs

| Verb | Does |
|---|---|
| `spawn(host, harness, cwd, options)` | start the harness in a new pane; returns a session |
| `adopt(host, pane, harness)` | take over an existing pane |
| `send(session, text, submit)` | type text into the agent's input box; optionally submit |
| `keys(session, keys…)` | press named keys |
| `read(session)` | current screen text |
| `wait(session, until, timeout)` | block until `idle`, `text(regex)`, `exit`, or timeout |
| `result(session)` | the last finished turn's answer, when the harness reports it |
| `status(session)` | busy / idle / waiting-for-input / exited, with the signal source |
| `close(session)` | close the pane and end the process |

Every `wait` has a mandatory timeout. A timeout is a normal outcome, not an
error, and reports the last screen and the last signal seen.

## Idle

"Idle" means *the agent finished its turn and is waiting for the next prompt*.
A quiet screen is not idle: agents pause while thinking, while a tool runs, or
while they wait on a question to the user.

agentdriver takes idle from the most reliable source available, in order:

1. **Harness signal** — a Stop hook, notify command or plugin event, written
   to the session's state directory. Exact and cheap.
2. **Host signal** — the host's own agent state (Herdr's agent status, Orca's
   `wait --for tui-idle`).
3. **Screen rules** — harness-specific patterns on the visible screen, used
   for prompts and as a fallback.

`status` always says which source produced its answer.

## Prompts

On start, and sometimes mid-turn, TUIs show prompts: trust this folder, trust
these hooks, install this update. agentdriver detects them through screen
rules. **By default it answers none of them** and reports
`waiting-for-input` with the prompt text. A caller can opt in to specific
answers per rule and per workspace — for example "accept the folder-trust
prompt, but only for the directory I spawned in". See
[decision 4](decisions.md#4-prompt-answers-are-opt-in).
