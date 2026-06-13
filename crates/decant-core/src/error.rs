//! Pipeline error type.

use thiserror::Error;

/// Failure running or capturing a child command.
#[derive(Debug, Error)]
pub enum RunError {
  /// The child process could not be spawned.
  #[error("failed to spawn `{program}`: {source}")]
  Spawn {
    program: String,
    #[source]
    source:  std::io::Error,
  },
  /// An I/O error occurred while draining the child's output.
  #[error("failed reading child output: {0}")]
  Io(#[from] std::io::Error),
}
