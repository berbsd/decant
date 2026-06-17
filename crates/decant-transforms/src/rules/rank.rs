//! [`Rank`] rule — keep the highest-signal lines within a budget.
//!
//! Unlike [`crate::rules::Truncate`], which selects lines *positionally* (the
//! first or last `max_lines`), `Rank` selects them by *intrinsic importance*:
//! error lines outrank indented stack frames, which outrank plain chatter. The
//! scoring is **query-free** — it uses only properties of each line, never any
//! knowledge of what the agent asked — so it works at decant's layer, where no
//! query is available. Errors are force-kept regardless of position, so a
//! failure buried in the middle of a noisy log survives a cut that a tail
//! truncate would sever.

use std::sync::LazyLock;

use regex::Regex;

use crate::rule::{Rule, join_lines};

/// Priority of a line that names a failure — force-kept, never dropped.
const ERROR_PRIORITY: u8 = 2;
/// Priority of an indented line (typically a stack frame or nested detail).
const INDENT_PRIORITY: u8 = 1;

#[allow(clippy::unwrap_used)] // a constant regex literal is correct by construction
static ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?i)\b(error|panic|fatal|failed|failure|fail|traceback)\b").unwrap()
});

/// Query-free intrinsic importance of a single line, scored against the
/// force-keep pattern `error_re` (the built-in [`ERROR_RE`] unless overridden).
fn priority(
  line: &str,
  error_re: &Regex,
) -> u8 {
  if error_re.is_match(line) {
    ERROR_PRIORITY
  } else if line.starts_with([' ', '\t']) {
    INDENT_PRIORITY
  } else {
    0
  }
}

/// Keeps the highest-priority lines within a line budget, dropping the rest
/// and marking each omitted run with `… N more lines`.
#[derive(Debug)]
pub struct Rank {
  /// Target number of lines to keep. Force-kept error lines may push the
  /// actual output above this; an error is never dropped to honour a budget.
  pub budget: usize,
  /// Always keep this many leading lines (positional anchor).
  pub head:   usize,
  /// Always keep this many trailing lines (positional anchor).
  pub tail:   usize,
  /// Pattern marking force-keep (priority-2) lines. `None` uses the built-in
  /// error/panic/failure pattern; a config may override it for tool-specific
  /// failure markers (e.g. cargo test's `FAILED` / `panicked`).
  pub error:  Option<Regex>,
}

impl Rule for Rank {
  fn apply(
    &self,
    input: &str,
  ) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let n = lines.len();
    if n <= self.budget {
      return join_lines(&lines);
    }

    // Decide which lines to keep. Forced keeps (head/tail anchors + every
    // error) are pinned first; the rest of the budget is filled by descending
    // priority, ties broken by original position.
    let mut keep = vec![false; n];
    let head = self.head.min(n);
    let tail = self.tail.min(n);
    for k in &mut keep[..head] {
      *k = true;
    }
    for k in &mut keep[n - tail..] {
      *k = true;
    }
    let error_re = self.error.as_ref().unwrap_or(&ERROR_RE);
    let prio: Vec<u8> = lines.iter().map(|l| priority(l, error_re)).collect();
    for (k, &p) in keep.iter_mut().zip(&prio) {
      if p == ERROR_PRIORITY {
        *k = true;
      }
    }

    let pinned = keep.iter().filter(|&&k| k).count();
    if self.budget > pinned {
      let mut fill: Vec<usize> = (0..n).filter(|&i| !keep[i]).collect();
      fill.sort_by(|&a, &b| prio[b].cmp(&prio[a]).then(a.cmp(&b)));
      for &i in fill.iter().take(self.budget - pinned) {
        keep[i] = true;
      }
    }

