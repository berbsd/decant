//! Built-in chains must meaningfully reduce real command output.

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
fn cargo_build_chain_keeps_errors_while_compressing() {
  // A failing build: 28 `Compiling` lines collapse away, and the `error[E0382]`
  // plus the `could not compile` summary are force-kept by rank's pattern.
  let raw = include_bytes!("fixtures/cargo-build-fail.txt");
  let out = run_builtin(&["cargo", "build"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "cargo build (fail) savings only {pct:.1}%");

  let text = String::from_utf8_lossy(&out);
  assert!(
    text.contains("error[E0382]: borrow of moved value"),
    "lost compile error"
  );
  assert!(
    text.contains("error: could not compile"),
    "lost build summary"
  );
  assert!(!text.contains("Compiling dep_1 "), "kept compile noise");
  insta::assert_snapshot!("cargo_build_fail", text);
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
fn cargo_test_chain_keeps_failures_while_compressing() {
  // A failing run: the `FAILED` marker is buried at line 30 among 45 passing
  // tests. The rank step must force-keep the failure signal while still hitting
  // the savings bar — the property a positional `truncate` could not guarantee.
  let raw = include_bytes!("fixtures/cargo-test-fail.txt");
  let out = run_builtin(&["cargo", "test"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "cargo test (fail) savings only {pct:.1}%");

  let text = String::from_utf8_lossy(&out);
  // The failure signal survives in full.
  assert!(
    text.contains("reduces_buried_error ... FAILED"),
    "lost FAILED marker"
  );
  assert!(text.contains("panicked at"), "lost panic line");
  assert!(
    text.contains("assertion `left == right` failed"),
    "lost assertion"
  );
  assert!(
    text.contains("test result: FAILED. 45 passed; 1 failed"),
    "lost result summary"
  );
  // The passing chatter is gone.
  assert!(!text.contains("case_1 ... ok"), "kept passing-test noise");
  insta::assert_snapshot!("cargo_test_fail", text);
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

#[test]
fn cargo_nextest_chain_keeps_failures_while_compressing() {
  // A `FAIL` marker buried among 45 `PASS` lines plus a panic block. The rank
  // step force-keeps the failure signal; the passing chatter is dropped.
  let raw = include_bytes!("fixtures/cargo-nextest-fail.txt");
  let out = run_builtin(&["cargo", "nextest", "run"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "cargo nextest (fail) savings only {pct:.1}%");

  let text = String::from_utf8_lossy(&out);
  assert!(text.contains("FAIL [   0.013s]"), "lost FAIL marker");
  assert!(text.contains("panicked at"), "lost panic line");
  assert!(text.contains("45 passed, 1 failed"), "lost run summary");
  assert!(!text.contains("PASS ["), "kept passing-test noise");
  insta::assert_snapshot!("cargo_nextest_fail", text);
}

#[test]
fn make_chain_keeps_a_buried_error_a_tail_cut_would_drop() {
  // 67 lines survive the drops (> the 50-line budget) and the compiler error is
  // at the TOP, followed by a long tail of unrelated object compiles. A
  // `truncate tail 50` would keep that tail and drop the error; rank force-keeps
  // it by signal. This is the property the swap buys.
  let raw = include_bytes!("fixtures/make-fail.txt");
  let out = run_builtin(&["make"], raw);
  let text = String::from_utf8_lossy(&out);
  assert!(out.len() < raw.len(), "output should be reduced");
  assert!(
    text.contains("error: 'tok' undeclared"),
    "the buried compiler error was dropped — rank failed to force-keep it"
  );
  assert!(
    text.contains("*** [Makefile:4: all] Error 2"),
    "lost make summary"
  );
  insta::assert_snapshot!("make_fail", text);
}

#[test]
fn make_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/make.txt");
  let out = run_builtin(&["make"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "make savings only {pct:.1}%");
  insta::assert_snapshot!("make", String::from_utf8_lossy(&out));
}

#[test]
fn rsync_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/rsync.txt");
  let out = run_builtin(&["rsync"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "rsync savings only {pct:.1}%");
  insta::assert_snapshot!("rsync", String::from_utf8_lossy(&out));
}

#[test]
fn git_log_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/git-log.txt");
  let out = run_builtin(&["git", "log"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "git log savings only {pct:.1}%");
  insta::assert_snapshot!("git_log", String::from_utf8_lossy(&out));
}

#[test]
fn du_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/du.txt");
  let out = run_builtin(&["du"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "du savings only {pct:.1}%");
  insta::assert_snapshot!("du", String::from_utf8_lossy(&out));
}

#[test]
fn git_diff_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/git-diff.txt");
  let out = run_builtin(&["git", "diff"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "git diff savings only {pct:.1}%");
  insta::assert_snapshot!("git_diff", String::from_utf8_lossy(&out));
}

#[test]
fn ls_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/ls.txt");
  let out = run_builtin(&["ls"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "ls savings only {pct:.1}%");
  insta::assert_snapshot!("ls", String::from_utf8_lossy(&out));
}

#[test]
fn find_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/find.txt");
  let out = run_builtin(&["find"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "find savings only {pct:.1}%");
  insta::assert_snapshot!("find", String::from_utf8_lossy(&out));
}

#[test]
fn terraform_plan_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/terraform-plan.txt");
  let out = run_builtin(&["terraform", "plan"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "terraform plan savings only {pct:.1}%");
  insta::assert_snapshot!("terraform_plan", String::from_utf8_lossy(&out));
}

#[test]
fn terraform_apply_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/terraform-apply.txt");
  let out = run_builtin(&["terraform", "apply"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "terraform apply savings only {pct:.1}%");
  insta::assert_snapshot!("terraform_apply", String::from_utf8_lossy(&out));
}

#[test]
fn terraform_init_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/terraform-init.txt");
  let out = run_builtin(&["terraform", "init"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "terraform init savings only {pct:.1}%");
  insta::assert_snapshot!("terraform_init", String::from_utf8_lossy(&out));
}
