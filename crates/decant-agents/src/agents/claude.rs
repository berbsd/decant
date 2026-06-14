//! Claude Code agent integration.
//!
//! Hook payload shape mirrors rtk's working implementation. If Claude Code's
//! `PreToolUse` schema has changed, adjust `parse_request`/`format_response`
//! here (use the `find-docs`/`claude-api` skill to confirm the current schema)
//! — the contract lives entirely in this file.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{
  agent::{Agent, AgentError, HookRequest, InstallOutcome, Scope},
  install::install_to_file,
};

const HOOK_COMMAND: &str = "decant hook claude";
const MATCHER: &str = "Bash";

/// Claude Code (`claude`) hook integration.
pub struct ClaudeAgent;

impl Agent for ClaudeAgent {
  fn id(&self) -> &'static str {
    "claude"
  }

  fn settings_path(
    &self,
    scope: Scope,
  ) -> Result<PathBuf, AgentError> {
    match scope {
      | Scope::Project => Ok(PathBuf::from(".claude").join("settings.json")),
      | Scope::Global => {
        if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
          if !dir.is_empty() {
            return Ok(PathBuf::from(dir).join("settings.json"));
          }
        }
        let home = std::env::var("HOME").map_err(|_e| AgentError::NoHome)?;
        Ok(PathBuf::from(home).join(".claude").join("settings.json"))
      },
    }
  }

  fn install_hook(
    &self,
    scope: Scope,
  ) -> Result<InstallOutcome, AgentError> {
    let path = self.settings_path(scope)?;
    install_to_file(&path, MATCHER, HOOK_COMMAND)
  }

  fn parse_request(
    &self,
    stdin: &str,
  ) -> Result<Option<HookRequest>, AgentError> {
    let v: Value = serde_json::from_str(stdin)?;
    if v.get("tool_name").and_then(Value::as_str) != Some("Bash") {
      return Ok(None);
    }
    let command = v
      .get("tool_input")
      .and_then(|t| t.get("command"))
      .and_then(Value::as_str);
    Ok(command.map(|c| HookRequest { command: c.to_string() }))
  }

  fn format_response(
    &self,
    rewritten: &str,
  ) -> String {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": { "command": rewritten }
        }
    })
    .to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_bash_command() {
    let req = ClaudeAgent
      .parse_request(r#"{"tool_name":"Bash","tool_input":{"command":"cargo test"}}"#)
      .expect("ok")
      .expect("some");
    assert_eq!(req.command, "cargo test");
  }

  #[test]
  fn ignores_non_bash_tools() {
    let req = ClaudeAgent
      .parse_request(r#"{"tool_name":"Read","tool_input":{"file_path":"/x"}}"#)
      .expect("ok");
    assert!(req.is_none());
  }

  #[test]
  fn invalid_json_is_an_error() {
    assert!(ClaudeAgent.parse_request("not json").is_err());
  }

  #[test]
  fn format_response_sets_updated_input() {
    let out = ClaudeAgent.format_response("decant run -- cargo test");
    let v: Value = serde_json::from_str(&out).expect("valid json");
    assert_eq!(
      v["hookSpecificOutput"]["updatedInput"]["command"],
      "decant run -- cargo test"
    );
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
  }
}
