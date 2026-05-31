//! Module resolution for the `zero test` harness, and the QuickJS (rquickjs)
//! resolver/loader pair that drives it. Resolves `"zero"`, `"zero/test"`,
//! `"zero/http"`, `"zero/components"`, and relative file paths.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{Ctx, Module};

use zero_runtime::{http_module, runtime_module, test_module};
use zero_transpile::{TranspileOptions, transpile_typescript};

use crate::coverage::{self, CoverageMap, CoverageScope};

/// Coverage instrumentation context held by the loader. Files in scope are
/// transformed via [`coverage::instrument`] on resolution, and the resulting
/// [`CoverageMap`]s are collected here.
pub struct CoverageContext {
    pub scope: CoverageScope,
    maps: RefCell<Vec<CoverageMap>>,
}

impl CoverageContext {
    /// Create a new context with the given scope.
    pub fn new(scope: CoverageScope) -> Self {
        Self {
            scope,
            maps: RefCell::new(Vec::new()),
        }
    }

    /// Drain the maps collected so far.
    pub fn drain_maps(&self) -> Vec<CoverageMap> {
        std::mem::take(&mut self.maps.borrow_mut())
    }

    fn record(&self, map: CoverageMap) {
        self.maps.borrow_mut().push(map);
    }
}

/// Result of resolving a module specifier: the module `name` (QuickJS cache key
/// and child-import base), the JS `source` to declare, and the `canonical`
/// path. Produced by [`ZeroModuleLoader::resolve_source`].
pub struct ResolvedModule {
    pub name: String,
    pub source: String,
    pub canonical: PathBuf,
}

/// Module loader for the `zero test` harness.
///
/// Resolves `"zero"` and `"zero/test"` from in-memory strings, and relative
/// specifiers (`./`, `../`) from disk, refusing anything that escapes the
/// project root. All other bare specifiers are rejected.
pub struct ZeroModuleLoader {
    root: PathBuf,
    runtime_src: String,
    test_src: String,
    http_src: String,
    /// Side table: module path string → absolute PathBuf (used to resolve relative imports).
    path_map: RefCell<HashMap<String, PathBuf>>,
    /// Optional coverage instrumentation context.
    coverage: Option<Rc<CoverageContext>>,
    /// Optional per-file overlay: canonical absolute path → pre-transpiled JS
    /// source. Used by `zero mutate` to serve mutated code without re-running
    /// SWC.
    overlay: HashMap<PathBuf, String>,
}

