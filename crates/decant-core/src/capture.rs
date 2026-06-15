//! `CaptureRunner` — capture-then-emit execution with concurrent stdout/stderr
//! draining and idle + wall-clock timeouts.

use std::{
  io::Read,
  os::unix::process::{CommandExt, ExitStatusExt},
  process::{Child, Command, Stdio},
  sync::mpsc::{self, RecvTimeoutError, Sender},
  thread::{self, JoinHandle},
  time::{Duration, Instant},
};

use nix::{
  sys::signal::{Signal, killpg},
  unistd::Pid,
};

use crate::{
  error::RunError,
  runner::Runner,
  types::{Captured, TimeoutKind},
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const TERM_GRACE: Duration = Duration::from_millis(200);
const TIMEOUT_EXIT_CODE: i32 = 124;
const READ_CHUNK: usize = 8192;

/// Capture-then-emit runner: drains stdout/stderr on threads while enforcing
/// idle and wall-clock timeouts. Shared core with the future `StreamRunner`.
///
/// Note: the child runs in its own process group, so terminal signals
/// (e.g. Ctrl-C / SIGINT to the foreground group) are NOT forwarded to it —
/// decant receives them instead. This suits non-interactive (LLM tool-call)
/// use; interactive signal forwarding is intentionally not implemented.
pub struct CaptureRunner {
  /// No output for this long ⇒ assume hung. `None` disables.
  pub idle_timeout:   Option<Duration>,
  /// Total wall-clock budget. `None` disables.
  pub wall_clock_cap: Option<Duration>,
}

impl CaptureRunner {
  /// Construct with explicit timeouts (`None` disables a given timeout).
  ///
  /// Pass `None` for either parameter to run without that limit. Both
  /// can be disabled simultaneously for an unbounded capture.
  #[must_use]
  pub fn new(
    idle_timeout: Option<Duration>,
    wall_clock_cap: Option<Duration>,
  ) -> Self {
    Self { idle_timeout, wall_clock_cap }
  }

  fn expired(
    &self,
    start: Instant,
    last_activity: Instant,
  ) -> Option<TimeoutKind> {
    if let Some(idle) = self.idle_timeout {
      if last_activity.elapsed() >= idle {
        return Some(TimeoutKind::Idle);
      }
    }
    if let Some(cap) = self.wall_clock_cap {
      if start.elapsed() >= cap {
        return Some(TimeoutKind::WallClock);
      }
    }
    None
  }
}

impl Default for CaptureRunner {
  fn default() -> Self {
    Self {
      idle_timeout:   Some(Duration::from_secs(60)),
      wall_clock_cap: Some(Duration::from_secs(600)),
    }
  }
}

enum Msg {
  Out(Vec<u8>),
  Err(Vec<u8>),
}

#[derive(Clone, Copy)]
enum Stream {
  Out,
  Err,
}

enum Outcome {
  Exited(i32),
  TimedOut(TimeoutKind),
}

fn spawn_reader<R: Read + Send + 'static>(
  mut src: R,
  tx: Sender<Msg>,
  which: Stream,
) -> JoinHandle<()> {
  thread::spawn(move || {
    let mut buf = [0u8; READ_CHUNK];
    loop {
      match src.read(&mut buf) {
        | Ok(0) => break,
        | Ok(n) => {
          let chunk = buf[..n].to_vec();
          let msg = match which {
            | Stream::Out => Msg::Out(chunk),
            | Stream::Err => Msg::Err(chunk),
          };
          if tx.send(msg).is_err() {
            break;
          }
        },
        | Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {},
        | Err(_) => break,
      }
    }
  })
}

/// SIGTERM the child's process group, grace, then SIGKILL if still alive.
#[cfg(unix)]
fn terminate(child: &mut Child) {
  // The child is spawned with `process_group(0)`, so it leads its own process
  // group whose PGID equals its PID. `killpg` signals that whole group, so any
  // grandchildren die with it. Errors (e.g. the group is already gone) are
  // ignored — we just escalate to SIGKILL if it outlives the grace period.
  let Ok(pgid) = i32::try_from(child.id()).map(Pid::from_raw) else {
    return;
  };

  let _unused = killpg(pgid, Signal::SIGTERM);

  thread::sleep(TERM_GRACE);

  if !matches!(child.try_wait(), Ok(Some(_))) {
    let _unused = killpg(pgid, Signal::SIGKILL);
  }
}

#[cfg(not(unix))]
fn terminate(child: &mut Child) {
  let _ = child.kill();
}

/// Map an [`ExitStatus`] to an `i32` exit code, following the shell convention
/// of `128 + signal` for signal-terminated children (whose `.code()` is
/// `None`).
#[cfg(unix)]
fn exit_code_of(status: std::process::ExitStatus) -> i32 {
  status
    .code()
    .unwrap_or_else(|| status.signal().map_or(-1, |s| 128 + s))
}

#[cfg(not(unix))]
fn exit_code_of(status: std::process::ExitStatus) -> i32 {
  status.code().unwrap_or(1)
}

