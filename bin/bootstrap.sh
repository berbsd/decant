#!/bin/sh
# bootstrap.sh — set up the decant development environment.
#
# Installs the toolchain components and CLI tools the workspace uses:
#   just · taplo · cargo-nextest · typos-cli · cargo-deny · git-cliff ·
#   cargo-release · cocogitto (cog), plus nightly rustfmt (for
#   `cargo +nightly fmt`). The git-hook runners lefthook and gitleaks are
#   Go binaries, installed via Homebrew when available.
#
# Prefers cargo-binstall (prebuilt binaries) for speed, falling back to
# `cargo install`. Re-runnable: already-installed tools are skipped.
set -eu

note() { echo "bootstrap: $*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- 1. Preflight: Rust must be installed -----------------------------------
missing=""
for tool in rustc cargo rustup; do
  have "$tool" || missing="$missing $tool"
done
if [ -n "$missing" ]; then
  note "missing required tool(s):$missing"
  note "install Rust from https://rustup.rs, then re-run this script:"
  note "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi
note "found $(rustc --version) / $(cargo --version)"

# --- 2. Rust toolchain components -------------------------------------------
note "ensuring the pinned toolchain + components (rustfmt, clippy, rust-src)"
rustup show >/dev/null 2>&1 || true                     # installs the rust-toolchain.toml channel
rustup component add rustfmt clippy rust-src >/dev/null 2>&1 || true
note "ensuring nightly rustfmt (used by 'just fmt' / 'cargo +nightly fmt')"
rustup toolchain install nightly --profile minimal --component rustfmt >/dev/null 2>&1 || true

# --- 3. Cargo CLI tools ------------------------------------------------------
if ! have cargo-binstall; then
  note "installing cargo-binstall (fast prebuilt installs)"
  cargo install cargo-binstall || note "cargo-binstall failed; will use 'cargo install'"
fi

install_tool() { # install_tool <binary-to-check> <crate-name>
  if have "$1"; then
    note "$1 already installed"
    return
  fi
  note "installing $2"
  if have cargo-binstall; then
    cargo binstall -y "$2" || cargo install "$2"
  else
    cargo install "$2"
  fi
}

install_tool just just
install_tool taplo taplo-cli
install_tool cargo-nextest cargo-nextest
install_tool typos typos-cli
install_tool cargo-deny cargo-deny
install_tool git-cliff git-cliff
install_tool cargo-release cargo-release
install_tool cog cocogitto          # `cog verify` runs the commit-msg hook

# --- 4. Git-hook tools (Go binaries — not on crates.io) ---------------------
# lefthook runs the hooks; gitleaks (pre-commit secret scan) and cog
# (commit-msg, installed above) are invoked by them. Prefer Homebrew.
if have brew; then
  for pkg in lefthook gitleaks; do
    if have "$pkg"; then
      note "$pkg already installed"
    else
      note "installing $pkg (brew)"
      brew install "$pkg" || note "brew install $pkg failed — install it manually"
    fi
  done
else
  have lefthook || note "lefthook not found and no brew — install it (runs the git hooks) via your package manager"
  have gitleaks || note "gitleaks not found and no brew — install it (pre-commit secret scan) via your package manager"
fi

# --- 5. Install the hooks ---------------------------------------------------
if have lefthook; then
  note "installing git hooks (lefthook)"
  lefthook install || true
else
  note "skipping hook install — lefthook not available"
fi
have shellcheck || note "shellcheck not found (optional, lints the shell scripts) — install via your package manager"

note "done — run 'just check' to verify the environment"
