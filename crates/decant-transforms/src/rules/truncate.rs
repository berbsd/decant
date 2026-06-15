//! [`Truncate`] rule — cap line count and add a `… N more lines` marker.

use crate::rule::{Rule, Side, join_lines};

/// Caps line count, emitting a `… N more lines` marker for the omitted section.
///
/// When `keep` is [`Side::Tail`], the marker precedes the kept lines
/// (useful for showing the end of compiler output). When `keep` is
/// [`Side::Head`], the marker follows them (useful for showing the beginning
/// of a long listing).
#[derive(Debug)]
pub struct Truncate {
  /// Maximum number of lines to include in the output.
  pub max_lines: usize,
  /// Which end of the output to keep.
  pub keep:      Side,
}

impl Rule for Truncate {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() <= self.max_lines {
      return join_lines(&lines);
    }
    let omitted = lines.len() - self.max_lines;
    let marker = format!("… {omitted} more lines");
    let mut out: Vec<String> = Vec::with_capacity(self.max_lines + 1);
    match self.keep {
      | Side::Head => {
        for l in &lines[..self.max_lines] {
          out.push((*l).to_string());
        }
        out.push(marker);
      },
      | Side::Tail => {
        out.push(marker);
        for l in &lines[lines.len() - self.max_lines..] {
          out.push((*l).to_string());
        }
      },
    }
    join_lines(&out.iter().map(String::as_str).collect::<Vec<_>>())
  }

  fn describe(&self) -> String {
    let side = match self.keep {
      | Side::Head => "head",
      | Side::Tail => "tail",
    };
    format!("truncate {side} {}", self.max_lines)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn truncate_tail_keeps_last_and_marks_omitted() {
    let rule = Truncate { max_lines: 2, keep: Side::Tail };
    assert_eq!(rule.apply("1\n2\n3\n4\n5"), "… 3 more lines\n4\n5\n");
  }

  #[test]
  fn truncate_head_keeps_first_and_marks_omitted() {
    let rule = Truncate { max_lines: 2, keep: Side::Head };
    assert_eq!(rule.apply("1\n2\n3\n4\n5"), "1\n2\n… 3 more lines\n");
  }

  #[test]
  fn truncate_under_limit_is_unchanged() {
    let rule = Truncate { max_lines: 5, keep: Side::Tail };
    assert_eq!(rule.apply("1\n2\n3"), "1\n2\n3\n");
  }

  #[test]
  fn describe_names_side_and_limit() {
    assert_eq!(
      Truncate { max_lines: 100, keep: Side::Tail }.describe(),
      "truncate tail 100"
    );
    assert_eq!(
      Truncate { max_lines: 40, keep: Side::Head }.describe(),
      "truncate head 40"
    );
  }
}
