//! Boa-based test runner: discovery, harness, reporter.

pub mod coverage;
pub mod discovery;
pub mod harness;
pub mod loader;
pub mod mutate;
pub mod reporter;
pub mod result;
pub mod timing;

pub use harness::run_file;
pub use result::{FileResult, Status, TestOutcome};
