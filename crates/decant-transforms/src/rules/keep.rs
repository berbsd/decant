//! [`Keep`] rule — retain only lines that match a regex.

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Retains only lines that match the regex; all others are discarded.
#[derive(Debug)]
pub struct Keep(
  /// The compiled regex; only lines for which [`Regex::is_match`] returns
  /// `true` are kept.
  pub Regex,
);

impl Rule for Keep {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    join_lines(
      &input
        .lines()
        .filter(|l| self.0.is_match(l))
        .collect::<Vec<_>>(),
    )
  }

  fn describe(&self) -> String {
    format!("keep /{}/", self.0.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn keep_retains_only_matching_lines() {
    let rule = Keep(Regex::new("error").expect("re"));
    assert_eq!(rule.apply("info x\nerror y\ninfo z"), "error y\n");
  }

  #[test]
  fn describe_shows_the_pattern() {
    assert_eq!(
      Keep(Regex::new("error").expect("re")).describe(),
      "keep /error/"
    );
  }
}