impl Runner for CaptureRunner {
  fn run(
    &self,
    mut cmd: Command,
  ) -> Result<Captured, RunError> {
    cmd
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .stdin(Stdio::null());
    #[cfg(unix)]
    {
      cmd.process_group(0);
    }

    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
      .spawn()
      .map_err(|source| RunError::Spawn { program, source })?;

    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| RunError::Io(std::io::Error::other("stdout pipe missing")))?;
    let stderr = child
      .stderr
      .take()
      .ok_or_else(|| RunError::Io(std::io::Error::other("stderr pipe missing")))?;

    let (tx, rx) = mpsc::channel::<Msg>();
    let out_handle = spawn_reader(stdout, tx.clone(), Stream::Out);
    let err_handle = spawn_reader(stderr, tx, Stream::Err);

    let start = Instant::now();
    let mut last_activity = Instant::now();
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();

    let outcome = loop {
      match rx.recv_timeout(POLL_INTERVAL) {
        | Ok(Msg::Out(b)) => {
          stdout_buf.extend_from_slice(&b);
          last_activity = Instant::now();
        },
        | Ok(Msg::Err(b)) => {
          stderr_buf.extend_from_slice(&b);
          last_activity = Instant::now();
        },
        | Err(RecvTimeoutError::Timeout) => {},
        | Err(RecvTimeoutError::Disconnected) => {
          last_activity = Instant::now();
          thread::sleep(POLL_INTERVAL);
        },
      }

      if let Some(status) = child.try_wait()? {
        break Outcome::Exited(exit_code_of(status));
      }
      if let Some(kind) = self.expired(start, last_activity) {
        terminate(&mut child);
        drop(child.wait());
        break Outcome::TimedOut(kind);
      }
    };

    drop(out_handle.join());
    drop(err_handle.join());
    while let Ok(msg) = rx.try_recv() {
      match msg {
        | Msg::Out(b) => stdout_buf.extend_from_slice(&b),
        | Msg::Err(b) => stderr_buf.extend_from_slice(&b),
      }
    }

    let (exit_code, timeout) = match outcome {
      | Outcome::Exited(code) => (code, None),
      | Outcome::TimedOut(kind) => (TIMEOUT_EXIT_CODE, Some(kind)),
    };

    Ok(Captured {
      stdout: stdout_buf,
      stderr: stderr_buf,
      exit_code,
      timeout,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sh(script: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", script]);
    cmd
  }

  #[test]
  fn captures_stdout_on_clean_exit() {
    let cap = CaptureRunner::default().run(sh("printf 'hello'")).unwrap();
    assert_eq!(cap.stdout, b"hello");
    assert_eq!(cap.exit_code, 0);
    assert!(cap.timeout.is_none());
  }

  #[test]
  fn captures_stderr() {
    let cap = CaptureRunner::default()
      .run(sh("printf 'oops' 1>&2"))
      .unwrap();
    assert_eq!(cap.stderr, b"oops");
  }

  #[test]
  fn propagates_nonzero_exit_code() {
    let cap = CaptureRunner::default().run(sh("exit 3")).unwrap();
    assert_eq!(cap.exit_code, 3);
    assert!(cap.timeout.is_none());
  }

  #[test]
  fn idle_timeout_fires_and_returns_124() {
    let runner = CaptureRunner::new(Some(Duration::from_millis(300)), None);
    let cap = runner.run(sh("sleep 5")).unwrap();
    assert_eq!(cap.timeout, Some(TimeoutKind::Idle));
    assert_eq!(cap.exit_code, TIMEOUT_EXIT_CODE);
  }

  #[test]
  fn large_output_does_not_deadlock() {
    // 200 KB exceeds the OS pipe buffer (~64 KB); proves concurrent drain.
    let cap = CaptureRunner::default()
      .run(sh("yes | head -c 200000"))
      .unwrap();
    assert_eq!(cap.stdout.len(), 200_000);
    assert!(cap.timeout.is_none());
  }

  #[test]
  fn spawn_failure_is_an_error() {
    let err = CaptureRunner::default()
      .run(Command::new("this-binary-does-not-exist-decant"))
      .unwrap_err();
    assert!(matches!(err, RunError::Spawn { .. }));
  }

  #[test]
  fn child_that_closes_pipes_early_is_not_idle_killed() {
    // Closes stdout+stderr immediately, then works for 600ms. A 200ms idle
    // timeout must NOT kill it: pipe closure is not "hung mid-output".
    let runner = CaptureRunner::new(Some(Duration::from_millis(200)), None);
    let cap = runner.run(sh("exec 1>&-; exec 2>&-; sleep 0.6")).unwrap();
    assert!(
      cap.timeout.is_none(),
      "child must not be idle-killed after closing its pipes"
    );
    assert_eq!(cap.exit_code, 0);
  }

  #[test]
  fn signal_death_reports_128_plus_signal() {
    // Child terminates itself with SIGTERM (15) → 128 + 15 = 143.
    let cap = CaptureRunner::default().run(sh("kill -TERM $$")).unwrap();
    assert_eq!(cap.exit_code, 143);
    assert!(cap.timeout.is_none());
  }
}
