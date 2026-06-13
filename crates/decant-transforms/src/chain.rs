//! An ordered chain of rules, applied to a captured command's output.

use decant_core::{Captured, Transform, TransformOutput};

use crate::rule::Rule;

/// An ordered list of boxed [`Rule`]s that implements
/// [`decant_core::Transform`].
///
/// Rules are applied left-to-right: the output of rule `n` is the input to
/// rule `n+1`. An empty chain is the identity transform — it passes raw bytes
/// through without any UTF-8 decode, preserving binary output exactly.
///
/// Build a named chain with [`RuleChain::new`], or get the identity with
/// [`RuleChain::passthrough`]. The TOML config layer ([`crate::config`]) is
/// the usual way to construct a chain from a config file.
#[derive(Debug)]
pub struct RuleChain {
  name:  String,
  rules: Vec<Box<dyn Rule>>,
}

impl RuleChain {
  /// Build a named chain from boxed rules.
  #[must_use]
  pub fn new(
    name: String,
    rules: Vec<Box<dyn Rule>>,
  ) -> Self {
    Self { name, rules }
  }

  /// A do-nothing chain that passes raw bytes through unchanged.
  #[must_use]
  pub fn passthrough() -> Self {
    Self { name: "identity".to_string(), rules: Vec::new() }
  }

  /// True when this chain applies no rules (raw passthrough).
  #[must_use]
  pub fn is_passthrough(&self) -> bool {
    self.rules.is_empty()
  }

  /// One human-readable line per rule, for `decant explain`.
  #[must_use]
  pub fn describe(&self) -> Vec<String> {
    self.rules.iter().map(|r| r.describe()).collect()
  }

  /// Apply the chain to one stream's bytes. An empty chain returns the bytes
  /// untouched (no UTF-8 decode), preserving binary output.
  #[must_use]
  pub fn run(
    &self,
    bytes: &[u8],
  ) -> Vec<u8> {
    if self.rules.is_empty() {
      return bytes.to_vec();
    }
    let text = String::from_utf8_lossy(bytes).into_owned();
    let out = self.rules.iter().fold(text, |t, rule| rule.apply(&t));
    out.into_bytes()
  }
}

impl Transform for RuleChain {
  fn name(&self) -> &str {
    &self.name
  }

  fn apply(
    &self,
    captured: &Captured,
  ) -> TransformOutput {
    TransformOutput {
      stdout: self.run(&captured.stdout),
      stderr: self.run(&captured.stderr),
    }
  }
}

#[cfg(test)]
mod tests {
  use decant_core::{Captured, Transform};
  use regex::Regex;

  use super::*;
  use crate::{
    rule::Side,
    rules::{Dedup, Drop, StripAnsi, Truncate},
  };

  fn cap(stdout: &[u8]) -> Captured {
    Captured {
      stdout:    stdout.to_vec(),
      stderr:    Vec::new(),
      exit_code: 0,
      timeout:   None,
    }
  }

  #[test]
  fn chain_applies_rules_in_order() {
    let chain = RuleChain::new("t".to_string(), vec![
      Box::new(StripAnsi),
      Box::new(Drop(Regex::new(r"\bok$").expect("re"))),
      Box::new(Truncate { max_lines: 10, keep: Side::Tail }),
    ]);
    let out = chain.apply(&cap(b"\x1b[32mtest a ... ok\x1b[0m\nfail b\n"));
    assert_eq!(out.stdout, b"fail b\n");
  }

  #[test]
  fn passthrough_preserves_raw_bytes() {
    let chain = RuleChain::passthrough();
    let raw = vec![0xff, 0xfe, b'\n'];
    assert_eq!(chain.run(&raw), raw);
    assert!(chain.is_passthrough());
  }

  #[test]
  fn describe_lists_each_step() {
    let chain = RuleChain::new("t".to_string(), vec![Box::new(StripAnsi), Box::new(Dedup)]);
    assert_eq!(chain.describe(), vec![
      "strip_ansi".to_string(),
      "dedup".to_string()
    ]);
  }
}
