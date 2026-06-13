//! Command-line interface definition.

use clap::{Parser, Subcommand};

use crate::run::RunArgs;

#[derive(Parser)]
#[command(
  name = "decant",
  version,
  about = "Reduce a command's output to save LLM tokens"
)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
  /// Run a command and emit its reduced output.
  Run(RunArgs),
}
