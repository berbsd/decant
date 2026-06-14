//! `decant update` — replace the running binary with the latest GitHub release
//! for this target. Downloads via `ureq`, verifies SHA256 in-process, extracts
//! with `tar`, and atomically renames the new binary over the current one.

use std::{
  io::Write,
  path::Path,
  process::{Command, ExitCode},
};

use anyhow::{Context, Result, bail};
use clap::Args;
use sha2::{Digest, Sha256};

const REPO: &str = "squadri/decant";
const USER_AGENT: &str = "decant-update";

/// Arguments for `decant update`.
#[derive(Args)]
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
  let current = env!("CARGO_PKG_VERSION");
  let latest_tag = fetch_latest_tag()?;
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

  apply_update(&latest_tag)?;
  let mut out = std::io::stdout().lock();
  writeln!(out, "decant: updated v{current} -> {latest_tag}")?;
  Ok(ExitCode::SUCCESS)
}

fn fetch_latest_tag() -> Result<String> {
  let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
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

fn apply_update(tag: &str) -> Result<()> {
  let target = env!("DECANT_TARGET");
  let asset = asset_name(target);
  let base = format!("https://github.com/{REPO}/releases/download/{tag}");

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
  let current_exe = std::env::current_exe().context("locating the current executable")?;
  // Stage beside the current binary so the final rename is same-filesystem
  // (atomic).
  let staged = current_exe.with_extension("new");
  std::fs::copy(&new_bin, &staged).context("staging the new binary")?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
      .context("setting permissions")?;
  }
  std::fs::rename(&staged, &current_exe).context("replacing the binary")?;
  let _unused = std::fs::remove_dir_all(&tmp);
  Ok(())
}

#[cfg(test)]
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
}
