# decant

CLI that wraps a command, captures its output, and reduces it to save LLM tokens.

## Usage

```bash
# Run a command and emit reduced output (stats go to stderr):
decant run -- cargo build
decant run -- cargo test --release

# Override timeouts:
decant run --idle-timeout 30 --timeout 120 -- ./long-task

# Skip all transforms (pass raw output through):
decant run --raw -- cargo clippy

# Show which config and steps would apply (no execution):
decant explain cargo build

# List all commands with a built-in config:
decant explain
```

## Subcommands

| Subcommand | Purpose |
|------------|---------|
| `run` | Spawn the command, capture, reduce, emit. Adds a stats line to stderr. |
| `explain` | Show the resolved config source and transform steps. No execution. |

## Flags (`run`)

| Flag | Default | Description |
|------|---------|-------------|
| `--idle-timeout <secs>` | 60 | Kill child if no output for this long (`0` disables). |
| `--timeout <secs>` | 600 | Kill child after this many seconds total (`0` disables). |
| `--no-stats` | off | Suppress the `[decant: … saved]` line on stderr. |
| `--raw` | off | Bypass all transforms; emit raw output. |

## Architecture note

`tools/decant` is a thin CLI layer. The reduction logic lives in
`crates/decant-transforms`; the capture engine and traits live in
`crates/decant-core`; metrics live in `crates/decant-metrics`. The library
crate (`src/lib.rs`) exposes `run_cli` and `dispatch` so integration tests
can exercise the real code paths without going through `argv`.
