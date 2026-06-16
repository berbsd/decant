//! `decant run` — execute a command and emit reduced output.

use std::{
  cell::Cell,
  io::{IsTerminal, Write},
  process::{Command, ExitCode},
  time::{Duration, Instant},
};

use anyhow::Context;
use clap::Args;
use decant_core::{CaptureProgress, CaptureRunner, TimeoutKind, execute};
use decant_metrics::measure;
use decant_store::{ConfigKind, RunRecord};
use decant_transforms::{ConfigSource, PipeSafe, Resolved, resolve};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

#[derive(Args)]
#[command(
  after_long_help = "EXAMPLES:\n  decant run -- cargo build            Reduce build output\n  \
                     decant run --raw -- git diff         Bypass reduction, emit raw output\n  \
                     decant run --reduce -- ls | less     Force reduction even when piped\n  \
                     decant run --timeout 120 -- npm ci   Kill the child after 120s wall-clock"
)]
pub struct RunArgs {
  /// Seconds with no output before assuming the command hung (0 disables).
  #[arg(
    long,
    value_name = "SECS",
    default_value_t = 60,
    help_heading = "Timeouts"
  )]
  idle_timeout: u64,

  /// Wall-clock seconds before forced termination (0 disables).
  #[arg(
    long = "timeout",
    value_name = "SECS",
    default_value_t = 600,
    help_heading = "Timeouts"
  )]
  wall_timeout: u64,

  /// Suppress the reduction-stats line on stderr.
  #[arg(long, help_heading = "Output")]
  no_stats: bool,

  /// Bypass all transforms and emit the command's raw output.
  #[arg(long, help_heading = "Output")]
  raw: bool,

  /// Force reduction even when stdout is piped or redirected (e.g. into a
  /// pager). By default decant only reduces for an interactive terminal.
  #[arg(long, conflicts_with = "raw", help_heading = "Output")]
  reduce: bool,

  /// The command and its arguments (everything after the flags).
  #[arg(value_name = "COMMAND", trailing_var_arg = true, required = true)]
  command: Vec<String>,
}

/// How to treat the child's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
  /// Emit the command's output untouched.
  Raw,
  /// Apply only line-preserving rules — safe to pipe into another program.
  PipeSafe,
  /// Apply the full transform chain.
  Full,
}

