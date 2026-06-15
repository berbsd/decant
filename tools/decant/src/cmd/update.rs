//! `decant update` — replace the running binary with the latest GitHub release
//! for this target. Downloads via `ureq`, verifies SHA256 in-process, extracts
//! with `tar`, and atomically renames the new binary over the current one.
//!
//! Testability: the network fetch/download and binary-replacement flow are made
//! testable by injecting the API/download endpoints and the target binary path
//! (see `run_with` / `apply_update`). Tests drive the full flow against a mock
//! HTTP server and a throwaway target file, so no network or real binary is
//! touched.

use std::{
  io::Write,
  path::Path,
  process::{Command, ExitCode},
};

use anyhow::{Context, Result, bail};
use clap::Args;
use sha2::{Digest, Sha256};

const REPO: &str = "berbsd/decant";
const USER_AGENT: &str = "decant-update";

/// Arguments for `decant update`.
#[derive(Args)]
#[command(
  after_long_help = "EXAMPLES:\n  decant update          Download and install the latest \
                     release\n  decant update --check  Report whether an update is available, \
                     without installing"
)]
pub struct UpdateArgs {
  /// Report whether an update is available without applying it.
  #[arg(long)]
  check: bool,
}

/// Release asset filename for a target triple (the Plan A1 contract).
fn asset_name(target: &str) -> String {
  format!("decant-{target}.tar.gz")
}

/// Strip a leading `v` from a tag (`v0.2.0` -> `0.2.0`).
fn normalize_tag(tag: &str) -> &str {
  tag.strip_prefix('v').unwrap_or(tag)
}

/// Parse `MAJOR.MINOR.PATCH` (ignoring any `-pre`/`+build` suffix).
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
  let mut parts = normalize_tag(v).split('.');
  let major = parts.next()?.parse().ok()?;
  let minor = parts.next()?.parse().ok()?;
  let patch_field = parts.next()?;
  let patch = patch_field.split(['-', '+']).next()?.parse().ok()?;
  Some((major, minor, patch))
}

/// True if `latest` is a higher version than `current`. Unparsable versions
/// yield `false` (never auto-update on garbage).
fn is_outdated(
  current: &str,
  latest: &str,
) -> bool {
  match (parse_version(current), parse_version(latest)) {
    | (Some(c), Some(l)) => l > c,
    | _ => false,
  }
}

/// Execute the `update` subcommand.
///
/// # Errors
/// Returns an error if the GitHub API is unreachable, the download or checksum
/// fails, or the binary cannot be replaced.
pub fn run(args: &UpdateArgs) -> Result<ExitCode> {
  let current_exe = std::env::current_exe().context("locating the current executable")?;
  let download_base = format!("https://github.com/{REPO}/releases/download");
  run_with(args, "https://api.github.com", &download_base, &current_exe)
}

/// The update flow with injectable endpoints and target binary, so tests can
/// drive it against a mock HTTP server and a throwaway target file. `run`
/// supplies the real GitHub endpoints and the running executable.
fn run_with(
  args: &UpdateArgs,
  api_base: &str,
  download_base: &str,
  target_exe: &Path,
) -> Result<ExitCode> {
  let current = env!("CARGO_PKG_VERSION");
  let latest_tag = fetch_latest_tag(api_base)?;
  let latest = normalize_tag(&latest_tag);

  let mut out = std::io::stdout().lock();
  if !is_outdated(current, latest) {
    writeln!(out, "decant: already up to date (v{current})")?;
    return Ok(ExitCode::SUCCESS);
  }
  if args.check {
    writeln!(out, "decant: update available: v{current} -> {latest_tag}")?;
    return Ok(ExitCode::SUCCESS);
  }
  drop(out);

  apply_update(download_base, &latest_tag, target_exe)?;
  let mut out = std::io::stdout().lock();
  writeln!(out, "decant: updated v{current} -> {latest_tag}")?;
  Ok(ExitCode::SUCCESS)
}

fn fetch_latest_tag(api_base: &str) -> Result<String> {
  let url = format!("{api_base}/repos/{REPO}/releases/latest");
  let body = ureq::get(&url)
    .set("User-Agent", USER_AGENT)
    .call()
    .context("querying the GitHub releases API")?
    .into_string()
    .context("reading the release response")?;
  let value: serde_json::Value = serde_json::from_str(&body).context("parsing the release JSON")?;
  value
    .get("tag_name")
    .and_then(serde_json::Value::as_str)
    .map(str::to_string)
    .context("no tag_name in the latest release")
}

