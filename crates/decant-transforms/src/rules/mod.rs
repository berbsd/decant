//! Built-in rule implementations — one struct, one file each.

mod collapse;
mod dedup;
mod drop;
mod keep;
mod keep_after;
mod strip_ansi;
mod truncate;

pub use collapse::Collapse;
pub use dedup::Dedup;
pub use drop::Drop;
pub use keep::Keep;
pub use keep_after::KeepAfter;
pub use strip_ansi::StripAnsi;
pub use truncate::Truncate;
