//! Measurement of byte/token reduction for one decant run.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::time::Duration;

/// A before/after measurement of a single decant run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
  pub bytes_in:   usize,
  pub bytes_out:  usize,
  pub tokens_in:  usize,
  pub tokens_out: usize,
  pub duration:   Duration,
}

impl Measurement {
  /// Percentage of bytes removed: `100 * (1 - out/in)`. Zero when input is
  /// empty.
  #[must_use]
  #[allow(clippy::cast_precision_loss)] // percentage; precision loss is irrelevant
  pub fn savings_pct(&self) -> f64 {
    if self.bytes_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.bytes_out as f64 / self.bytes_in as f64))
  }
}

/// Estimate token count as whitespace-separated words (RTK's heuristic).
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
  text.split_whitespace().count()
}

/// Build a [`Measurement`] from raw input and emitted output byte slices.
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
