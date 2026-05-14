//! Boa-based test runner: discovery, harness, reporter.

pub mod discovery;
pub mod harness;
pub mod loader;
pub mod reporter;
pub mod result;

pub use harness::run_file;
pub use result::{FileResult, Status, TestOutcome};
