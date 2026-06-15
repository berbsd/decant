//! Black-box CLI tests: drive the built `decant` binary end-to-end.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
  io::Write,
  process::{Command, Stdio},
};

fn decant() -> Command {
  Command::new(env!("CARGO_BIN_EXE_decant"))
}

#[test]
fn passthrough_emits_stdout_and_zero_exit() {
  let out = decant()
    .args(["run", "--no-stats", "--", "printf", "hello"])
    .output()
    .unwrap();
  assert_eq!(out.stdout, b"hello");
  assert_eq!(out.status.code(), Some(0));
}

#[test]
fn propagates_child_exit_code() {
  let out = decant()
    .args(["run", "--no-stats", "--", "sh", "-c", "exit 7"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(7));
}

#[test]
fn idle_timeout_kills_and_returns_124() {
  let out = decant()
    .args(["run", "--idle-timeout", "1", "--", "sh", "-c", "sleep 5"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(124));
  let stderr = String::from_utf8_lossy(&out.stderr);
  assert!(stderr.contains("idle timeout"), "stderr was: {stderr}");
}

#[test]
fn explain_lists_builtin_chain() {
  let out = decant()
    .args(["explain", "--", "cargo", "build"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(
    stdout.contains("built-in cargo-build"),
    "stdout was: {stdout}"
  );
  assert!(stdout.contains("collapse"), "stdout was: {stdout}");
}

#[test]
fn hook_rewrites_a_bash_command() {
  let mut child = decant()
    .args(["hook", "claude"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(br#"{"tool_name":"Bash","tool_input":{"command":"cargo test"}}"#)
    .unwrap();
  let out = child.wait_with_output().unwrap();
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(
    stdout.contains("decant run --reduce -- cargo test"),
    "stdout was: {stdout}"
  );
}

#[test]
fn init_project_writes_settings() {
  let dir = tempfile::tempdir().unwrap();
  let out = decant()
    .args(["init", "--project"])
    .current_dir(dir.path())
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  let settings = std::fs::read_to_string(dir.path().join(".claude").join("settings.json")).unwrap();
  assert!(
    settings.contains("decant hook claude"),
    "settings: {settings}"
  );
}

#[test]
fn history_reports_a_recorded_run() {
  let dir = tempfile::tempdir().unwrap();
  let db = dir.path().join("metrics.db");

  // A run records a row...
  let run = decant()
    .env("DECANT_DB_PATH", &db)
    .args(["run", "--no-stats", "--", "printf", "hi"])
    .output()
    .unwrap();
  assert_eq!(run.status.code(), Some(0));

  // ...which `history` then reports.
  let out = decant()
    .env("DECANT_DB_PATH", &db)
    .args(["history"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(stdout.contains("printf"), "stdout was: {stdout}");
  assert!(stdout.contains("runs"), "stdout was: {stdout}");
}

#[test]
fn explain_with_no_args_lists_builtins() {
  let out = decant().args(["explain"]).output().unwrap();
  assert_eq!(out.status.code(), Some(0));
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert!(stdout.contains("Built-in command configs:"), "{stdout}");
  assert!(stdout.contains("cargo-build"), "{stdout}");
}

#[test]
fn explain_unknown_command_is_identity() {
  let out = decant()
    .args(["explain", "--", "totally-unknown-xyz"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  assert!(
    String::from_utf8_lossy(&out.stdout).contains("identity"),
    "{:?}",
    out
  );
}

#[test]
fn init_is_idempotent_on_second_run() {
  let dir = tempfile::tempdir().unwrap();
  decant()
    .args(["init", "--project"])
    .current_dir(dir.path())
    .output()
    .unwrap();
  let second = decant()
    .args(["init", "--project"])
    .current_dir(dir.path())
    .output()
    .unwrap();
  assert_eq!(second.status.code(), Some(0));
  assert!(
    String::from_utf8_lossy(&second.stdout).contains("already present"),
    "{:?}",
    second
  );
}

#[test]
fn init_global_scope_uses_config_dir() {
  let dir = tempfile::tempdir().unwrap();
  let out = decant()
    .args(["init"]) // no --project => global scope
    .env("CLAUDE_CONFIG_DIR", dir.path())
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  assert!(dir.path().join("settings.json").exists(), "{:?}", out);
}

#[test]
fn init_unknown_agent_fails() {
  let out = decant()
    .args(["init", "--agent", "bogus", "--project"])
    .output()
    .unwrap();
  assert_ne!(out.status.code(), Some(0));
  assert!(
    String::from_utf8_lossy(&out.stderr).contains("unknown agent"),
    "{:?}",
    out
  );
}

#[test]
fn hook_unknown_agent_emits_empty_object() {
  let out = decant().args(["hook", "no-such-agent"]).output().unwrap();
  assert_eq!(out.status.code(), Some(0));
  assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "{}");
}

#[test]
fn hook_passes_through_non_bash_input() {
  let mut child = decant()
    .args(["hook", "claude"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(br#"{"tool_name":"Read","tool_input":{"file_path":"/x"}}"#)
    .unwrap();
  let out = child.wait_with_output().unwrap();
  assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "{}");
}

#[test]
fn history_empty_db_reports_no_runs() {
  let dir = tempfile::tempdir().unwrap();
  let db = dir.path().join("metrics.db");
  let out = decant()
    .env("DECANT_DB_PATH", &db)
    .args(["history"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  assert!(
    String::from_utf8_lossy(&out.stdout).contains("no runs recorded"),
    "{:?}",
    out
  );
}

#[test]
fn history_json_output() {
  let dir = tempfile::tempdir().unwrap();
  let db = dir.path().join("metrics.db");
  let out = decant()
    .env("DECANT_DB_PATH", &db)
    .args(["history", "--json"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  assert!(
    String::from_utf8_lossy(&out.stdout).contains("total_runs"),
    "{:?}",
    out
  );
}

#[test]
fn history_reports_a_reduced_run() {
  let dir = tempfile::tempdir().unwrap();
  let db = dir.path().join("metrics.db");
  // `ls` has a built-in config, so its run is recorded as "reduced".
  decant()
    .env("DECANT_DB_PATH", &db)
    .args(["run", "--no-stats", "--", "ls", "/"])
    .output()
    .unwrap();
  let out = decant()
    .env("DECANT_DB_PATH", &db)
    .args(["history"])
    .output()
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  assert!(
    String::from_utf8_lossy(&out.stdout).contains("Reduced"),
    "{:?}",
    out
  );
}

#[test]
fn reduce_flag_overrides_pipe_safe_when_piped() {
  // A hook-wrapped command's stdout is captured via a pipe (non-TTY), so by
  // default decant is pipe-safe and skips lossy rules. `--reduce` must force
  // full reduction even when piped — this is what makes the agent path (which
  // emits `decant run --reduce -- ...`) actually reduce. The `ls` built-in
  // truncates at 40 lines, so 50 files distinguish the two modes.
  let dir = tempfile::tempdir().unwrap();
  for i in 0..50 {
    std::fs::write(dir.path().join(format!("f{i:02}")), b"").unwrap();
  }

  // Default piped run: pipe-safe -> truncate skipped -> every line kept.
  let safe = decant()
    .args(["run", "--no-stats", "--", "ls", "-1"])
    .arg(dir.path())
    .output()
    .unwrap();
  assert!(
    !String::from_utf8_lossy(&safe.stdout).contains("more lines"),
    "piped (pipe-safe) must keep all lines: {:?}",
    safe
  );

  // --reduce: full reduction even when piped -> truncated.
  let reduced = decant()
    .args(["run", "--no-stats", "--reduce", "--", "ls", "-1"])
    .arg(dir.path())
    .output()
    .unwrap();
  assert!(
    String::from_utf8_lossy(&reduced.stdout).contains("more lines"),
    "--reduce must truncate even when piped: {:?}",
    reduced
  );
}
