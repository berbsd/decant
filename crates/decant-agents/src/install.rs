//! Idempotent merging of a `PreToolUse` hook entry into a Claude-style
//! `settings.json`, preserving all other content.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::agent::InstallOutcome;

/// Merge a `PreToolUse` hook entry (`matcher` -> `command`) into `settings`,
/// creating any missing structure and preserving everything else. Idempotent:
/// returns [`InstallOutcome::AlreadyPresent`] if an equivalent entry exists.
pub fn merge_pretooluse_hook(
  settings: &mut Value,
  matcher: &str,
  command: &str,
) -> InstallOutcome {
  if !settings.is_object() {
    *settings = Value::Object(Map::new());
  }
  let Some(root) = settings.as_object_mut() else {
    return InstallOutcome::Installed;
  };
  let hooks = root
    .entry("hooks")
    .or_insert_with(|| Value::Object(Map::new()));
  if !hooks.is_object() {
    *hooks = Value::Object(Map::new());
  }
  let Some(hooks) = hooks.as_object_mut() else {
    return InstallOutcome::Installed;
  };
  let pre = hooks
    .entry("PreToolUse")
    .or_insert_with(|| Value::Array(Vec::new()));
  if !pre.is_array() {
    *pre = Value::Array(Vec::new());
  }
  let Some(arr) = pre.as_array_mut() else {
    return InstallOutcome::Installed;
  };

  let present = arr.iter().any(|entry| {
    entry.get("matcher").and_then(Value::as_str) == Some(matcher)
      && entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hs| {
          hs.iter()
            .any(|h| h.get("command").and_then(Value::as_str) == Some(command))
        })
  });
  if present {
    return InstallOutcome::AlreadyPresent;
  }

  arr.push(json!({
      "matcher": matcher,
      "hooks": [ { "type": "command", "command": command } ],
  }));
  InstallOutcome::Installed
}

/// Read a settings file into a JSON value (empty object if the file is absent
/// or blank).
///
/// # Errors
/// Returns an error if the file exists but cannot be read or is invalid JSON.
pub fn read_settings(path: &Path) -> Result<Value> {
  if !path.exists() {
    return Ok(Value::Object(Map::new()));
  }
  let text =
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
  if text.trim().is_empty() {
    return Ok(Value::Object(Map::new()));
  }
  serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Merge the hook into the settings file at `path`, writing only if changed.
///
/// # Errors
/// Returns an error if the file cannot be read, parsed, created, or written.
pub fn install_to_file(
  path: &Path,
  matcher: &str,
  command: &str,
) -> Result<InstallOutcome> {
  let mut settings = read_settings(path)?;
  let outcome = merge_pretooluse_hook(&mut settings, matcher, command);
  if matches!(outcome, InstallOutcome::Installed) {
    if let Some(parent) = path.parent() {
      if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
          .with_context(|| format!("creating {}", parent.display()))?;
      }
    }
    let pretty = serde_json::to_string_pretty(&settings).context("serializing settings")?;
    std::fs::write(path, format!("{pretty}\n"))
      .with_context(|| format!("writing {}", path.display()))?;
  }
  Ok(outcome)
}

#[cfg(test)]
mod tests {
  use super::*;

  const CMD: &str = "decant hook claude";

  #[test]
  fn merges_into_empty_object() {
    let mut v = json!({});
    let outcome = merge_pretooluse_hook(&mut v, "Bash", CMD);
    assert_eq!(outcome, InstallOutcome::Installed);
    let entry = &v["hooks"]["PreToolUse"][0];
    assert_eq!(entry["matcher"], "Bash");
    assert_eq!(entry["hooks"][0]["command"], CMD);
  }

  #[test]
  fn second_merge_is_idempotent() {
    let mut v = json!({});
    merge_pretooluse_hook(&mut v, "Bash", CMD);
    let outcome = merge_pretooluse_hook(&mut v, "Bash", CMD);
    assert_eq!(outcome, InstallOutcome::AlreadyPresent);
    assert_eq!(v["hooks"]["PreToolUse"].as_array().expect("array").len(), 1);
  }

  #[test]
  fn preserves_existing_hooks_and_keys() {
    let mut v = json!({
        "model": "opus",
        "hooks": { "PreToolUse": [ { "matcher": "Edit", "hooks": [ { "type": "command", "command": "other" } ] } ] }
    });
    merge_pretooluse_hook(&mut v, "Bash", CMD);
    assert_eq!(v["model"], "opus");
    let arr = v["hooks"]["PreToolUse"].as_array().expect("array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["matcher"], "Edit"); // existing preserved
    assert_eq!(arr[1]["matcher"], "Bash"); // ours appended
  }

  #[test]
  fn install_to_file_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("settings.json");
    assert_eq!(
      install_to_file(&path, "Bash", CMD).expect("install"),
      InstallOutcome::Installed
    );
    assert_eq!(
      install_to_file(&path, "Bash", CMD).expect("install"),
      InstallOutcome::AlreadyPresent
    );
    let written = std::fs::read_to_string(&path).expect("read");
    assert!(written.contains(CMD), "{written}");
  }
}
