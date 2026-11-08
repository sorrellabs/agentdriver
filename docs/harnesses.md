# Harnesses

How each harness tells the outside world that a turn has finished, and how a
harness adapter installs that signal **without touching the user's global
configuration**: per launch through flags, or per workspace through project
files that the adapter writes and removes.

Every signal writes one line to the session's state directory and returns at
once. A hook must never block the agent.

## Tier 1

### Claude Code

- Signal: `Stop` hook.
- Install: per launch with `--settings <file>` containing
  `{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"…"}]}]}}`.
- Result: the hook payload includes `last_assistant_message`.

### Codex

- Signal: `notify` command, or the `Stop` lifecycle hook.
- Install: per launch with
  `-c 'notify=["sh","-c","… >> $STATE/codex","x"]'`, or a project
  `hooks.json` with `SessionStart`, `UserPromptSubmit`, `Stop`.
- Caveat: `notify` fired **twice** on the first turn in testing. The adapter
  de-duplicates by turn.

### Cursor

- Signal: `stop` and `afterAgentResponse` hooks.
- Install: project `.cursor/hooks.json`, `{"version":1,"hooks":{…}}`.
- Caveat: both events fire together at the end of a turn. Use one.
  `afterAgentResponse` carries the answer text.

### Devin

- Signal: `Stop` hook.
- Install: project `.devin/config.json`,
  `{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"…"}]}]}}`.

### OpenCode (2.x)

- Signal: plugin event `session.execution.succeeded`.
- Install: project `.opencode/plugins/<name>.js` in the V2 plugin format:

  ```js
  export default {
    id: "agentdriver.idle",
    setup(ctx) {
      const controller = new AbortController();
      void (async () => {
        for await (const event of ctx.event.subscribe({ signal: controller.signal })) {
          if (event.type.startsWith("session.execution.")) record(event.type);
        }
      })();
      return () => controller.abort();
    },
  };
  ```

- Caveats: OpenCode 2.x rejects V1 plugins ("must export a default definition
  with an id and an effect or setup function"). `session.idle` is never
  emitted in V2; completion is `session.execution.succeeded`.

## Verification (2026-11-08, tmux, macOS)

Each Tier 1 harness ran in a tmux pane on a private socket with its signal
installed as above. Two turns each:

1. a plain answer turn;
2. a tool-using turn (`sleep 6 && ls`, `cat README.md`).

| Harness | Fired at turn end | Fired mid-turn | Notes |
|---|---|---|---|
| Claude Code | once per turn | no | |
| Codex | once per turn (twice on turn 1) | no | needs de-duplication |
| Cursor | once per turn (both events) | no | pick one event |
| Devin | once per turn | no | |
| OpenCode 2.0.20 | once per turn | no | `session.execution.succeeded` |

Not yet covered, and part of the conformance suite before Tier 1 ships:
long and stalled turns, questions asked mid-turn, failed and interrupted
turns, and Linux.

## Tier 2 and experimental

OMP, Cline, Command Code and Pi need their signals measured. Until then their
adapters fall back to screen rules, and `status` reports the source as
`screen`. Copilot CLI and Gemini / Antigravity are screen-rules only.

## Screen rules

Screen rules are harness-specific recognisers for the visible screen:
folder-trust prompts, hook-trust prompts, MCP-trust prompts, update offers,
and the "ready for input" state. brgr's set (about 1,300 lines) is the
starting point. Each rule has a stable name (for example `workspace-trust`,
`codex-hooks-trust`, `mcp-trust`) so callers can opt in to answering it by
name.

A folder-trust prompt is only ever answerable for the directory the session
was spawned in. A trust prompt for any other folder is reported, never
answered.
