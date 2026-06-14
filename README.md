# decant

`decant` wraps a command, captures its output, and reduces it to minimize the
tokens an LLM spends reading tool output.

```bash
decant run -- cargo build          # capture, transform, emit, report savings
decant run --idle-timeout 30 -- ./long-task
```

## Installation

From a release (downloads the prebuilt binary for your platform):

```sh
curl -fsSL https://raw.githubusercontent.com/squadri/decant/main/install.sh | sh
```

This installs to `~/.local/bin` by default (override with `DECANT_INSTALL_DIR`).
Once installed, `decant update` upgrades to the latest release.

To build and install from source: `cargo install --path tools/decant`.

## Examples

### Basic reduction

```bash
decant run -- cargo build              # reduce build output, print savings to stderr
decant run -- git status               # works for any command with a built-in config
decant run -- ls -la /usr/lib          # falls back to raw passthrough if no config matches
```

### Piping into other tools

By default decant reduces **only when stdout is an interactive terminal**. When
its output is piped or redirected, it switches to *pipe-safe* mode: it still
strips ANSI codes and de-dups, but never drops, collapses, or truncates lines —
so a downstream `grep`/`awk`/`wc` sees every line.

```bash
decant run -- cargo build | grep warning   # pipe-safe: grep still finds every match
decant run -- cargo build > build.log       # file redirect is faithful too
```

Force the behavior explicitly when you need it:

```bash
decant run --reduce -- cargo build | less -R   # full reduction into a pager (human reading)
decant run --raw    -- cargo test              # bypass all transforms, emit raw output
```

### Inspecting what will happen

```bash
decant explain cargo build   # show the resolved config source and rule chain
decant explain               # list every built-in command config
```

### Timeouts and quieter output

```bash
decant run --idle-timeout 30 -- ./long-task   # kill if no output for 30s (0 disables)
decant run --timeout 120 -- ./flaky-task      # hard wall-clock limit of 120s
decant run --no-stats -- npm install          # suppress the stats line on stderr
```

### Reviewing past runs

```bash
decant history   # recent runs and their reduction savings (from the SQLite store)
```

## Workspace

- `crates/decant-core` — engine: `Runner` + `Transform` seams, `execute` pipeline, `CaptureRunner`
- `crates/decant-transforms` — TOML-defined chainable rule engine + per-command router
- `crates/decant-metrics` — byte/token reduction measurement
- `crates/decant-store` — SQLite metrics persistence (`decant history`)
- `crates/decant-agents` — agent hook integration (`decant init` / `hook`)
- `tools/decant` — the CLI

## Development

Set up the dev environment (installs `just`, `taplo`, `cargo-nextest`,
`typos-cli`, `cargo-deny`, `git-cliff`, `cargo-release`, and nightly rustfmt):

```sh
./bin/bootstrap.sh
```

Then:

```bash
just check     # fmt (nightly) · clippy · nextest · typos · cargo-deny
just test      # nextest
just fmt       # format Rust + TOML
```

Requires Rust 1.95.0 (pinned in `rust-toolchain.toml`); `bootstrap.sh` installs
the rest.

## Design

See `docs/superpowers/specs/2026-06-13-decant-foundation-design.md`.

## Releasing

Releases are cut with `cargo-release` and published by CI on tag push.

Prerequisites (one-time): `cargo install cargo-release git-cliff`, and a GitHub
repo at `github.com/squadri/decant` with `origin` set.

To release:

```sh
just changelog          # refresh CHANGELOG.md from commits
git add CHANGELOG.md && git commit -m "docs: update changelog"   # if changed
just release-dry-run patch   # preview the bump + tag
just release patch           # bump version, tag vX.Y.Z, push
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds binaries
for macOS (arm64/x64) and Linux (x64/arm64, musl) and attaches them plus SHA256
checksums to a GitHub Release whose notes come from `git-cliff`.

Each release publishes, per target:

```
decant-<target-triple>.tar.gz
decant-<target-triple>.tar.gz.sha256
```
