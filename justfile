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
    @cargo +nightly fmt --all
    @RUST_LOG="warn" taplo format

# Clippy with automatic fixes
fix:
    cargo clippy --fix --allow-dirty --allow-staged

# Run tests (nextest)
test:
    cargo nextest run

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

# Cut a release: run checks, bump version, tag, and push (CI builds binaries).
# level = patch | minor | major
release level="patch":
    cargo release {{ level }} --execute

# Preview a release without changing anything (cargo-release dry-run is default)
release-dry-run level="patch":
    cargo release {{ level }}
