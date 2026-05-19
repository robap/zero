//! Integration tests for the Rust-side timer host installed by
//! `install_timer_host`. Each test writes a tiny module-style JS file that
//! uses `setTimeout` / `setInterval` / `requestAnimationFrame` and asserts
//! the timer fired after the await drains the job queue.

use std::io::Write as _;

use tempfile::NamedTempFile;
use zero_test_runner::{Status, run_file};

fn write_temp_js(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::with_suffix(".js").unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

fn assert_passed(content: &str) {
    let f = write_temp_js(content);
    let result = run_file(f.path().parent().unwrap(), f.path());
    assert!(
        result.load_error.is_none(),
        "load_error: {:?}",
        result.load_error
    );
    assert_eq!(result.outcomes.len(), 1, "outcomes: {:?}", result.outcomes);
    let o = &result.outcomes[0];
    assert!(
        matches!(o.status, Status::Passed),
        "expected Passed, got {:?}: {:?}",
        o.status,
        o.failure
    );
}

#[test]
fn set_timeout_fires_after_await() {
    assert_passed(
        r#"import { it, expect } from "zero/test";
it("ran", async () => {
  globalThis.__hit__ = 0;
  setTimeout(() => { globalThis.__hit__ = 1; }, 0);
  await Promise.resolve();
  expect(globalThis.__hit__).toBe(1);
});
"#,
    );
}

#[test]
fn clear_timeout_cancels_pending_timer() {
    assert_passed(
        r#"import { it, expect } from "zero/test";
it("cancels", async () => {
  globalThis.__hit__ = 0;
  const id = setTimeout(() => { globalThis.__hit__ = 1; }, 0);
  clearTimeout(id);
  await Promise.resolve();
  expect(globalThis.__hit__).toBe(0);
});
"#,
    );
}

#[test]
fn set_interval_repeats_until_cleared() {
    assert_passed(
        r#"import { it, expect } from "zero/test";
it("interval", async () => {
  globalThis.__n__ = 0;
  const id = setInterval(() => {
    globalThis.__n__++;
    if (globalThis.__n__ >= 3) clearInterval(id);
  }, 0);
  for (let i = 0; i < 6; i++) await Promise.resolve();
  expect(globalThis.__n__).toBe(3);
});
"#,
    );
}

#[test]
fn request_animation_frame_invokes_callback_with_counter() {
    assert_passed(
        r#"import { it, expect } from "zero/test";
it("raf", async () => {
  globalThis.__t__ = -1;
  requestAnimationFrame(t => { globalThis.__t__ = t; });
  await Promise.resolve();
  expect(globalThis.__t__).toBe(16);
});
"#,
    );
}

#[test]
fn clear_all_timers_drains_pending_work() {
    assert_passed(
        r#"import { it, expect } from "zero/test";
it("clearAll", async () => {
  globalThis.__hit__ = 0;
  setTimeout(() => { globalThis.__hit__++; }, 0);
  setTimeout(() => { globalThis.__hit__++; }, 0);
  setTimeout(() => { globalThis.__hit__++; }, 0);
  __clearAllTimers__();
  await Promise.resolve();
  expect(globalThis.__hit__).toBe(0);
});
"#,
    );
}
