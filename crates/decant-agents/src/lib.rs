//! Agent hook integration for decant.
//!
//! [`rewrite::rewrite_command`] is the agent-agnostic command rewriter. The
//! [`Agent`] trait (one impl per agent under `agents/`) installs hooks and
//! speaks each agent's hook JSON protocol; [`registry::resolve`] maps an agent
//! id to its impl.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod agents {
  mod claude;

  pub use claude::ClaudeAgent;
}

pub mod agent;
pub mod install;
pub mod registry;
pub mod rewrite;

pub use agent::{Agent, AgentError, HookRequest, InstallOutcome, Scope};
pub use agents::ClaudeAgent;