impl ZeroModuleLoader {
    /// Create a new loader rooted at `root`.
    ///
    /// # Parameters
    /// - `root`: absolute path to the project root.
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            runtime_src: runtime_module(),
            test_src: test_module(),
            http_src: http_module(),
            path_map: RefCell::new(HashMap::new()),
            coverage: None,
            overlay: HashMap::new(),
        }
    }

    /// Create a loader that runs coverage instrumentation on in-scope files.
    pub fn new_with_coverage(root: &Path, ctx: Rc<CoverageContext>) -> Self {
        let mut loader = Self::new(root);
        loader.coverage = Some(ctx);
        loader
    }

    /// Attach a per-file overlay (canonical path → pre-transpiled JS).
    ///
    /// During relative resolution, paths in `overlay` short-circuit the SWC
    /// transpile and use the supplied JS as-is. Used by `zero mutate` to
    /// serve mutated source for one file while every other module is loaded
    /// normally.
    pub fn with_overlay(mut self, overlay: HashMap<PathBuf, String>) -> Self {
        self.overlay = overlay;
        self
    }

    /// The project root this loader is sandboxed to. Used by the QuickJS
    /// resolver to anchor `zero/components` and fall back for path-less bases.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Register an entry-point file path so the loader can resolve relative imports from it.
    ///
    /// Call this before evaluating the test file so the loader knows its directory.
    pub fn register_path(&self, key: &str, path: &Path) {
        self.path_map
            .borrow_mut()
            .insert(key.to_string(), path.to_path_buf());
    }

    /// Snapshot every distinct canonical path resolved by this loader, sorted.
    /// Used by `zero mutate` to build the `src_file → tests-that-load-it` map.
    pub fn loaded_paths(&self) -> Vec<PathBuf> {
        let m = self.path_map.borrow();
        let mut out: Vec<PathBuf> = m.values().cloned().collect();
        out.sort();
        out.dedup();
        out
    }

    /// Module resolution policy. Given a specifier and the referrer's directory,
    /// return the source to evaluate plus the module name and canonical path.
    /// Called by [`ZeroLoader`] (the QuickJS loader half).
    ///
    /// Records resolved file paths into `path_map` (for [`loaded_paths`]) and
    /// instruments in-scope files for coverage.
    ///
    /// [`loaded_paths`]: ZeroModuleLoader::loaded_paths
    pub fn resolve_source(
        &self,
        spec: &str,
        referrer_dir: &Path,
    ) -> Result<ResolvedModule, String> {
        match spec {
            "zero" => Ok(ResolvedModule {
                name: "zero".to_string(),
                source: self.runtime_src.clone(),
                canonical: PathBuf::from("zero"),
            }),
            "zero/test" => Ok(ResolvedModule {
                name: "zero/test".to_string(),
                source: self.test_src.clone(),
                canonical: PathBuf::from("zero/test"),
            }),
            "zero/http" => Ok(ResolvedModule {
                name: "zero/http".to_string(),
                source: self.http_src.clone(),
                canonical: PathBuf::from("zero/http"),
            }),
            "zero/components" => self.resolve_components_source(),
            // The QuickJS resolver hands `resolve_source` already-canonicalized
            // absolute paths (it resolves the path before loading); relative
            // specifiers are also accepted. Both go through the file path —
            // `referrer_dir.join(abs)` yields `abs`.
            s if s.starts_with("./") || s.starts_with("../") || Path::new(s).is_absolute() => {
                self.resolve_relative_source(s, referrer_dir)
            }
            _ => Err(format!("unsupported module specifier: \"{spec}\"")),
        }
    }

    /// Engine-agnostic resolution of a relative specifier against `referrer_dir`.
    /// Canonicalizes, enforces the project-root sandbox, applies the mutate
    /// overlay or transpile/instrument, and records the path for `loaded_paths`.
    fn resolve_relative_source(
        &self,
        spec: &str,
        referrer_dir: &Path,
    ) -> Result<ResolvedModule, String> {
        let candidate = referrer_dir.join(spec);
        let canonical = candidate
            .canonicalize()
            .map_err(|e| format!("cannot resolve \"{spec}\": {e}"))?;

        if !canonical.starts_with(&self.root) {
            return Err(format!(
                "path escape: \"{spec}\" resolves outside the project root"
            ));
        }

        let key = canonical.to_string_lossy().into_owned();

        // Overlay short-circuit: pre-transpiled JS (e.g. from `zero mutate`).
        if let Some(js) = self.overlay.get(&canonical) {
            self.path_map
                .borrow_mut()
                .insert(key.clone(), canonical.clone());
            return Ok(ResolvedModule {
                name: key,
                source: js.clone(),
                canonical,
            });
        }

        let raw = fs::read_to_string(&canonical)
            .map_err(|e| format!("cannot read \"{}\": {e}", canonical.display()))?;

        let logical = canonical.to_string_lossy().into_owned();
        let instrument_cov = self
            .coverage
            .as_ref()
            .filter(|c| c.scope.covers(&canonical))
            .cloned();
        let t_start = crate::timing::start();
        let source = if let Some(cov) = instrument_cov {
            let opts = TranspileOptions {
                filename: &logical,
                inline_source_map: false,
                emit_source_map: false,
            };
            match coverage::instrument(&raw, &opts) {
                Ok(out) => {
                    cov.record(out.map);
                    out.code
                }
                Err(e) => {
                    return Err(format!(
                        "coverage instrument error in {}: {e}",
                        canonical.display()
                    ));
                }
            }
        } else if canonical.extension().and_then(|e| e.to_str()) == Some("ts") {
            match transpile_typescript(
                &raw,
                &TranspileOptions {
                    filename: &logical,
                    inline_source_map: false,
                    emit_source_map: false,
                },
            ) {
                Ok(out) => out.code,
                Err(e) => {
                    return Err(format!("transpile error in {}: {e}", canonical.display()));
                }
            }
        } else {
            raw
        };
        crate::timing::record_since(crate::timing::Phase::Transpile, t_start);

        self.path_map
            .borrow_mut()
            .insert(key.clone(), canonical.clone());

        Ok(ResolvedModule {
            name: key,
            source,
            canonical,
        })
    }

    /// Engine-agnostic resolution of the `"zero/components"` index. The cache
    /// `name` stays `"zero/components"` while `canonical` is the on-disk index
    /// path (so its own `./Foo.ts` imports resolve relatively).
    fn resolve_components_source(&self) -> Result<ResolvedModule, String> {
        let path = self.root.join(".zero").join("components").join("index.ts");

        let canonical = path
            .canonicalize()
            .map_err(|e| format!("cannot resolve \"zero/components\": {e}"))?;

        if !canonical.starts_with(&self.root) {
            return Err(
                "path escape: \"zero/components\" resolves outside the project root".to_string(),
            );
        }

        let raw = fs::read_to_string(&canonical)
            .map_err(|e| format!("cannot read \"{}\": {e}", canonical.display()))?;

        let logical = canonical.to_string_lossy().into_owned();
        let source = match transpile_typescript(
            &raw,
            &TranspileOptions {
                filename: &logical,
                inline_source_map: false,
                emit_source_map: false,
            },
        ) {
            Ok(out) => out.code,
            Err(e) => {
                return Err(format!("transpile error in {}: {e}", canonical.display()));
            }
        };

        self.path_map
            .borrow_mut()
            .insert("zero/components".to_string(), canonical.clone());

        Ok(ResolvedModule {
            name: "zero/components".to_string(),
            source,
            canonical,
        })
    }
}

