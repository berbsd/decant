//! The `Runner` trait — swappable child-execution strategy.

use std::process::Command;

use crate::{error::RunError, types::Captured};

/// Runs a child command and returns its captured output.
pub trait Runner {
  /// Execute `cmd`, returning captured stdout/stderr, exit code, and timeout
  /// status.
  fn run(
    &self,
    cmd: Command,
  ) -> Result<Captured, RunError>;
}
