//! [`Dedup`] rule — collapse consecutive duplicate lines.

use crate::rule::{Rule, join_lines};

/// Collapses consecutive duplicate lines.
#[derive(Debug)]
pub struct Dedup;

impl Rule for Dedup {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in input.lines() {
      if out.last() != Some(&line) {
        out.push(line);
      }
    }
    join_lines(&out)
  }

  fn describe(&self) -> String {
    "dedup".to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn dedup_collapses_consecutive_duplicates() {
    assert_eq!(Dedup.apply("a\na\nb\nb\nb\na"), "a\nb\na\n");
  }
}
