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
fn git_status_chain_saves_at_least_60pct() {
  let raw = include_bytes!("fixtures/git-status.txt");
  let out = run_builtin(&["git", "status"], raw);
  let pct = savings_pct(raw, &out);
  assert!(pct >= 60.0, "git status savings only {pct:.1}%");
  insta::assert_snapshot!("git_status", String::from_utf8_lossy(&out));
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
