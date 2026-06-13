//! decant binary entry point — a thin wrapper around the library.

use std::process::ExitCode;

fn main() -> ExitCode {
  decant::run_cli()
}
