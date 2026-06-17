# decant

[![CI](https://github.com/berbsd/decant/actions/workflows/ci.yml/badge.svg)](https://github.com/berbsd/decant/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/berbsd/decant/graph/badge.svg)](https://codecov.io/gh/berbsd/decant)
[![Release](https://img.shields.io/github/v/release/berbsd/decant)](https://github.com/berbsd/decant/releases/latest)
[![License: MIT](https://img.shields.io/github/license/berbsd/decant)](LICENSE)

`decant` wraps a command, captures its output, and reduces it to minimize the
tokens an LLM spends reading tool output. It strips noise, collapses repetition,
and caps runaway logs — while preserving the command's exit code and raw byte
stream.

```bash
decant run -- cargo build          # capture, transform, emit, report savings
decant run --idle-timeout 30 -- ./long-task
```

The same reduction can run automatically: install a hook into your agent
(`decant init`) and its shell commands are routed through `decant run` for you.

---

## Contents

- [Installation](#installation)
- [Usage](#usage) — the CLI from a user's perspective
- [Repository structure](#repository-structure)
- [Development](#development) — **the focus of this document**
  - [Environment setup](#environment-setup)
  - [How reduction works](#how-reduction-works)
  - [The rule vocabulary](#the-rule-vocabulary)
  - [Adding a new target (built-in config)](#adding-a-new-target-built-in-config)
  - [Adding a new rule (extend the vocabulary)](#adding-a-new-rule-extend-the-vocabulary)
  - [Core principles](#core-principles)
  - [Testing](#testing)
- [Releasing](#releasing)
- [Design docs](#design-docs)

---

## Installation

From a release (downloads the prebuilt binary for your platform):

```sh
curl -fsSL https://raw.githubusercontent.com/berbsd/decant/main/install.sh | sh
```

This installs to `~/.local/bin` by default (override with `DECANT_INSTALL_DIR`).
Once installed, `decant update` upgrades to the latest release.

To build and install from source: `cargo install --path tools/decant`.

---

## Usage

`decant --help` is the source of truth; every subcommand has its own
`decant <cmd> --help` with examples. A summary:

| Subcommand | Purpose |
|------------|---------|
| `run`      | Spawn the command, capture its output, reduce it, emit the result. A savings line goes to stderr. |
| `explain`  | Show the resolved config source and transform chain for a command. No execution. |
| `init`     | Install decant's hook into an agent's settings (global, or `--project`). |
| `hook`     | The runtime hook processor an agent invokes — not run by hand. |
| `history`  | Report recorded savings per command from the SQLite store. |
| `dashboard`| Interactive terminal view of recorded savings (live `--watch`). Requires a TTY; for scripting use `history --json`. |
| `update`   | Replace the running binary with the latest release. |

### `run` flags

| Flag | Default | Description |
|------|---------|-------------|
| `--idle-timeout <SECS>` | 60 | Kill the child if it produces no output for this long (`0` disables). |
| `--timeout <SECS>` | 600 | Hard wall-clock limit; kill the child after this many seconds (`0` disables). |
| `--no-stats` | off | Suppress the `[decant: … saved]` line on stderr. |
| `--raw` | off | Bypass all transforms; emit raw output. |
| `--reduce` | off | Force full reduction even when stdout is piped (conflicts with `--raw`). |

### Reducing vs. piping

By default decant reduces **only when stdout is an interactive terminal**. When
its output is piped or redirected, it switches to *pipe-safe* mode: it still
strips ANSI codes and de-dups, but never drops, collapses, truncates, or ranks
lines — so a downstream `grep`/`awk`/`wc` sees every line.

```bash
decant run -- cargo build                 # full reduction (terminal)
decant run -- cargo build | grep warning  # pipe-safe: grep still finds every match
decant run --reduce -- cargo build | less -R  # force full reduction into a pager
decant run --raw    -- cargo test         # bypass all transforms
```

### Inspecting and reviewing

```bash
decant explain cargo build   # show the resolved config source and rule chain
decant explain               # list every command with a built-in config
decant history               # recent runs and their reduction savings
decant dashboard             # same savings as a scrollable TUI (add --watch for live)
```

---

## Repository structure

decant is a Cargo workspace: a thin CLI over a few focused library crates.

```
decant/
├── tools/decant/                 # the CLI binary (argument parsing, dispatch)
├── crates/
│   ├── decant-core/              # engine: Runner + Transform traits, execute(), CaptureRunner
│   ├── decant-transforms/        # rule vocabulary, RuleChain, TOML config, per-command router
│   │   ├── src/rules/            #   one Rule impl per file
│   │   └── src/builtins/         #   embedded <key>.toml configs (the "targets")
│   ├── decant-metrics/           # byte/token reduction measurement
│   ├── decant-store/             # SQLite metrics persistence (decant history)
│   ├── decant-dashboard/         # ratatui rendering for the decant dashboard TUI
│   └── decant-agents/            # agent hook integration (decant init / hook)
├── bin/bootstrap.sh              # dev environment setup
├── justfile                      # task runner (just check / test / fmt / release)
└── docs/                         # design specs and plans
```

The dependency direction is one-way: `decant-transforms` depends on
`decant-core` for the `Transform` trait but knows nothing about the CLI.
`tools/decant` is the only crate that wires `resolve` (routing) together with
`decant_core::execute` (capture + transform). The library crate
(`tools/decant/src/lib.rs`) exposes `run_cli` and `dispatch` so integration
tests can drive the real code paths without going through `argv`.

---

## Development

### Environment setup

```sh
./bin/bootstrap.sh   # installs just, taplo, cargo-nextest, typos-cli, cargo-deny,
                     # git-cliff, cargo-release, cocogitto, nightly rustfmt
                     # (+ lefthook & gitleaks via brew, for the git hooks)
```

The toolchain is pinned to **Rust 1.95.0** (edition 2024) in
`rust-toolchain.toml`; `bootstrap.sh` installs everything else.

Day-to-day commands (run `just` with no argument to list them all):

| Command | What it does |
|---------|--------------|
| `just check` | **The pre-commit gate.** Nightly `fmt --check` · `clippy --all-targets --all-features` · `nextest` · `typos` · `cargo deny`. Must be green before any commit. |
| `just test`  | Run the test suite (`cargo nextest run`). |
| `just fmt`   | Format Rust (nightly rustfmt) and TOML (taplo). |
| `just fix`   | `clippy --fix` for mechanical lint fixes. |
| `just hooks` | Install git hooks via lefthook. |

### How reduction works

A run flows through three seams:

1. **Capture** — `decant_core::execute` spawns the command via a `Runner`
   (`CaptureRunner` enforces the idle / wall-clock timeouts) and collects its
   stdout and stderr.
2. **Resolve** — `decant_transforms::resolve(&argv)` picks the config for the
   command and compiles it into a `RuleChain` (which implements the
   `decant_core::Transform` trait).
3. **Transform** — the captured bytes pass through the chain's rules in order.
   In pipe-safe mode only line-preserving rules run.

**Routing key.** The key is derived from argv, most specific first:
`program-subcommand`, then `program`. `program` is the basename of argv[0] and
`subcommand` is the first bare sub-verb — the first argument that is not a flag,
a path, or a `key=value` token, so a flag's value is skipped rather than
mistaken for the subcommand. So `cargo build` tries `cargo-build`, then `cargo`,
and `git -C /repo status` correctly resolves `git-status` (not the `-C` path).

**Resolution order** (first match wins; on any error decant warns to stderr and
falls back to identity passthrough — output is never blocked):

```
1. ./.decant/<key>.toml                      project-local override
2. $XDG_CONFIG_HOME/decant/<key>.toml        user config (or ~/.config/decant/)
3. embedded built-in (src/builtins/<key>.toml)   compiled into the binary
4. identity passthrough                       no config found → raw output
```

A config is an optional `[args]` table (which rewrites the command before it
runs — see [below](#rewriting-the-command-args)) followed by an ordered list of
`[[step]]` tables that transform the output. Each step has a `type` that selects
a rule:

```toml
[[step]]
type = "strip_ansi"

[[step]]
type    = "collapse"
pattern = '^\s*Compiling '
label   = "{n} crates compiled"

[[step]]
type      = "truncate"
max_lines = 100
keep      = "tail"   # optional; default is "tail"
```

Each rule receives the full text produced by the previous step, so order
matters: strip ANSI first, drop/collapse noise, cap last (with `truncate` or
`rank` — see below).

### The rule vocabulary

Ten rules make up the vocabulary. Write `pattern` fields as TOML **literal
single-quoted strings** (`'...'`) so regex backslashes need no escaping;
patterns use the `regex` crate (RE2 syntax) and are matched per line.

| `type` | Fields | Effect | Pipe-safe |
|--------|--------|--------|:---------:|
| `strip_ansi` | — | Remove ANSI escape / color codes from every line. | ✅ |
| `dedup` | — | Collapse consecutive duplicate lines into one. | ✅ |
| `drop` | `pattern` | Remove every line that matches `pattern`. | ❌ |
| `keep` | `pattern` | Keep only lines that match `pattern` (inverse of `drop`). | ❌ |
| `keep_after` | `pattern` | Discard everything before the first matching line; keep that line and the rest. | ❌ |
| `cut` | `begin`, `end` | Drop a section from a `begin` match to an `end` match; the `begin` line is dropped, the `end` line is kept. Removes multiple sections; an unclosed `begin` runs to EOF. | ❌ |
| `collapse` | `pattern`, `label` | Replace all matching lines with a single summary line; `{n}` in `label` becomes the match count. | ❌ |
| `transform` | `pattern`, `replacement`, `multiline` | Rewrite by replacing all matches of `pattern`; `replacement` supports `$1` / `${name}` backrefs. Per line by default; set `multiline = true` to match across newlines over the whole buffer. | ❌ |
| `truncate` | `max_lines`, `keep` | Cap output at `max_lines`, inserting a `… N more lines` marker. `keep = "tail"` (default) keeps the end; `"head"` keeps the start. | ❌ |
| `rank` | `budget`, `head`, `tail`, `pattern` | Keep the highest-signal lines within `budget` *by importance, not position*: error/panic/failure lines are force-kept (so a buried failure survives), as are the first `head` and last `tail` lines (default 2 each); the rest of the budget fills by priority and dropped runs become `… N more lines`. `pattern` overrides the force-keep regex (e.g. `'FAILED or panicked'`). | ❌ |

**Pipe-safe** means the rule preserves every input line's content, so a
downstream `grep`/`awk` still sees the same matches. Lossy rules (everything
except `strip_ansi` and `dedup`) are skipped automatically in pipe-safe mode.
This is enforced by the `Rule::preserves_lines` method, which **defaults to
`false`** — a new rule is assumed lossy until it proves otherwise, so adding one
can never silently hide piped output.

#### Capping output: `truncate` vs `rank`

`truncate` and `rank` do the **same job** — cap a long output to a line budget —
and occupy the **same final slot** in a chain. They differ only in *which* lines
they drop when the output is over budget:

- `truncate tail N` keeps the **last N lines** (positional). An error earlier in
  the output is dropped.
- `rank budget N` keeps the **N highest-signal lines** — error/panic/failure
  lines are force-kept wherever they fall, so a buried failure survives.

Use **one or the other, never both**: they enforce the same cap, so a `truncate`
after a `rank` would re-cut positionally and undo rank's protection (and a
`rank` after a `truncate` only sees the lines `truncate` already kept). `rank` is
a smarter drop-in *replacement* for `truncate`, not a guard layered on top of it.

Prefer **`rank`** when a failure can appear anywhere in the output (compiler and
test runs — `cargo-test`, `cargo-nextest`, `make`). Prefer **`truncate`** when
the output is uniform or already ordered so position is meaningful (`git log`
newest-first, `ls`, `find`, `du`). Both are no-ops when the output already fits
within the budget, so the choice only matters once something has to be cut.

### Rewriting the command (`[args]`)

The `[[step]]` rules above transform a command's *output*. The optional `[args]`
table instead rewrites the command's *arguments* before it runs, so the tool can
be asked to produce lean output itself — often denser than any output rule could
achieve. The built-in `git-status` config uses it to turn `git status` into
`git status --short`:

```toml
[args]
append  = ["--short"]
skip_if = ["-s", "--short", "--porcelain", "--long"]
```

| Field | Effect |
|-------|--------|
| `append` | Tokens appended to the command's argv before it is spawned. |
| `skip_if` | If **any** of these tokens already appears in the command, the append is skipped — the caller already chose a format. Defaults to `append`, so a flag is never added twice. |

`decant explain -- git status` shows the resolved rewrite as an `appends:` line,
and the `[decant: …]` stats line notes `appended --short` at run time. Because
the captured output is already lean, such a config reports ≈0% *output* savings —
the real win is that the verbose form is never produced.

### Adding a new target (built-in config)

A "target" is a command that gets its own reduction recipe. Adding one is pure
TOML plus a fixture and a test — no router or registry edits, because
`include_dir!` auto-discovers files in `src/builtins/`.

1. **Create the config** at
   `crates/decant-transforms/src/builtins/<key>.toml`, where `<key>` is the
   routing key (`<program>-<subcommand>` or just `<program>`). Compose an
   ordered list of `[[step]]` tables from the rule vocabulary above.

2. **Capture a real fixture.** Run the actual command and save its combined
   stdout+stderr to `crates/decant-transforms/tests/fixtures/<key>.txt`
   (committed, so CI snapshots are deterministic). Do **not** synthesize
   fixtures.

3. **Add a savings + snapshot test** in
   `crates/decant-transforms/tests/builtins.rs`: assert ≥ 60% byte savings on
   the fixture and add an `insta` snapshot of the reduced output.

4. **(Optional) add a live test** in
   `crates/decant-transforms/tests/live.rs`, marked `#[ignore]`, that runs the
   real command. CI skips these; run locally with
   `cargo test -p decant-transforms --test live -- --ignored`.

5. **Verify:**

   ```bash
   cargo test -p decant-transforms      # runs the savings + snapshot test
   cargo insta review                   # accept the new snapshot
   decant explain -- <program> <sub>    # confirm the chain resolves
   ```

A config is kept only if its chain clears the **≥ 60% byte-savings bar** on its
fixture; weaker configs are dropped (note it in the commit message).

### Adding a new rule (extend the vocabulary)

Reach for this only when no combination of the seven rules can express a
reduction. A rule is one struct in one file (trait-per-file), wired into the
TOML model in two places:

1. **Implement the rule** in `crates/decant-transforms/src/rules/<name>.rs`:
   a struct that implements the `Rule` trait — `apply(&self, &str) -> String`
   and `describe(&self) -> String` (the label `decant explain` shows). Override
   `preserves_lines(&self) -> bool` to return `true` **only** if the rule keeps
   every line's content intact (it defaults to `false`).

2. **Export it** from `crates/decant-transforms/src/rules/mod.rs`
   (`mod <name>;` + `pub use <name>::<Type>;`).

3. **Add a config variant** in `crates/decant-transforms/src/config.rs`: a new
   `StepSpec` enum variant (serde uses the `snake_case` of the variant name as
   the TOML `type`) carrying any config fields, then map it to your rule inside
   `StepSpec::into_rule()`. Use `compile_regex(..)?` for any `pattern` field so
   bad regexes are rejected at compile time.

4. **Test** the rule in its own file (`#[cfg(test)] mod tests`) with
   `Arrange-Act-Assert` cases, including an empty-input case.

5. **Document it** in the rule table above and in
   `crates/decant-transforms/README.md`.

### Core principles

These are enforced by `just check`; see [`CONTRIBUTING.md`](CONTRIBUTING.md) for
the full version.

- **Zero clippy warnings.** CI runs `-Dwarnings`; `unwrap`/`expect`/`panic` are
  denied outside tests.
- **Errors: `thiserror` in libraries, `anyhow` in the binary.**
- **No async.** Single-threaded by design for fast startup.
- **Emit via `std::io::Write`, not `println!`** (raw bytes; satisfies the
  `print_stdout` lint). Return `ExitCode` rather than calling `process::exit`.
- **Trait-per-file for vocabularies.** One rule = one struct = one file.
- **TDD + real fixtures + ≥ 60% savings**, verified with `insta` snapshots.

### Testing

- **Unit tests** live in-module (`#[cfg(test)] mod tests`).
- **Built-in configs** have per-target ≥ 60% savings assertions and `insta`
  snapshots in `crates/decant-transforms/tests/builtins.rs`.
- **CLI black-box tests** live under `tools/decant/tests/`.
- **Live tests** (`crates/decant-transforms/tests/live.rs`) run real commands
  and are `#[ignore]`d by default — run with `--ignored`.

```bash
just test            # nextest
just check           # the full gate before committing
cargo insta review   # review/accept snapshot changes
```

---

## Releasing

Releases are cut with `cargo-release` and published by CI on tag push.
One-time prerequisites: `cargo install cargo-release git-cliff`, and a GitHub
repo at `github.com/berbsd/decant` with `origin` set.

```sh
just changelog                                                  # refresh CHANGELOG.md
git add CHANGELOG.md && git commit -m "docs: update changelog"  # if it changed
just release-dry-run patch                                      # preview the bump + tag
just release patch                                              # bump, tag vX.Y.Z, push
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds binaries
for macOS (arm64/x64) and Linux (x64/arm64, musl) and attaches them plus SHA256
checksums to a GitHub Release whose notes come from `git-cliff`. Each release
publishes, per target:

```
decant-<target-triple>.tar.gz
decant-<target-triple>.tar.gz.sha256
```
