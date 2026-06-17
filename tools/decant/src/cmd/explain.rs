//! `decant explain` — show the transform chain that would apply to a command.

use std::{io::Write, process::ExitCode};

use clap::Args;
use decant_transforms::{builtin_keys, resolve};

/// Arguments for `decant explain`.
#[derive(Args)]
#[command(
  after_long_help = "EXAMPLES:\n  decant explain -- git status     Show the transform chain for a \
                     command\n  decant explain                   List every command with a \
                     built-in config"
)]
pub struct ExplainArgs {
  /// The command to explain (everything after the flags). Omit to list all
  /// commands that have a built-in config.
  #[arg(value_name = "COMMAND", trailing_var_arg = true)]
  command: Vec<String>,
}

/// Execute the `explain` subcommand.
///
/// With no arguments, lists all built-in command keys. With a command, shows
/// the resolved config source and each transform step in order. Always exits
/// with [`ExitCode::SUCCESS`].
///
/// # Errors
///
/// Returns an error only if writing to stdout fails.
pub fn run(args: &ExplainArgs) -> anyhow::Result<ExitCode> {
  let mut out = std::io::stdout().lock();

  if args.command.is_empty() {
    writeln!(out, "Built-in command configs:")?;
    for key in builtin_keys() {
      writeln!(out, "  {key}")?;
    }
    return Ok(ExitCode::SUCCESS);
  }

  let resolved = resolve(&args.command);
  writeln!(out, "command: {}", args.command.join(" "))?;
  writeln!(out, "config:  {}", resolved.source)?;
  if !resolved.append_args.is_empty() {
    writeln!(out, "appends: {}", resolved.append_args.join(" "))?;
  }
  let steps = resolved.chain.describe();
  if steps.is_empty() {
    writeln!(out, "steps:   (identity — no transforms)")?;
  } else {
    writeln!(out, "steps:")?;
    for (i, step) in steps.iter().enumerate() {
      writeln!(out, "  {}. {step}", i + 1)?;
    }
  }
  out.flush()?;
  Ok(ExitCode::SUCCESS)
}
