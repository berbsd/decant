# decant task runner. Run `just` to list commands.

default:
    @just --list

# Run all checks (format, lint, test, typos, security)
check:
    @echo "=== Format Check ==="
    @cargo +nightly fmt --check
    @echo "=== Clippy ==="
    @cargo clippy --all-targets --all-features
    @echo "=== Tests ==="
    @cargo nextest run
    @echo "=== Typos ==="
    @typos
    @echo "=== Security Check ==="
    @cargo deny check
    @echo "=== All Checks Passed ==="

# Format all code (Rust + TOML)
fmt:
    @echo "Formatting Rust files..."
    @cargo +nightly fmt --all
    @echo "Formatting TOML files..."
    @RUST_LOG="warn" taplo format
    @echo "Done!"

# Clippy with automatic fixes
fix:
    cargo clippy --fix --allow-dirty --allow-staged

# Run tests (nextest)
test:
    cargo nextest run

# Code coverage summary in the terminal (llvm-cov + nextest)
coverage:
    cargo llvm-cov nextest

# Code coverage as an HTML report, opened in the browser when done
coverage-html:
    cargo llvm-cov nextest --html --open

# Build release binary
build-release:
    cargo build --release

# Remove build artifacts
clean:
    cargo clean

# Install git hooks via lefthook
hooks:
    lefthook install

# Security audit
audit:
    cargo deny check

# =============================================================================
# Releases
# =============================================================================

# Regenerate CHANGELOG.md from conventional commits
changelog:
    git cliff -o CHANGELOG.md

# Preview unreleased changelog entries (no file written)
changelog-preview:
    git cliff --unreleased

# Release: refresh changelog, then cargo-release checks, previews, prompts, bumps, tags, pushes (level=patch|minor|major)
release level:
    #!/usr/bin/env bash
    set -euo pipefail
    just changelog
    git add CHANGELOG.md
    if git diff --cached --quiet -- CHANGELOG.md; then
      echo "changelog: unchanged"
    else
      git commit -m "docs: update changelog" -- CHANGELOG.md
    fi
    cargo release {{ level }} --execute

# Preview a release without changing anything (cargo-release dry-run is default)
release-dry-run level:
    cargo release {{ level }}
