//! The [`CaptureProgress`] observer — a UI-agnostic seam for live capture
//! feedback.
//!
//! A [`Runner`](crate::Runner) buffers a child's output until it exits, so a
//! long-running command shows nothing until completion. Implementors of this
//! trait receive periodic byte/elapsed updates during the capture so a CLI can
//! render a spinner or progress line. Keeping it a trait in `decant-core` lets
//! the UI dependency (e.g. `indicatif`) live entirely in the binary crate.

use std::time::Duration;

/// Receives periodic progress updates while a [`Runner`](crate::Runner) is
/// capturing a child's output.
///
/// [`CaptureRunner`](crate::CaptureRunner) calls [`update`](Self::update) once
/// per poll tick with the total bytes captured so far and the elapsed time,
/// then calls [`finish`](Self::finish) exactly once when capture ends (whether
/// the child exited or timed out). Implementations should be cheap: `update`
/// runs on the capture loop's hot path.
pub trait CaptureProgress {
  /// Called each poll tick with cumulative bytes captured (stdout + stderr)
  /// and the elapsed time since capture started.
  fn update(
    &self,
    bytes: usize,
    elapsed: Duration,
  );

  /// Called once when capture ends, so the observer can clear any rendered
  /// line before the captured output is written.
  fn finish(&self);
}
