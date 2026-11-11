# Captured screens

Each file is a pane's visible text as the host read it, named
`<harness>-<version>-<what>.txt`. Rule tests load them; a synthetic screen is
labelled as such in its test instead of living here.

For screens captured here, shell prompts outside the agent's UI are cut and
local paths are replaced by `/work/task`; nothing else is edited.

| File | Captured with | What it shows |
|---|---|---|
| `claude-2.1.319-folder-trust.txt` | Herdr 0.9.0, macOS, `pane read --source visible` | Folder-trust menu. The selection starts on **No, exit**. |
| `devin-3000.12.43-folder-trust.txt` | tmux 3.4, Ubuntu 24.04, `capture-pane -p` | Folder-trust menu. The selection starts on **Yes, trust**. No rule recognises it yet (`❭` cursor, `1 Yes` without a dot). |

Copied verbatim from brgr (`testdata/fixtures/screens/`, v2.15.21) with the
rule tests that load them; their paths are the captured ones, so they are not
redacted:

| File | What it shows |
|---|---|
| `claude-2.1.318-renderer-offer.txt` | First-run fullscreen renderer offer; cursor on "Yes, try it". |
| `codex-0.160-worktree-trust.txt` | Folder trust in a worktree; path and repository root wrapped. |
| `codex-0.166-hooks-review.txt` | Review of new hooks. |
| `codex-0.166-worktree-trust.txt` | Folder trust in a worktree; path with a space, root on two lines. |
| `copilot-1.0.86-trust.txt` | Folder trust in a nested box, path wrapped. |
| `copilot-1.0.86-idle.txt` | Idle screen: nothing to answer. |
| `cursor-2026.10.22-trust-wide.txt` | Workspace trust box in a wide pane. |
| `cursor-2026.10.22-trust.txt` | The same box after it was answered: nothing to answer. |
