//! Live integration tests: run the REAL command, then check decant's chain
//! actually reduces its output. Marked `#[ignore]` — they need the tools
//! installed and are not run in CI. Run locally with:
//!   cargo test -p decant-transforms --test live -- --ignored
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::format_push_string)]

use std::process::Command;

use decant_transforms::resolve;

/// Run `command`'s resolved chain over `raw` and return the reduced bytes.
fn reduce(
  command: &[&str],
  raw: &[u8],
) -> Vec<u8> {
  let argv: Vec<String> = command.iter().map(|s| (*s).to_string()).collect();
  resolve(&argv).chain.run(raw)
}

fn combined(out: std::process::Output) -> Vec<u8> {
  [out.stdout, out.stderr].concat()
}

#[allow(clippy::cast_precision_loss)]
fn savings_pct(
  raw: &[u8],
  reduced: &[u8],
) -> f64 {
  if raw.is_empty() {
    return 0.0;
  }
  100.0 * (1.0 - (reduced.len() as f64 / raw.len() as f64))
}

// ---------------------------------------------------------------------------
// make
// ---------------------------------------------------------------------------
// Mirrors the fixture: 5 modules, each with a sub/src level, invoked with -w
// so GNU Make emits `make[N]: Entering/Leaving directory` lines. Those are the
// noise the chain drops. On macOS the system `make` is GNU Make 3.81 and does
// emit them when -w is passed (or when called recursively from a -w parent).

