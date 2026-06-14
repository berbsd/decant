//! The `Agent` trait and its supporting types — the per-agent hook seam.

use std::path::PathBuf;

/// Errors from agent hook operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
  /// The home/config-dir environment variable is not set.
  #[error("HOME is not set")]
  NoHome,
  /// Reading or writing a settings file failed.
  #[error("settings file I/O error: {0}")]
  Io(#[from] std::io::Error),
  /// A settings file or hook payload was not valid JSON.
  #[error("invalid JSON: {0}")]
  Json(#[from] serde_json::Error),
}

/// Where a hook is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
  /// The user's global agent config (e.g. `~/.claude/settings.json`).
  Global,
  /// The current project (e.g. `./.claude/settings.json`).
  Project,
}

/// Result of an install attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
  /// A new hook entry was written.
  Installed,
  /// An equivalent entry already existed; nothing changed.
  AlreadyPresent,
}

/// A command extracted from an agent's hook payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRequest {
  /// The Bash command string the agent is about to run.
  pub command: String,
}

/// Per-agent hook integration. One implementation per agent.
pub trait Agent {
  /// Stable id, e.g. `"claude"`.
  fn id(&self) -> &'static str;

  /// Path to the agent's settings file for `scope`.
  ///
  /// # Errors
  /// Returns [`AgentError::NoHome`] if a required environment variable (e.g.
  /// `HOME`) is unset.
  fn settings_path(
    &self,
    scope: Scope,
  ) -> Result<PathBuf, AgentError>;

  /// Merge decant's hook entry into the settings file. Idempotent.
  ///
  /// # Errors
  /// Returns [`AgentError::Io`] or [`AgentError::Json`] if the settings file
  /// cannot be read, parsed, or written.
  fn install_hook(
    &self,
    scope: Scope,
  ) -> Result<InstallOutcome, AgentError>;

  /// Parse the agent's hook stdin JSON. `Ok(None)` means "not a command tool
  /// call decant should touch" — the caller emits a passthrough.
  ///
  /// # Errors
  /// Returns [`AgentError::Json`] if `stdin` is not valid JSON.
  fn parse_request(
    &self,
    stdin: &str,
  ) -> Result<Option<HookRequest>, AgentError>;

  /// The agent's hook JSON that rewrites the command to `rewritten`.
  fn format_response(
    &self,
    rewritten: &str,
  ) -> String;

  /// The agent's no-op hook JSON (leave the command unchanged). Defaults to an
  /// empty JSON object, which every current agent treats as "no modification".
  fn passthrough_response(&self) -> String {
    "{}".to_string()
  }
}
