# Repository Guidelines

agentdriver drives coding-agent TUIs inside terminal hosts: spawn, read,
send, wait for idle, close. It also replaces brgr: delegated runs, worktrees,
sealed results and owner messages move here
([decision 17](docs/decisions.md#17-agentdriver-replaces-brgr)). Start with [README.md](README.md), then
[docs/concepts.md](docs/concepts.md) and [docs/decisions.md](docs/decisions.md).
The repository is at the design stage; code lands milestone by milestone
([docs/roadmap.md](docs/roadmap.md)).

## Tech Stack

- **Language:** Rust, edition 2024. One toolchain for everyone:
  `rust-toolchain.toml` pins `1.95.0` with `clippy` and `rustfmt`, and
  `rust-version = "1.95"` is the MSRV. Raise both together, in a dedicated
  change, when brgr raises its toolchain.
- **Rust only.** The one exception is a file a harness requires in its own
  language, such as the OpenCode plugin (JavaScript). Keep each one minimal:
  it only appends a signal line and calls nothing else. Store it under
  `assets/<harness>/`, embed it with `include_str!`, and pin it with a test.
  Hook commands call the `agentdriver` binary. Shell scripts are not allowed.
  Other languages use the CLI's JSON output or the MCP server; no
  Python/TypeScript SDK or FFI bindings until a real user needs one.
- **Synchronous core.** The library uses `std::process` and blocking, bounded
  waits; it does not depend on an async runtime. Every host call takes a
  timeout and an output cap. Waiting on several sessions is a single polling
  loop (`wait_any`), not one thread per session. Only the MCP server uses
  `tokio`, and it calls the core through `spawn_blocking`.
- **One package, `agentdriver`.** It contains the library, the `agentdriver`
  binary, and the MCP server as `agentdriver mcp`.
  - `cargo install agentdriver` installs the CLI and MCP server.
  - Library users write `agentdriver = { version = "0.1", default-features = false, features = [...] }`
    to leave out the CLI and MCP dependencies.
  - Features: `cli` and `mcp` (default), one per host (`herdr`, `tmux`,
    `orca`, …, Tier 1 hosts in default). Harness adapters are not
    feature-gated; they are data and small parsers.
  - Split into more crates only when a real user needs one part without the
    rest, and say why in `docs/decisions.md`.
- **Dependencies:** `serde`/`serde_json` for wire data, `thiserror` for
  library errors, `anyhow` only in the binary, `clap` (derive) for the CLI,
  `rmcp` for MCP, `libc` where a syscall is needed. Add a dependency only when
  the standard library cannot do the job in a few lines; name the reason in
  the PR.
- **Platforms:** macOS 15+ on Apple Silicon and x86_64 Linux are Tier 1.
  aarch64 Linux builds in CI. Native Windows is not supported.

## Project Structure

See [docs/architecture.md](docs/architecture.md) for the module map.

The crate is hexagonal, and `tests/architecture.rs` enforces the direction.

- `src/domain/` — shared vocabulary. Uses no other layer.
- `src/ports/` — `Host`, `Harness`, `Store`, `Clock`. Uses only `domain`.
- `src/core/` — `Driver`, `Session`, `Wait`, `classify`. Uses `ports` and `domain`.
  No files, processes, threads, real time or environment variables here: add a
  port instead.
- `src/adapters/` — hosts (one per feature), harness recipes and their rules,
  stores, clocks, and `exec.rs`, the only place that runs external commands.
- `src/bin/agentdriver/` — CLI and MCP; no logic the library lacks, and the only
  code that may use clap, rmcp or tokio.

A new host or harness is a new adapter. If it needs a change in `core`, the port
is wrong: fix the port. Never add a list of hosts or harnesses to `core`.

- `tests/` — black-box tests; `testdata/screens/` captured screens;
  `testdata/fake-host/` and `testdata/fake-agent/` the deterministic fakes.
- `docs/` — design, research, the [brgr coverage checklist](docs/brgr-coverage/README.md).

## Build, Test, and Development Commands

- `cargo build`
- `cargo test` — unit, contract and fake-host tests. Needs no real host or harness.
- `cargo test --all-features`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo tree --no-default-features -e normal` — must not list clap, anyhow,
  rmcp or tokio.
- `cargo test --features live -- --ignored` — live conformance against real
  hosts and logged-in harnesses. Opt-in, never in CI.

## Coding Style

Standard `rustfmt`. Small modules, explicit enums, typed identifiers
(`SessionId`, `PaneRef`). `snake_case` functions and modules, `PascalCase`
types, `SCREAMING_SNAKE_CASE` constants. No `unsafe` outside a reviewed
syscall wrapper. Execute argv arrays, never shell strings; when a host only
accepts a command string (for example `pane run`), quote through one tested
function. Library errors are typed (`thiserror`) and name the layer, verb,
external command and exit status; context strings only at the CLI boundary.

## Testing

- Unit tests beside the code; black-box tests in `tests/`.
- Name tests after behaviour: `trust_prompt_for_another_folder_is_reported_not_answered`.
- Every screen rule is pinned by a test on a captured screen in
  `testdata/screens/`, named `<harness>-<version>-<what>.txt`. A synthetic
  screen is labelled as such in the test.
- Every host adapter passes the shared adapter contract suite against the fake
  agent.
- A brgr behaviour is ported together with its brgr test; tick the item in
  `docs/brgr-coverage/` in the same change.

## Versioning

- SemVer in the 0.x range, starting at `0.1.0`. 1.0 happens only when the
  owner says so.
- **Default bump is patch.** Use a minor bump (`0.1.x → 0.2.0`) only for a
  change that breaks the public Rust API, the CLI's JSON output, or the MCP
  tool schemas — Cargo treats a 0.x minor as breaking, so a breaking patch
  would break every dependent, brgr included.
- Wire formats carry their own `schema` version (`"agentdriver.session/1"`).
  Readers accept older versions they know; a new field is not a new version,
  a removed or changed field is.
- Batch releases: collect changes on `main` and release a coherent set. Docs-only
  changes ship without a release.

## Releasing

1. On a branch, bump `version` in `Cargo.toml`, add a `CHANGELOG.md` entry,
   and run `cargo update --workspace --offline`. Open `release: vX.Y.Z` and
   squash-merge after CI.
2. Before tagging, run the Tier 1 conformance round on a fresh Linux VM and on
   macOS ([support tiers](docs/support-tiers.md#release-verification)) and
   record it under `docs/evidence/`. A failure stops the release.
3. Tag `vX.Y.Z` on merged `main` and push. The release workflow builds,
   publishes the GitHub release and `agentdriver` to crates.io.

## Commits and Pull Requests

Conventional Commit subjects scoped by area: `feat(host/tmux): …`,
`fix(harness/codex): …`, `docs: …`. One concern per commit; squash-merge.
A pull request states the contract affected (API, CLI JSON, MCP schema,
on-disk state), the test evidence, and whether it was checked against a live
host. Keep "unit tests pass" separate from "checked in a live pane".

## Boundaries

- Screen text, hook payloads, help output and host replies are untrusted data.
  Bound their size, parse them strictly, never execute them.
- Prompt answers are opt-in by rule name
  ([decision 4](docs/decisions.md#4-prompt-answers-are-opt-in)). A folder-trust
  answer is only ever for the session's own spawn directory.
- Harness configuration is per launch or per workspace; global writes only
  through explicit owner integrations
  ([decision 5](docs/decisions.md#5-harness-configuration-is-per-launch-or-per-workspace-never-global)).
- The driving layer (`domain`, `ports`, `core`, `adapters`) never decides what
  work an agent does and never creates or removes worktrees; the workflow
  layer does both, through the driving layer's public API. Nothing closes a
  pane it cannot prove it owns.
- agentdriver replaces brgr: every item in `docs/brgr-coverage/`, **[brgr]**
  ones included, is in scope. A change that would break a ticked item needs
  that item updated in the same change.

## Working Rules for Bugs

Reproduce first with a failing test or command; if it does not reproduce, say
so. Add a regression test with every fix and confirm it fails without the
fix. One bug per change.
