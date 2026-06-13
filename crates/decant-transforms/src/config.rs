//! TOML config model and compilation to a [`RuleChain`].
//!
//! A config file is a list of `[[step]]` tables, each with a `type` field
//! that selects a [`StepSpec`] variant. Unknown top-level keys (e.g.
//! `command`, `subcommand`) are silently ignored by serde.
//!
//! # TOML format
//!
//! ```toml
//! [[step]]
//! type = "strip_ansi"
//!
//! [[step]]
//! type = "collapse"
//! pattern = '^Compiling '
//! label = "{n} crates compiled"
//!
//! [[step]]
//! type = "truncate"
//! max_lines = 50
//! keep = "tail"   # optional; default is "tail"
//! ```

use regex::Regex;
use serde::Deserialize;

use crate::{
  chain::RuleChain,
  rule::{Rule, Side},
  rules,
};

/// A parsed command config: an ordered list of steps.
///
/// The routing key comes from the filename or argv, so the config itself
/// carries no command/subcommand fields. Unknown fields are silently ignored.
#[derive(Debug, Deserialize)]
pub struct CommandConfig {
  /// Ordered list of rule steps to apply.
  #[serde(default)]
  pub step: Vec<StepSpec>,
}

/// One step as written in TOML, discriminated by the `type` field.
///
/// Each variant maps directly to a [`crate::Rule`] implementation. See each
/// rule's struct docs for behaviour details.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepSpec {
  /// Strip ANSI escape/color codes. Maps to [`crate::rules::StripAnsi`].
  StripAnsi,
  /// Drop every line that matches `pattern`. Maps to [`crate::rules::Drop`].
  Drop {
    /// RE2 regex; the step is rejected at compile time if the pattern is
    /// invalid.
    pattern: String,
  },
  /// Keep only lines that match `pattern`. Maps to [`crate::rules::Keep`].
  Keep {
    /// RE2 regex applied to each line.
    pattern: String,
  },
  /// Keep everything from the first matching line onward.
  /// Maps to [`crate::rules::KeepAfter`].
  KeepAfter {
    /// RE2 regex; lines before the first match are discarded.
    pattern: String,
  },
  /// Replace all matching lines with a single summary line.
  /// Maps to [`crate::rules::Collapse`].
  Collapse {
    /// RE2 regex; lines that match are counted and collapsed.
    pattern: String,
    /// Summary text; `{n}` is replaced by the match count.
    label:   String,
  },
  /// Remove consecutive duplicate lines. Maps to [`crate::rules::Dedup`].
  Dedup,
  /// Cap line count, emitting a `… N more lines` marker.
  /// Maps to [`crate::rules::Truncate`].
  Truncate {
    /// Maximum number of lines to keep.
    max_lines: usize,
    /// Which end to keep. Defaults to `tail`.
    #[serde(default)]
    keep:      KeepSpec,
  },
}

/// Which end of the output [`StepSpec::Truncate`] keeps (TOML-facing name).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeepSpec {
  /// Keep the last `max_lines` lines (the default).
  #[default]
  Tail,
  /// Keep the first `max_lines` lines.
  Head,
}

impl From<KeepSpec> for Side {
  fn from(k: KeepSpec) -> Self {
    match k {
      | KeepSpec::Head => Side::Head,
      | KeepSpec::Tail => Side::Tail,
    }
  }
}

/// Failure parsing or compiling a TOML config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
  /// The TOML text is syntactically invalid or has an unrecognised `type`
  /// value in a `[[step]]` table.
  #[error("invalid TOML: {0}")]
  Toml(#[from] toml::de::Error),
  /// A `pattern` field contains an invalid RE2 regex.
  #[error("invalid regex `{pattern}`: {source}")]
  Regex {
    /// The regex string that failed to compile.
    pattern: String,
    /// The underlying compile error from the `regex` crate.
    source:  regex::Error,
  },
}

fn compile_regex(pattern: String) -> Result<Regex, ConfigError> {
  Regex::new(&pattern).map_err(|source| ConfigError::Regex { pattern, source })
}

impl StepSpec {
  fn into_rule(self) -> Result<Box<dyn Rule>, ConfigError> {
    let rule: Box<dyn Rule> = match self {
      | StepSpec::StripAnsi => Box::new(rules::StripAnsi),
      | StepSpec::Drop { pattern } => Box::new(rules::Drop(compile_regex(pattern)?)),
      | StepSpec::Keep { pattern } => Box::new(rules::Keep(compile_regex(pattern)?)),
      | StepSpec::KeepAfter { pattern } => Box::new(rules::KeepAfter(compile_regex(pattern)?)),
      | StepSpec::Collapse { pattern, label } => {
        Box::new(rules::Collapse { pattern: compile_regex(pattern)?, label })
      },
      | StepSpec::Dedup => Box::new(rules::Dedup),
      | StepSpec::Truncate { max_lines, keep } => {
        Box::new(rules::Truncate { max_lines, keep: keep.into() })
      },
    };
    Ok(rule)
  }
}

/// Parse `toml_text` and compile it into a named [`RuleChain`].
///
/// `name` becomes [`RuleChain::name`](crate::RuleChain) and is used in metrics
/// and debug output. Typically it is the routing key (e.g. `"cargo-build"`).
///
/// # Errors
///
/// Returns [`ConfigError::Toml`] if the text is not valid TOML or contains an
/// unrecognised step `type`. Returns [`ConfigError::Regex`] if any `pattern`
/// field is not a valid RE2 regex.
pub fn load_and_compile(
  toml_text: &str,
  name: String,
) -> Result<RuleChain, ConfigError> {
  let cfg: CommandConfig = toml::from_str(toml_text)?;
  let mut rules = Vec::with_capacity(cfg.step.len());
  for step in cfg.step {
    rules.push(step.into_rule()?);
  }
  Ok(RuleChain::new(name, rules))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compiles_a_valid_chain() {
    let toml = r#"
command = "cargo"
subcommand = "build"
[[step]]
type = "strip_ansi"
[[step]]
type = "collapse"
pattern = '^Compiling '
label = "{n} crates compiled"
[[step]]
type = "truncate"
max_lines = 50
"#;
    let chain = load_and_compile(toml, "cargo-build".to_string()).expect("compiles");
    assert_eq!(chain.describe().len(), 3);
    assert_eq!(chain.describe()[0], "strip_ansi");
  }

  #[test]
  fn rejects_invalid_regex() {
    // `command` MUST precede `[[step]]`: once a [[step]] table opens, later
    // top-level keys would be parsed into that table.
    let toml = "command = \"x\"\n[[step]]\ntype = \"drop\"\npattern = '([unclosed'\n";
    let err = load_and_compile(toml, "x".to_string()).expect_err("bad regex");
    assert!(matches!(err, ConfigError::Regex { .. }));
  }

  #[test]
  fn rejects_malformed_toml() {
    let err = load_and_compile("not = valid = toml", "x".to_string()).expect_err("bad toml");
    assert!(matches!(err, ConfigError::Toml(_)));
  }

  #[test]
  fn truncate_keep_defaults_to_tail() {
    let toml = "command = \"x\"\n[[step]]\ntype = \"truncate\"\nmax_lines = 3\n";
    let chain = load_and_compile(toml, "x".to_string()).expect("compiles");
    assert_eq!(chain.describe()[0], "truncate tail 3");
  }
}
