//! Built-in chains must meaningfully reduce real command output.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use decant_transforms::resolve;

#[allow(clippy::cast_precision_loss)]
fn savings_pct(
  input: &[u8],
  output: &[u8],
) -> f64 {
  if input.is_empty() {
    return 0.0;
  }
  100.0 * (1.0 - (output.len() as f64 / input.len() as f64))
}

fn run_builtin(
  command: &[&str],
  raw: &[u8],
) -> Vec<u8> {
  let cmd: Vec<String> = command.iter().map(|s| (*s).to_string()).collect();
  let resolved = resolve(&cmd);
  assert!(
    !resolved.chain.is_passthrough(),
    "expected a built-in chain for {command:?}"
  );
  resolved.chain.run(raw)
}

#[test]
fn cargo_build_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/cargo-build.txt");
  let out = run_builtin(&["cargo", "build"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "cargo build savings only {pct:.1}%");
  let text = String::from_utf8_lossy(&out);
  insta::assert_snapshot!("cargo_build", text);
}

#[test]
fn cargo_test_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/cargo-test.txt");
  let out = run_builtin(&["cargo", "test"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "cargo test savings only {pct:.1}%");
  let text = String::from_utf8_lossy(&out);
  insta::assert_snapshot!("cargo_test", text);
}

#[test]
fn cargo_nextest_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/cargo-nextest.txt");
  let out = run_builtin(&["cargo", "nextest", "run"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "cargo nextest savings only {pct:.1}%");
  let text = String::from_utf8_lossy(&out);
  insta::assert_snapshot!("cargo_nextest", text);
}
