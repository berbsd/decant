//! Rule vocabulary, TOML config loading, and per-command routing for `decant`.
//!
//! This crate provides everything needed to turn a TOML config file into a
//! [`decant_core::Transform`] that can be passed to [`decant_core::execute`].
//!
//! # Layers
//!
//! | Layer | Types |
//! |-------|-------|
//! | Rules | [`rules::StripAnsi`], [`rules::Drop`], [`rules::Keep`], [`rules::KeepAfter`], [`rules::Collapse`], [`rules::Dedup`], [`rules::Truncate`] — each implements [`Rule`]. |
//! | Chain | [`RuleChain`] — an ordered list of boxed [`Rule`]s; implements `decant_core::Transform`. |
//! | Config | [`config::CommandConfig`] / [`config::StepSpec`] — serde types for TOML; [`config::load_and_compile`] compiles them into a [`RuleChain`]. |
//! | Router | [`resolve`] — finds the right config (project → user → built-in → identity) for a given command argv. |
//!
//! # Example: resolve and apply a chain
//!
//! ```no_run
//! use decant_transforms::resolve;
//!
//! let argv: Vec<String> = vec!["cargo".into(), "build".into()];
//! let resolved = resolve(&argv);
//! println!("config source: {}", resolved.source);
//! // resolved.chain implements decant_core::Transform
//! ```
//!
//! # Passthrough / identity
//!
//! An empty chain (no rules) passes raw bytes through unchanged and is used
//! for unknown commands or when `--raw` is requested. Obtain one with
//! [`RuleChain::passthrough`].
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod chain;
pub mod config;
pub mod router;
pub mod rule;
pub mod rules;

pub use chain::RuleChain;
pub use config::ConfigError;
pub use router::{ConfigSource, Resolved, builtin_keys, resolve};
pub use rule::Rule;
