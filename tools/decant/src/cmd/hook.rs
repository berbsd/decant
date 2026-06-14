//! `decant hook <agent>` — the runtime hook processor an agent invokes per
//! command. Reads the agent's JSON from stdin, rewrites the command, writes the
//! agent's JSON to stdout. Robust by contract: stdout-only, valid JSON always,
//! passthrough on any error — it must never break the agent's hook protocol.

use std::{
  io::{Read, Write},
  process::ExitCode,
};

use anyhow::Result;
use clap::Args;
use decant_agents::{registry, rewrite};

/// Maximum stdin we will read from the agent (1 MiB).
const STDIN_CAP: u64 = 1024 * 1024;

/// Arguments for `decant hook`.
#[derive(Args)]
pub struct HookArgs {
  /// Agent id whose hook protocol to speak (e.g. `claude`).
  ///
  /// This subcommand is invoked by the agent itself, via the hook installed by
  /// `decant init` — you normally never run it by hand.
  #[arg(value_name = "AGENT")]
  agent: String,
}

/// Execute the `hook` subcommand. Always returns `Ok(ExitCode::SUCCESS)`; any
/// failure degrades to a passthrough response so the agent is never broken.
///
/// # Errors
/// Never returns `Err` in practice; the signature matches the other
/// subcommands.
pub fn run(args: HookArgs) -> Result<ExitCode> {
  let HookArgs { agent: agent_id } = args;
  let mut stdout = std::io::stdout().lock();

  let Some(agent) = registry::resolve(&agent_id) else {
    let _unused = write!(stdout, "{{}}");
    return Ok(ExitCode::SUCCESS);
  };

  let mut buf = String::new();
  let _unused = std::io::stdin().take(STDIN_CAP).read_to_string(&mut buf);

  let response = match agent.parse_request(&buf) {
    | Ok(Some(req)) => {
      let rewritten = rewrite::rewrite_command(&req.command);
      if rewritten == req.command {
        agent.passthrough_response()
      } else {
        agent.format_response(&rewritten)
      }
    },
    | _ => agent.passthrough_response(),
  };

  let _unused = write!(stdout, "{response}");

  Ok(ExitCode::SUCCESS)
}
