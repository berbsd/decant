//! Pipeline engine for `decant`: the [`Runner`] + [`Transform`] extension
//! points and the [`execute`] entry point that wires them together.
//!
//! # Overview
//!
//! `decant-core` is deliberately small. It defines two traits and one function:
//!
//! - [`Runner`] — how to spawn a child command and collect its output.
//! - [`Transform`] — how to reduce that output to save LLM tokens.
//! - [`execute`] — run a command via a [`Runner`], then apply a [`Transform`].
//!
//! [`CaptureRunner`] is the production [`Runner`]: it drains stdout and stderr
//! concurrently on background threads and enforces idle + wall-clock timeouts.
//!
//! # Fallback principle
//!
//! If [`CaptureRunner`] kills a child due to a timeout, [`execute`] skips the
//! transform and returns the raw (partial) buffer untouched. A [`Transform`]
//! that assumes a complete buffer would misread partial output; passing it
//! through unchanged is always safe.
//!
//! # Example
//!
//! ```no_run
//! use std::process::Command;
//!
//! use decant_core::{CaptureRunner, execute};
//!
//! // A trivial pass-through transform.
//! struct Identity;
//! impl decant_core::Transform for Identity {
//!   fn name(&self) -> &str {
//!     "identity"
//!   }
//!
//!   fn apply(
//!     &self,
//!     c: &decant_core::Captured,
//!   ) -> decant_core::TransformOutput {
//!     decant_core::TransformOutput { stdout: c.stdout.clone(), stderr: c.stderr.clone() }
//!   }
//! }
//!
//! let runner = CaptureRunner::default();
//! let (output, captured) = execute(Command::new("echo"), &runner, &Identity)?;
//! println!("exit {}", captured.exit_code);
//! # Ok::<(), decant_core::RunError>(())
//! ```
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod capture;
pub mod error;
pub mod pipeline;
pub mod progress;
pub mod runner;
pub mod transform;
pub mod types;

pub use capture::CaptureRunner;
pub use error::RunError;
pub use pipeline::execute;
pub use progress::CaptureProgress;
pub use runner::Runner;
pub use transform::{Transform, TransformMode};
pub use types::{Captured, TimeoutKind, TransformOutput};
