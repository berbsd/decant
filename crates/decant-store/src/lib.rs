//! `SQLite` persistence of decant run metrics.
//!
//! [`record`] appends one [`RunRecord`] per `decant run`; [`summary`]
//! aggregates the DB into a [`Summary`] for `decant history`. The DB path is
//! `$DECANT_DB_PATH`, else `$XDG_DATA_HOME/decant/metrics.db`, else
//! `~/.local/share/decant/metrics.db`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::{
  collections::HashMap,
  path::PathBuf,
  time::{SystemTime, UNIX_EPOCH},
};

use decant_metrics::Measurement;
use rusqlite::{Connection, params};
use serde::Serialize;

/// Errors from the metrics store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
  /// `HOME` is not set and no `DECANT_DB_PATH`/`XDG_DATA_HOME` was given.
  #[error("HOME is not set")]
  NoHome,
  /// A `SQLite` operation failed.
  #[error("database error: {0}")]
  Sqlite(#[from] rusqlite::Error),
  /// A filesystem operation failed.
  #[error("filesystem error: {0}")]
  Io(#[from] std::io::Error),
}

/// Which config produced a run's reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ConfigKind {
  /// An embedded built-in config.
  Builtin,
  /// A user config (`~/.config/decant`).
  User,
  /// A project config (`./.decant`).
  Project,
  /// No config — raw passthrough.
  Identity,
}

impl ConfigKind {
  fn as_db(self) -> &'static str {
    match self {
      | ConfigKind::Builtin => "builtin",
      | ConfigKind::User => "user",
      | ConfigKind::Project => "project",
      | ConfigKind::Identity => "identity",
    }
  }

  fn from_db(s: &str) -> Self {
    match s {
      | "builtin" => ConfigKind::Builtin,
      | "user" => ConfigKind::User,
      | "project" => ConfigKind::Project,
      | _ => ConfigKind::Identity,
    }
  }

  /// Human-facing label for this config tier, e.g. `"builtin"` or `"identity"`.
  #[must_use]
  pub fn label(self) -> &'static str {
    self.as_db()
  }
}

/// One decant run to persist.
pub struct RunRecord {
  /// Program basename (e.g. `cargo`).
  pub program:       String,
  /// First non-flag argument (e.g. `test`), if any.
  pub subcommand:    Option<String>,
  /// The full command line as run.
  pub raw_command:   String,
  /// Byte/token measurement for this run.
  pub measurement:   Measurement,
  /// The child's exit code.
  pub exit_code:     i32,
  /// Working-directory basename, if available.
  pub project:       Option<String>,
  /// Which config reduced the output.
  pub config_source: ConfigKind,
}

/// Bounds a history query.
#[derive(Default)]
pub struct HistoryFilter {
  /// Only runs within the last N days.
  pub since_days: Option<u64>,
  /// Only runs whose project name contains this substring.
  pub project:    Option<String>,
}

/// Aggregated stats for one command.
#[derive(Debug, Serialize)]
pub struct CommandStat {
  /// `program` or `program subcommand`.
  pub command:       String,
  /// Number of runs.
  pub count:         u64,
  /// Total raw bytes across runs.
  pub bytes_in:      u64,
  /// Total reduced bytes across runs.
  pub bytes_out:     u64,
  /// Total estimated tokens in the raw output across runs.
  pub tokens_in:     u64,
  /// Total estimated tokens in the reduced output across runs.
  pub tokens_out:    u64,
  /// Which config reduced this command.
  pub config_source: ConfigKind,
}

impl CommandStat {
  /// Bytes removed across all runs of this command.
  #[must_use]
  pub fn saved_bytes(&self) -> u64 {
    self.bytes_in.saturating_sub(self.bytes_out)
  }

  /// Estimated tokens removed across all runs of this command.
  #[must_use]
  pub fn saved_tokens(&self) -> u64 {
    self.tokens_in.saturating_sub(self.tokens_out)
  }

  /// Percentage of bytes removed.
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn savings_pct(&self) -> f64 {
    if self.bytes_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.bytes_out as f64 / self.bytes_in as f64))
  }

  /// Percentage of tokens removed.
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn token_savings_pct(&self) -> f64 {
    if self.tokens_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.tokens_out as f64 / self.tokens_in as f64))
  }
}

