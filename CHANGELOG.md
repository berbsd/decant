# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] - 2026-06-17

### Documentation

- Document rank, the [args] directive, and the dashboard
- Update changelog

### Features

- **transforms:** Add rank rule and [args] command rewrite
- **store:** Enrich history metrics and aggregate noisy commands
- **dashboard:** Add interactive savings TUI
- **history:** Show per-config-source token breakdown

### Miscellaneous

- Echo progress steps in `just fmt`

### Testing

- **cli:** Isolate metrics DB and cover [args] append

## [0.2.0] - 2026-06-16

### Documentation

- Update changelog

### Features

- **transforms:** Add cut and transform rules, slim terraform output
- **core:** Live capture spinner via CaptureProgress observer
- **run:** Round stats-line duration to whole seconds
- **run:** Reveal capture spinner after 3s instead of 10s

## [0.1.8] - 2026-06-15

### Documentation

- Update changelog

### Styling

- **imports:** Hoist inner use statements to module level

### Build

- **deps:** Upgrade ureq to 3.x (+ rusqlite, sha2) and adapt update.rs

## [0.1.7] - 2026-06-15

### Bug Fixes

- **update:** Report a clear message when the GitHub API rate-limits

### Documentation

- Update changelog

## [0.1.6] - 2026-06-15

### Bug Fixes

- **agents:** Force --reduce in the hook rewrite

### Documentation

- Update changelog

### Miscellaneous

- **dependabot:** Group minor/patch updates to cut PR noise

## [0.1.5] - 2026-06-15

### Bug Fixes

- **install:** Correct checksum asset name (stem, not .tar.gz)
- **typos:** Exclude generated CHANGELOG.md from spell-check

### Documentation

- Update changelog
- **readme:** Remove Design docs section and fix a stray typo
- Update changelog

### Miscellaneous

- Bump actions to node24 versions and add Dependabot
- **just:** Combine release flow into one recipe via cargo-release

## [0.1.4] - 2026-06-15

### Documentation

- **readme:** Add CI, release, and license badges
- Update changelog

### Features

- **transforms:** Add terraform plan/apply/init built-in configs

## [0.1.3] - 2026-06-15

### Documentation

- Update changelog

### Miscellaneous

- **coverage:** Add llvm-cov + Codecov upload, badge, and tooling

### Refactoring

- **core:** Signal the process group via nix, not unsafe libc

### Testing

- Raise coverage above 80% across all files

## [0.1.2] - 2026-06-15

### Bug Fixes

- **release:** Correct workspace message templates, pin shared-version

### Miscellaneous

- **release:** Publish draft only after assets upload

## [0.1.1] - 2026-06-15

### Bug Fixes

- **update:** Correct 'unparseable' typo flagged by typos hook

### Documentation

- Document decant-core and decant-metrics
- Add MIT license
- Add CONTRIBUTING guide
- **cli:** Enrich --help with long_about, examples, and grouping
- Consolidate decant README into the workspace root
- Update changelog
- Update changelog

### Features

- Scaffold decant workspace with v1 capture pipeline
- **transforms:** Add TOML-driven chainable transform system
- **agents:** Add agent hook integration crate
- **store:** Add SQLite metrics store crate
- **cli:** Add init, hook, and history subcommands
- **update:** Add install script and self-update
- **run:** Isatty-gated pipe-safe reduction
- **transforms:** Add built-in configs for 8 common commands

### Miscellaneous

- Adopt the platform .gitignore
- **cargo:** Tidy workspace manifests
- **dev:** Add bootstrap.sh dev-environment setup
- Rename project org to berbsd and unify author identity

### Refactoring

- **errors:** Migrate library crates to thiserror
- **transforms:** Consolidate built-ins into a BTreeMap
- **cli:** Group subcommands under a cmd module

### Styling

- **toml:** Align key spacing in cliff.toml and cargo-nextest.toml

### Build

- **bootstrap:** Install cocogitto, lefthook, and gitleaks


