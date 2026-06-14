//! CLI logic for `decant`, exposed as a library so integration tests can drive
//! the real implementation without going through argv.
//!
//! The binary (`src/main.rs`) is a one-liner that calls [`run_cli`].
//!
//! # Entry points
//!
//! | Function | Purpose |
//! |----------|---------|
//! | [`run_cli`] | Parse `std::env::args`, dispatch, return [`ExitCode`]. |
//! | [`dispatch`] | Same, but takes an already-parsed [`Cli`] — used by tests. |
//! | [`run::run`] | Execute the `run` subcommand (spawn child, reduce, emit). |
//! | [`explain::run`] | Execute the `explain` subcommand (show chain, no spawn). |
//!
//! [`ExitCode`]: std::process::ExitCode
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod cli;
pub mod explain;
pub mod history;
pub mod hook;
pub mod init;
pub mod run;
pub mod update;

use std::{io::Write, process::ExitCode};

use clap::Parser;
pub use cli::{Cli, Commands};
pub use run::{RunArgs, run};

/// Parse `std::env::args`, dispatch the subcommand, and return an exit code.
///
/// Any error returned by the subcommand is printed to stderr; the function
/// always returns a valid [`ExitCode`] rather than panicking.
#[must_use]
pub fn run_cli() -> ExitCode {
  dispatch(Cli::parse())
}

/// Dispatch an already-parsed [`Cli`], returning the appropriate exit code.
///
/// Separating parsing from dispatch allows tests to construct a [`Cli`]
/// directly and exercise the real logic without touching argv.
#[must_use]
pub fn dispatch(cli: Cli) -> ExitCode {
  let result = match cli.command {
    | Commands::Run(args) => run::run(args),
    | Commands::Explain(ref args) => explain::run(args),
    | Commands::Init(args) => init::run(args),
    | Commands::Hook(args) => hook::run(args),
    | Commands::History(args) => history::run(args),
    | Commands::Update(ref args) => update::run(args),
  };
  match result {
    | Ok(code) => code,
    | Err(e) => {
      let _unused = writeln!(std::io::stderr(), "decant: {e:#}");
      ExitCode::from(1)
    },
  }
}
