//! [`Cut`] rule — drop a whole section delimited by two markers.

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Drops every line from the first `begin` match up to (but not including) the
/// next `end` match.
///
/// The `begin` line itself is dropped; the `end` line is **kept** (it usually
/// carries a summary worth retaining, e.g. terraform's `Plan:` line). Scanning
/// then resumes, so multiple non-overlapping sections are removed. If a `begin`
/// match is never closed by an `end` match, the section extends to end of
/// input.
///
/// Unlike line-wise rules ([`crate::rules::Drop`], [`crate::rules::Keep`]),
/// `Cut` is positional: it distinguishes otherwise-identical lines by the
/// section they fall in — e.g. a terraform plan diff versus an `Outputs:`
/// block.
///
/// # Example
///
/// Input:
/// ```text
/// header
/// Terraform will perform the following actions:
///   ~ resource "x" {}
/// Plan: 0 to add, 1 to change, 0 to destroy.
/// ```
/// With `begin = r"^Terraform will perform"` and `end = r"^Plan: "`:
/// ```text
/// header
/// Plan: 0 to add, 1 to change, 0 to destroy.
/// ```
#[derive(Debug)]
pub struct Cut {
  /// Regex marking the first line of a section to drop (the line is dropped).
  pub begin: Regex,
  /// Regex marking the end of the section (this line is kept).
  pub end:   Regex,
}

impl Rule for Cut {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut in_section = false;
    for line in input.lines() {
      if in_section {
        if self.end.is_match(line) {
          in_section = false;
          out.push(line);
        }
      } else if self.begin.is_match(line) {
        in_section = true;
      } else {
        out.push(line);
      }
    }
    join_lines(&out)
  }

  fn describe(&self) -> String {
    format!("cut /{}/../{}/", self.begin.as_str(), self.end.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn cut(
    begin: &str,
    end: &str,
  ) -> Cut {
    Cut {
      begin: Regex::new(begin).expect("re"),
      end:   Regex::new(end).expect("re"),
    }
  }

  #[test]
  fn drops_begin_and_body_keeps_end() {
    let rule = cut("^START", "^END");
    assert_eq!(
      rule.apply("head\nSTART\nbody1\nbody2\nEND\ntail"),
      "head\nEND\ntail\n"
    );
  }

  #[test]
  fn unclosed_section_runs_to_eof() {
    let rule = cut("^START", "^END");
    assert_eq!(rule.apply("head\nSTART\nbody\nmore"), "head\n");
  }

  #[test]
  fn removes_multiple_sections() {
    let rule = cut("^START", "^END");
    assert_eq!(
      rule.apply("a\nSTART\nx\nEND\nb\nSTART\ny\nEND\nc"),
      "a\nEND\nb\nEND\nc\n"
    );
  }

  #[test]
  fn passes_through_when_no_begin() {
    let rule = cut("^START", "^END");
    assert_eq!(rule.apply("a\nb\nc"), "a\nb\nc\n");
  }

  #[test]
  fn describe_shows_both_patterns() {
    assert_eq!(cut("^START", "^END").describe(), "cut /^START/../^END/");
  }
}
