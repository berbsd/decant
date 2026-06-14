//! `SQLite` persistence of decant run metrics.
//!
//! [`record`] appends one [`RunRecord`] per `decant run`; [`summary`]
//! aggregates the DB into a [`Summary`] for `decant history`. The DB path is
//! `$DECANT_DB_PATH`, else `$XDG_DATA_HOME/decant/metrics.db`, else
//! `~/.local/share/decant/metrics.db`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::{
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
  /// Which config reduced this command.
  pub config_source: ConfigKind,
}

impl CommandStat {
  /// Bytes removed across all runs of this command.
  #[must_use]
  pub fn saved_bytes(&self) -> u64 {
    self.bytes_in.saturating_sub(self.bytes_out)
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
}

/// The history report.
#[derive(Debug, Serialize)]
pub struct Summary {
  /// Total runs in scope.
  pub total_runs:      u64,
  /// Total raw bytes in scope.
  pub total_bytes_in:  u64,
  /// Total reduced bytes in scope.
  pub total_bytes_out: u64,
  /// Commands decant reduced (`config_source != Identity`), by bytes saved.
  pub reduced:         Vec<CommandStat>,
  /// Recurring no-config commands (`config_source == Identity`), by count.
  pub opportunities:   Vec<CommandStat>,
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
  conn.execute(
    "INSERT INTO runs
         (ts, program, subcommand, raw_command, bytes_in, bytes_out,
          tokens_in, tokens_out, duration_ms, exit_code, project, config_source)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
    params![
      now_secs(),
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
                COUNT(*) AS cnt, SUM(bytes_in) AS bin, SUM(bytes_out) AS bout
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
    Ok(CommandStat {
      command:       if sub.is_empty() {
        program
      } else {
        format!("{program} {sub}")
      },
      count:         u64::try_from(cnt).unwrap_or(0),
      bytes_in:      u64::try_from(bin).unwrap_or(0),
      bytes_out:     u64::try_from(bout).unwrap_or(0),
      config_source: ConfigKind::from_db(&cs),
    })
  })?;

  let mut reduced = Vec::new();
  let mut opportunities = Vec::new();
  let (mut total_runs, mut total_in, mut total_out) = (0u64, 0u64, 0u64);
  for r in rows {
    let stat = r?;
    total_runs += stat.count;
    total_in += stat.bytes_in;
    total_out += stat.bytes_out;
    if stat.config_source == ConfigKind::Identity {
      opportunities.push(stat);
    } else {
      reduced.push(stat);
    }
  }
  reduced.sort_by_key(|b| std::cmp::Reverse(b.saved_bytes()));
  opportunities.sort_by_key(|b| std::cmp::Reverse(b.count));

  Ok(Summary {
    total_runs,
    total_bytes_in: total_in,
    total_bytes_out: total_out,
    reduced,
    opportunities,
  })
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
  ) -> RunRecord {
    RunRecord {
      program:       program.to_string(),
      subcommand:    None,
      raw_command:   program.to_string(),
      measurement:   Measurement {
        bytes_in:   bin,
        bytes_out:  bout,
        tokens_in:  0,
        tokens_out: 0,
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
    insert(&c, &rec("cargo", ConfigKind::Builtin, 100, 20)).unwrap();
    insert(&c, &rec("cargo", ConfigKind::Builtin, 100, 20)).unwrap();
    insert(&c, &rec("git", ConfigKind::Identity, 50, 50)).unwrap();

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
