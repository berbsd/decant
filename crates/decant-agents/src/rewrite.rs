//! Agent-agnostic, shell-aware command rewriting.
//!
//! Wraps a simple command so its output flows through `decant run`, e.g.
//! `cargo test` -> `decant run --reduce -- cargo test`. `--reduce` is forced
//! because the agent captures stdout via a pipe (non-TTY), where decant's
//! pipe-safe default would otherwise skip the lossy rules. Anything it cannot
//! safely wrap (shell pipelines, redirects, builtins) is returned unchanged.

use std::sync::LazyLock;

use regex::Regex;

/// Shell constructs decant cannot safely wrap in v1 — their presence forces a
/// passthrough (the command is returned unchanged).
const UNSAFE_CONSTRUCTS: &[&str] = &["|", "&&", "||", ";", "&", ">", "<", "`", "$(", "\n"];

/// Shell builtins that are not executables and must never be wrapped.
const SHELL_BUILTINS: &[&str] = &[
  "cd", "export", "source", ".", ":", "eval", "exec", "set", "unset", "alias", "unalias", "pushd",
  "popd", "read", "trap", "wait", "local", "declare", "true", "false", "test", "[",
];

#[allow(clippy::unwrap_used)] // a constant regex literal is correct by construction
static ENV_ASSIGN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*=").unwrap());

/// Rewrite `command` to route through `decant run`, or return it unchanged when
/// it cannot be wrapped safely (shell constructs, builtins, already-`decant`).
#[must_use]
pub fn rewrite_command(command: &str) -> String {
  let trimmed = command.trim();
  if trimmed.is_empty() {
    return command.to_string();
  }
  if UNSAFE_CONSTRUCTS.iter().any(|c| trimmed.contains(c)) {
    return command.to_string();
  }

  let tokens: Vec<&str> = trimmed.split_whitespace().collect();
  let mut idx = 0;
  while idx < tokens.len() && ENV_ASSIGN.is_match(tokens[idx]) {
    idx += 1;
  }
  let Some(program) = tokens.get(idx) else {
    return command.to_string(); // only env assignments, no program
  };
  if *program == "decant" || SHELL_BUILTINS.contains(program) {
    return command.to_string();
  }

  let env_prefix = tokens[..idx].join(" ");
  let rest = tokens[idx..].join(" ");
  // The hook's consumer is the agent/LLM reading the captured output, so force
  // full reduction (`--reduce`) — decant's default pipe-safe mode would skip
  // the lossy rules because the agent captures stdout via a pipe (non-TTY).
  if env_prefix.is_empty() {
    format!("decant run --reduce -- {rest}")
  } else {
    format!("{env_prefix} decant run --reduce -- {rest}")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wraps_a_simple_command() {
    assert_eq!(
      rewrite_command("cargo test"),
      "decant run --reduce -- cargo test"
    );
  }

  #[test]
  fn forces_reduce_so_the_agent_path_actually_reduces() {
    // The agent reads decant's stdout via a pipe (non-TTY); without --reduce,
    // pipe-safe mode skips the lossy rules and the LLM sees ~unreduced output.
    // This guards the hook against silently regressing to a no-op reduction.
    assert!(rewrite_command("terraform plan").contains(" --reduce -- "));
    assert!(rewrite_command("FOO=1 cargo build").contains(" decant run --reduce -- "));
  }

  #[test]
  fn preserves_env_prefix() {
    assert_eq!(
      rewrite_command("RUST_LOG=debug cargo test"),
      "RUST_LOG=debug decant run --reduce -- cargo test"
    );
    assert_eq!(
      rewrite_command("FOO=1 BAR=2 make"),
      "FOO=1 BAR=2 decant run --reduce -- make"
    );
  }

  #[test]
  fn round_trips_quoted_args() {
    assert_eq!(
      rewrite_command("git commit -m \"hello world\""),
      "decant run --reduce -- git commit -m \"hello world\""
    );
  }

  #[test]
  fn skips_builtins() {
    assert_eq!(rewrite_command("cd foo"), "cd foo");
    assert_eq!(rewrite_command("export X=1"), "export X=1");
  }

  #[test]
  fn skips_already_decant() {
    assert_eq!(
      rewrite_command("decant run -- cargo test"),
      "decant run -- cargo test"
    );
  }

  #[test]
  fn passes_through_shell_constructs() {
    assert_eq!(
      rewrite_command("cargo build && cargo test"),
      "cargo build && cargo test"
    );
    assert_eq!(
      rewrite_command("cargo test | grep FAIL"),
      "cargo test | grep FAIL"
    );
    assert_eq!(rewrite_command("ls > out.txt"), "ls > out.txt");
    assert_eq!(rewrite_command("echo $(date)"), "echo $(date)");
  }

  #[test]
  fn passes_through_empty() {
    assert_eq!(rewrite_command("   "), "   ");
  }
}
