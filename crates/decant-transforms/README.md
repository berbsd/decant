# decant-transforms

Rule vocabulary, TOML config loading, and per-command routing for `decant`.

## Layers

| Layer | Key types |
|-------|-----------|
| **Rules** | `StripAnsi`, `Drop`, `Keep`, `KeepAfter`, `Collapse`, `Dedup`, `Truncate` — each implements the `Rule` trait. |
| **Chain** | `RuleChain` — ordered list of boxed rules; implements `decant_core::Transform`. |
| **Config** | `load_and_compile(toml_text, name)` — parse TOML and compile it into a `RuleChain`. |
| **Router** | `resolve(&argv)` — find the right config and return a `Resolved` (source + chain). |

## Config format

```toml
[[step]]
type = "strip_ansi"

[[step]]
type = "collapse"
pattern = '^Compiling '
label = "{n} crates compiled"

[[step]]
type = "truncate"
max_lines = 50
keep = "tail"   # optional; default is "tail"
```

## Resolution order

`resolve` searches for a matching config in priority order:

1. `./.decant/<key>.toml` — project-local override.
2. `$XDG_CONFIG_HOME/decant/<key>.toml` (or `~/.config/decant/<key>.toml`) — user config.
3. Embedded built-in (compiled into the binary from `src/builtins/`).
4. Identity passthrough — no transforms applied.

The routing key is derived from the command argv: `cargo build` → tries
`cargo-build` first, then `cargo`.

## Example

```rust,no_run
use decant_transforms::resolve;

let argv: Vec<String> = vec!["cargo".into(), "build".into()];
let resolved = resolve(&argv);
println!("using: {}", resolved.source);
// resolved.chain implements decant_core::Transform
```

## Architecture note

`decant-transforms` depends on `decant-core` for the `Transform` trait but
knows nothing about the CLI. `tools/decant` is the only consumer that calls
both `resolve` and `decant_core::execute`.
