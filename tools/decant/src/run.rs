//! `decant run` — execute a command and emit reduced output.

use std::{
  io::Write,
  process::{Command, ExitCode},
  time::{Duration, Instant},
};

use anyhow::Context;
use clap::Args;
use decant_core::{CaptureRunner, TimeoutKind, execute};
use decant_metrics::measure;
use decant_transforms::{Resolved, resolve};

#[derive(Args)]
pub struct RunArgs {
  /// Seconds with no output before assuming the command hung (0 disables).
  #[arg(long, default_value_t = 60)]
  idle_timeout: u64,

  /// Wall-clock seconds before forced termination (0 disables).
  #[arg(long = "timeout", default_value_t = 600)]
  wall_timeout: u64,

  /// Suppress the reduction-stats line on stderr.
  #[arg(long)]
  no_stats: bool,

  /// Bypass all transforms and emit the command's raw output.
  #[arg(long)]
  raw: bool,

  /// The command and its arguments (everything after the flags).
  #[arg(trailing_var_arg = true, required = true)]
  command: Vec<String>,
}

fn opt_secs(secs: u64) -> Option<Duration> {
  if secs == 0 {
    None
  } else {
    Some(Duration::from_secs(secs))
  }
}

fn exit_code_to_status(code: i32) -> ExitCode {
  let byte = u8::try_from(code).unwrap_or_else(|_| u8::from(code != 0));
  ExitCode::from(byte)
}

fn timeout_marker(
  kind: TimeoutKind,
  idle_secs: u64,
  wall_secs: u64,
) -> String {
  let (label, secs) = match kind {
    | TimeoutKind::Idle => ("idle", idle_secs),
    | TimeoutKind::WallClock => ("wall-clock", wall_secs),
  };
  format!("[decant: {label} timeout after {secs}s — child killed, output truncated]")
}

fn stats_line(m: &decant_metrics::Measurement) -> String {
  format!(
    "[decant: {} -> {} bytes ({:.1}% saved), {} -> {} tokens, {:?}]",
    m.bytes_in,
    m.bytes_out,
    m.savings_pct(),
    m.tokens_in,
    m.tokens_out,
    m.duration
  )
}

/// Execute the `run` subcommand, returning the child's exit status.
///
/// Spawns the command, captures its output via [`CaptureRunner`], applies the
/// resolved transform chain, and writes reduced bytes to stdout/stderr. A
/// stats line is printed to stderr unless `--no-stats` was given.
///
/// # Errors
///
/// Returns an error if the command argument list is empty (should not happen
/// given `required = true` on the clap field) or if writing to stdout/stderr
/// fails.  Spawn failures from the child command are also propagated as errors.
pub fn run(args: RunArgs) -> anyhow::Result<ExitCode> {
  let RunArgs { idle_timeout, wall_timeout, no_stats, raw, command } = args;

  let (program, rest) = command
    .split_first()
    .context("no command given to `decant run`")?;

  let resolved = if raw {
    Resolved::identity()
  } else {
    resolve(&command)
  };

  let mut cmd = Command::new(program);
  cmd.args(rest);

  let runner = CaptureRunner::new(opt_secs(idle_timeout), opt_secs(wall_timeout));

  let start = Instant::now();
  let (output, captured) = execute(cmd, &runner, &resolved.chain)?;
  let elapsed = start.elapsed();

  // Emit via std::io::Write (not println!) — raw bytes, no UTF-8/newline
  // mangling.
  {
    let mut out = std::io::stdout().lock();
    out.write_all(&output.stdout)?;
    out.flush()?;
  }
  let mut err = std::io::stderr().lock();
  err.write_all(&output.stderr)?;

  if let Some(kind) = captured.timeout {
    writeln!(err, "{}", timeout_marker(kind, idle_timeout, wall_timeout))?;
  }

  if !no_stats {
    let raw = [captured.stdout.as_slice(), captured.stderr.as_slice()].concat();
    let red = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
    let m = measure(&raw, &red, elapsed);
    writeln!(err, "{}", stats_line(&m))?;
  }

  err.flush()?;
  Ok(exit_code_to_status(captured.exit_code))
}

#[cfg(test)]
mod tests {
  use decant_core::TimeoutKind;

  use super::*;

  #[test]
  fn opt_secs_zero_disables_and_nonzero_enables() {
    assert_eq!(opt_secs(0), None);
    assert_eq!(opt_secs(1), Some(Duration::from_secs(1)));
  }

  #[test]
  fn timeout_marker_formats_idle_and_wall() {
    assert!(timeout_marker(TimeoutKind::Idle, 30, 600).contains("idle timeout after 30s"));
    assert!(
      timeout_marker(TimeoutKind::WallClock, 30, 600).contains("wall-clock timeout after 600s")
    );
  }

  #[test]
  fn stats_line_reports_savings() {
    let m = decant_metrics::Measurement {
      bytes_in:   100,
      bytes_out:  25,
      tokens_in:  10,
      tokens_out: 2,
      duration:   std::time::Duration::ZERO,
    };
    let line = stats_line(&m);
    assert!(line.contains("100 -> 25 bytes"));
    assert!(line.contains("75.0% saved"));
  }
}
