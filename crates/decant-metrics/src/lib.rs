//! Before/after measurement of byte and token reduction for a single decant
//! run.
//!
//! The two main entry points are [`measure`] (builds a [`Measurement`] from
//! raw byte slices) and [`Measurement::savings_pct`] (the headline metric).
//!
//! Token counts are estimated with [`estimate_tokens`]: whitespace-separated
//! word count, which matches RTK's counting heuristic and avoids a tokeniser
//! dependency.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//!
//! use decant_metrics::{estimate_tokens, measure};
//!
//! let m = measure(b"hello world foo bar", b"hello", Duration::from_millis(5));
//! assert_eq!(m.tokens_in, 4);
//! assert_eq!(m.tokens_out, 1);
//! assert!(m.savings_pct() > 70.0);
//! ```
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::time::Duration;

/// Before/after measurement of a single decant run.
///
/// Built by [`measure`]; inspect with [`Measurement::savings_pct`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
  /// Total bytes in the raw (pre-transform) output.
  pub bytes_in:   usize,
  /// Total bytes in the reduced (post-transform) output.
  pub bytes_out:  usize,
  /// Estimated token count of the raw output.
  pub tokens_in:  usize,
  /// Estimated token count of the reduced output.
  pub tokens_out: usize,
  /// Wall-clock time from command start to output emission.
  pub duration:   Duration,
}

impl Measurement {
  /// Percentage of bytes removed: `100 × (1 − out/in)`.
  ///
  /// Returns `0.0` when the input is empty to avoid division by zero.
  ///
  /// ```
  /// use std::time::Duration;
  ///
  /// use decant_metrics::Measurement;
  ///
  /// let m = Measurement {
  ///   bytes_in:   200,
  ///   bytes_out:  50,
  ///   tokens_in:  0,
  ///   tokens_out: 0,
  ///   duration:   Duration::ZERO,
  /// };
  /// assert!((m.savings_pct() - 75.0).abs() < 1e-9);
  /// ```
  #[must_use]
  #[allow(clippy::cast_precision_loss)] // percentage; precision loss is irrelevant
  pub fn savings_pct(&self) -> f64 {
    if self.bytes_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.bytes_out as f64 / self.bytes_in as f64))
  }
}

/// Estimate token count as the number of whitespace-separated words.
///
/// This matches RTK's counting heuristic and requires no tokeniser dependency.
/// Results are approximate — LLM tokenisers split differently — but are
/// consistent enough for comparative savings reporting.
///
/// ```
/// use decant_metrics::estimate_tokens;
///
/// assert_eq!(estimate_tokens("the quick  brown\nfox"), 4);
/// assert_eq!(estimate_tokens(""), 0);
/// ```
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
  text.split_whitespace().count()
}

/// Build a [`Measurement`] from raw input and emitted output byte slices.
///
/// `input` should be the concatenated raw stdout+stderr before transformation;
/// `output` the concatenated reduced bytes after. `duration` is the total
/// wall-clock time for the run.
///
/// ```
/// use std::time::Duration;
///
/// use decant_metrics::measure;
///
/// let m = measure(b"a b c d", b"a", Duration::ZERO);
/// assert_eq!(m.bytes_in, 7);
/// assert_eq!(m.tokens_in, 4);
/// assert_eq!(m.tokens_out, 1);
/// ```
#[must_use]
pub fn measure(
  input: &[u8],
  output: &[u8],
  duration: Duration,
) -> Measurement {
  Measurement {
    bytes_in: input.len(),
    bytes_out: output.len(),
    tokens_in: estimate_tokens(&String::from_utf8_lossy(input)),
    tokens_out: estimate_tokens(&String::from_utf8_lossy(output)),
    duration,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn savings_pct_is_fifty_for_half() {
    let m = Measurement {
      bytes_in:   100,
      bytes_out:  50,
      tokens_in:  0,
      tokens_out: 0,
      duration:   Duration::ZERO,
    };
    assert!((m.savings_pct() - 50.0).abs() < 1e-9);
  }

  #[test]
  fn savings_pct_is_zero_for_empty_input() {
    let m = Measurement {
      bytes_in:   0,
      bytes_out:  0,
      tokens_in:  0,
      tokens_out: 0,
      duration:   Duration::ZERO,
    };
    assert!(m.savings_pct().abs() < 1e-9);
  }

  #[test]
  fn estimate_tokens_counts_whitespace_words() {
    assert_eq!(estimate_tokens("the quick  brown\nfox"), 4);
  }

  #[test]
  fn measure_populates_all_fields() {
    let m = measure(b"a b c", b"a", Duration::ZERO);
    assert_eq!(m.bytes_in, 5);
    assert_eq!(m.bytes_out, 1);
    assert_eq!(m.tokens_in, 3);
    assert_eq!(m.tokens_out, 1);
  }
}
