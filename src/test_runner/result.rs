//! Result types shared between the harness, reporter, and CLI.

use std::path::PathBuf;

/// The outcome of a single `it()` test case.
#[derive(Debug)]
pub struct TestOutcome {
    /// Describe names from outermost to innermost, followed by the `it` name.
    pub name_chain: Vec<String>,
    /// Whether the test passed, failed, or was skipped.
    pub status: Status,
    /// Wall-clock milliseconds the test body (and its hooks) took.
    pub duration_ms: u128,
    /// Non-None on `Failed`; the assertion or thrown error details.
    pub failure: Option<Failure>,
}

/// Test outcome status.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Passed,
    Failed,
    /// Skipped because a `beforeAll` or `beforeEach` hook failed; carries the reason.
    Skipped(String),
}

/// Details of a test failure or load error.
#[derive(Debug)]
pub struct Failure {
    pub message: String,
    pub stack: Option<String>,
}

/// All outcomes for a single test file.
#[derive(Debug)]
pub struct FileResult {
    /// Path relative to the project root.
    pub path: PathBuf,
    pub outcomes: Vec<TestOutcome>,
    /// Non-None if the file itself failed to load (syntax error, top-level throw, etc.).
    pub load_error: Option<Failure>,
}
