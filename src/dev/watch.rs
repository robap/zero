//! File watcher: watches `<root>` and publishes reload events to `ReloadBus`.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_mini::{
    DebounceEventResult, Debouncer, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};

use crate::dev::sse::ReloadBus;

/// Owns the debouncer; drop to stop watching.
pub struct WatchHandle {
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Start watching `root` recursively. Returns `Ok(None)` if the watcher
/// fails to set up (logs a warning and lets the server keep running).
pub fn start(
    root: PathBuf,
    out_dir: PathBuf,
    bus: Arc<ReloadBus>,
) -> anyhow::Result<Option<WatchHandle>> {
    use std::sync::mpsc;
    let (tx, rx): (
        mpsc::Sender<DebounceEventResult>,
        mpsc::Receiver<DebounceEventResult>,
    ) = mpsc::channel();

    let mut debouncer = match new_debouncer(Duration::from_millis(100), tx) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("zero dev: failed to create file watcher: {e}; auto-reload disabled");
            return Ok(None);
        }
    };
    if let Err(e) = debouncer.watcher().watch(&root, RecursiveMode::Recursive) {
        eprintln!(
            "zero dev: failed to watch {}: {e}; auto-reload disabled",
            root.display()
        );
        return Ok(None);
    }

    let root_for_thread = root.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(result) = rx.recv() {
            let events = match result {
                Ok(events) => events,
                Err(e) => {
                    eprintln!("zero dev: watcher error: {e:?}");
                    continue;
                }
            };
            if let Some(rel_path) = representative_path(
                events.iter().map(|ev| ev.path.as_path()),
                &root_for_thread,
                &out_dir,
            ) {
                println!("zero dev — reload: {rel_path}");
                bus.send(rel_path);
            }
        }
    });

    Ok(Some(WatchHandle {
        _debouncer: debouncer,
    }))
}

/// Returns `true` if `path` should be filtered out of reload events.
/// Filters: any path component starting with `.`, or any path under `out_dir`.
pub fn is_ignored(path: &Path, _root: &Path, out_dir: &Path) -> bool {
    if path.starts_with(out_dir) {
        return true;
    }
    path.components().any(|c| match c {
        Component::Normal(s) => s.to_string_lossy().starts_with('.'),
        _ => false,
    })
}

/// Pick the first non-ignored event path and render it relative to `root`.
///
/// Used by the watcher loop to decide what to broadcast on the reload bus.
/// Extracted from `start` so its logic is unit-testable without spinning up
/// a real `notify` watcher.
pub fn representative_path<'a, I>(events: I, root: &Path, out_dir: &Path) -> Option<String>
where
    I: IntoIterator<Item = &'a Path>,
{
    events.into_iter().find_map(|p| {
        if is_ignored(p, root, out_dir) {
            None
        } else {
            Some(relative_to_root(p, root))
        }
    })
}

/// Renders `path` relative to `root` as a forward-slash string.
pub fn relative_to_root(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_hidden_dotfiles() {
        assert!(is_ignored(
            Path::new("/x/.git/HEAD"),
            Path::new("/x"),
            Path::new("/x/dist")
        ));
    }

    #[test]
    fn ignored_under_out_dir() {
        assert!(is_ignored(
            Path::new("/x/dist/asset.js"),
            Path::new("/x"),
            Path::new("/x/dist")
        ));
    }

    #[test]
    fn not_ignored_normal_source_path() {
        assert!(!is_ignored(
            Path::new("/x/src/routes/home.js"),
            Path::new("/x"),
            Path::new("/x/dist")
        ));
    }

    #[test]
    fn not_ignored_when_out_dir_outside_root() {
        assert!(!is_ignored(
            Path::new("/x/src/app.js"),
            Path::new("/x"),
            Path::new("/y/dist")
        ));
    }

    #[test]
    fn relative_to_root_strips_prefix() {
        assert_eq!(
            relative_to_root(Path::new("/x/src/a.js"), Path::new("/x")),
            "src/a.js"
        );
    }

    #[test]
    fn relative_to_root_falls_back_to_filename_when_outside_root() {
        assert_eq!(
            relative_to_root(Path::new("/y/other/a.js"), Path::new("/x")),
            "a.js"
        );
    }

    #[test]
    fn representative_path_picks_first_non_ignored() {
        let root = PathBuf::from("/x");
        let out = PathBuf::from("/x/dist");
        let events = [
            PathBuf::from("/x/dist/asset.js"),
            PathBuf::from("/x/.git/HEAD"),
            PathBuf::from("/x/src/a.js"),
            PathBuf::from("/x/src/b.js"),
        ];
        let got = representative_path(events.iter().map(|p| p.as_path()), &root, &out);
        assert_eq!(got.as_deref(), Some("src/a.js"));
    }

    #[test]
    fn representative_path_returns_none_when_all_ignored() {
        let root = PathBuf::from("/x");
        let out = PathBuf::from("/x/dist");
        let events = [PathBuf::from("/x/dist/a.js"), PathBuf::from("/x/.git/HEAD")];
        let got = representative_path(events.iter().map(|p| p.as_path()), &root, &out);
        assert!(got.is_none());
    }

    #[test]
    fn representative_path_returns_none_on_empty_input() {
        let got = representative_path(
            std::iter::empty::<&Path>(),
            Path::new("/x"),
            Path::new("/x/dist"),
        );
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn start_with_valid_root_returns_some_handle() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let out_dir = root.join("dist");
        std::fs::create_dir_all(&out_dir).unwrap();
        let bus = Arc::new(ReloadBus::new());
        let handle = start(root, out_dir, bus).unwrap();
        assert!(handle.is_some());
    }

    #[tokio::test]
    async fn start_with_missing_root_returns_none_and_disables_autoreload() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("does_not_exist");
        let out_dir = tmp.path().join("dist");
        std::fs::create_dir_all(&out_dir).unwrap();
        let bus = Arc::new(ReloadBus::new());
        let handle = start(root, out_dir, bus).unwrap();
        // notify fails to watch a missing path; start() logs and yields None.
        assert!(handle.is_none());
    }
}
