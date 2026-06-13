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
