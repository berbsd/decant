//! Core value types passed through the pipeline.

/// Why [`crate::CaptureRunner`] terminated a child early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutKind {
  /// No output for the idle-timeout window — the child is assumed hung.
  Idle,
  /// The total wall-clock cap was exceeded.
  WallClock,
}

/// Raw result of running a child command.
#[derive(Debug, Clone)]
pub struct Captured {
  pub stdout:    Vec<u8>,
  pub stderr:    Vec<u8>,
  /// Child exit code, or `124` when we timed the child out.
  pub exit_code: i32,
  /// `Some` if we killed the child; when set, the buffers are partial.
  pub timeout:   Option<TimeoutKind>,
}

/// Bytes to emit after transformation.
#[derive(Debug, Clone)]
pub struct TransformOutput {
  pub stdout: Vec<u8>,
  pub stderr: Vec<u8>,
}