fn download(
  url: &str,
  dest: &Path,
) -> Result<()> {
  let resp = ureq::get(url)
    .set("User-Agent", USER_AGENT)
    .call()
    .with_context(|| format!("downloading {url}"))?;
  let mut reader = resp.into_reader();
  let mut file =
    std::fs::File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
  std::io::copy(&mut reader, &mut file).context("writing the download")?;
  Ok(())
}

fn verify_sha256(
  archive: &Path,
  sha_file: &Path,
) -> Result<()> {
  use std::fmt::Write as _;
  let sums = std::fs::read_to_string(sha_file).context("reading the checksum file")?;
  let expected = sums
    .split_whitespace()
    .next()
    .context("empty checksum file")?
    .to_lowercase();
  let bytes = std::fs::read(archive).context("reading the archive")?;
  let mut hasher = Sha256::new();
  hasher.update(&bytes);
  // sha2's GenericArray output does not impl LowerHex — encode hex by hand.
  let mut actual = String::with_capacity(64);
  for byte in hasher.finalize() {
    let _unused = write!(actual, "{byte:02x}");
  }
  if actual != expected {
    bail!("checksum mismatch (expected {expected}, got {actual}) — not replacing the binary");
  }
  Ok(())
}

fn apply_update(
  download_base: &str,
  tag: &str,
  target_exe: &Path,
) -> Result<()> {
  let target = env!("DECANT_TARGET");
  let asset = asset_name(target);
  let base = format!("{download_base}/{tag}");

  let tmp = std::env::temp_dir().join(format!("decant-update-{tag}"));
  std::fs::create_dir_all(&tmp).context("creating the temp dir")?;

  let archive = tmp.join(&asset);
  download(&format!("{base}/{asset}"), &archive)?;
  let sha = tmp.join(format!("{asset}.sha256"));
  download(&format!("{base}/{asset}.sha256"), &sha)?;

  verify_sha256(&archive, &sha)?;

  let status = Command::new("tar")
    .arg("-xzf")
    .arg(&archive)
    .arg("-C")
    .arg(&tmp)
    .status()
    .context("running tar")?;
  if !status.success() {
    bail!("tar extraction failed");
  }

  let new_bin = tmp.join("decant");
  // Stage beside the target binary so the final rename is same-filesystem
  // (atomic).
  let staged = target_exe.with_extension("new");
  std::fs::copy(&new_bin, &staged).context("staging the new binary")?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
      .context("setting permissions")?;
  }
  std::fs::rename(&staged, target_exe).context("replacing the binary")?;
  let _unused = std::fs::remove_dir_all(&tmp);
  Ok(())
}

#[cfg(test)]
// The mockito `Server` guard must stay alive for the duration of each test's
// HTTP calls; `significant_drop_tightening` misreads it as droppable early.
#[allow(clippy::significant_drop_tightening)]
mod tests {
  use super::*;

  #[test]
  fn asset_name_follows_the_contract() {
    assert_eq!(
      asset_name("aarch64-apple-darwin"),
      "decant-aarch64-apple-darwin.tar.gz"
    );
  }

  #[test]
  fn normalize_tag_strips_leading_v() {
    assert_eq!(normalize_tag("v1.2.3"), "1.2.3");
    assert_eq!(normalize_tag("1.2.3"), "1.2.3");
  }

  #[test]
  fn parse_version_handles_suffixes() {
    assert_eq!(parse_version("v0.2.0"), Some((0, 2, 0)));
    assert_eq!(parse_version("1.0.0-rc1"), Some((1, 0, 0)));
    assert_eq!(parse_version("nonsense"), None);
  }

  #[test]
  fn is_outdated_compares_semver() {
    assert!(is_outdated("0.1.0", "0.2.0"));
    assert!(is_outdated("0.1.0", "v0.1.1"));
    assert!(!is_outdated("0.2.0", "0.2.0"));
    assert!(!is_outdated("0.2.0", "0.1.0"));
    assert!(!is_outdated("0.1.0", "garbage")); // never update on unparsable
  }

  fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
      let _unused = write!(hex, "{byte:02x}");
    }
    hex
  }

  #[test]
  fn verify_sha256_accepts_match_and_rejects_mismatch_or_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("decant.tar.gz");
    let sha = dir.path().join("decant.tar.gz.sha256");
    std::fs::write(&archive, b"the archive bytes").expect("write archive");

    // Matching checksum (with a trailing filename field, as shasum emits).
    let good = sha256_hex(b"the archive bytes");
    std::fs::write(&sha, format!("{good}  decant.tar.gz\n")).expect("write sha");
    assert!(verify_sha256(&archive, &sha).is_ok());

    // Wrong checksum is rejected.
    std::fs::write(&sha, "deadbeef  decant.tar.gz\n").expect("write bad sha");
    assert!(verify_sha256(&archive, &sha).is_err());

    // Empty checksum file is rejected.
    std::fs::write(&sha, "").expect("write empty sha");
    assert!(verify_sha256(&archive, &sha).is_err());
  }

  #[test]
  fn fetch_latest_tag_reads_tag_name() {
    let mut server = mockito::Server::new();
    let m = server
      .mock("GET", "/repos/berbsd/decant/releases/latest")
      .with_body(r#"{"tag_name":"v9.9.9"}"#)
      .create();
    assert_eq!(fetch_latest_tag(&server.url()).expect("tag"), "v9.9.9");
    m.assert();
  }

  #[test]
  fn fetch_latest_tag_errors_without_tag_name() {
    let mut server = mockito::Server::new();
    server
      .mock("GET", "/repos/berbsd/decant/releases/latest")
      .with_body(r#"{"nope":1}"#)
      .create();
    assert!(fetch_latest_tag(&server.url()).is_err());
  }

  #[test]
  fn download_writes_the_response_body() {
    let mut server = mockito::Server::new();
    server
      .mock("GET", "/asset")
      .with_body("payload-bytes")
      .create();
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().join("out");
    download(&format!("{}/asset", server.url()), &dest).expect("download");
    assert_eq!(std::fs::read(&dest).expect("read"), b"payload-bytes");
  }

  #[test]
  fn run_with_reports_up_to_date_for_the_same_version() {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let mut server = mockito::Server::new();
    server
      .mock("GET", "/repos/berbsd/decant/releases/latest")
      .with_body(format!(r#"{{"tag_name":"{tag}"}}"#))
      .create();
    let args = UpdateArgs { check: false };
    // No download endpoints needed — it short-circuits as up to date.
    run_with(&args, &server.url(), "unused", Path::new("/nonexistent")).expect("run_with");
  }

  #[test]
  fn run_with_check_reports_available_without_installing() {
    let mut server = mockito::Server::new();
    server
      .mock("GET", "/repos/berbsd/decant/releases/latest")
      .with_body(r#"{"tag_name":"v999.0.0"}"#)
      .create();
    let args = UpdateArgs { check: true };
    run_with(&args, &server.url(), "unused", Path::new("/nonexistent")).expect("run_with");
  }

  #[test]
  fn run_with_performs_a_full_install_against_a_mock_server() {
    let target = env!("DECANT_TARGET");
    let asset = asset_name(target);
    let tag = "v999.1.0";

    // Build a real gzipped tar containing a `decant` payload file.
    let work = tempfile::tempdir().expect("tempdir");
    let payload = work.path().join("payload");
    std::fs::create_dir_all(&payload).expect("mkdir");
    std::fs::write(payload.join("decant"), b"NEW BINARY").expect("payload");
    let archive = work.path().join(&asset);
    let status = Command::new("tar")
      .arg("-czf")
      .arg(&archive)
      .arg("-C")
      .arg(&payload)
      .arg("decant")
      .status()
      .expect("tar");
    assert!(status.success());
    let archive_bytes = std::fs::read(&archive).expect("read archive");
    let sha = sha256_hex(&archive_bytes);

    let mut server = mockito::Server::new();
    let api = server
      .mock("GET", "/repos/berbsd/decant/releases/latest")
      .with_body(format!(r#"{{"tag_name":"{tag}"}}"#))
      .create();
    let gz = server
      .mock("GET", format!("/{tag}/{asset}").as_str())
      .with_body(archive_bytes)
      .create();
    let shasum = server
      .mock("GET", format!("/{tag}/{asset}.sha256").as_str())
      .with_body(format!("{sha}  {asset}\n"))
      .create();

    let target_exe = work.path().join("decant-installed");
    std::fs::write(&target_exe, b"OLD BINARY").expect("write target");

    let args = UpdateArgs { check: false };
    run_with(&args, &server.url(), &server.url(), &target_exe).expect("run_with");

    assert_eq!(std::fs::read(&target_exe).expect("read"), b"NEW BINARY");
    api.assert();
    gz.assert();
    shasum.assert();
  }
}
