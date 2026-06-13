//! The single pipeline entry point.

use std::process::Command;

use crate::{
  error::RunError,
  runner::Runner,
  transform::Transform,
  types::{Captured, TransformOutput},
};

/// Run `cmd` via `runner`, then apply `transform` to the captured output.
///
/// The transform runs **only** when the child exited naturally (no timeout).
/// On timeout the raw, possibly-partial buffer is returned untouched — a
/// [`TransformMode::Buffered`](crate::TransformMode::Buffered) transform would
/// misread an incomplete buffer.
///
/// # Errors
///
/// Returns [`RunError::Spawn`] if the child process could not be started, or
/// [`RunError::Io`] if an I/O error occurs while draining its output.
pub fn execute(
  cmd: Command,
  runner: &dyn Runner,
  transform: &dyn Transform,
) -> Result<(TransformOutput, Captured), RunError> {
  let captured = runner.run(cmd)?;

  let output = if captured.timeout.is_some() {
    raw(&captured)
  } else {
    transform.apply(&captured)
  };

  Ok((output, captured))
}

fn raw(captured: &Captured) -> TransformOutput {
  TransformOutput {
    stdout: captured.stdout.clone(),
    stderr: captured.stderr.clone(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{transform::TransformMode, types::TimeoutKind};

  struct FakeRunner(Captured);
  impl Runner for FakeRunner {
    fn run(
      &self,
      _cmd: Command,
    ) -> Result<Captured, RunError> {
      Ok(self.0.clone())
    }
  }

  struct UpperTransform;
  impl Transform for UpperTransform {
    fn name(&self) -> &'static str {
      "upper"
    }

    fn apply(
      &self,
      c: &Captured,
    ) -> TransformOutput {
      TransformOutput {
        stdout: c.stdout.to_ascii_uppercase(),
        stderr: c.stderr.clone(),
      }
    }
  }

  fn cap(timeout: Option<TimeoutKind>) -> Captured {
    Captured {
      stdout: b"hi".to_vec(),
      stderr: Vec::new(),
      exit_code: 0,
      timeout,
    }
  }

  #[test]
  fn applies_transform_on_clean_exit() {
    let (out, _) = execute(
      Command::new("true"),
      &FakeRunner(cap(None)),
      &UpperTransform,
    )
    .unwrap();
    assert_eq!(out.stdout, b"HI");
  }

  #[test]
  fn skips_transform_on_timeout() {
    let (out, _) = execute(
      Command::new("true"),
      &FakeRunner(cap(Some(TimeoutKind::Idle))),
      &UpperTransform,
    )
    .unwrap();
    assert_eq!(out.stdout, b"hi"); // raw, not uppercased
  }

  #[test]
  fn default_transform_mode_is_buffered() {
    assert_eq!(UpperTransform.mode(), TransformMode::Buffered);
  }
}
