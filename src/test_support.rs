//! Internal test-only helpers shared by unit tests across the crate.

use std::sync::Mutex;

/// Process-wide lock serializing tests that change the current working
/// directory. `std::env::set_current_dir` is process-global, so any test
/// that calls it must acquire this lock for the duration of the change.
pub static CWD_LOCK: Mutex<()> = Mutex::new(());
