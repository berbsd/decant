//! [`Collapse`] rule — replace matching lines with a single count summary.

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Removes matching lines, emitting one summary line with their count.
///
/// `{n}` in `label` is replaced by the number of matched lines. The summary
/// line appears at the position of the first matched line; all subsequent
/// matches are silently dropped. Non-matching lines are preserved in order.
///
/// # Example
///
/// Input:
/// ```text
/// Compiling foo v1.0
/// Compiling bar v2.0
/// Finished dev [unoptimized] target(s) in 0.52s
/// ```
/// With `pattern = r"^Compiling "` and `label = "{n} crates compiled"`:
/// ```text
/// 2 crates compiled
/// Finished dev [unoptimized] target(s) in 0.52s
/// ```
#[derive(Debug)]
pub struct Collapse {
  /// Regex applied to each line; matching lines are counted and collapsed.
  pub pattern: Regex,
  /// Summary text emitted in place of all matched lines. `{n}` is replaced
  /// by the match count.
  pub label:   String,
}

impl Rule for Collapse {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut count = 0usize;
    let mut slot: Option<usize> = None;
    for line in input.lines() {
      if self.pattern.is_match(line) {
        if slot.is_none() {
          slot = Some(out.len());
          out.push(String::new());
        }
        count += 1;
      } else {
        out.push(line.to_string());
      }
    }
    if let Some(i) = slot {
      out[i] = self.label.replace("{n}", &count.to_string());
    }
    join_lines(&out.iter().map(String::as_str).collect::<Vec<_>>())
  }

  fn describe(&self) -> String {
    format!("collapse /{}/ -> \"{}\"", self.pattern.as_str(), self.label)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn collapse_counts_and_substitutes() {
    let rule = Collapse {
      pattern: Regex::new(r"^Compiling ").expect("re"),
      label:   "{n} crates compiled".to_string(),
    };
    assert_eq!(
      rule.apply("Compiling a\nCompiling b\nCompiling c\nFinished"),
      "3 crates compiled\nFinished\n"
    );
  }
}
