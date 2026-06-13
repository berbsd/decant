//! The [`Transform`] trait — decant's filter contract.

use crate::types::{Captured, TransformOutput};

/// Whether a transform needs the full output buffer or can work line-by-line.
///
/// Currently informational — [`crate::execute`] always provides the full
/// buffer. The variant is exposed so future streaming infrastructure can
/// route accordingly without a breaking API change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformMode {
  /// Needs the complete buffer (e.g. failure-only filters, dedup, summarize).
  /// This is the default.
  Buffered,
  /// Reduces output incrementally; safe for unbounded or streaming output.
  Streaming,
}

/// Reduces captured command output to minimize LLM token consumption.
///
/// Implement this trait to plug a custom reduction strategy into the
/// [`crate::execute`] pipeline. The production implementation is
/// [`decant_transforms::RuleChain`](https://docs.rs/decant-transforms).
pub trait Transform {
  /// Stable identifier used in metrics and debug output.
  fn name(&self) -> &str;

  /// Buffering requirement of this transform. Defaults to
  /// [`TransformMode::Buffered`].
  fn mode(&self) -> TransformMode {
    TransformMode::Buffered
  }

  /// Reduce `captured` and return the bytes to emit.
  ///
  /// This method is only called when the child exited naturally (no timeout).
  /// Partial / timeout captures are passed through raw by [`crate::execute`].
  fn apply(
    &self,
    captured: &Captured,
  ) -> TransformOutput;
}
