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
