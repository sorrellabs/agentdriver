# Captured screens

Each file is a pane's visible text as the host read it, named
`<harness>-<version>-<what>.txt`. Rule tests load them; a synthetic screen is
labelled as such in its test instead of living here.

Shell prompts outside the agent's UI are cut and local paths are replaced by
`/work/task`; nothing else is edited.

| File | Captured with | What it shows |
|---|---|---|
| `claude-2.1.319-folder-trust.txt` | Herdr 0.9.0, macOS, `pane read --source visible` | Folder-trust menu. The selection starts on **No, exit**. |
| `devin-3000.12.43-folder-trust.txt` | tmux 3.4, Ubuntu 24.04, `capture-pane -p` | Folder-trust menu. The selection starts on **Yes, trust**. |
