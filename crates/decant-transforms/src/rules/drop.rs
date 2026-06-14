//! [`Drop`] rule — remove lines that match a regex.

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Removes every line that matches the regex.
///
/// Lines that do not match are preserved in order.
#[derive(Debug)]
pub struct Drop(
  /// The compiled regex; any line for which [`Regex::is_match`] returns `true`
  /// is discarded.
  pub Regex,
);

impl Rule for Drop {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    join_lines(
      &input
        .lines()
        .filter(|l| !self.0.is_match(l))
        .collect::<Vec<_>>(),
    )
  }

  fn describe(&self) -> String {
    format!("drop /{}/", self.0.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn drop_removes_matching_lines() {
    let rule = Drop(Regex::new(r"\bok$").expect("re"));
    assert_eq!(
      rule.apply("test a ... ok\nfail b\ntest c ... ok"),
      "fail b\n"
    );
  }

  #[test]
  fn drop_is_not_pipe_safe() {
    assert!(!Drop(Regex::new("x").expect("re")).preserves_lines());
  }
}