/// Aggregated stats for one config tier (builtin / user / project / identity),
/// answering "where is the reduction coming from?".
#[derive(Debug, Serialize)]
pub struct ConfigStat {
  /// Which config tier these runs resolved to.
  pub source:     ConfigKind,
  /// Number of runs in this tier.
  pub count:      u64,
  /// Total raw bytes across runs.
  pub bytes_in:   u64,
  /// Total reduced bytes across runs.
  pub bytes_out:  u64,
  /// Total estimated raw tokens across runs.
  pub tokens_in:  u64,
  /// Total estimated reduced tokens across runs.
  pub tokens_out: u64,
}

impl ConfigStat {
  /// Estimated tokens removed across this tier's runs.
  #[must_use]
  pub fn saved_tokens(&self) -> u64 {
    self.tokens_in.saturating_sub(self.tokens_out)
  }

  /// Percentage of tokens removed in this tier (`identity` is always `0.0`).
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn token_savings_pct(&self) -> f64 {
    if self.tokens_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.tokens_out as f64 / self.tokens_in as f64))
  }
}

/// Estimated token totals for one calendar day (UTC), for trend charts.
#[derive(Debug, Serialize)]
pub struct DailyBucket {
  /// Day index: Unix epoch second divided by 86 400 (UTC day number).
  pub day:        i64,
  /// Total estimated raw tokens across the day's runs.
  pub tokens_in:  u64,
  /// Total estimated reduced tokens across the day's runs.
  pub tokens_out: u64,
}

impl DailyBucket {
  /// Estimated tokens removed during this day.
  #[must_use]
  pub fn saved_tokens(&self) -> u64 {
    self.tokens_in.saturating_sub(self.tokens_out)
  }
}

/// The history report.
#[derive(Debug, Serialize)]
pub struct Summary {
  /// Total runs in scope.
  pub total_runs:       u64,
  /// Total raw bytes in scope.
  pub total_bytes_in:   u64,
  /// Total reduced bytes in scope.
  pub total_bytes_out:  u64,
  /// Total estimated raw tokens in scope.
  pub total_tokens_in:  u64,
  /// Total estimated reduced tokens in scope.
  pub total_tokens_out: u64,
  /// Commands decant reduced (`config_source != Identity`), by tokens saved.
  pub reduced:          Vec<CommandStat>,
  /// Recurring no-config commands (`config_source == Identity`), by count.
  pub opportunities:    Vec<CommandStat>,
  /// Per-tier rollup (where the reduction comes from), by tokens saved.
  pub by_config:        Vec<ConfigStat>,
}

impl Summary {
  /// Overall percentage of bytes removed.
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn savings_pct(&self) -> f64 {
    if self.total_bytes_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.total_bytes_out as f64 / self.total_bytes_in as f64))
  }

  /// Overall percentage of tokens removed — the dashboard's headline metric.
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn token_savings_pct(&self) -> f64 {
    if self.total_tokens_in == 0 {
      return 0.0;
    }
    100.0 * (1.0 - (self.total_tokens_out as f64 / self.total_tokens_in as f64))
  }
}

/// Resolve the metrics DB path.
///
/// # Errors
/// Returns [`StoreError::NoHome`] if neither `DECANT_DB_PATH`/`XDG_DATA_HOME`
/// nor `HOME` is set.
pub fn db_path() -> Result<PathBuf, StoreError> {
  if let Ok(p) = std::env::var("DECANT_DB_PATH") {
    if !p.is_empty() {
      return Ok(PathBuf::from(p));
    }
  }
  let base = match std::env::var("XDG_DATA_HOME") {
    | Ok(x) if !x.is_empty() => PathBuf::from(x),
    | _ => {
      let home = std::env::var("HOME").map_err(|_e| StoreError::NoHome)?;
      PathBuf::from(home).join(".local").join("share")
    },
  };
  Ok(base.join("decant").join("metrics.db"))
}

fn to_i64(n: usize) -> i64 {
  i64::try_from(n).unwrap_or(i64::MAX)
}

