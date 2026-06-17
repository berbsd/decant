//! Resolve a wrapped command to a transform chain.
//!
//! The main entry point is [`resolve`], which searches for a matching config
//! in three locations (project → user → built-in) and falls back to an
//! identity passthrough if nothing is found. Config loading errors are
//! printed to stderr and also fall back to identity — output is never blocked.

use std::{
  collections::BTreeMap,
  fmt,
  io::Write,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use include_dir::{Dir, include_dir};

use crate::{
  chain::RuleChain,
  config::{ArgsRewrite, Compiled, load_and_compile},
};

/// Built-in command configs embedded from `src/builtins/` at compile time.
///
/// An installed binary ships them with no files on disk. Adding a
/// `<key>.toml` to that directory is auto-discovered — no code change needed.
static BUILTINS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/builtins");

/// Where a resolved config came from.
///
/// The [`fmt::Display`] impl produces a short human-readable label used by
/// `decant explain`.
#[derive(Debug)]
pub enum ConfigSource {
  /// A `.decant/<key>.toml` file in the current working directory.
  Project(PathBuf),
  /// A `<config_dir>/decant/<key>.toml` file in the user's config directory
  /// (`$XDG_CONFIG_HOME/decant` or `~/.config/decant`).
  User(PathBuf),
  /// An embedded built-in config. The `String` is the routing key (e.g.
  /// `"cargo-build"`).
  Builtin(String),
  /// No config was found; the chain is the raw-passthrough identity.
  Identity,
}

impl fmt::Display for ConfigSource {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    match self {
      | ConfigSource::Project(p) => write!(f, "project {}", p.display()),
      | ConfigSource::User(p) => write!(f, "user {}", p.display()),
      | ConfigSource::Builtin(k) => write!(f, "built-in {k}"),
      | ConfigSource::Identity => write!(f, "identity (no config)"),
    }
  }
}

/// The outcome of [`resolve`]: a config source, the compiled chain to apply,
/// and any arguments to append to the command before it runs.
#[derive(Debug)]
pub struct Resolved {
  /// Where the config was found (or [`ConfigSource::Identity`] if none).
  pub source:      ConfigSource,
  /// The compiled [`RuleChain`] to pass to [`decant_core::execute`].
  pub chain:       RuleChain,
  /// Tokens to append to argv before spawning (from the config's `[args]`
  /// table), already filtered by `skip_if` against the actual command. Empty
  /// when there is no rewrite or it was skipped.
  pub append_args: Vec<String>,
}

impl Resolved {
  /// Construct the raw-passthrough resolution used for unknown commands and
  /// `--raw` mode.
  #[must_use]
  pub fn identity() -> Self {
    Self {
      source:      ConfigSource::Identity,
      chain:       RuleChain::passthrough(),
      append_args: Vec::new(),
    }
  }
}

/// Every embedded built-in, keyed by filename without the `.toml` suffix.
///
/// A [`BTreeMap`] serves both access patterns from one structure: O(log n)
/// content lookup ([`builtin`]) and sorted-key iteration ([`builtin_keys`],
/// used by `decant explain`). Entries with non-UTF-8 names or contents are
/// silently skipped.
static BUILTINS_MAP: LazyLock<BTreeMap<String, &'static str>> = LazyLock::new(|| {
  BUILTINS
    .files()
    .filter_map(|f| {
      let key = f.path().file_stem()?.to_str()?.to_string();
      Some((key, f.contents_utf8()?))
    })
    .collect()
});

/// Keys of every embedded built-in config, in sorted order.
pub fn builtin_keys() -> impl Iterator<Item = &'static str> {
  BUILTINS_MAP.keys().map(String::as_str)
}

/// The embedded TOML text for `key`, if a `<key>.toml` built-in exists.
fn builtin(key: &str) -> Option<&'static str> {
  BUILTINS_MAP.get(key).copied()
}

fn program_of(first: &str) -> &str {
  Path::new(first)
    .file_name()
    .and_then(|s| s.to_str())
    .unwrap_or(first)
}

/// Whether `token` is a bare sub-verb (e.g. `status`, `build`) rather than a
/// flag, a path, or a flag's value.
///
/// Skips flags (`-x`), paths (`/p`, `./p`, `~/p`), and `key=value` tokens. This
/// matters for value-taking flags: in `git -C <path> status`, the `<path>` is
/// the value of `-C`, not the subcommand, so it must be skipped to reach
/// `status`.
fn is_subcommand_token(token: &str) -> bool {
  !token.starts_with('-')
    && !token.starts_with('.')
    && !token.starts_with('~')
    && !token.contains('/')
    && !token.contains('=')
}

