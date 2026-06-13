//! [`KeepAfter`] rule — discard everything before the first matching line.

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Discards all lines before the first one that matches the regex, then keeps
/// that line and everything that follows.
///
/// If no line matches, the entire output is discarded.
#[derive(Debug)]
pub struct KeepAfter(
  /// The compiled regex used to locate the start of the interesting section.
  pub Regex,
);

impl Rule for KeepAfter {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    join_lines(
      &input
        .lines()
        .skip_while(|l| !self.0.is_match(l))
        .collect::<Vec<_>>(),
    )
  }

  fn describe(&self) -> String {
    format!("keep_after /{}/", self.0.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn keep_after_drops_until_first_match() {
    let rule = KeepAfter(Regex::new("RESULT").expect("re"));
    assert_eq!(
      rule.apply("noise\nmore\nRESULT ok\ntail"),
      "RESULT ok\ntail\n"
    );
  }
}
