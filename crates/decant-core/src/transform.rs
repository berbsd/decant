//! The `Transform` trait — decant's filter contract.

use crate::types::{Captured, TransformOutput};

/// Whether a transform needs the full output buffer or can work line-by-line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformMode {
  /// Needs the complete buffer (failure-only, dedup, summarize). The default.
  Buffered,
  /// Reduces incrementally; safe for unbounded output.
  Streaming,
}

/// Transforms captured output to reduce token usage.
pub trait Transform {
  /// Stable name for metrics/debugging.
  fn name(&self) -> &str;

  /// How much context this transform needs. Defaults to
  /// [`TransformMode::Buffered`].
  fn mode(&self) -> TransformMode {
    TransformMode::Buffered
  }

  /// Produce reduced output from a complete capture.
  fn apply(
    &self,
    captured: &Captured,
  ) -> TransformOutput;
}