/// Resolver half of the QuickJS loader: turns a `(base, specifier)` pair into a
/// resolved module name (bare `zero*` pass through; `zero/components` and
/// relative specifiers become canonical absolute paths).
pub struct ZeroResolver {
    loader: Rc<ZeroModuleLoader>,
}

impl ZeroResolver {
    /// Create a resolver backed by the shared [`ZeroModuleLoader`].
    pub fn new(loader: Rc<ZeroModuleLoader>) -> Self {
        Self { loader }
    }
}

/// Loader half of the QuickJS loader: reads + transpiles the resolved name via
/// [`ZeroModuleLoader::resolve_source`] and declares the module.
pub struct ZeroLoader {
    loader: Rc<ZeroModuleLoader>,
}

impl ZeroLoader {
    /// Create a loader backed by the shared [`ZeroModuleLoader`].
    pub fn new(loader: Rc<ZeroModuleLoader>) -> Self {
        Self { loader }
    }
}

/// Canonicalize a path into a `String` module name.
fn abs(p: &Path) -> rquickjs::Result<String> {
    p.canonicalize()
        .map(|c| c.to_string_lossy().into_owned())
        .map_err(|e| rquickjs::Error::new_loading(format!("resolve {}: {e}", p.display())))
}

impl Resolver for ZeroResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attrs: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        match name {
            "zero" | "zero/test" | "zero/http" => Ok(name.to_string()),
            // The component index lives on disk; resolve to its absolute path so
            // its own `./Foo.ts` imports resolve relative to the index's dir.
            "zero/components" => abs(&self
                .loader
                .root()
                .join(".zero")
                .join("components")
                .join("index.ts")),
            s if s.starts_with("./") || s.starts_with("../") => {
                let base_dir = Path::new(base)
                    .parent()
                    .unwrap_or_else(|| self.loader.root());
                abs(&base_dir.join(s))
            }
            other => Err(rquickjs::Error::new_resolving(
                base.to_string(),
                format!("unsupported specifier: {other}"),
            )),
        }
    }
}

