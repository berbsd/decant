//! TOML config model and compilation to a [`RuleChain`].
//!
//! A config file is an optional `[args]` table (a pre-execution argument
//! rewrite) followed by a list of `[[step]]` output rules, each with a `type`
//! field that selects a [`StepSpec`] variant. Unknown top-level keys (e.g.
//! `command`, `subcommand`) are silently ignored by serde.
//!
//! # TOML format
//!
//! ```toml
//! # Optional: rewrite the command before it runs so the tool emits lean
//! # output natively (e.g. `git status` -> `git status --short`). `skip_if`
//! # leaves the command untouched when the caller already chose a format;
//! # it defaults to `append`, so a flag is never added twice.
//! [args]
//! append  = ["--short"]
//! skip_if = ["-s", "--short", "--porcelain"]
//!
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
//!
//! [[step]]
//! type = "cut"            # drop a whole section between two markers
//! begin = '^Terraform will perform'
//! end = '^Plan: '         # the end line is kept
//!
//! [[step]]
//! type = "transform"      # rewrite each line via regex substitution
//! pattern = '\s*\[id=[^\]]*\]'
//! replacement = ''        # supports $1 / ${name} backrefs
//! ```

use regex::Regex;
use serde::Deserialize;

use crate::{
  chain::RuleChain,
  rule::{Rule, Side},
  rules,
};

/// A parsed command config: an optional argument rewrite plus an ordered list
/// of output steps.
///
/// The routing key comes from the filename or argv, so the config itself
/// carries no command/subcommand fields. Unknown fields are silently ignored.
#[derive(Debug, Deserialize)]
pub struct CommandConfig {
  /// Optional pre-execution argument rewrite (the `[args]` table).
  #[serde(default)]
  pub args: ArgsRewrite,
  /// Ordered list of rule steps to apply.
  #[serde(default)]
  pub step: Vec<StepSpec>,
}

/// A pre-execution argument rewrite — the `[args]` table.
///
/// Unlike `[[step]]` rules (which transform captured *output*), this mutates
/// the command's *argv* before it runs, so a tool can be asked to produce
/// lean output natively (e.g. `git status` → `git status --short`).
#[derive(Debug, Default, Deserialize)]
pub struct ArgsRewrite {
  /// Tokens appended to argv before the command is spawned.
  #[serde(default)]
  pub append:  Vec<String>,
  /// If any of these tokens is already present in argv, the append is skipped
  /// (the caller already chose a format). Defaults to [`Self::append`] so a
  /// flag is never added twice.
  #[serde(default)]
  pub skip_if: Vec<String>,
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
  /// Drop a section from a `begin` match to an `end` match.
  /// Maps to [`crate::rules::Cut`].
  Cut {
    /// RE2 regex marking the first line of the section (dropped).
    begin: String,
    /// RE2 regex marking the end of the section (kept).
    end:   String,
  },
  /// Rewrite each line via a regex substitution.
  /// Maps to [`crate::rules::Transform`].
  Transform {
    /// RE2 regex matched against each line; every match is replaced.
    pattern:     String,
    /// Replacement text; supports `$1` / `${name}` capture references.
    replacement: String,
    /// Match over the whole buffer instead of per line. Defaults to `false`.
    #[serde(default)]
    multiline:   bool,
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
  /// Keep the highest-priority lines within a budget (errors first), dropping
  /// the rest. Maps to [`crate::rules::Rank`].
  Rank {
    /// Target number of lines to keep.
    budget:  usize,
    /// Leading lines always kept. Defaults to 2.
    #[serde(default = "default_anchor")]
    head:    usize,
    /// Trailing lines always kept. Defaults to 2.
    #[serde(default = "default_anchor")]
    tail:    usize,
    /// Optional RE2 pattern marking force-keep lines, overriding the built-in
    /// error/panic/failure signal (e.g. cargo test's `FAILED|panicked`).
    #[serde(default)]
    pattern: Option<String>,
  },
}

/// Default number of leading/trailing lines [`StepSpec::Rank`] pins.
fn default_anchor() -> usize {
  2
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
      | StepSpec::Cut { begin, end } => Box::new(rules::Cut {
        begin: compile_regex(begin)?,
        end:   compile_regex(end)?,
      }),
      | StepSpec::Transform { pattern, replacement, multiline } => Box::new(rules::Transform {
        pattern: compile_regex(pattern)?,
        replacement,
        multiline,
      }),
      | StepSpec::Collapse { pattern, label } => {
        Box::new(rules::Collapse { pattern: compile_regex(pattern)?, label })
      },
      | StepSpec::Dedup => Box::new(rules::Dedup),
      | StepSpec::Truncate { max_lines, keep } => {
        Box::new(rules::Truncate { max_lines, keep: keep.into() })
      },
      | StepSpec::Rank { budget, head, tail, pattern } => {
        let error = pattern.map(compile_regex).transpose()?;
        Box::new(rules::Rank { budget, head, tail, error })
      },
    };
    Ok(rule)
  }
}