/// Choose the output mode from the flags and whether stdout is a terminal.
///
/// On an interactive terminal decant applies the full chain. When stdout is a
/// pipe or file the output feeds another program (e.g. `... | grep foo`), so it
/// runs only the pipe-safe rules — lossy steps that could hide a downstream
/// match are skipped. `--raw` forces untouched output; `--reduce` forces the
/// full chain even when piped (e.g. into a pager).
fn output_mode(
  raw: bool,
  reduce: bool,
  stdout_is_terminal: bool,
) -> OutputMode {
  if raw {
    OutputMode::Raw
  } else if reduce || stdout_is_terminal {
    OutputMode::Full
  } else {
    OutputMode::PipeSafe
  }
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

/// Format a run duration for the stats line: whole seconds (rounded) once it
/// reaches a second, otherwise whole milliseconds — so `102.040413833s`
/// becomes `102s` while fast runs stay readable as `108ms`.
fn fmt_duration(d: Duration) -> String {
  if d.as_secs() >= 1 {
    format!("{}s", (d.as_millis() + 500) / 1000)
  } else {
    format!("{}ms", d.as_millis())
  }
}

fn stats_line(m: &decant_metrics::Measurement) -> String {
  format!(
    "[decant: {} -> {} bytes ({:.1}% saved), {} -> {} tokens, {}]",
    m.bytes_in,
    m.bytes_out,
    m.savings_pct(),
    m.tokens_in,
    m.tokens_out,
    fmt_duration(m.duration)
  )
}

fn config_kind(source: &ConfigSource) -> ConfigKind {
  match source {
    | ConfigSource::Builtin(_) => ConfigKind::Builtin,
    | ConfigSource::User(_) => ConfigKind::User,
    | ConfigSource::Project(_) => ConfigKind::Project,
    | ConfigSource::Identity => ConfigKind::Identity,
  }
}

/// Delay before the capture spinner is revealed — short commands finish before
/// this and never show one.
const SPINNER_REVEAL_AFTER: Duration = Duration::from_secs(3);

/// A [`CaptureProgress`] observer that renders a live `indicatif` spinner on
/// stderr while a long-running command is being captured.
///
/// The bar starts hidden and is only revealed once capture exceeds
/// [`SPINNER_REVEAL_AFTER`], so fast commands produce no output. `indicatif`
/// itself draws nothing when stderr is not a terminal.
struct Spinner {
  pb:       ProgressBar,
  revealed: Cell<bool>,
}

impl Spinner {
  fn new() -> Self {
    let pb = ProgressBar::with_draw_target(None, ProgressDrawTarget::hidden());
    pb.set_style(
      ProgressStyle::with_template(
        "{spinner:.cyan} decant: capturing — [{elapsed_precise}] {bytes}",
      )
      .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    Self { pb, revealed: Cell::new(false) }
  }
}

impl CaptureProgress for Spinner {
  fn update(
    &self,
    bytes: usize,
    elapsed: Duration,
  ) {
    if !self.revealed.get() && elapsed >= SPINNER_REVEAL_AFTER {
      self
        .pb
        .set_draw_target(ProgressDrawTarget::stderr_with_hz(5));
      self.revealed.set(true);
    }
    self
      .pb
      .set_position(u64::try_from(bytes).unwrap_or(u64::MAX));
  }

  fn finish(&self) {
    self.pb.finish_and_clear();
  }
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
  let RunArgs {
    idle_timeout,
    wall_timeout,
    no_stats,
    raw,
    reduce,
    command,
  } = args;

  let (program, rest) = command
    .split_first()
    .context("no command given to `decant run`")?;

  let mode = output_mode(raw, reduce, std::io::stdout().is_terminal());
  let resolved = if mode == OutputMode::Raw {
    Resolved::identity()
  } else {
    resolve(&command)
  };

  let mut cmd = Command::new(program);
  cmd.args(rest);

  let mut runner = CaptureRunner::new(opt_secs(idle_timeout), opt_secs(wall_timeout));
  // Show a live capture spinner only when stderr is an interactive terminal, so
  // piped/redirected stderr stays clean (mirrors the output-mode TTY gating).
  if std::io::stderr().is_terminal() {
    runner = runner.with_progress(Box::new(Spinner::new()));
  }

  let start = Instant::now();
  let (output, captured) = match mode {
    | OutputMode::PipeSafe => execute(cmd, &runner, &PipeSafe(&resolved.chain))?,
    | OutputMode::Raw | OutputMode::Full => execute(cmd, &runner, &resolved.chain)?,
  };
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

  let raw_bytes = [captured.stdout.as_slice(), captured.stderr.as_slice()].concat();
  let reduced_bytes = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
  let m = measure(&raw_bytes, &reduced_bytes, elapsed);

  // Raw mode runs no transform, so "0.0% saved" would be noise. PipeSafe and
  // Full both reduce, so their stats are meaningful.
  if !no_stats && mode != OutputMode::Raw {
    writeln!(err, "{}", stats_line(&m))?;
  }
  err.flush()?;

  // Best-effort: persist this run for `decant history`. A DB failure must never
  // affect the command's output or exit code.
  let program_base = std::path::Path::new(program.as_str())
    .file_name()
    .and_then(|s| s.to_str())
    .unwrap_or(program.as_str())
    .to_string();
  let _unused = decant_store::record(&RunRecord {
    program:       program_base,
    subcommand:    rest.iter().find(|a| !a.starts_with('-')).cloned(),
    raw_command:   command.join(" "),
    measurement:   m,
    exit_code:     captured.exit_code,
    project:       std::env::current_dir()
      .ok()
      .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned())),
    config_source: config_kind(&resolved.source),
  });

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
  fn output_mode_follows_terminal_by_default() {
    // Terminal → full reduction; piped → pipe-safe (lossless) reduction only.
    assert_eq!(output_mode(false, false, true), OutputMode::Full);
    assert_eq!(output_mode(false, false, false), OutputMode::PipeSafe);
  }

  #[test]
  fn output_mode_flags_override_terminal_state() {
    // --raw forces untouched output, even on a terminal.
    assert_eq!(output_mode(true, false, true), OutputMode::Raw);
    assert_eq!(output_mode(true, false, false), OutputMode::Raw);
    // --reduce forces the full chain even when piped (the `| less` case).
    assert_eq!(output_mode(false, true, false), OutputMode::Full);
  }

  #[test]
  fn timeout_marker_formats_idle_and_wall() {
    assert!(timeout_marker(TimeoutKind::Idle, 30, 600).contains("idle timeout after 30s"));
    assert!(
      timeout_marker(TimeoutKind::WallClock, 30, 600).contains("wall-clock timeout after 600s")
    );
  }

  #[test]
  fn fmt_duration_rounds_seconds_and_keeps_ms_for_fast_runs() {
    assert_eq!(fmt_duration(Duration::from_millis(108)), "108ms");
    assert_eq!(fmt_duration(Duration::from_millis(1_400)), "1s");
    assert_eq!(fmt_duration(Duration::from_millis(1_600)), "2s");
    assert_eq!(fmt_duration(Duration::from_nanos(102_040_413_833)), "102s");
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
