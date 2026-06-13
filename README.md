# decant

`decant` wraps a command, captures its output, and reduces it to minimize the
tokens an LLM spends reading tool output.

```bash
decant run -- cargo build          # capture, transform, emit, report savings
decant run --idle-timeout 30 -- ./long-task
```

## Workspace

- `crates/decant-core` — engine: `Runner` + `Transform` seams, `execute` pipeline, `CaptureRunner`
- `crates/decant-transforms` — filter implementations (v1: `Identity`)
- `crates/decant-metrics` — byte/token reduction measurement
- `tools/decant` — the CLI

## Development

```bash
just check     # fmt (nightly) · clippy · nextest · typos · cargo-deny
just test      # nextest
just fmt       # format Rust + TOML
```

Requires Rust 1.95.0, nightly (for rustfmt), and `cargo-nextest`, `typos-cli`,
`cargo-deny`.

## Design

See `docs/superpowers/specs/2026-06-13-decant-foundation-design.md`.