fn now_secs() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .ok()
    .and_then(|d| i64::try_from(d.as_secs()).ok())
    .unwrap_or(0)
}

fn init_schema(conn: &Connection) -> Result<(), StoreError> {
  conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY,
            ts INTEGER NOT NULL,
            program TEXT NOT NULL,
            subcommand TEXT,
            raw_command TEXT NOT NULL,
            bytes_in INTEGER, bytes_out INTEGER,
            tokens_in INTEGER, tokens_out INTEGER,
            duration_ms INTEGER, exit_code INTEGER,
            project TEXT, config_source TEXT NOT NULL
        );",
  )?;
  Ok(())
}

fn open() -> Result<Connection, StoreError> {
  let path = db_path()?;
  if let Some(parent) = path.parent() {
    if !parent.as_os_str().is_empty() {
      std::fs::create_dir_all(parent)?;
    }
  }
  let conn = Connection::open(&path)?;
  init_schema(&conn)?;
  Ok(conn)
}

fn insert(
  conn: &Connection,
  r: &RunRecord,
) -> Result<(), StoreError> {
  insert_at(conn, r, now_secs())
}

fn insert_at(
  conn: &Connection,
  r: &RunRecord,
  ts: i64,
) -> Result<(), StoreError> {
  conn.execute(
    "INSERT INTO runs
         (ts, program, subcommand, raw_command, bytes_in, bytes_out,
          tokens_in, tokens_out, duration_ms, exit_code, project, config_source)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
    params![
      ts,
      r.program,
      r.subcommand,
      r.raw_command,
      to_i64(r.measurement.bytes_in),
      to_i64(r.measurement.bytes_out),
      to_i64(r.measurement.tokens_in),
      to_i64(r.measurement.tokens_out),
      i64::try_from(r.measurement.duration.as_millis()).unwrap_or(i64::MAX),
      i64::from(r.exit_code),
      r.project,
      r.config_source.as_db(),
    ],
  )?;
  Ok(())
}

/// Whether a command's first argument is a path rather than a sub-verb.
///
/// Commands like `ls /tmp/x` or `find .` take a path as their first argument,
/// so every distinct path would otherwise become its own history row. Verb
/// subcommands like `git diff` or `cargo build` are not path-like and stay
/// distinct.
fn looks_like_path(arg: &str) -> bool {
  arg.contains('/') || arg.starts_with('.') || arg.starts_with('~')
}

/// Build the display label for a command, collapsing path arguments to `…` so
/// `ls /a`, `ls /b`, … aggregate into a single `ls …` row.
fn command_label(
  program: &str,
  sub: &str,
) -> String {
  if sub.is_empty() {
    program.to_string()
  } else if looks_like_path(sub) {
    format!("{program} …")
  } else {
    format!("{program} {sub}")
  }
}

