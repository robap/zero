//! Result types shared between the harness, reporter, and CLI.

use std::path::PathBuf;

/// Source location of a failing assertion or load error, source-mapped back to
/// the user's original `.ts`/`.js` file.
#[derive(Debug, Clone)]
pub struct SourceLoc {
    /// Absolute path to the original source file.
    pub file: PathBuf,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
}

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
    pub location: Option<SourceLoc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_round_trips_source_location() {
        let loc = SourceLoc {
            file: PathBuf::from("/abs/path/src/foo.test.ts"),
            line: 14,
            column: 7,
        };
        let failure = Failure {
            message: "boom".to_string(),
            stack: Some("at fn (/abs/path/src/foo.test.ts:14:7)".to_string()),
            location: Some(loc),
        };
        let unwrapped = failure.location.expect("location present");
        assert_eq!(unwrapped.file, PathBuf::from("/abs/path/src/foo.test.ts"));
        assert_eq!(unwrapped.line, 14);
        assert_eq!(unwrapped.column, 7);
    }

    #[test]
    fn failure_supports_absent_location() {
        let failure = Failure {
            message: "x".to_string(),
            stack: None,
            location: None,
        };
        assert!(failure.location.is_none());
    }
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
