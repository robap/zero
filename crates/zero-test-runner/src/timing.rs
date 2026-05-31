//! Per-run phase timing for `zero test`, gated behind the `ZERO_TEST_TIMING`
//! env var.
//!
//! The runner is single-threaded, so a thread-local accumulator is sufficient
//! and lock-free. [`add`] records into the thread-local unconditionally (it is
//! cheap); call sites guard the surrounding `Instant::now()` with [`enabled`]
//! so a default run pays nothing. The breakdown is printed to stderr only when
//! `ZERO_TEST_TIMING` is set.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// A measured phase of a single test-file run.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    /// Test-file discovery (walking the project tree).
    Discovery,
    /// Building the Boa `Context` plus installing the console / workspace globals.
    ContextBuild,
    /// Evaluating the DOM/web-platform shim blob.
    DomShim,
    /// Parsing + evaluating the entry module and the runtime modules it pulls in.
    RuntimeEval,
    /// SWC transpile / coverage instrument of imported source modules.
    Transpile,
    /// Executing the test tree (`walk_describe`).
    TestExec,
}

impl Phase {
    /// All phases, in display order.
    const ALL: [Phase; 6] = [
        Phase::Discovery,
        Phase::ContextBuild,
        Phase::DomShim,
        Phase::RuntimeEval,
        Phase::Transpile,
        Phase::TestExec,
    ];

    /// Human-readable label for printing.
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Discovery => "discovery",
            Phase::ContextBuild => "context-build",
            Phase::DomShim => "dom-shim",
            Phase::RuntimeEval => "runtime-eval",
            Phase::Transpile => "transpile",
            Phase::TestExec => "test-exec",
        }
    }
}

/// Pure helper: is timing enabled given the raw env-var value?
///
/// Enabled when the variable is present and non-empty.
pub fn parse_enabled(v: Option<&str>) -> bool {
    v.is_some_and(|s| !s.is_empty())
}

thread_local! {
    static TOTALS: RefCell<HashMap<Phase, (Duration, u64)>> = RefCell::new(HashMap::new());
}

/// Whether `ZERO_TEST_TIMING` is set (read once via a `OnceLock`).
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| parse_enabled(std::env::var("ZERO_TEST_TIMING").ok().as_deref()))
}

/// Record `d` against `phase`, bumping that phase's call count.
///
/// Always records into the thread-local; the per-call cost is a single map
/// entry update. Call sites guard the surrounding measurement with [`enabled`]
/// so a default (untimed) run does no extra work.
pub fn add(phase: Phase, d: Duration) {
    TOTALS.with(|t| {
        let mut map = t.borrow_mut();
        let entry = map.entry(phase).or_insert((Duration::ZERO, 0));
        entry.0 += d;
        entry.1 += 1;
    });
}

/// Clear all accumulated timing for the current thread.
pub fn reset() {
    TOTALS.with(|t| t.borrow_mut().clear());
}

/// Start a measurement: returns `Some(Instant)` when timing is enabled, else
/// `None`. Pair with [`record_since`] to attribute the elapsed time to a phase.
/// When disabled this is a cheap `Option` with no clock read.
pub fn start() -> Option<Instant> {
    enabled().then(Instant::now)
}

/// Record the time elapsed since `start` against `phase`. A no-op when `start`
/// is `None` (timing disabled).
pub fn record_since(phase: Phase, start: Option<Instant>) {
    if let Some(s) = start {
        add(phase, s.elapsed());
    }
}

/// Snapshot the per-phase totals: `(phase, total, call_count)` for every phase
/// with at least one recorded call, in [`Phase::ALL`] order.
pub fn snapshot() -> Vec<(Phase, Duration, u64)> {
    TOTALS.with(|t| {
        let map = t.borrow();
        Phase::ALL
            .iter()
            .filter_map(|p| map.get(p).map(|(d, n)| (*p, *d, *n)))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_enabled_none_is_false() {
        assert!(!parse_enabled(None));
    }

    #[test]
    fn parse_enabled_empty_is_false() {
        assert!(!parse_enabled(Some("")));
    }

    #[test]
    fn parse_enabled_nonempty_is_true() {
        assert!(parse_enabled(Some("1")));
    }

    #[test]
    fn add_accumulates_per_phase_and_counts_calls() {
        reset();
        add(Phase::Transpile, Duration::from_millis(10));
        add(Phase::Transpile, Duration::from_millis(5));
        add(Phase::ContextBuild, Duration::from_millis(3));
        let snap = snapshot();
        let transpile = snap.iter().find(|(p, ..)| *p == Phase::Transpile).unwrap();
        assert_eq!(transpile.1, Duration::from_millis(15));
        assert_eq!(transpile.2, 2);
        let ctx = snap
            .iter()
            .find(|(p, ..)| *p == Phase::ContextBuild)
            .unwrap();
        assert_eq!(ctx.1, Duration::from_millis(3));
        assert_eq!(ctx.2, 1);
    }

    #[test]
    fn snapshot_is_in_phase_order_and_omits_unrecorded() {
        reset();
        add(Phase::TestExec, Duration::from_millis(1));
        add(Phase::Discovery, Duration::from_millis(1));
        let snap = snapshot();
        let phases: Vec<Phase> = snap.iter().map(|(p, ..)| *p).collect();
        assert_eq!(phases, vec![Phase::Discovery, Phase::TestExec]);
    }

    #[test]
    fn reset_clears_totals() {
        add(Phase::DomShim, Duration::from_millis(7));
        reset();
        assert!(snapshot().is_empty());
    }
}
