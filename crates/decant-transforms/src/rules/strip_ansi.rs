//! [`StripAnsi`] rule — remove ANSI escape/color codes from output.

use std::sync::LazyLock;

use regex::Regex;

use crate::rule::Rule;

const ANSI_PATTERN: &str = r"\x1b\[[0-9;?]*[ -/]*[@-~]";

#[allow(clippy::unwrap_used)] // a constant regex literal is correct by construction
static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(ANSI_PATTERN).unwrap());

/// Removes ANSI color/escape codes.
#[derive(Debug)]
pub struct StripAnsi;

impl Rule for StripAnsi {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    ANSI_RE.replace_all(input, "").into_owned()
  }

  fn describe(&self) -> String {
    "strip_ansi".to_string()
  }

  // Only escape sequences are removed; every line and all its visible text
  // survive, so a downstream `grep` is unaffected (in fact made more reliable).
  fn preserves_lines(&self) -> bool {
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn strip_ansi_removes_color_codes() {
    assert_eq!(StripAnsi.apply("\x1b[32mok\x1b[0m done"), "ok done");
  }

  #[test]
  fn strip_ansi_is_pipe_safe() {
    assert!(StripAnsi.preserves_lines());
  }
}