/// The command's subcommand — the first bare sub-verb among `args`.
///
/// Used both to resolve config (here) and to record runs for `decant history`,
/// so the two always agree on what the subcommand is.
#[must_use]
pub fn subcommand_of(args: &[String]) -> Option<&str> {
  args
    .iter()
    .map(String::as_str)
    .find(|a| is_subcommand_token(a))
}

/// Candidate config keys, most specific first: `program-subcommand`, then
/// `program`.
fn keys(command: &[String]) -> Vec<String> {
  let Some(first) = command.first() else {
    return Vec::new();
  };
  let program = program_of(first);
  let mut out = Vec::new();
  if let Some(sub) = subcommand_of(&command[1..]) {
    out.push(format!("{program}-{sub}"));
  }
  out.push(program.to_string());
  out
}

fn user_config_dir() -> Option<PathBuf> {
  if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
    if !xdg.is_empty() {
      return Some(PathBuf::from(xdg).join("decant"));
    }
  }
  std::env::var("HOME")
    .ok()
    .map(|h| PathBuf::from(h).join(".config").join("decant"))
}

/// Resolve `command` to a [`Resolved`]. On any config error, warn to stderr and
/// fall back to identity (never block output).
#[must_use]
pub fn resolve(command: &[String]) -> Resolved {
  for key in keys(command) {
    // 1. project ./.decant/<key>.toml
    let project = PathBuf::from(".decant").join(format!("{key}.toml"));
    if let Ok(text) = std::fs::read_to_string(&project) {
      return finish(command, &text, key, ConfigSource::Project(project));
    }
    // 2. user <config>/decant/<key>.toml
    if let Some(dir) = user_config_dir() {
      let path = dir.join(format!("{key}.toml"));
      if let Ok(text) = std::fs::read_to_string(&path) {
        return finish(command, &text, key, ConfigSource::User(path));
      }
    }
    // 3. embedded built-in
    if let Some(text) = builtin(&key) {
      let source = ConfigSource::Builtin(key.clone());
      return finish(command, text, key, source);
    }
  }
  Resolved::identity()
}

/// Resolve the actual tokens to append: nothing if the rewrite is empty or any
/// `skip_if` token (default: the `append` list) already appears in `command`.
fn append_for(
  command: &[String],
  rewrite: &ArgsRewrite,
) -> Vec<String> {
  if rewrite.append.is_empty() {
    return Vec::new();
  }
  let guard = if rewrite.skip_if.is_empty() {
    &rewrite.append
  } else {
    &rewrite.skip_if
  };
  let already_present = command.iter().any(|tok| guard.iter().any(|g| g == tok));
  if already_present {
    Vec::new()
  } else {
    rewrite.append.clone()
  }
}

fn finish(
  command: &[String],
  text: &str,
  key: String,
  source: ConfigSource,
) -> Resolved {
  match load_and_compile(text, key) {
    | Ok(Compiled { chain, args }) => Resolved {
      source,
      chain,
      append_args: append_for(command, &args),
    },
    | Err(e) => {
      let _ = eprintln_warn(&source, &e).ok();
      Resolved::identity()
    },
  }
}