    // Emit kept lines in original order, collapsing each dropped run into one
    // `… N more lines` marker.
    let mut out: Vec<String> = Vec::with_capacity(self.budget + 1);
    let mut gap = 0usize;
    for (i, line) in lines.iter().enumerate() {
      if keep[i] {
        if gap > 0 {
          out.push(format!("… {gap} more lines"));
          gap = 0;
        }
        out.push((*line).to_string());
      } else {
        gap += 1;
      }
    }
    if gap > 0 {
      out.push(format!("… {gap} more lines"));
    }
    join_lines(&out.iter().map(String::as_str).collect::<Vec<_>>())
  }

  fn describe(&self) -> String {
    format!("rank {}", self.budget)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rank_under_budget_is_unchanged() {
    let rule = Rank { budget: 10, head: 2, tail: 2, error: None };
    assert_eq!(rule.apply("1\n2\n3"), "1\n2\n3\n");
  }

  #[test]
  fn rank_marks_omitted_runs() {
    // 5 lines, no errors, budget 2 → keep first + last, drop the 3 in between.
    let rule = Rank { budget: 2, head: 1, tail: 1, error: None };
    assert_eq!(
      rule.apply("first\nn1\nn2\nn3\nlast\n"),
      "first\n… 3 more lines\nlast\n"
    );
  }

  #[test]
  fn rank_force_keeps_a_buried_error() {
    use std::fmt::Write as _;

    // An error in the *middle* of noise. A positional tail-truncate at this
    // budget would keep only trailing chatter and drop it; rank force-keeps it.
    let mut input = String::new();
    for i in 0..10 {
      writeln!(input, "   Compiling crate{i}").unwrap();
    }
    input.push_str("error: cannot find value `x`\n");
    for i in 0..10 {
      writeln!(input, "   Compiling more{i}").unwrap();
    }
    let out = Rank { budget: 4, head: 1, tail: 1, error: None }.apply(&input);
    assert!(
      out.contains("error: cannot find value"),
      "buried error survives"
    );
    assert!(out.contains("more lines"), "noise is elided");
  }

  #[test]
  fn rank_emits_kept_lines_in_original_order() {
    // head pins "first"; the error is force-kept; tail pins "last". Output must
    // read top-to-bottom, not in score order.
    let out = Rank { budget: 3, head: 1, tail: 1, error: None }
      .apply("first\nnoise1\nnoise2\nerror: boom\nnoise3\nlast\n");
    let first = out.find("first").expect("kept");
    let boom = out.find("boom").expect("kept");
    let last = out.find("last").expect("kept");
    assert!(
      first < boom && boom < last,
      "kept lines stay in input order"
    );
    assert!(out.contains("more lines"), "dropped noise is marked");
  }

  #[test]
  fn rank_never_drops_an_error_even_past_budget() {
    // Three errors but budget 2 and no head/tail pinning: all errors survive.
    let out = Rank { budget: 2, head: 0, tail: 0, error: None }
      .apply("error: a\nnoise\nerror: b\nnoise\nerror: c\n");
    assert!(out.contains("error: a") && out.contains("error: b") && out.contains("error: c"));
    assert!(!out.contains("noise"), "low-priority lines are dropped");
  }

  #[test]
  fn custom_pattern_overrides_the_builtin_error_signal() {
    // With an override that matches only `FAILED`, a line containing the word
    // "error" is NOT force-kept, but the buried `FAILED` line is.
    // budget 1, no anchors → only force-kept (priority-2) lines survive. Under
    // the default pattern "error: ignored" would be force-kept too; the override
    // matches only FAILED, so it is dropped.
    let rule = Rank {
      budget: 1,
      head:   0,
      tail:   0,
      error:  Some(Regex::new(r"FAILED").unwrap()),
    };
    let out = rule.apply("error: ignored\nnoise\ntest x ... FAILED\nnoise\nnoise\n");
    assert!(out.contains("FAILED"), "override pattern is force-kept");
    assert!(
      !out.contains("error: ignored"),
      "default error signal no longer applies"
    );
  }

  #[test]
  fn describe_names_the_budget() {
    assert_eq!(
      Rank { budget: 100, head: 2, tail: 2, error: None }.describe(),
      "rank 100"
    );
  }

  #[test]
  fn rank_is_lossy_so_pipe_safe_mode_skips_it() {
    assert!(!Rank { budget: 5, head: 2, tail: 2, error: None }.preserves_lines());
  }
}
