//! Terminal rendering for the `decant dashboard` savings view.
//!
//! This crate is pure presentation: [`render`] paints a [`DashboardData`]
//! snapshot into a ratatui [`Frame`]. It performs no I/O, owns no terminal
//! state, and runs no event loop — the `decant dashboard` driver supplies the
//! data and owns the terminal lifecycle. Keeping rendering side-effect-free
//! lets it be unit-tested against ratatui's in-memory `TestBackend`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use decant_store::{DailyBucket, Summary};
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Layout},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Paragraph, Row, Table},
};

/// Catppuccin Frappé palette (the subset this view paints with).
mod palette {
  use ratatui::style::Color;

  pub(crate) const TEXT: Color = Color::Rgb(0xc6, 0xd0, 0xf5);
  pub(crate) const SUBTEXT: Color = Color::Rgb(0xa5, 0xad, 0xce);
  pub(crate) const GREEN: Color = Color::Rgb(0xa6, 0xd1, 0x89);
  pub(crate) const MAUVE: Color = Color::Rgb(0xca, 0x9e, 0xe6);
  pub(crate) const BLUE: Color = Color::Rgb(0x8c, 0xaa, 0xee);
  pub(crate) const YELLOW: Color = Color::Rgb(0xe5, 0xc8, 0x90);
  pub(crate) const PEACH: Color = Color::Rgb(0xef, 0x9f, 0x76);
}

/// One immutable snapshot to paint: aggregate totals plus the daily trend.
pub struct DashboardData<'a> {
  /// Aggregated per-command totals and headline figures.
  pub summary:    &'a Summary,
  /// Per-day token totals, oldest first, for the trend sparkline.
  pub daily:      &'a [DailyBucket],
  /// The `--since` window in days, or `None` for all time (for the title).
  pub since_days: Option<u64>,
  /// Row offset into the reduced-command table (for scrolling).
  pub scroll:     usize,
}

const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Render per-day saved-token totals as a Unicode block-char sparkline.
///
/// Each day is scaled to the maximum saved-token day in the window. An empty
/// window yields an empty string; a window where nothing was saved yields all
/// lowest-level bars.
fn sparkline(daily: &[DailyBucket]) -> String {
  if daily.is_empty() {
    return String::new();
  }
  let vals: Vec<u64> = daily.iter().map(DailyBucket::saved_tokens).collect();
  let max = vals.iter().copied().max().unwrap_or(0);
  if max == 0 {
    return SPARK[0].to_string().repeat(vals.len());
  }
  #[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
  )]
  vals
    .iter()
    .map(|&v| {
      let idx = ((v as f64 / max as f64) * (SPARK.len() - 1) as f64).round() as usize;
      SPARK[idx.min(SPARK.len() - 1)]
    })
    .collect()
}

/// Format a count compactly: `1.2M`, `3.4K`, or the bare number.
#[allow(clippy::cast_precision_loss)]
fn human(n: u64) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1e6)
  } else if n >= 1_000 {
    format!("{:.1}K", n as f64 / 1e3)
  } else {
    n.to_string()
  }
}

/// Format a byte count with a binary unit suffix.
#[allow(clippy::cast_precision_loss)]
fn human_bytes(n: u64) -> String {
  const UNIT: [&str; 4] = ["B", "KB", "MB", "GB"];
  let mut v = n as f64;
  let mut i = 0;
  while v >= 1024.0 && i < UNIT.len() - 1 {
    v /= 1024.0;
    i += 1;
  }
  if i == 0 {
    format!("{n} B")
  } else {
    format!("{v:.1} {}", UNIT[i])
  }
}

/// Paint the dashboard snapshot into `frame`.
///
/// With no recorded runs, paints a single empty-state message. Otherwise lays
/// out, top to bottom: a header of headline figures, a daily saved-tokens
/// sparkline, a scrollable table of reduced commands (ranked by tokens saved),
/// a table of recurring no-config "opportunity" commands, and a key-hint
/// footer.
pub fn render(
  frame: &mut Frame,
  data: &DashboardData,
) {
  let area = frame.area();
  let s = data.summary;

  if s.total_runs == 0 {
    let msg = Paragraph::new(
      "No runs recorded yet — run `decant init` to install the hook, then work as usual.",
    )
    .style(Style::default().fg(palette::SUBTEXT))
    .alignment(Alignment::Center)
    .block(Block::bordered().title(" decant dashboard "));
    frame.render_widget(msg, area);
    return;
  }

  // Size the by-config panel to its content (1 border + header + N tiers + 1
  // border) so a short rollup doesn't starve the reduced table on small
  // terminals. There are at most four tiers.
  let tiers = s.by_config.len().clamp(1, 4);
  let config_h = u16::try_from(tiers + 3).unwrap_or(7);
  let chunks = Layout::vertical([
    Constraint::Length(4),        // header
    Constraint::Length(3),        // sparkline
    Constraint::Length(config_h), // by-config tiers (where the reduction comes from)
    Constraint::Min(3),           // reduced table
    Constraint::Length(6),        // opportunities
    Constraint::Length(1),        // footer
  ])
  .split(area);

  render_header(frame, chunks[0], data);
  render_sparkline(frame, chunks[1], data);
  render_config(frame, chunks[2], data);
  render_reduced(frame, chunks[3], data);
  render_opportunities(frame, chunks[4], data);
  render_footer(frame, chunks[5]);
}