fn query(
  conn: &Connection,
  filter: &HistoryFilter,
) -> Result<Summary, StoreError> {
  let since_cutoff: i64 = match filter.since_days {
    | Some(d) => {
      now_secs().saturating_sub(i64::try_from(d).unwrap_or(i64::MAX).saturating_mul(86_400))
    },
    | None => 0,
  };
  let project_pat = match &filter.project {
    | Some(p) => format!("%{p}%"),
    | None => "%".to_string(),
  };

  let mut stmt = conn.prepare(
    "SELECT program, COALESCE(subcommand,'') AS sub, config_source,
                COUNT(*) AS cnt, SUM(bytes_in) AS bin, SUM(bytes_out) AS bout,
                SUM(tokens_in) AS tin, SUM(tokens_out) AS tout
         FROM runs
         WHERE ts >= ?1 AND COALESCE(project,'') LIKE ?2
         GROUP BY program, sub, config_source",
  )?;
  let rows = stmt.query_map(params![since_cutoff, project_pat], |row| {
    let program: String = row.get(0)?;
    let sub: String = row.get(1)?;
    let cs: String = row.get(2)?;
    let cnt: i64 = row.get(3)?;
    let bin: i64 = row.get(4)?;
    let bout: i64 = row.get(5)?;
    let tin: i64 = row.get(6)?;
    let tout: i64 = row.get(7)?;
    Ok(CommandStat {
      command:       command_label(&program, &sub),
      count:         u64::try_from(cnt).unwrap_or(0),
      bytes_in:      u64::try_from(bin).unwrap_or(0),
      bytes_out:     u64::try_from(bout).unwrap_or(0),
      tokens_in:     u64::try_from(tin).unwrap_or(0),
      tokens_out:    u64::try_from(tout).unwrap_or(0),
      config_source: ConfigKind::from_db(&cs),
    })
  })?;

  // SQL groups by the raw first argument, so path-argument commands arrive as
  // many rows sharing one label (`ls …`); merge them here by (label, config).
  let mut merged: HashMap<(String, ConfigKind), CommandStat> = HashMap::new();
  let mut by_config_map: HashMap<ConfigKind, ConfigStat> = HashMap::new();
  let (mut total_runs, mut total_bin, mut total_bout) = (0u64, 0u64, 0u64);
  let (mut total_tin, mut total_tout) = (0u64, 0u64);
  for r in rows {
    let stat = r?;
    total_runs += stat.count;
    total_bin += stat.bytes_in;
    total_bout += stat.bytes_out;
    total_tin += stat.tokens_in;
    total_tout += stat.tokens_out;
    // Roll up by config tier independently of the per-command merge — this
    // counts every run, including configured commands that also have stale
    // no-config rows (which the reduced/opportunity split would otherwise drop).
    let tier = by_config_map
      .entry(stat.config_source)
      .or_insert_with(|| ConfigStat {
        source:     stat.config_source,
        count:      0,
        bytes_in:   0,
        bytes_out:  0,
        tokens_in:  0,
        tokens_out: 0,
      });
    tier.count += stat.count;
    tier.bytes_in += stat.bytes_in;
    tier.bytes_out += stat.bytes_out;
    tier.tokens_in += stat.tokens_in;
    tier.tokens_out += stat.tokens_out;
    let entry = merged
      .entry((stat.command.clone(), stat.config_source))
      .or_insert_with(|| CommandStat {
        command:       stat.command.clone(),
        count:         0,
        bytes_in:      0,
        bytes_out:     0,
        tokens_in:     0,
        tokens_out:    0,
        config_source: stat.config_source,
      });
    entry.count += stat.count;
    entry.bytes_in += stat.bytes_in;
    entry.bytes_out += stat.bytes_out;
    entry.tokens_in += stat.tokens_in;
    entry.tokens_out += stat.tokens_out;
  }

  // A command that has any configured run is "covered" — its config exists, so
  // it must not also surface as an opportunity via stale no-config runs.
  let covered: std::collections::HashSet<&str> = merged
    .values()
    .filter(|s| s.config_source != ConfigKind::Identity)
    .map(|s| s.command.as_str())
    .collect();
  let opportunity_labels: std::collections::HashSet<String> = merged
    .values()
    .filter(|s| s.config_source == ConfigKind::Identity && !covered.contains(s.command.as_str()))
    .map(|s| s.command.clone())
    .collect();

  let mut reduced = Vec::new();
  let mut opportunities = Vec::new();
  for stat in merged.into_values() {
    if stat.config_source == ConfigKind::Identity {
      if opportunity_labels.contains(&stat.command) {
        opportunities.push(stat);
      }
    } else {
      reduced.push(stat);
    }
  }
  // Tiebreak on the command name so merged output is deterministic despite
  // the HashMap's arbitrary iteration order.
  reduced.sort_by(|a, b| {
    b.saved_tokens()
      .cmp(&a.saved_tokens())
      .then_with(|| a.command.cmp(&b.command))
  });
  opportunities.sort_by(|a, b| {
    b.count
      .cmp(&a.count)
      .then_with(|| a.command.cmp(&b.command))
  });

  // Tiers ranked by tokens saved (descending), tiebroken by label for stable
  // output despite the HashMap's arbitrary iteration order.
  let mut by_config: Vec<ConfigStat> = by_config_map.into_values().collect();
  by_config.sort_by(|a, b| {
    b.saved_tokens()
      .cmp(&a.saved_tokens())
      .then_with(|| a.source.label().cmp(b.source.label()))
  });

  Ok(Summary {
    total_runs,
    total_bytes_in: total_bin,
    total_bytes_out: total_bout,
    total_tokens_in: total_tin,
    total_tokens_out: total_tout,
    reduced,
    opportunities,
    by_config,
  })
}

