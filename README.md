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
