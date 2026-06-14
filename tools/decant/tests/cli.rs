//! Black-box CLI tests: drive the built `decant` binary end-to-end.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

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
  use std::{io::Write, process::Stdio};

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
    stdout.contains("decant run -- cargo test"),
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
