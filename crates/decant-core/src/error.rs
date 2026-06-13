//! Pipeline error type.

use thiserror::Error;

/// Failure running or capturing a child command.
///
/// Both variants preserve the underlying [`std::io::Error`] as the error
/// source so callers can inspect the OS-level cause.
#[derive(Debug, Error)]
pub enum RunError {
  /// The child process could not be spawned (e.g. binary not found, permission
  /// denied). `program` names the executable that was attempted.
  #[error("failed to spawn `{program}`: {source}")]
  Spawn {
    /// Name of the program that failed to start.
    program: String,
    #[source]
    source:  std::io::Error,
  },
  /// An I/O error occurred while draining the child's stdout or stderr pipes.
  /// The child was already running when this error occurred.
  #[error("failed reading child output: {0}")]
  Io(#[from] std::io::Error),
}
