//! `decant init` — install decant's hook into an agent's settings.

use std::{io::Write, process::ExitCode};

use anyhow::{Result, bail};
use clap::Args;
use decant_agents::{InstallOutcome, Scope, registry};

/// Arguments for `decant init`.
#[derive(Args)]
pub struct InitArgs {
  /// Agent to install the hook for.
  #[arg(long, default_value = "claude")]
  agent:   String,
  /// Install into the project (`./.claude`) instead of the global config.
  #[arg(long)]
  project: bool,
}

/// Execute the `init` subcommand.
///
/// # Errors
/// Returns an error if the agent is unknown or the settings file cannot be
/// written.
pub fn run(args: InitArgs) -> Result<ExitCode> {
  let InitArgs { agent: agent_id, project } = args;
  let Some(agent) = registry::resolve(&agent_id) else {
    bail!(
      "unknown agent `{}` (known: {})",
      agent_id,
      registry::known_agents().join(", ")
    );
  };
  let scope = if project {
    Scope::Project
  } else {
    Scope::Global
  };
  let outcome = agent.install_hook(scope)?;
  let path = agent.settings_path(scope)?;

  let mut out = std::io::stdout().lock();
  match outcome {
    | InstallOutcome::Installed => {
      writeln!(
        out,
        "decant: installed {} hook into {}",
        agent.id(),
        path.display()
      )?;
    },
    | InstallOutcome::AlreadyPresent => {
      writeln!(
        out,
        "decant: {} hook already present in {}",
        agent.id(),
        path.display()
      )?;
    },
  }
  writeln!(
    out,
    "decant: ensure `decant` is on your PATH for the hook to take effect"
  )?;

  Ok(ExitCode::SUCCESS)
}