fn daily_query(
  conn: &Connection,
  filter: &HistoryFilter,
) -> Result<Vec<DailyBucket>, StoreError> {
  let since_cutoff: i64 = match filter.since_days {
    | Some(d) => {
      now_secs().saturating_sub(i64::try_from(d).unwrap_or(i64::MAX).saturating_mul(86_400))
    },
    | None => 0,
  };
  let project_pat = match &filter.project {
    | Some(p) => format!("%{p}%"),
    | None => "%".to_string(),
  };

  let mut stmt = conn.prepare(
    "SELECT ts / 86400 AS day, SUM(tokens_in) AS tin, SUM(tokens_out) AS tout
         FROM runs
         WHERE ts >= ?1 AND COALESCE(project,'') LIKE ?2
         GROUP BY day
         ORDER BY day",
  )?;
  let rows = stmt.query_map(params![since_cutoff, project_pat], |row| {
    let day: i64 = row.get(0)?;
    let tin: i64 = row.get(1)?;
    let tout: i64 = row.get(2)?;
    Ok(DailyBucket {
      day,
      tokens_in: u64::try_from(tin).unwrap_or(0),
      tokens_out: u64::try_from(tout).unwrap_or(0),
    })
  })?;

  let mut buckets = Vec::new();
  for r in rows {
    buckets.push(r?);
  }
  Ok(buckets)
}

/// Append a run to the metrics DB.
///
/// # Errors
/// Returns [`StoreError`] if the DB cannot be opened or written. Callers that
/// must not fail on a DB problem should ignore the result.
pub fn record(record: &RunRecord) -> Result<(), StoreError> {
  let conn = open()?;
  insert(&conn, record)
}

/// Aggregate the metrics DB into a [`Summary`].
///
/// # Errors
/// Returns [`StoreError`] if the DB cannot be opened or queried.
pub fn summary(filter: &HistoryFilter) -> Result<Summary, StoreError> {
  let conn = open()?;
  query(&conn, filter)
}

