//! [`Transform`] rule — rewrite each line via a regex substitution.

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Applies a regex substitution to text, replacing all matches.
///
/// `replacement` supports the `regex` crate's capture syntax (`$1`, `${name}`).
/// Lines with no match pass through untouched.
///
/// By default matching is **per line**: each line is substituted on its own, so
/// the pattern never sees a `\n` and `^`/`$` mean line start/end. Set
/// [`multiline`](Self::multiline) to run the substitution over the whole buffer
/// instead, letting a pattern span lines (use `(?s)` for `.` to match `\n`,
/// `(?m)` for per-line anchors). A multiline substitution can change the line
/// *count*.
///
/// Because it alters content, `Transform` is not pipe-safe: a substitution can
/// remove text a downstream `grep` would have matched, so
/// [`Rule::preserves_lines`] stays `false`.
///
/// # Example
///
/// Input:
/// ```text
/// aws_instance.web: Creating... [id=i-0abc123]
/// ```
/// With `pattern = r"\s*\[id=[^\]]*\]"` and `replacement = ""`:
/// ```text
/// aws_instance.web: Creating...
/// ```
#[derive(Debug)]
pub struct Transform {
  /// Regex matched against each line (or the whole buffer when
  /// [`multiline`](Self::multiline) is set); every match is replaced.
  pub pattern:     Regex,
  /// Replacement text; supports `$1` / `${name}` capture references.
  pub replacement: String,
  /// Match over the whole buffer instead of line by line, allowing a pattern
  /// to span newlines.
  pub multiline:   bool,
}

impl Rule for Transform {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    if self.multiline {
      let replaced = self.pattern.replace_all(input, self.replacement.as_str());
      return join_lines(&replaced.lines().collect::<Vec<_>>());
    }
    let lines: Vec<String> = input
      .lines()
      .map(|l| {
        self
          .pattern
          .replace_all(l, self.replacement.as_str())
          .into_owned()
      })
      .collect();
    join_lines(&lines.iter().map(String::as_str).collect::<Vec<_>>())
  }

  fn describe(&self) -> String {
    let tag = if self.multiline { " (multiline)" } else { "" };
    format!(
      "transform{} /{}/ -> \"{}\"",
      tag,
      self.pattern.as_str(),
      self.replacement
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn transform(
    pattern: &str,
    replacement: &str,
  ) -> Transform {
    Transform {
      pattern:     Regex::new(pattern).expect("re"),
      replacement: replacement.to_string(),
      multiline:   false,
    }
  }

  fn transform_ml(
    pattern: &str,
    replacement: &str,
  ) -> Transform {
    Transform {
      pattern:     Regex::new(pattern).expect("re"),
      replacement: replacement.to_string(),
      multiline:   true,
    }
  }

  #[test]
  fn replaces_all_matches_on_each_line() {
    let rule = transform(r"\s*\[id=[^\]]*\]", "");
    assert_eq!(
      rule.apply("web: Creating... [id=i-1]\ndb: Creating... [id=i-2]"),
      "web: Creating...\ndb: Creating...\n"
    );
  }

  #[test]
  fn supports_capture_references() {
    let rule = transform(r"module\.(\w+)\.", "$1/");
    assert_eq!(
      rule.apply("module.platform.resource"),
      "platform/resource\n"
    );
  }

  #[test]
  fn leaves_non_matching_lines_untouched() {
    let rule = transform(r"xyz", "");
    assert_eq!(rule.apply("abc\ndef"), "abc\ndef\n");
  }

  #[test]
  fn per_line_pattern_cannot_span_newlines() {
    // Without multiline, the `(?s).` never sees the `\n` between lines.
    let rule = transform(r"(?s)foo.*bar", "X");
    assert_eq!(rule.apply("foo\nbar"), "foo\nbar\n");
  }

  #[test]
  fn multiline_pattern_spans_newlines() {
    let rule = transform_ml(r"(?s)jsonencode\(.*?\)", "jsonencode(…)");
    assert_eq!(
      rule.apply("x = jsonencode(\n  {a=1}\n)\nend"),
      "x = jsonencode(…)\nend\n"
    );
  }

  #[test]
  fn is_not_pipe_safe() {
    assert!(!transform("x", "y").preserves_lines());
  }

  #[test]
  fn describe_shows_pattern_and_replacement() {
    assert_eq!(
      transform("foo", "bar").describe(),
      "transform /foo/ -> \"bar\""
    );
    assert_eq!(
      transform_ml("foo", "bar").describe(),
      "transform (multiline) /foo/ -> \"bar\""
    );
  }
}