fn window_label(since_days: Option<u64>) -> String {
  match since_days {
    | Some(d) => format!(" last {d} days "),
    | None => " all time ".to_string(),
  }
}

fn render_header(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
  data: &DashboardData,
) {
  let s = data.summary;
  let headline = Line::from(vec![
    Span::styled(
      format!("{} runs", s.total_runs),
      Style::default().fg(palette::TEXT),
    ),
    Span::raw("   "),
    Span::styled(
      format!(
        "{} → {} tokens",
        human(s.total_tokens_in),
        human(s.total_tokens_out)
      ),
      Style::default().fg(palette::BLUE),
    ),
    Span::raw("   "),
    Span::styled(
      format!("{:.1}% saved", s.token_savings_pct()),
      Style::default()
        .fg(palette::GREEN)
        .add_modifier(Modifier::BOLD),
    ),
  ]);
  let detail = Line::from(Span::styled(
    format!(
      "{} tokens saved · {} bytes saved",
      human(s.total_tokens_in.saturating_sub(s.total_tokens_out)),
      human_bytes(s.total_bytes_in.saturating_sub(s.total_bytes_out)),
    ),
    Style::default().fg(palette::SUBTEXT),
  ));
  let block = Block::bordered()
    .title(" decant dashboard ")
    .title_top(Line::from(window_label(data.since_days)).right_aligned());
  frame.render_widget(Paragraph::new(vec![headline, detail]).block(block), area);
}

fn render_sparkline(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
  data: &DashboardData,
) {
  let spark = sparkline(data.daily);
  let line = if spark.is_empty() {
    Line::from(Span::styled(
      "daily tokens saved   (no data)",
      Style::default().fg(palette::SUBTEXT),
    ))
  } else {
    Line::from(vec![
      Span::styled(
        "daily tokens saved  ",
        Style::default().fg(palette::SUBTEXT),
      ),
      Span::styled(spark, Style::default().fg(palette::MAUVE)),
    ])
  };
  frame.render_widget(Paragraph::new(line).block(Block::bordered()), area);
}

fn render_config(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
  data: &DashboardData,
) {
  let header = Row::new(["Config source", "runs", "tokens in → out", "% saved"]).style(
    Style::default()
      .fg(palette::MAUVE)
      .add_modifier(Modifier::BOLD),
  );
  let rows = data.summary.by_config.iter().map(|c| {
    Row::new([
      c.source.label().to_string(),
      c.count.to_string(),
      format!("{} → {}", human(c.tokens_in), human(c.tokens_out)),
      format!("{:.1}%", c.token_savings_pct()),
    ])
    .style(Style::default().fg(palette::TEXT))
  });
  let widths = [
    Constraint::Min(14),
    Constraint::Length(7),
    Constraint::Length(18),
    Constraint::Length(9),
  ];
  let table = Table::new(rows, widths)
    .header(header)
    .block(Block::bordered().title(" where from (tokens) "));
  frame.render_widget(table, area);
}

fn render_reduced(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
  data: &DashboardData,
) {
  let header = Row::new(["Reduced", "runs", "tok saved", "% saved"]).style(
    Style::default()
      .fg(palette::YELLOW)
      .add_modifier(Modifier::BOLD),
  );
  let rows = data.summary.reduced.iter().skip(data.scroll).map(|c| {
    Row::new([
      c.command.clone(),
      c.count.to_string(),
      human(c.saved_tokens()),
      format!("{:.1}%", c.token_savings_pct()),
    ])
    .style(Style::default().fg(palette::TEXT))
  });
  let widths = [
    Constraint::Min(20),
    Constraint::Length(7),
    Constraint::Length(11),
    Constraint::Length(9),
  ];
  let table = Table::new(rows, widths)
    .header(header)
    .block(Block::bordered().title(" reduced "));
  frame.render_widget(table, area);
}

fn render_opportunities(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
  data: &DashboardData,
) {
  let header = Row::new(["Opportunities (no config)", "runs"]).style(
    Style::default()
      .fg(palette::PEACH)
      .add_modifier(Modifier::BOLD),
  );
  let rows = data.summary.opportunities.iter().map(|c| {
    Row::new([c.command.clone(), c.count.to_string()]).style(Style::default().fg(palette::SUBTEXT))
  });
  let table = Table::new(rows, [Constraint::Min(20), Constraint::Length(7)])
    .header(header)
    .block(Block::bordered().title(" opportunities "));
  frame.render_widget(table, area);
}

fn render_footer(
  frame: &mut Frame,
  area: ratatui::layout::Rect,
) {
  frame.render_widget(
    Paragraph::new(Line::from(Span::styled(
      " q quit · r refresh · ↑↓ scroll",
      Style::default().fg(palette::SUBTEXT),
    ))),
    area,
  );
}

