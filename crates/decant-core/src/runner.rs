//! The `Runner` trait — swappable child-execution strategy.

use std::process::Command;

use crate::{error::RunError, types::Captured};

/// Runs a child command and returns its captured output.
///
/// Implementors decide how to spawn the child, drain its streams, and apply
/// any timeout policy. The production implementation is
/// [`crate::CaptureRunner`].
pub trait Runner {
  /// Execute `cmd`, returning captured stdout/stderr, exit code, and timeout
  /// status.
  ///
  /// # Errors
  ///
  /// Returns [`RunError::Spawn`] if the child process could not be spawned,
  /// or [`RunError::Io`] if an I/O error occurs while reading its output.
  fn run(
    &self,
    cmd: Command,
  ) -> Result<Captured, RunError>;
}