impl Loader for ZeroLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attrs: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, Declared>> {
        // `name` is already resolved: a bare `zero*` specifier or a canonical
        // absolute path. For abs paths `referrer_dir` is irrelevant (join with
        // an absolute path discards the base), so the root is a fine anchor.
        let resolved = self
            .loader
            .resolve_source(name, self.loader.root())
            .map_err(rquickjs::Error::new_loading)?;
        Module::declare(ctx.clone(), name, resolved.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_root() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    use rquickjs::promise::PromiseState;
    use rquickjs::{Context, Runtime};

    /// Evaluate `src` as an entry module (named after `entry_path`, so relative
    /// imports resolve from its dir) through the qjs loader rooted at `dir`.
    /// Returns Ok if the module resolves, Err on rejection.
    fn run_entry(dir: &Path, entry_path: &Path, src: &str) -> Result<(), String> {
        run_entry_with_loader(Rc::new(ZeroModuleLoader::new(dir)), entry_path, src)
    }

    /// Like [`run_entry`] but with a caller-supplied loader (e.g. with an overlay).
    fn run_entry_with_loader(
        loader: Rc<ZeroModuleLoader>,
        entry_path: &Path,
        src: &str,
    ) -> Result<(), String> {
        let rt = Runtime::new().map_err(|e| format!("runtime: {e}"))?;
        rt.set_loader(
            ZeroResolver::new(loader.clone()),
            ZeroLoader::new(loader.clone()),
        );
        let ctx = Context::full(&rt).map_err(|e| format!("context: {e}"))?;
        let name = entry_path.to_string_lossy().into_owned();
        ctx.with(|ctx| {
            let promise =
                Module::evaluate(ctx.clone(), name, src).map_err(|e| format!("declare: {e}"))?;
            while ctx.execute_pending_job() {}
            match promise.state() {
                PromiseState::Resolved => Ok(()),
                PromiseState::Rejected => Err("module evaluation rejected".to_string()),
                PromiseState::Pending => Err("module still pending after drain".to_string()),
            }
        })
    }

    #[test]
    fn resolve_source_returns_zero_test_module() {
        let dir = make_root();
        let loader = ZeroModuleLoader::new(dir.path());
        let resolved = loader
            .resolve_source("zero/test", dir.path())
            .expect("resolve zero/test");
        assert_eq!(resolved.name, "zero/test");
        assert!(
            resolved.source.contains("function describe("),
            "zero/test source should define describe"
        );
    }

    #[test]
    fn resolves_zero_and_signal() {
        let dir = make_root();
        let entry = dir.path().join("entry.js");
        run_entry(
            dir.path(),
            &entry,
            "import { signal } from 'zero'; export { signal };",
        )
        .expect("zero import");
    }

    #[test]
    fn resolves_zero_test_and_has_describe() {
        let dir = make_root();
        let entry = dir.path().join("entry.js");
        run_entry(dir.path(), &entry, "import { describe } from 'zero/test';")
            .expect("zero/test import");
    }

    #[test]
    fn resolves_relative_file() {
        let dir = make_root();
        std::fs::write(dir.path().join("foo.js"), b"export const x = 42;").unwrap();
        let entry = dir.path().join("entry.js");
        run_entry(
            dir.path(),
            &entry,
            "import { x } from './foo.js'; if (x !== 42) throw new Error('nope');",
        )
        .expect("relative import");
    }

    #[test]
    fn resolves_relative_ts_file() {
        let dir = make_root();
        std::fs::write(dir.path().join("foo.ts"), b"export const x: number = 42;").unwrap();
        let entry = dir.path().join("entry.js");
        run_entry(
            dir.path(),
            &entry,
            "import { x } from './foo.ts'; if (x !== 42) throw new Error('nope');",
        )
        .expect("ts relative import");
    }

    #[test]
    fn parse_error_in_ts_dependency_surfaces() {
        let dir = make_root();
        std::fs::write(dir.path().join("foo.ts"), b"const x: = ;").unwrap();
        let entry = dir.path().join("entry.js");
        let res = run_entry(dir.path(), &entry, "import { } from './foo.ts';");
        assert!(res.is_err(), "expected rejection on bad TS, got: {res:?}");
    }

    #[test]
    fn resolves_zero_components_module() {
        let dir = make_root();
        let components = dir.path().join(".zero").join("components");
        std::fs::create_dir_all(&components).unwrap();
        std::fs::write(
            components.join("index.ts"),
            b"export const placeholder: number = 1;",
        )
        .unwrap();
        let entry = dir.path().join("entry.js");
        run_entry(
            dir.path(),
            &entry,
            "import { placeholder } from 'zero/components'; if (placeholder !== 1) throw new Error('nope');",
        )
        .expect("zero/components import");
    }

    #[test]
    fn resolves_zero_http_module() {
        let dir = make_root();
        let entry = dir.path().join("entry.js");
        run_entry(
            dir.path(),
            &entry,
            "import { createHttp } from 'zero/http';",
        )
        .expect("zero/http import");
    }

    #[test]
    fn overlay_short_circuits_transpile() {
        // The overlay should serve a pre-transpiled JS body for the
        // matching canonical path instead of reading the on-disk file.
        let dir = make_root();
        let foo_path = dir.path().join("foo.js");
        std::fs::write(&foo_path, b"export const x = 1;").unwrap();
        let canonical = foo_path.canonicalize().unwrap();

        let mut overlay: HashMap<PathBuf, String> = HashMap::new();
        overlay.insert(canonical, "export const x = 99;".to_string());

        let loader = Rc::new(ZeroModuleLoader::new(dir.path()).with_overlay(overlay));
        let entry = dir.path().join("entry.js");
        run_entry_with_loader(
            loader,
            &entry,
            "import { x } from './foo.js'; if (x !== 99) throw new Error('wanted 99, got ' + x);",
        )
        .expect("overlay should override on-disk source");
    }

    #[test]
    fn rejects_bare_unknown_specifier() {
        let dir = make_root();
        let entry = dir.path().join("entry.js");
        let res = run_entry(dir.path(), &entry, "import 'lodash';");
        assert!(res.is_err(), "expected rejection for unknown specifier");
    }
}
