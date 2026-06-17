//! Command-line interface definition — parsed by [`clap`] in
//! [`crate::run_cli`].

use clap::{Parser, Subcommand};

use crate::cmd::{
  dashboard::DashboardArgs, explain::ExplainArgs, history::HistoryArgs, hook::HookArgs,
  init::InitArgs, run::RunArgs, update::UpdateArgs,
};

/// Top-level CLI struct parsed from `std::env::args`.
#[derive(Parser)]
#[command(
  name = "decant",
  version,
  about = "Reduce a command's output to save LLM tokens",
  long_about = "decant wraps a command, runs it, and reduces its output before it reaches an LLM \
                agent — trimming noise, collapsing repetition, and capping runaway logs so you \
                spend fewer tokens on the same signal.\n\nInstall the hook once with `decant \
                init`; thereafter your agent's shell commands are routed through `decant run` \
                automatically. Use `decant explain` to preview a command's reduction without \
                running it, and `decant history` to see how much you've actually saved.",
  after_help = "Run `decant <command> --help` for details on any subcommand.",
  after_long_help = "EXAMPLES:\n  decant run -- cargo test         Reduce a command's output\n  \
                     decant explain -- git status     Preview the transform chain (no run)\n  \
                     decant explain                   List commands with a built-in config\n  \
                     decant init                      Install the hook for Claude\n  decant \
                     history --since 7         Show savings over the last 7 days\n\nInstall the \
                     hook once, then your agent's commands are reduced automatically."
)]
pub struct Cli {
  /// The subcommand to execute.
  #[command(subcommand)]
  pub command: Commands,
}

/// Available `decant` subcommands.
#[derive(Subcommand)]
pub enum Commands {
  /// Run a command and emit its reduced output.
  ///
  /// Spawns the command, captures stdout and stderr, applies the transform
  /// chain resolved for it, and prints the reduced result. The child's exit
  /// code and raw byte stream are preserved; a one-line savings summary is
  /// written to stderr unless `--no-stats` is given.
  Run(RunArgs),
  /// Show which transforms apply to a command (no execution).
  ///
  /// Resolves the config for a command and lists each transform step in order,
  /// without running anything. With no command, lists every command that has a
  /// built-in config — handy for discovering what decant already reduces.
  Explain(ExplainArgs),
  /// Install decant's hook into an agent's settings.
  ///
  /// Writes the hook entry into the agent's settings file (global by default,
  /// or `./.claude` with `--project`). Idempotent: re-running reports that the
  /// hook is already present rather than duplicating it.
  Init(InitArgs),
  /// Hook processor invoked by an agent (reads stdin, rewrites, writes stdout).
  ///
  /// Invoked automatically by the agent once `decant init` has installed the
  /// hook — you normally never run this by hand. Robust by contract: it always
  /// emits valid JSON and passes the command through unchanged on any error.
  Hook(HookArgs),
  /// Show recorded run metrics: actual savings and opportunities.
  ///
  /// Aggregates every recorded run into per-command savings, and flags
  /// frequently-run commands that have no config yet ("opportunities"). Filter
  /// by time or project, or emit JSON for further processing.
  History(HistoryArgs),
  /// Interactive terminal dashboard of recorded savings.
  ///
  /// Renders the same metrics as `history` as a full-screen, scrollable view:
  /// headline token savings, a daily trend sparkline, the top reduced commands,
  /// and unconfigured "opportunity" commands. Requires a terminal; add
  /// `--watch` to refresh live as new runs land.
  Dashboard(DashboardArgs),
  /// Update decant to the latest release.
  ///
  /// Downloads the release for this target, verifies its SHA256 in-process,
  /// and atomically replaces the running binary. Use `--check` to report
  /// whether a newer version exists without installing it.
  Update(UpdateArgs),
}
