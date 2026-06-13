//! The [`Rule`] trait and shared helpers. Each rule lives in its own file
//! under [`crate::rules`].

/// Which end of the output [`crate::rules::Truncate`] keeps.
///
/// Named `Side` rather than `Keep` to avoid a name collision with the
/// [`crate::rules::Keep`] rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
  /// Keep the first `max_lines` lines and append a `… N more lines` marker.
  Head,
  /// Prepend a `… N more lines` marker and keep the last `max_lines` lines.
  Tail,
}

/// A single line-oriented reduction step.
///
/// Rules are chained together inside a [`crate::RuleChain`] and applied in
/// order. Each rule takes the full text produced by the previous step, so
/// earlier rules can significantly change what later rules see.
pub trait Rule: std::fmt::Debug {
  /// Apply this rule to `input` and return the reduced text.
  fn apply(
    &self,
    input: &str,
  ) -> String;
  /// Short, human-readable summary shown by `decant explain`.
  fn describe(&self) -> String;
}

/// Join lines back into text, terminating non-empty output with a newline.
pub(crate) fn join_lines(lines: &[&str]) -> String {
  if lines.is_empty() {
    return String::new();
  }
  let mut out = lines.join("\n");
  out.push('\n');
  out
}
