# decant-core

Pipeline engine for `decant`: the `Runner` + `Transform` extension points and
the `execute` entry point that wires them together.

## Key types

| Type | Role |
|------|------|
| `Runner` | Trait — spawn a child command and capture its output. |
| `Transform` | Trait — reduce captured output to save LLM tokens. |
| `execute` | One call: run via a `Runner`, apply a `Transform`, return results. |
| `CaptureRunner` | Production `Runner`: concurrent stdout/stderr drain with idle + wall-clock timeouts. |
| `Captured` | Raw result: stdout, stderr, exit code, optional timeout kind. |
| `TransformOutput` | Reduced bytes ready to emit. |
| `RunError` | Spawn failure or I/O error during capture. |

## Quick example

```rust,no_run
use std::process::Command;
use decant_core::{CaptureRunner, execute, Captured, Transform, TransformOutput};

struct Identity;
impl Transform for Identity {
    fn name(&self) -> &str { "identity" }
    fn apply(&self, c: &Captured) -> TransformOutput {
        TransformOutput { stdout: c.stdout.clone(), stderr: c.stderr.clone() }
    }
}

let runner = CaptureRunner::default(); // 60 s idle, 600 s wall-clock
let (output, captured) = execute(Command::new("cargo"), &runner, &Identity).unwrap();
println!("exit {}", captured.exit_code);
```

## Architecture note

`decant-core` has no knowledge of TOML configs or specific rules — it is a
pure seam. `decant-transforms` provides the `RuleChain` that implements
`Transform`; `tools/decant` wires everything into the CLI.

The fallback principle: if a child is killed by a timeout, `execute` returns
the raw (partial) buffer without calling the transform. This ensures the user
always sees output even when a command hangs.
