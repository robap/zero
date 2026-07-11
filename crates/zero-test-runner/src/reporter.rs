//! Human-readable test reporter.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::result::{FileResult, SourceLoc, Status};

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
    /// Used to render project-relative paths on the `at` line. Empty means
    /// "render absolute paths as-is".
    project_root: PathBuf,
}

impl<'a, W: Write> Reporter<'a, W> {
    /// Create a new reporter writing to `writer`. `at` lines render absolute
    /// paths; use [`Reporter::new_with_root`] to get project-relative paths.
    ///
    /// # Parameters
    /// - `writer`: destination for all output.
    ///
    /// # Returns
    /// A new `Reporter`.
    pub fn new(writer: &'a mut W) -> Self {
        Self::new_with_root(writer, PathBuf::new())
    }

    /// Create a new reporter writing to `writer` that renders `at` lines as
    /// `<project-relative path>:<line>:<col>`.
    ///
    /// # Parameters
    /// - `writer`: destination for all output.
    /// - `project_root`: absolute path used to strip the prefix off source
    ///   locations.
    ///
    /// # Returns
    /// A new `Reporter`.
    pub fn new_with_root(writer: &'a mut W, project_root: PathBuf) -> Self {
        Self {
            writer,
            totals: ReporterTotals {
                passed: 0,
                failed: 0,
                skipped: 0,
            },
            started_at: Instant::now(),
            project_root,
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
            if let Some(ref loc) = err.location {
                write_location(self.writer, loc, &self.project_root)?;
                writeln!(self.writer)?;
                write_snippet(self.writer, loc)?;
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
                        if let Some(ref loc) = f.location {
                            write_location(self.writer, loc, &self.project_root)?;
                            writeln!(self.writer)?;
                            write_snippet(self.writer, loc)?;
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

/// Render the `        at <relpath>:<line>:<col>` line for a failure with a
/// known source location. Falls back to the absolute path when the file does
/// not live under `project_root` (or when `project_root` is empty).
fn write_location<W: Write>(w: &mut W, loc: &SourceLoc, project_root: &Path) -> io::Result<()> {
    let rel = if project_root.as_os_str().is_empty() {
        loc.file.as_path()
    } else {
        loc.file.strip_prefix(project_root).unwrap_or(&loc.file)
    };
    let path_str = rel.to_string_lossy().replace('\\', "/");
    writeln!(w, "        at {}:{}:{}", path_str, loc.line, loc.column)
}

/// Render a five-line source snippet centred on `loc.line` (clamped to file
/// bounds), with the failing line prefixed `> ` and a caret line underneath.
/// Silently returns `Ok(())` if `loc.file` cannot be read.
fn write_snippet<W: Write>(w: &mut W, loc: &SourceLoc) -> io::Result<()> {
    let content = match std::fs::read_to_string(&loc.file) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let file_lines: Vec<&str> = content.lines().collect();
    let total = file_lines.len() as u32;
    if loc.line == 0 || loc.line > total {
        return Ok(());
    }
    let start = if loc.line > 2 { loc.line - 2 } else { 1 };
    let end = (loc.line + 2).min(total);
    let width = end.to_string().len();
    for ln in start..=end {
        let idx = (ln as usize) - 1;
        let marker = if ln == loc.line { "> " } else { "  " };
        writeln!(
            w,
            "        {}{:>width$} | {}",
            marker,
            ln,
            file_lines[idx],
            width = width
        )?;
        if ln == loc.line {
            // Gutter prefix matches "        " + "  " + width + " " before `|`
            let gutter = " ".repeat(8 + 2 + width + 1);
            let pad = " ".repeat(loc.column.saturating_sub(1) as usize);
            writeln!(w, "{gutter}|{pad}^")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{Failure, TestOutcome};
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
            timed_out: false,
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
                location: None,
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
            (out, totals)
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
                location: None,
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

    fn failed_outcome_with_loc(
        names: &[&str],
        msg: &str,
        loc: crate::result::SourceLoc,
    ) -> TestOutcome {
        TestOutcome {
            name_chain: names.iter().map(|s| s.to_string()).collect(),
            status: Status::Failed,
            duration_ms: 0,
            failure: Some(Failure {
                message: msg.to_string(),
                stack: None,
                location: Some(loc),
            }),
        }
    }

    fn run_reporter_with_root(files: &[FileResult], root: &std::path::Path) -> String {
        let mut buf = Vec::new();
        {
            let mut r = Reporter::new_with_root(&mut buf, root.to_path_buf());
            for f in files {
                r.record_file(f).unwrap();
            }
            let _ = r.finish().unwrap();
        }
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn reporter_renders_at_line_when_location_present() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let src_path = root.join("src").join("foo.test.ts");
        std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();
        let body = "line1\nline2\nline3\nline4\nline5\n";
        std::fs::write(&src_path, body).unwrap();

        let outcome = failed_outcome_with_loc(
            &["g", "fails"],
            "boom",
            crate::result::SourceLoc {
                file: src_path.clone(),
                line: 3,
                column: 1,
            },
        );
        let file = make_file("src/foo.test.ts", vec![outcome], None);
        let out = run_reporter_with_root(&[file], root);
        assert!(
            out.contains("at src/foo.test.ts:3:1"),
            "missing project-relative `at` line:\n{out}"
        );
        // snippet contains some surrounding lines
        assert!(
            out.contains("line2") && out.contains("line3") && out.contains("line4"),
            "snippet missing context lines:\n{out}"
        );
        // failing line is marked with `>`
        assert!(
            out.contains("> 3 | line3") || out.contains(">  3 | line3"),
            "failing line marker missing:\n{out}"
        );
    }

    #[test]
    fn reporter_omits_snippet_when_source_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let missing = root.join("src").join("ghost.ts");
        let outcome = failed_outcome_with_loc(
            &["g", "fails"],
            "boom",
            crate::result::SourceLoc {
                file: missing,
                line: 9,
                column: 3,
            },
        );
        let file = make_file("src/foo.test.ts", vec![outcome], None);
        let out = run_reporter_with_root(&[file], root);
        // `at` line still prints
        assert!(
            out.contains("at "),
            "expected `at` line even when source missing:\n{out}"
        );
        // No panic / IO error: implicitly verified by reaching this point.
        // No snippet body, e.g. no ` | ` pipe-separator line should be present.
        assert!(
            !out.lines().any(|l| l.contains(" | ")),
            "snippet should be omitted when source missing:\n{out}"
        );
    }

    #[test]
    fn reporter_clamps_snippet_at_file_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let src_path = root.join("src").join("tiny.ts");
        std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();
        std::fs::write(&src_path, "first\nsecond\n").unwrap();

        let outcome = failed_outcome_with_loc(
            &["g", "fails"],
            "boom",
            crate::result::SourceLoc {
                file: src_path.clone(),
                line: 1,
                column: 1,
            },
        );
        let file = make_file("src/foo.test.ts", vec![outcome], None);
        let out = run_reporter_with_root(&[file], root);
        // Should include lines 1 and 2 (file only has two lines).
        assert!(out.contains("first"), "missing line 1: {out}");
        assert!(out.contains("second"), "missing line 2: {out}");
        // Failing line should be marked.
        assert!(
            out.contains("> 1 | first"),
            "expected failing-line marker on line 1:\n{out}"
        );
        // No phantom line numbers like `0 |` or `3 |`.
        assert!(!out.contains("0 |"), "phantom line 0 in snippet:\n{out}");
        assert!(!out.contains("3 |"), "phantom line 3 in snippet:\n{out}");
    }

    #[test]
    fn reporter_renders_caret_at_correct_column() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let src_path = root.join("src").join("col.ts");
        std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();
        std::fs::write(&src_path, "abcdefghij\n").unwrap();

        let outcome = failed_outcome_with_loc(
            &["g", "fails"],
            "boom",
            crate::result::SourceLoc {
                file: src_path.clone(),
                line: 1,
                column: 7,
            },
        );
        let file = make_file("src/foo.test.ts", vec![outcome], None);
        let out = run_reporter_with_root(&[file], root);

        // Find the caret line: after `> 1 | abcdefghij`, the next line should
        // start with the same prefix up to `|`, then 6 spaces (column 7 → 6
        // spaces of padding), then `^`.
        let caret_line = out
            .lines()
            .find(|l| l.contains('^'))
            .expect("caret line present");
        // Take everything after the `|` in the caret line.
        let after_pipe = caret_line.split_once('|').expect("caret line has |").1;
        assert_eq!(
            after_pipe, "      ^",
            "expected 6 spaces then ^, got {after_pipe:?}; full line: {caret_line:?}\nfull out: {out}"
        );
    }
}
