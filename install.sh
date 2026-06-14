#!/bin/sh
# decant installer — detects host arch, downloads the latest release binary.
#   curl -fsSL https://raw.githubusercontent.com/squadri/decant/main/install.sh | sh
# Env: DECANT_VERSION (pin a tag), DECANT_INSTALL_DIR (default ~/.local/bin)
set -eu

REPO="squadri/decant"
BIN="decant"
INSTALL_DIR="${DECANT_INSTALL_DIR:-$HOME/.local/bin}"

err() { echo "decant-install: $*" >&2; exit 1; }

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Darwin)
      case "$arch" in
        arm64 | aarch64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) err "unsupported macOS arch: $arch" ;;
      esac ;;
    Linux)
      case "$arch" in
        x86_64) echo "x86_64-unknown-linux-musl" ;;
        aarch64 | arm64) echo "aarch64-unknown-linux-musl" ;;
        *) err "unsupported Linux arch: $arch" ;;
      esac ;;
    *) err "unsupported OS: $os" ;;
  esac
}

latest_version() {
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -1 \
    | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/'
}

main() {
  command -v curl >/dev/null 2>&1 || err "curl is required"
  command -v tar >/dev/null 2>&1 || err "tar is required"

  target=$(detect_target)
  version="${DECANT_VERSION:-$(latest_version)}"
  [ -n "$version" ] || err "could not determine the latest version"

  asset="$BIN-$target.tar.gz"
  base="https://github.com/$REPO/releases/download/$version"
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT

  echo "decant-install: downloading $asset ($version)"
  curl -fsSL "$base/$asset" -o "$tmp/$asset" || err "download failed: $base/$asset"
  curl -fsSL "$base/$asset.sha256" -o "$tmp/$asset.sha256" || err "checksum download failed"

  echo "decant-install: verifying checksum"
  (
    cd "$tmp"
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum -c "$asset.sha256"
    elif command -v shasum >/dev/null 2>&1; then
      shasum -a 256 -c "$asset.sha256"
    else
      err "no sha256sum/shasum available to verify the download"
    fi
  ) >/dev/null || err "checksum mismatch — aborting"

  tar -xzf "$tmp/$asset" -C "$tmp"
  mkdir -p "$INSTALL_DIR"
  cp "$tmp/$BIN" "$INSTALL_DIR/$BIN"
  chmod 0755 "$INSTALL_DIR/$BIN"

  echo "decant-install: installed $BIN $version to $INSTALL_DIR/$BIN"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      echo "decant-install: $INSTALL_DIR is not on your PATH. Add:"
      echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
      ;;
  esac
}

# `DECANT_INSTALL_TEST=1 . ./install.sh` sources the functions without running.
[ "${DECANT_INSTALL_TEST:-}" = "1" ] || main
