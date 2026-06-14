//! Map an agent id to its [`Agent`] implementation.

use crate::{agent::Agent, agents::ClaudeAgent};

/// Resolve an agent id (e.g. `"claude"`) to its implementation, or `None` if
/// the id is unknown.
#[must_use]
pub fn resolve(id: &str) -> Option<Box<dyn Agent>> {
  match id {
    | "claude" => Some(Box::new(ClaudeAgent)),
    | _ => None,
  }
}

/// Ids of every supported agent.
#[must_use]
pub fn known_agents() -> &'static [&'static str] {
  &["claude"]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolves_claude() {
    assert_eq!(resolve("claude").expect("some").id(), "claude");
  }

  #[test]
  fn unknown_agent_is_none() {
    assert!(resolve("nope").is_none());
    assert!(known_agents().contains(&"claude"));
  }
}
