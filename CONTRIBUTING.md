# Contributing to decant

`decant` wraps a command, captures its output, and reduces it to minimise the
tokens an LLM spends reading tool output.

## Dev setup

```sh
./bin/bootstrap.sh   # installs just, taplo, cargo-nextest, typos, cargo-deny, git-cliff, cocogitto, nightly rustfmt (+ lefthook & gitleaks via brew)
just check           # the pre-commit gate (must be green before any commit)
```

## Core principles

- **Zero clippy warnings.** CI runs strict lints with `-Dwarnings`;
  `unwrap`/`expect`/`panic` are denied outside tests. `just check` (nightly fmt ·
  clippy · nextest · typos · cargo-deny) must pass before committing.
- **Clear crate separation** — one responsibility each:
  - `decant-core` — engine: `Runner` / `Transform` traits, `execute`, `CaptureRunner`.
  - `decant-transforms` — rule vocabulary (`rules/`), `RuleChain`, TOML config, router.
  - `decant-metrics` — byte/token measurement.
  - `decant-store` — SQLite metrics persistence.
  - `decant-dashboard` — ratatui rendering for the `decant dashboard` TUI.
  - `decant-agents` — agent hook integration.
  - `tools/decant` — the CLI.
- **Errors: `thiserror` in libraries, `anyhow` in the binary.** Library crates
  expose typed error enums; the binary uses `anyhow` and converts via `?`.
- **No async.** Single-threaded by design (fast startup). No `tokio`/`async`.
- **Emit via `std::io::Write`, not `println!`** (raw bytes; satisfies the
  `print_stdout` lint). Exit by returning `ExitCode` from `main` (never
  `process::exit`).
- **Trait-per-file for vocabularies.** A new rule is one struct in one file under
  `crates/decant-transforms/src/rules/`, implementing the `Rule` trait (matches
  the existing pattern).
- **TDD + real fixtures + ≥60% savings.** Filters use `insta` snapshots and a
  byte-savings assertion against real captured command output.
- **Edition 2024 / toolchain 1.95 / nightly rustfmt** (`cargo +nightly fmt`).

## Adding a built-in transform config

Built-ins live in `crates/decant-transforms/src/builtins/<key>.toml`. The
filename is the routing key: `decant run -- <program> <subcommand>` resolves to
`<program>-<subcommand>.toml` (or `<program>.toml` for program-only recipes).
`include_dir` auto-discovers new files — no router or registry edits needed.

A config reduces in one of two ways: output `[[step]]` rules (below), or an
`[args]` table that rewrites the command before it runs (e.g. `git-status`
appends `--short`). Prefer `[args]` when the tool can emit lean output itself —
it is denser than any output rule and needs no fixture. Its fields:

- `append` — tokens appended to argv before the command is spawned.
- `skip_if` — if any of these tokens is already present, the append is skipped
  (the caller chose a format). Defaults to `append`, so a flag is never doubled.

An `[args]`-only config reports ≈0% output savings, so it is tested by asserting
the rewrite (a router/`explain` test), not byte savings — see `git-status.toml`.

### Steps

1. **Create the TOML** at `crates/decant-transforms/src/builtins/<key>.toml`.
   Write an ordered list of `[[step]]` tables using the 10 rules:
   `strip_ansi`, `drop`, `keep`, `keep_after`, `cut`, `collapse`, `transform`,
   `dedup`, `truncate`, `rank`.
   Use TOML **literal single-quoted strings** for `pattern` fields (no escaping
   needed). Example:

   ```toml
   [[step]]
   type = "strip_ansi"

   [[step]]
   type = "drop"
   pattern = '^\s*$'

   [[step]]
   type = "truncate"
   max_lines = 50
   keep = "tail"
   ```

   For the final line cap, choose **`truncate`** (keep the last/first N lines) or
   **`rank`** (keep the N highest-signal lines, force-keeping error/panic/failure
   lines wherever they fall). They share the same slot — use one, not both.
   Prefer `rank` for compiler/test output where a failure can be buried
   (`cargo-test`, `cargo-nextest`, `make`); `truncate` for uniform or
   already-ordered output (`ls`, `git log`). Give `rank` a `pattern` to override
   its built-in error signal with the tool's failure markers (e.g. nextest's
   `'FAIL|panicked|failed'`).

2. **Capture a real fixture.** Run the actual command in a small scenario and
   save its stdout+stderr to
   `crates/decant-transforms/tests/fixtures/<key>.txt` (committed — so CI
   snapshots are deterministic). Do not synthesise fixtures.

3. **Add a savings + snapshot test** in
   `crates/decant-transforms/tests/builtins.rs` asserting ≥60% byte savings
   and an `insta` snapshot of the reduced output.

   If the config uses `rank`, also capture a **failing-run fixture**
   (`<key>-fail.txt`) and add a test asserting the failure signal survives the
   chain — that guarantee is the reason to use `rank` over `truncate`, and an
   all-passing fixture cannot exercise it. See `cargo-test-fail.txt` /
   `make-fail.txt` and their tests.

4. **Optionally add a live test** in
   `crates/decant-transforms/tests/live.rs` marked `#[ignore]` that runs the
   real command and checks the chain reduces it. CI skips these; run locally
   with `cargo test -p decant-transforms --test live -- --ignored`.

5. **Verify** with `cargo test` and `decant explain -- <program> <subcommand>`.

### Override precedence

Higher precedence wins over built-ins:

```
./.decant/<key>.toml          (project-local)
~/.config/decant/<key>.toml  (user)
embedded built-in             (lowest)
```

### Savings bar

An output-`[[step]]` config is kept only if its chain clears ≥60% byte savings
on its fixture. Weaker configs are dropped (documented in the commit message).
This bar does not apply to `[args]`-only configs (e.g. `git-status`), whose win
is the command rewrite rather than output reduction; they are verified by the
rewrite assertion instead.

## Commits & changelog

- **Conventional Commits**: `feat` / `fix` / `refactor` / `docs` / `chore` /
  `test` / `ci` / `perf`, imperative mood, 72-col body wrap.
- The changelog is generated by `git-cliff` (`just changelog`).

## Testing

- Unit tests in-module; `insta` snapshots; per-config ≥60% savings assertions;
  black-box CLI tests under `tools/decant/tests/`; opt-in live tests in
  `crates/decant-transforms/tests/live.rs` (run with `--ignored`).
- `just test` (nextest); the full gate is `just check`.

## Releasing

See the "Releasing" section in `README.md` (`just release`).
