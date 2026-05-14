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
            let representative = events.iter().find_map(|ev| {
                let p = &ev.path;
                if is_ignored(p, &root_for_thread, &out_dir) {
                    None
                } else {
                    Some(relative_to_root(p, &root_for_thread))
                }
            });
            if let Some(rel_path) = representative {
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
}
