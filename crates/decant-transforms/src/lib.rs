//! Built-in transforms. v1 provides only [`Identity`].
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use decant_core::{Captured, Transform, TransformOutput};

/// Passes output through unchanged. The trivial `Buffered` transform.
pub struct Identity;

impl Transform for Identity {
  fn name(&self) -> &'static str {
    "identity"
  }

  fn apply(
    &self,
    captured: &Captured,
  ) -> TransformOutput {
    TransformOutput {
      stdout: captured.stdout.clone(),
      stderr: captured.stderr.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn identity_passes_stdout_and_stderr_through() {
    let cap = Captured {
      stdout:    b"abc".to_vec(),
      stderr:    b"err".to_vec(),
      exit_code: 0,
      timeout:   None,
    };
    let out = Identity.apply(&cap);
    assert_eq!(out.stdout, b"abc");
    assert_eq!(out.stderr, b"err");
  }

  #[test]
  fn identity_reports_its_name() {
    assert_eq!(Identity.name(), "identity");
  }
}
