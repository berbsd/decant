//! decant library: the CLI logic, exposed so it can be exercised by tests.
//! The binary (`src/main.rs`) is a thin wrapper around [`run_cli`].
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod cli;
pub mod run;

use std::{io::Write, process::ExitCode};

use clap::Parser;
pub use cli::{Cli, Commands};
pub use run::{RunArgs, run};

/// Parse argv, dispatch, and map errors to an exit status.
#[must_use]
pub fn run_cli() -> ExitCode {
  dispatch(Cli::parse())
}

/// Dispatch an already-parsed [`Cli`]. Lets tests bypass argv parsing and drive
/// the real logic with a constructed command line.
#[must_use]
pub fn dispatch(cli: Cli) -> ExitCode {
  let result = match cli.command {
    | Commands::Run(args) => run::run(args),
  };
  match result {
    | Ok(code) => code,
    | Err(e) => {
      let _unused = writeln!(std::io::stderr(), "decant: {e:#}");
      ExitCode::from(1)
    },
  }
}