#[test]
#[ignore = "needs `make`; run with --ignored"]
fn make_live_reduces() {
  let dir = tempfile::tempdir().unwrap();

  // Top-level Makefile: 5 modules, each invoked with $(MAKE) -w -C
  let mut top_mk = String::from("all:\n");
  for i in 1..=5 {
    top_mk.push_str(&format!("\t$(MAKE) -w -C m{i}\n"));
  }
  top_mk.push_str("\t@echo '==> Final link'\n");
  std::fs::write(dir.path().join("Makefile"), &top_mk).unwrap();

  for i in 1..=5 {
    let module = dir.path().join(format!("m{i}"));
    let src = module.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let mod_mk = format!("all:\n\t$(MAKE) -w -C src\n\t@echo '  LD m{i}.a'\n");
    std::fs::write(module.join("Makefile"), &mod_mk).unwrap();
    let src_mk =
      format!("all:\n\t@echo '  CC m{i}_a.o'\n\t@echo '  CC m{i}_b.o'\n\t@echo '  CC m{i}_c.o'\n");
    std::fs::write(src.join("Makefile"), &src_mk).unwrap();
  }

  let raw = combined(
    Command::new("make")
      .arg("-w")
      .current_dir(dir.path())
      .output()
      .unwrap(),
  );
  assert!(!raw.is_empty(), "make produced no output");
  let reduced = reduce(&["make"], &raw);
  assert!(reduced.len() <= raw.len(), "make output should not grow");
  assert!(!reduced.is_empty(), "make output should not be fully eaten");
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "make live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// rsync
// ---------------------------------------------------------------------------
// Mirrors the fixture: 5 subdirectories × 20 files each (100 files total)
// so the file-listing output is large enough that truncating to 20 lines
// achieves ≥60% savings even after the header/summary drops.

#[test]
#[ignore = "needs `rsync`; run with --ignored"]
fn rsync_live_reduces() {
  let dir = tempfile::tempdir().unwrap();
  let src = dir.path().join("src");
  let dst = dir.path().join("dst");
  let subdir_names = ["alpha", "beta", "gamma", "delta", "epsilon"];
  for name in subdir_names {
    std::fs::create_dir_all(src.join(name)).unwrap();
    for i in 1..=20 {
      std::fs::write(src.join(name).join(format!("file_{i}.txt")), "x").unwrap();
    }
  }
  std::fs::create_dir_all(&dst).unwrap();
  let raw = combined(
    Command::new("rsync")
      .arg("-av")
      .arg(format!("{}/", src.display()))
      .arg(format!("{}/", dst.display()))
      .output()
      .unwrap(),
  );
  assert!(!raw.is_empty());
  let reduced = reduce(&["rsync"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "rsync live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// git status
// ---------------------------------------------------------------------------
// Run with `color.ui=always` so the output contains ANSI escape codes that
// the `strip_ansi` step removes, contributing to savings alongside the
// dropped `(use "git …")` hint lines and blank lines.

#[test]
#[ignore = "needs `git`; run with --ignored"]
fn git_status_live_reduces() {
  let dir = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    Command::new("git")
      .args(args)
      .current_dir(dir.path())
      .output()
      .unwrap();
  };
  run(&["init", "-q"]);
  run(&["config", "user.email", "t@t"]);
  run(&["config", "user.name", "t"]);
  std::fs::write(dir.path().join("a.txt"), "a").unwrap();
  run(&["add", "a.txt"]);
  run(&["commit", "-qm", "init"]);
  std::fs::write(dir.path().join("a.txt"), "a\nb").unwrap();
  std::fs::write(dir.path().join("u.txt"), "u").unwrap();
  // Stage a new file so we get the "staged" section with help hints
  std::fs::write(dir.path().join("b.txt"), "c").unwrap();
  run(&["add", "b.txt"]);
  let raw = combined(
    Command::new("git")
      .args(["-c", "color.ui=always", "status"])
      .current_dir(dir.path())
      .output()
      .unwrap(),
  );
  assert!(!raw.is_empty());
  let reduced = reduce(&["git", "status"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "git status live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// git log
// ---------------------------------------------------------------------------
// Use the decant repo itself (real commit history). If somehow no commits
// exist, skip gracefully.

#[test]
#[ignore = "needs `git`; run with --ignored"]
fn git_log_live_reduces() {
  let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .unwrap()
    .parent()
    .unwrap();
  let raw = combined(
    Command::new("git")
      .args(["log"])
      .current_dir(repo_root)
      .output()
      .unwrap(),
  );
  if raw.is_empty() {
    // No commits yet; skip gracefully.
    return;
  }
  let reduced = reduce(&["git", "log"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "git log live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// git diff
// ---------------------------------------------------------------------------
// Create a repo with 10 files, each changed by 20 lines, for a ~280-line
// diff. The chain truncates to 60 lines (head), giving ≥80% savings.

#[test]
#[ignore = "needs `git`; run with --ignored"]
fn git_diff_live_reduces() {
  let dir = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    Command::new("git")
      .args(args)
      .current_dir(dir.path())
      .output()
      .unwrap();
  };
  run(&["init", "-q"]);
  run(&["config", "user.email", "t@t"]);
  run(&["config", "user.name", "t"]);
  for i in 1..=10usize {
    let mut content = format!("fn func_{i}() {{}}\n");
    for j in 1..=5usize {
      content.push_str(&format!("fn helper_{i}_{j}() {{}}\n"));
    }
    std::fs::write(dir.path().join(format!("file_{i}.rs")), &content).unwrap();
  }
  run(&["add", "."]);
  run(&["commit", "-qm", "init"]);
  // Add 20 lines to each file to produce a sizeable diff (≥280 lines).
  for i in 1..=10usize {
    let path = dir.path().join(format!("file_{i}.rs"));
    let existing = std::fs::read_to_string(&path).unwrap();
    let mut content = existing;
    for j in 1..=20usize {
      content.push_str(&format!("// added line {j} in file {i}\n"));
    }
    std::fs::write(&path, content).unwrap();
  }
  let raw = combined(
    Command::new("git")
      .args(["diff"])
      .current_dir(dir.path())
      .output()
      .unwrap(),
  );
  if raw.is_empty() {
    return;
  }
  let reduced = reduce(&["git", "diff"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "git diff live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// du
// ---------------------------------------------------------------------------
// Create 100 subdirectories so `du -h` has 101 lines; truncating to 25 lines
// yields ~75% savings.

#[test]
#[ignore = "needs `du`; run with --ignored"]
fn du_live_reduces() {
  let dir = tempfile::tempdir().unwrap();
  for i in 0..100usize {
    let sub = dir.path().join(format!("d{i}"));
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("f.txt"), "data").unwrap();
  }
  let raw = combined(
    Command::new("du")
      .arg("-h")
      .arg(dir.path())
      .output()
      .unwrap(),
  );
  if raw.is_empty() {
    return;
  }
  let reduced = reduce(&["du"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "du live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// ls
// ---------------------------------------------------------------------------
// /usr/bin has hundreds of entries, providing ≥60% savings when truncated
// to 40 lines.

#[test]
#[ignore = "needs `ls`; run with --ignored"]
fn ls_live_reduces() {
  let raw = combined(
    Command::new("ls")
      .arg("-la")
      .arg("/usr/bin")
      .output()
      .unwrap(),
  );
  if raw.is_empty() {
    return;
  }
  let reduced = reduce(&["ls"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "ls live savings only {pct:.1}%");
}

// ---------------------------------------------------------------------------
// find
// ---------------------------------------------------------------------------
// Create a temp tree with 200 files so find's output is large enough that
// truncating to 60 lines yields ≥60% savings.

#[test]
#[ignore = "needs `find`; run with --ignored"]
fn find_live_reduces() {
  let dir = tempfile::tempdir().unwrap();
  for i in 0..200usize {
    std::fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
  }
  let raw = combined(
    Command::new("find")
      .args([dir.path().to_str().unwrap(), "-type", "f"])
      .output()
      .unwrap(),
  );
  if raw.is_empty() {
    return;
  }
  let reduced = reduce(&["find"], &raw);
  assert!(reduced.len() <= raw.len());
  assert!(!reduced.is_empty());
  let pct = savings_pct(&raw, &reduced);
  assert!(pct >= 60.0, "find live savings only {pct:.1}%");
}