/// A compiled config: the output [`RuleChain`] plus the parsed [`ArgsRewrite`].
#[derive(Debug)]
pub struct Compiled {
  /// The compiled output-transform chain.
  pub chain: RuleChain,
  /// The pre-execution argument rewrite (empty if no `[args]` table).
  pub args:  ArgsRewrite,
}

/// Parse `toml_text` and compile it into a [`Compiled`] config.
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
) -> Result<Compiled, ConfigError> {
  let cfg: CommandConfig = toml::from_str(toml_text)?;
  let mut rules = Vec::with_capacity(cfg.step.len());
  for step in cfg.step {
    rules.push(step.into_rule()?);
  }
  Ok(Compiled {
    chain: RuleChain::new(name, rules),
    args:  cfg.args,
  })
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
    let chain = load_and_compile(toml, "cargo-build".to_string())
      .expect("compiles")
      .chain;
    assert_eq!(chain.describe().len(), 3);
    assert_eq!(chain.describe()[0], "strip_ansi");
  }

  #[test]
  fn parses_args_rewrite_directive() {
    let toml = "[args]\nappend = [\"--short\"]\nskip_if = [\"-s\", \"--short\"]\n\n[[step]]\ntype \
                = \"strip_ansi\"\n";
    let compiled = load_and_compile(toml, "git-status".to_string()).expect("compiles");
    assert_eq!(compiled.args.append, vec!["--short".to_string()]);
    assert_eq!(compiled.args.skip_if, vec![
      "-s".to_string(),
      "--short".to_string()
    ]);
    assert_eq!(compiled.chain.describe().len(), 1);
  }

  #[test]
  fn args_rewrite_defaults_empty_when_absent() {
    let compiled =
      load_and_compile("[[step]]\ntype = \"strip_ansi\"\n", "x".to_string()).expect("compiles");
    assert!(compiled.args.append.is_empty());
    assert!(compiled.args.skip_if.is_empty());
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
  fn compiles_a_rank_step_with_defaults() {
    let toml = "command = \"x\"\n[[step]]\ntype = \"rank\"\nbudget = 80\n";
    let chain = load_and_compile(toml, "x".to_string())
      .expect("compiles")
      .chain;
    assert_eq!(chain.describe()[0], "rank 80");
  }

  #[test]
  fn rank_step_accepts_a_pattern_override() {
    let toml =
      "command = \"x\"\n[[step]]\ntype = \"rank\"\nbudget = 80\npattern = 'FAILED|panicked'\n";
    let chain = load_and_compile(toml, "x".to_string())
      .expect("compiles")
      .chain;
    assert_eq!(chain.describe()[0], "rank 80");
  }

  #[test]
  fn rank_step_rejects_an_invalid_pattern() {
    let toml = "command = \"x\"\n[[step]]\ntype = \"rank\"\nbudget = 80\npattern = '([unclosed'\n";
    let err = load_and_compile(toml, "x".to_string()).expect_err("bad regex");
    assert!(matches!(err, ConfigError::Regex { .. }));
  }

  #[test]
  fn truncate_keep_defaults_to_tail() {
    let toml = "command = \"x\"\n[[step]]\ntype = \"truncate\"\nmax_lines = 3\n";
    let chain = load_and_compile(toml, "x".to_string())
      .expect("compiles")
      .chain;
    assert_eq!(chain.describe()[0], "truncate tail 3");
  }
}