fn eprintln_warn(
  source: &ConfigSource,
  e: &crate::config::ConfigError,
) -> std::io::Result<()> {
  writeln!(
    std::io::stderr(),
    "decant: config {source} invalid: {e} — passing through"
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn cmd(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
  }

  #[test]
  fn derives_program_subcommand_key() {
    assert_eq!(keys(&cmd(&["/usr/bin/cargo", "test", "--release"])), vec![
      "cargo-test".to_string(),
      "cargo".to_string(),
    ]);
  }

  #[test]
  fn skips_leading_flags_for_subcommand() {
    assert_eq!(
      subcommand_of(&cmd(&["--quiet", "build"])[..]),
      Some("build")
    );
  }

  #[test]
  fn skips_flag_value_path_for_subcommand() {
    // `git -C <path> status`: the path is the value of `-C`, not the
    // subcommand. The real subcommand is the bare verb that follows.
    assert_eq!(
      subcommand_of(&cmd(&["-C", "/Users/x/repo", "status", "--short"])[..]),
      Some("status")
    );
  }

  #[test]
  fn skips_key_value_flag_argument() {
    // `git -c user.name=x commit`: a `key=value` token is a flag value.
    assert_eq!(
      subcommand_of(&cmd(&["-c", "user.name=x", "commit"])[..]),
      Some("commit")
    );
  }

  #[test]
  fn append_for_adds_when_absent() {
    let rw = ArgsRewrite {
      append:  vec!["--short".to_string()],
      skip_if: vec![
        "-s".to_string(),
        "--short".to_string(),
        "--porcelain".to_string(),
      ],
    };
    assert_eq!(append_for(&cmd(&["git", "status"]), &rw), vec![
      "--short".to_string()
    ]);
  }

  #[test]
  fn append_for_skips_when_equivalent_present() {
    let rw = ArgsRewrite {
      append:  vec!["--short".to_string()],
      skip_if: vec![
        "-s".to_string(),
        "--short".to_string(),
        "--porcelain".to_string(),
      ],
    };
    assert!(append_for(&cmd(&["git", "status", "-s"]), &rw).is_empty());
    assert!(append_for(&cmd(&["git", "status", "--porcelain"]), &rw).is_empty());
  }

  #[test]
  fn git_status_resolves_short_append() {
    // The git-status built-in appends --short via the [args] table.
    let r = resolve(&cmd(&["git", "-C", "/Users/x/repo", "status"]));
    assert_eq!(r.append_args, vec!["--short".to_string()]);
  }

  #[test]
  fn git_status_short_is_not_doubled() {
    let r = resolve(&cmd(&["git", "status", "--short"]));
    assert!(r.append_args.is_empty(), "must not re-append --short");
  }

  #[test]
  fn git_dash_c_resolves_builtin() {
    // Regression: `git -C <path> status` must resolve the git-status builtin,
    // not fall through to identity because the path was read as the subcommand.
    let r = resolve(&cmd(&["git", "-C", "/Users/x/repo", "status"]));
    assert!(
      matches!(r.source, ConfigSource::Builtin(ref k) if k == "git-status"),
      "expected git-status builtin, got {:?}",
      r.source
    );
  }

  #[test]
  fn resolves_builtin_cargo_build() {
    let r = resolve(&cmd(&["cargo", "build"]));
    assert!(matches!(r.source, ConfigSource::Builtin(ref k) if k == "cargo-build"));
    assert!(!r.chain.is_passthrough());
  }

  #[test]
  fn every_builtin_compiles() {
    // Guards against shipping a built-in with bad TOML or an invalid regex:
    // each embedded config must parse and compile to a real chain.
    let keys: Vec<&str> = builtin_keys().collect();
    assert!(!keys.is_empty(), "no built-ins were embedded");
    for key in keys {
      let text = builtin(key).expect("builtin file exists for its key");
      let compiled = load_and_compile(text, key.to_string());
      assert!(
        compiled.is_ok(),
        "built-in `{key}` failed to compile: {:?}",
        compiled.err()
      );
    }
  }

  #[test]
  fn cargo_build_pipe_grep_can_lose_matches() {
    // Simulates `decant run -- cargo build | grep foo`: the shell pipe greps
    // decant's REDUCED stdout, so anything the cargo-build chain collapses or
    // truncates becomes invisible to grep.
    let chain = load_and_compile(
      builtin("cargo-build").expect("cargo-build builtin exists"),
      "cargo-build".to_string(),
    )
    .expect("cargo-build compiles")
    .chain;

    // Case A — `foo` only appears as a crate name on a `Compiling` line, which
    // the chain collapses into a count. `grep foo` would now find NOTHING.
    let raw_a = "   Compiling foo v0.1.0 (/path/to/foo)\n   Compiling bar v0.2.0\n    Finished \
                 dev [unoptimized] in 1.20s\n";
    let reduced_a = String::from_utf8_lossy(&chain.run(raw_a.as_bytes())).into_owned();
    assert!(
      raw_a.contains("foo"),
      "raw cargo output really does mention foo"
    );
    assert!(
      !reduced_a.contains("foo"),
      "BROKEN: `Compiling foo` was collapsed away — grep foo finds nothing.\nreduced:\n{reduced_a}"
    );

    // Case B — `foo` appears in a diagnostic line the chain leaves untouched.
    // `grep foo` still works.
    let raw_b =
      "   Compiling myapp v0.1.0\nwarning: unused variable: `foo`\n    Finished dev in 0.80s\n";
    let reduced_b = String::from_utf8_lossy(&chain.run(raw_b.as_bytes())).into_owned();
    assert!(
      reduced_b.contains("foo"),
      "PRESERVED: diagnostics survive — grep foo still works.\nreduced:\n{reduced_b}"
    );
  }

  #[test]
  fn unknown_command_is_identity() {
    let r = resolve(&cmd(&["totally-unknown-xyz"]));
    assert!(matches!(r.source, ConfigSource::Identity));
    assert!(r.chain.is_passthrough());
  }
}
