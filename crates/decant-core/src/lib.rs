//! decant-core: the pipeline engine — `Runner` + `Transform` seams and
//! `execute`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod capture;
pub mod error;
pub mod pipeline;
pub mod runner;
pub mod transform;
pub mod types;

pub use capture::CaptureRunner;
pub use error::RunError;
pub use pipeline::execute;
pub use runner::Runner;
pub use transform::{Transform, TransformMode};
pub use types::{Captured, TimeoutKind, TransformOutput};