/// Aggregate the metrics DB into per-day token totals for trend charts.
///
/// # Errors
/// Returns [`StoreError`] if the DB cannot be opened or queried.
pub fn daily(filter: &HistoryFilter) -> Result<Vec<DailyBucket>, StoreError> {
  let conn = open()?;
  daily_query(&conn, filter)
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use super::*;

  fn conn() -> Connection {
    let c = Connection::open_in_memory().expect("in-memory db");
    init_schema(&c).expect("schema");
    c
  }

  fn rec(
    program: &str,
    cs: ConfigKind,
    bin: usize,
    bout: usize,
    tin: usize,
    tout: usize,
  ) -> RunRecord {
    RunRecord {
      program:       program.to_string(),
      subcommand:    None,
      raw_command:   program.to_string(),
      measurement:   Measurement {
        bytes_in:   bin,
        bytes_out:  bout,
        tokens_in:  tin,
        tokens_out: tout,
        duration:   Duration::ZERO,
      },
      exit_code:     0,
      project:       None,
      config_source: cs,
    }
  }

  #[test]
  fn summary_splits_reduced_and_opportunities() {
    let c = conn();
    insert(&c, &rec("cargo", ConfigKind::Builtin, 100, 20, 0, 0)).unwrap();
    insert(&c, &rec("cargo", ConfigKind::Builtin, 100, 20, 0, 0)).unwrap();
    insert(&c, &rec("git", ConfigKind::Identity, 50, 50, 0, 0)).unwrap();

    let s = query(&c, &HistoryFilter::default()).unwrap();
    assert_eq!(s.total_runs, 3);
    assert_eq!(s.reduced.len(), 1);
    assert_eq!(s.reduced[0].command, "cargo");
    assert_eq!(s.reduced[0].count, 2);
    assert_eq!(s.reduced[0].saved_bytes(), 160);
    assert_eq!(s.opportunities.len(), 1);
    assert_eq!(s.opportunities[0].command, "git");
  }

  #[test]
  fn summary_aggregates_tokens() {
    let c = conn();
    insert(&c, &rec("cargo", ConfigKind::Builtin, 100, 20, 40, 8)).unwrap();
    insert(&c, &rec("cargo", ConfigKind::Builtin, 100, 20, 60, 12)).unwrap();

    let s = query(&c, &HistoryFilter::default()).unwrap();
    assert_eq!(s.total_tokens_in, 100);
    assert_eq!(s.total_tokens_out, 20);
    assert!((s.token_savings_pct() - 80.0).abs() < 1e-9);
    assert_eq!(s.reduced[0].tokens_in, 100);
    assert_eq!(s.reduced[0].tokens_out, 20);
    assert_eq!(s.reduced[0].saved_tokens(), 80);
    assert!((s.reduced[0].token_savings_pct() - 80.0).abs() < 1e-9);
  }

  #[test]
  fn by_config_rolls_up_tiers_by_tokens_saved() {
    let c = conn();
    // builtin: 100 -> 40 tokens over 2 runs; project: 50 -> 10; identity: 30 -> 30.
    insert(&c, &rec("cargo", ConfigKind::Builtin, 0, 0, 60, 24)).unwrap();
    insert(&c, &rec("make", ConfigKind::Builtin, 0, 0, 40, 16)).unwrap();
    insert(&c, &rec("cat", ConfigKind::Project, 0, 0, 50, 10)).unwrap();
    insert(&c, &rec("grep", ConfigKind::Identity, 0, 0, 30, 30)).unwrap();

    let s = query(&c, &HistoryFilter::default()).unwrap();
    // Ordered by tokens saved desc: builtin (60) > project (40) > identity (0).
    let tiers: Vec<_> = s.by_config.iter().map(|c| c.source).collect();
    assert_eq!(tiers, vec![
      ConfigKind::Builtin,
      ConfigKind::Project,
      ConfigKind::Identity
    ]);
    let builtin = &s.by_config[0];
    assert_eq!(builtin.count, 2);
    assert_eq!(builtin.tokens_in, 100);
    assert_eq!(builtin.tokens_out, 40);
    assert_eq!(builtin.saved_tokens(), 60);
    assert!((builtin.token_savings_pct() - 60.0).abs() < 1e-9);
    // identity never reduces.
    assert_eq!(s.by_config[2].saved_tokens(), 0);
    assert!((s.by_config[2].token_savings_pct() - 0.0).abs() < 1e-9);
  }

  fn rec_sub(
    program: &str,
    sub: &str,
    cs: ConfigKind,
    tin: usize,
    tout: usize,
  ) -> RunRecord {
    RunRecord {
      subcommand: Some(sub.to_string()),
      ..rec(program, cs, tin * 4, tout * 4, tin, tout)
    }
  }

  #[test]
  fn path_argument_commands_aggregate_under_ellipsis() {
    let c = conn();
    // Different path args to `ls`/`find` should collapse into one row each.
    insert(&c, &rec_sub("ls", "/tmp/a", ConfigKind::Builtin, 50, 20)).unwrap();
    insert(
      &c,
      &rec_sub("ls", "/var/folders/x/.tmpAbc", ConfigKind::Builtin, 50, 20),
    )
    .unwrap();
    insert(&c, &rec_sub("find", ".", ConfigKind::Builtin, 5, 5)).unwrap();
    insert(
      &c,
      &rec_sub("find", "/Users/david/src", ConfigKind::Builtin, 5, 5),
    )
    .unwrap();
    // A real verb subcommand must stay distinct.
    insert(&c, &rec_sub("git", "diff", ConfigKind::Builtin, 60, 30)).unwrap();
    insert(&c, &rec_sub("cargo", "build", ConfigKind::Builtin, 80, 10)).unwrap();

    let s = query(&c, &HistoryFilter::default()).unwrap();
    let labels: Vec<&str> = s.reduced.iter().map(|r| r.command.as_str()).collect();
    assert!(
      labels.contains(&"ls …"),
      "expected collapsed ls in {labels:?}"
    );
    assert!(
      labels.contains(&"find …"),
      "expected collapsed find in {labels:?}"
    );
    assert!(
      labels.contains(&"git diff"),
      "git diff must stay distinct in {labels:?}"
    );
    assert!(
      labels.contains(&"cargo build"),
      "cargo build must stay distinct in {labels:?}"
    );

    let ls = s.reduced.iter().find(|r| r.command == "ls …").unwrap();
    assert_eq!(ls.count, 2);
    assert_eq!(ls.tokens_in, 100);
    assert_eq!(ls.tokens_out, 40);
  }

  #[test]
  fn command_with_config_is_not_an_opportunity() {
    let c = conn();
    // `terraform apply` ran twice without config (historical) and once with the
    // builtin — it is covered now, so it must not appear as an opportunity.
    insert(
      &c,
      &rec_sub("terraform", "apply", ConfigKind::Identity, 100, 100),
    )
    .unwrap();
    insert(
      &c,
      &rec_sub("terraform", "apply", ConfigKind::Identity, 100, 100),
    )
    .unwrap();
    insert(
      &c,
      &rec_sub("terraform", "apply", ConfigKind::Builtin, 100, 10),
    )
    .unwrap();
    // `git push` only ever ran without config — a genuine opportunity.
    insert(&c, &rec_sub("git", "push", ConfigKind::Identity, 50, 50)).unwrap();

    let s = query(&c, &HistoryFilter::default()).unwrap();
    let has = |v: &[CommandStat], cmd: &str| v.iter().any(|r| r.command == cmd);
    assert!(has(&s.reduced, "terraform apply"));
    assert!(
      !has(&s.opportunities, "terraform apply"),
      "covered command must not be an opportunity"
    );
    assert!(
      has(&s.opportunities, "git push"),
      "uncovered command should remain an opportunity"
    );
  }

  #[test]
  fn reduced_ranked_by_tokens_saved() {
    let c = conn();
    // `big-bytes` removes more bytes; `big-tokens` removes more tokens.
    // Ranking is by tokens saved, so `big-tokens` must come first.
    insert(&c, &rec("big-bytes", ConfigKind::Builtin, 10_000, 1, 10, 9)).unwrap();
    insert(
      &c,
      &rec("big-tokens", ConfigKind::Builtin, 100, 90, 500, 50),
    )
    .unwrap();

    let s = query(&c, &HistoryFilter::default()).unwrap();
    assert_eq!(s.reduced[0].command, "big-tokens");
    assert_eq!(s.reduced[1].command, "big-bytes");
  }

  #[test]
  fn daily_buckets_split_by_day() {
    let c = conn();
    let day = 86_400;
    // Two runs on day 0, one run on day 1.
    insert_at(&c, &rec("cargo", ConfigKind::Builtin, 0, 0, 100, 20), 10).unwrap();
    insert_at(&c, &rec("cargo", ConfigKind::Builtin, 0, 0, 100, 30), 20).unwrap();
    insert_at(
      &c,
      &rec("cargo", ConfigKind::Builtin, 0, 0, 200, 40),
      day + 5,
    )
    .unwrap();

    let buckets = daily_query(&c, &HistoryFilter::default()).unwrap();
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].day, 0);
    assert_eq!(buckets[0].tokens_in, 200);
    assert_eq!(buckets[0].tokens_out, 50);
    assert_eq!(buckets[0].saved_tokens(), 150);
    assert_eq!(buckets[1].day, 1);
    assert_eq!(buckets[1].tokens_in, 200);
    assert_eq!(buckets[1].saved_tokens(), 160);
  }

  #[test]
  fn config_kind_db_roundtrips() {
    assert_eq!(
      ConfigKind::from_db(ConfigKind::Builtin.as_db()),
      ConfigKind::Builtin
    );
    assert_eq!(
      ConfigKind::from_db(ConfigKind::Project.as_db()),
      ConfigKind::Project
    );
    assert_eq!(ConfigKind::from_db("nonsense"), ConfigKind::Identity);
  }
}
