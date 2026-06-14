# decant (CLI)

The `decant` command-line binary: parses arguments, dispatches subcommands, and
wires the routing layer (`decant-transforms`) to the capture engine
(`decant-core`). The library crate (`src/lib.rs`) exposes `run_cli` and
`dispatch` so integration tests can exercise the real code paths without going
through `argv`.

The reduction logic itself lives in the workspace crates, not here.

**See the [workspace README](../../README.md)** for installation, usage, the
repository structure, and the development guide (how reduction works, the rule
vocabulary, and how to add a new rule or target).
