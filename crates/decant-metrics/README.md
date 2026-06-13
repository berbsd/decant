# decant-metrics

Before/after measurement of byte and token reduction for a single `decant` run.

## Key items

| Item | Role |
|------|------|
| `Measurement` | Struct holding `bytes_in`, `bytes_out`, `tokens_in`, `tokens_out`, `duration`. |
| `Measurement::savings_pct` | Returns `100 × (1 − out/in)` — the headline reduction percentage. |
| `measure` | Build a `Measurement` from raw input and reduced output byte slices. |
| `estimate_tokens` | Word-count heuristic that matches RTK's token estimator. |

## Example

```rust
use std::time::Duration;
use decant_metrics::{measure, estimate_tokens};

let m = measure(b"hello world foo bar baz", b"hello", Duration::from_millis(12));
println!("{:.1}% saved ({} → {} tokens)", m.savings_pct(), m.tokens_in, m.tokens_out);
// 95.7% saved (5 → 1 tokens)
```

## Architecture note

`decant-metrics` is dependency-free beyond `std`. It is intentionally a leaf
crate: `tools/decant` calls `measure` after `decant-core::execute` returns and
writes the stats line to stderr. No other crate in the workspace depends on it.
