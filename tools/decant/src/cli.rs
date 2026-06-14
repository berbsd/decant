//! Command-line interface definition — parsed by [`clap`] in
//! [`crate::run_cli`].

use clap::{Parser, Subcommand};

use crate::{
  explain::ExplainArgs, history::HistoryArgs, hook::HookArgs, init::InitArgs, run::RunArgs,
};

/// Top-level CLI struct parsed from `std::env::args`.
#[derive(Parser)]
#[command(
  name = "decant",
  version,
  about = "Reduce a command's output to save LLM tokens"
)]
pub struct Cli {
  /// The subcommand to execute.
  #[command(subcommand)]
  pub command: Commands,
}

/// Available `decant` subcommands.
#[derive(Subcommand)]
pub enum Commands {
  /// Run a command and emit its reduced output.
  Run(RunArgs),
  /// Show which transforms apply to a command (no execution).
  Explain(ExplainArgs),
  /// Install decant's hook into an agent's settings.
  Init(InitArgs),
  /// Hook processor invoked by an agent (reads stdin, rewrites, writes stdout).
  Hook(HookArgs),
  /// Show recorded run metrics: actual savings and opportunities.
  History(HistoryArgs),
}
