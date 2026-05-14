//! Human-readable test reporter.

use std::io::{self, Write};
use std::time::Instant;

use crate::test_runner::result::{FileResult, Status};

/// Running totals across all files processed.
pub struct ReporterTotals {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

/// Formats test results to a writer and accumulates totals.
pub struct Reporter<'a, W: Write> {
    writer: &'a mut W,
    totals: ReporterTotals,
    started_at: Instant,
}

impl<'a, W: Write> Reporter<'a, W> {
    /// Create a new reporter writing to `writer`.
    ///
    /// # Parameters
    /// - `writer`: destination for all output.
    ///
    /// # Returns
    /// A new `Reporter`.
    pub fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            totals: ReporterTotals {
                passed: 0,
                failed: 0,
                skipped: 0,
            },
            started_at: Instant::now(),
        }
    }

    /// Emit output for one file's results and accumulate totals.
    ///
    /// # Parameters
    /// - `file`: the file result to report.
    ///
    /// # Returns
    /// `Ok(())` on success.
    pub fn record_file(&mut self, file: &FileResult) -> io::Result<()> {
        let path_str = file.path.to_string_lossy().replace('\\', "/");

        if let Some(ref err) = file.load_error {
            writeln!(self.writer, "ERROR loading {path_str}")?;
            writeln!(self.writer, "        {}", err.message)?;
            if let Some(ref stack) = err.stack {
                writeln!(self.writer, "        {stack}")?;
            }
            self.totals.failed += 1;
            return Ok(());
        }

        for outcome in &file.outcomes {
            let name = outcome.name_chain.join(" > ");
            match &outcome.status {
                Status::Passed => {
                    writeln!(self.writer, "PASS  {name}   ({path_str})")?;
                    self.totals.passed += 1;
                }
                Status::Failed => {
                    writeln!(self.writer, "FAIL  {name}   ({path_str})")?;
                    if let Some(ref f) = outcome.failure {
                        writeln!(self.writer, "        {}", f.message)?;
                        if let Some(ref stack) = f.stack {
                            writeln!(self.writer, "        {stack}")?;
                        }
                    }
                    self.totals.failed += 1;
                }
                Status::Skipped(reason) => {
                    writeln!(self.writer, "SKIP  {name} ({reason})")?;
                    self.totals.skipped += 1;
                }
            }
        }

        Ok(())
    }

    /// Emit the final summary line and return the totals.
    ///
    /// # Returns
    /// The accumulated `ReporterTotals`.
    pub fn finish(self) -> io::Result<ReporterTotals> {
        let elapsed = self.started_at.elapsed();
        let secs = elapsed.as_secs_f64();
        let p = self.totals.passed;
        let f = self.totals.failed;
        let s = self.totals.skipped;
        let w = self.writer;
        writeln!(w, "{p} passed, {f} failed, {s} skipped in {secs:.3}s")?;
        Ok(self.totals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_runner::result::{Failure, TestOutcome};
    use std::path::PathBuf;

    fn make_file(
        path: &str,
        outcomes: Vec<TestOutcome>,
        load_error: Option<Failure>,
    ) -> FileResult {
        FileResult {
            path: PathBuf::from(path),
            outcomes,
            load_error,
        }
    }

    fn passed_outcome(names: &[&str]) -> TestOutcome {
        TestOutcome {
            name_chain: names.iter().map(|s| s.to_string()).collect(),
            status: Status::Passed,
            duration_ms: 0,
            failure: None,
        }
    }

    fn failed_outcome(names: &[&str], msg: &str) -> TestOutcome {
        TestOutcome {
            name_chain: names.iter().map(|s| s.to_string()).collect(),
            status: Status::Failed,
            duration_ms: 0,
            failure: Some(Failure {
                message: msg.to_string(),
                stack: None,
            }),
        }
    }

    fn run_reporter(files: &[FileResult]) -> (String, ReporterTotals) {
        let mut buf = Vec::new();
        {
            let mut r = Reporter::new(&mut buf);
            for f in files {
                r.record_file(f).unwrap();
            }
            let totals = r.finish().unwrap();
            let out = String::from_utf8(buf).unwrap();
            return (out, totals);
        }
    }

    #[test]
    fn pass_line_contains_name_and_path() {
        let file = make_file(
            "src/routes/home.test.js",
            vec![passed_outcome(&["Home", "renders"])],
            None,
        );
        let (out, _) = run_reporter(&[file]);
        assert!(out.contains("PASS"), "output: {out}");
        assert!(out.contains("Home > renders"), "output: {out}");
        assert!(out.contains("home.test.js"), "output: {out}");
    }

    #[test]
    fn fail_line_contains_name_message_and_path() {
        let file = make_file(
            "src/routes/home.test.js",
            vec![failed_outcome(&["Home", "fails"], "expected 1 to be 2")],
            None,
        );
        let (out, _) = run_reporter(&[file]);
        assert!(out.contains("FAIL"), "output: {out}");
        assert!(out.contains("expected 1 to be 2"), "output: {out}");
        assert!(out.contains("home.test.js"), "output: {out}");
    }

    #[test]
    fn finish_produces_summary_with_correct_counts() {
        let file = make_file(
            "foo.test.js",
            vec![passed_outcome(&["a"]), failed_outcome(&["b"], "oops")],
            None,
        );
        let (out, totals) = run_reporter(&[file]);
        assert_eq!(totals.passed, 1);
        assert_eq!(totals.failed, 1);
        assert_eq!(totals.skipped, 0);
        assert!(
            out.contains("1 passed, 1 failed, 0 skipped"),
            "output: {out}"
        );
    }

    #[test]
    fn load_error_is_reported_and_counts_as_failed() {
        let file = make_file(
            "broken.test.js",
            vec![],
            Some(Failure {
                message: "SyntaxError: Unexpected token".to_string(),
                stack: None,
            }),
        );
        let (out, totals) = run_reporter(&[file]);
        assert!(out.contains("ERROR loading"), "output: {out}");
        assert!(out.contains("broken.test.js"), "output: {out}");
        assert!(out.contains("SyntaxError"), "output: {out}");
        assert_eq!(totals.failed, 1);
    }

    #[test]
    fn empty_file_contributes_zero_to_totals() {
        let file = make_file("empty.test.js", vec![], None);
        let (_, totals) = run_reporter(&[file]);
        assert_eq!(totals.passed, 0);
        assert_eq!(totals.failed, 0);
        assert_eq!(totals.skipped, 0);
    }
}
