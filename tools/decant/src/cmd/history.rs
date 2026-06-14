//! `decant history` — report actual reduction per command from the metrics DB.

use std::{io::Write, process::ExitCode};

use anyhow::Result;
use clap::Args;
use decant_store::{HistoryFilter, Summary, summary};

/// Arguments for `decant history`.
#[derive(Args)]
#[command(
  after_long_help = "EXAMPLES:\n  decant history                   Savings across all recorded \
                     runs\n  decant history --since 7         Only runs from the last 7 days\n  \
                     decant history --project decant  Only runs from a matching project\n  decant \
                     history --json            Machine-readable summary"
)]
pub struct HistoryArgs {
  /// Only include runs from the last N days.
  #[arg(long, value_name = "DAYS")]
  since:   Option<u64>,
  /// Only include runs whose project name contains this substring.
  #[arg(long, value_name = "SUBSTR")]
  project: Option<String>,
  /// Emit the summary as JSON.
  #[arg(long)]
  json:    bool,
}

/// Execute the `history` subcommand.
///
/// # Errors
/// Returns an error if the metrics DB cannot be read or stdout cannot be
/// written.
pub fn run(args: HistoryArgs) -> Result<ExitCode> {
  let filter = HistoryFilter { since_days: args.since, project: args.project };
  let summary = summary(&filter)?;
  let mut out = std::io::stdout().lock();

  if args.json {
    writeln!(out, "{}", serde_json::to_string_pretty(&summary)?)?;
    return Ok(ExitCode::SUCCESS);
  }
  if summary.total_runs == 0 {
    writeln!(
      out,
      "decant: no runs recorded yet — run `decant init` to install the hook"
    )?;
    return Ok(ExitCode::SUCCESS);
  }
  print_report(&mut out, &summary)?;
  Ok(ExitCode::SUCCESS)
}

fn print_report(
  out: &mut impl Write,
  summary: &Summary,
) -> Result<()> {
  writeln!(
    out,
    "decant history — {} runs, {} -> {} bytes ({:.1}% saved)",
    summary.total_runs,
    summary.total_bytes_in,
    summary.total_bytes_out,
    summary.savings_pct()
  )?;
  if !summary.reduced.is_empty() {
    writeln!(out, "\nReduced:")?;
    for s in &summary.reduced {
      writeln!(
        out,
        "  {:<28} {:>5}x  {:>5.1}% saved",
        s.command,
        s.count,
        s.savings_pct()
      )?;
    }
  }
  if !summary.opportunities.is_empty() {
    writeln!(out, "\nOpportunities (no config):")?;
    for s in &summary.opportunities {
      writeln!(out, "  {:<28} {:>5}x", s.command, s.count)?;
    }
  }
  Ok(())
}
