//! `decant dashboard` — an interactive terminal view of recorded savings.
//!
//! Renders a snapshot of the metrics DB ([`decant_store::summary`] plus
//! [`decant_store::daily`]) via the pure [`decant_dashboard`] crate, then runs
//! an input loop: `q`/`Esc` quits, `r` refetches, `↑`/`↓` scroll the reduced
//! table. With `--watch`, it also refetches every `--interval` seconds and
//! redraws only when the figures actually change.
//!
//! The terminal lifecycle (raw mode, alternate screen, panic-safe restore) is
//! owned here via [`ratatui::init`] / [`ratatui::restore`]; all rendering lives
//! in [`decant_dashboard`].

use std::{
  io::{IsTerminal, Write},
  process::ExitCode,
  time::{Duration, Instant},
};

use anyhow::Result;
use clap::Args;
use decant_dashboard::{DashboardData, render};
use decant_store::{DailyBucket, HistoryFilter, Summary, daily, summary};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

/// Arguments for `decant dashboard`.
#[derive(Args)]
#[command(
  after_long_help = "EXAMPLES:\n  decant dashboard                 Live-scrollable savings \
                     view\n  decant dashboard --since 7       Only the last 7 days\n  decant \
                     dashboard --watch          Auto-refresh as new runs land\n\nThis view needs \
                     a terminal; for scripting use `decant history --json`."
)]
pub struct DashboardArgs {
  /// Only include runs from the last N days.
  #[arg(long, value_name = "DAYS")]
  since:    Option<u64>,
  /// Only include runs whose project name contains this substring.
  #[arg(long, value_name = "SUBSTR")]
  project:  Option<String>,
  /// Refetch and redraw periodically as new runs are recorded.
  #[arg(long)]
  watch:    bool,
  /// Seconds between refetches in `--watch` mode.
  #[arg(long, value_name = "SECS", default_value_t = 1)]
  interval: u64,
}

/// A fetched snapshot of the metrics DB.
struct Snapshot {
  summary: Summary,
  daily:   Vec<DailyBucket>,
}

fn fetch(filter: &HistoryFilter) -> Result<Snapshot> {
  Ok(Snapshot { summary: summary(filter)?, daily: daily(filter)? })
}

/// Whether a refetched summary differs enough to warrant a redraw.
///
/// Compares the headline figures only; the per-command tables are derived from
/// these totals, so a change here implies the tables changed too.
fn summary_changed(
  a: &Summary,
  b: &Summary,
) -> bool {
  a.total_runs != b.total_runs
    || a.total_tokens_in != b.total_tokens_in
    || a.total_tokens_out != b.total_tokens_out
    || a.total_bytes_in != b.total_bytes_in
    || a.total_bytes_out != b.total_bytes_out
}

/// The message shown when stdout is not a terminal.
const NO_TTY_MSG: &str =
  "decant: dashboard requires a terminal — for scripting use `decant history --json`";

/// Execute the `dashboard` subcommand.
///
/// # Errors
/// Returns an error if the metrics DB cannot be read or the terminal cannot be
/// driven.
pub fn run(args: DashboardArgs) -> Result<ExitCode> {
  if !std::io::stdout().is_terminal() {
    writeln!(std::io::stderr().lock(), "{NO_TTY_MSG}")?;
    return Ok(ExitCode::from(1));
  }

  let filter = HistoryFilter { since_days: args.since, project: args.project };
  let mut snap = fetch(&filter)?;

  // Nothing recorded and not watching: print the plain hint rather than
  // dropping into an empty full-screen view.
  if snap.summary.total_runs == 0 && !args.watch {
    writeln!(
      std::io::stdout().lock(),
      "decant: no runs recorded yet — run `decant init` to install the hook"
    )?;
    return Ok(ExitCode::SUCCESS);
  }

  let interval = Duration::from_secs(args.interval.max(1));
  let mut terminal = ratatui::init();
  let result = event_loop(
    &mut terminal,
    &filter,
    &mut snap,
    args.watch,
    interval,
    args.since,
  );
  ratatui::restore();
  result?;
  Ok(ExitCode::SUCCESS)
}

fn event_loop(
  terminal: &mut ratatui::DefaultTerminal,
  filter: &HistoryFilter,
  snap: &mut Snapshot,
  watch: bool,
  interval: Duration,
  since_days: Option<u64>,
) -> Result<()> {
  let mut scroll = 0usize;
  let mut dirty = true;
  // In snapshot mode there is no auto-refresh, so block on input for a long
  // time; in watch mode wake every `interval` to refetch.
  let tick = if watch {
    interval
  } else {
    Duration::from_secs(3600)
  };
  let mut last = Instant::now();

  loop {
    if dirty {
      let data = DashboardData {
        summary: &snap.summary,
        daily: &snap.daily,
        since_days,
        scroll,
      };
      terminal.draw(|f| render(f, &data))?;
      dirty = false;
    }

    if event::poll(tick)? {
      if let Event::Key(key) = event::read()? {
        if key.kind != KeyEventKind::Press {
          continue;
        }
        let ctrl_c =
          key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
          | KeyCode::Char('q') | KeyCode::Esc => break,
          | _ if ctrl_c => break,
          | KeyCode::Char('r') => {
            *snap = fetch(filter)?;
            dirty = true;
          },
          | KeyCode::Up => {
            scroll = scroll.saturating_sub(1);
            dirty = true;
          },
          | KeyCode::Down => {
            let max = snap.summary.reduced.len().saturating_sub(1);
            if scroll < max {
              scroll += 1;
              dirty = true;
            }
          },
          | _ => {},
        }
      }
    } else if watch && last.elapsed() >= interval {
      last = Instant::now();
      let fresh = fetch(filter)?;
      if summary_changed(&snap.summary, &fresh.summary) {
        *snap = fresh;
        scroll = scroll.min(snap.summary.reduced.len().saturating_sub(1));
        dirty = true;
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn summary_with(
    runs: u64,
    tin: u64,
    tout: u64,
  ) -> Summary {
    Summary {
      total_runs:       runs,
      total_bytes_in:   tin * 4,
      total_bytes_out:  tout * 4,
      total_tokens_in:  tin,
      total_tokens_out: tout,
      reduced:          Vec::new(),
      opportunities:    Vec::new(),
      by_config:        Vec::new(),
    }
  }

  #[test]
  fn summary_changed_detects_new_runs() {
    let a = summary_with(10, 1000, 200);
    let b = summary_with(11, 1100, 220);
    assert!(summary_changed(&a, &b));
  }

  #[test]
  fn summary_changed_false_for_identical_totals() {
    let a = summary_with(10, 1000, 200);
    let b = summary_with(10, 1000, 200);
    assert!(!summary_changed(&a, &b));
  }
}