#[cfg(test)]
mod tests {
  use decant_store::{CommandStat, ConfigKind, ConfigStat, DailyBucket, Summary};
  use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

  use super::{DashboardData, render};

  /// Flatten a rendered buffer into newline-joined rows for substring asserts.
  fn buffer_text(buf: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
      for x in 0..buf.area.width {
        s.push_str(buf[(x, y)].symbol());
      }
      s.push('\n');
    }
    s
  }

  fn stat(
    command: &str,
    count: u64,
    tin: u64,
    tout: u64,
    cs: ConfigKind,
  ) -> CommandStat {
    CommandStat {
      command: command.to_string(),
      count,
      bytes_in: tin * 4,
      bytes_out: tout * 4,
      tokens_in: tin,
      tokens_out: tout,
      config_source: cs,
    }
  }

  fn sample() -> Summary {
    Summary {
      total_runs:       42,
      total_bytes_in:   4_000,
      total_bytes_out:  1_000,
      total_tokens_in:  1_000,
      total_tokens_out: 250,
      reduced:          vec![
        stat("cargo build", 30, 800, 150, ConfigKind::Builtin),
        stat("cargo test", 10, 200, 100, ConfigKind::Builtin),
      ],
      opportunities:    vec![stat("git status", 99, 0, 0, ConfigKind::Identity)],
      by_config:        vec![
        ConfigStat {
          source:     ConfigKind::Builtin,
          count:      40,
          bytes_in:   4_000,
          bytes_out:  1_000,
          tokens_in:  1_000,
          tokens_out: 250,
        },
        ConfigStat {
          source:     ConfigKind::Identity,
          count:      99,
          bytes_in:   0,
          bytes_out:  0,
          tokens_in:  0,
          tokens_out: 0,
        },
      ],
    }
  }

  fn draw(
    data: &DashboardData,
    w: u16,
    h: u16,
  ) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| render(f, data)).unwrap();
    buffer_text(terminal.backend().buffer())
  }

  #[test]
  fn header_shows_token_savings_pct() {
    let summary = sample();
    let daily = Vec::new();
    let data = DashboardData {
      summary:    &summary,
      daily:      &daily,
      since_days: Some(30),
      scroll:     0,
    };
    let text = draw(&data, 80, 24);
    // 1000 -> 250 tokens is 75.0% saved.
    assert!(text.contains("75.0%"), "missing headline pct in:\n{text}");
    assert!(text.contains("42"), "missing run count in:\n{text}");
  }

  #[test]
  fn reduced_table_lists_top_command() {
    let summary = sample();
    let daily = Vec::new();
    let data = DashboardData {
      summary:    &summary,
      daily:      &daily,
      since_days: None,
      scroll:     0,
    };
    let text = draw(&data, 80, 24);
    assert!(
      text.contains("cargo build"),
      "missing reduced command in:\n{text}"
    );
    assert!(
      text.contains("git status"),
      "missing opportunity in:\n{text}"
    );
  }

  #[test]
  fn config_panel_shows_tiers_and_token_flow() {
    let summary = sample();
    let daily = Vec::new();
    let data = DashboardData {
      summary:    &summary,
      daily:      &daily,
      since_days: None,
      scroll:     0,
    };
    let text = draw(&data, 80, 30);
    // The "where from" panel names each tier and shows tokens in → out.
    assert!(text.contains("builtin"), "missing builtin tier in:\n{text}");
    assert!(
      text.contains("identity"),
      "missing identity tier in:\n{text}"
    );
    // builtin: 1000 -> 250 tokens is 75.0% saved; the arrow proves it's a flow.
    assert!(
      text.contains("1.0K → 250"),
      "missing token flow in:\n{text}"
    );
    assert!(text.contains("75.0%"), "missing tier pct in:\n{text}");
  }

  #[test]
  fn sparkline_scales_to_max() {
    let daily = vec![
      DailyBucket { day: 0, tokens_in: 100, tokens_out: 100 }, // saved 0   -> lowest
      DailyBucket { day: 1, tokens_in: 100, tokens_out: 0 },   // saved 100 -> highest
    ];
    let s = super::sparkline(&daily);
    let chars: Vec<char> = s.chars().collect();
    assert_eq!(chars.len(), 2);
    assert_eq!(chars[0], '▁');
    assert_eq!(chars[1], '█');
  }

  #[test]
  fn empty_summary_shows_no_runs_message() {
    let summary = Summary {
      total_runs:       0,
      total_bytes_in:   0,
      total_bytes_out:  0,
      total_tokens_in:  0,
      total_tokens_out: 0,
      reduced:          Vec::new(),
      opportunities:    Vec::new(),
      by_config:        Vec::new(),
    };
    let daily: Vec<DailyBucket> = Vec::new();
    let data = DashboardData {
      summary:    &summary,
      daily:      &daily,
      since_days: None,
      scroll:     0,
    };
    let text = draw(&data, 80, 24);
    assert!(
      text.to_lowercase().contains("no runs"),
      "missing empty state in:\n{text}"
    );
  }
}
