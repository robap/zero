//! Custom Boa module loader that resolves `"zero"`, `"zero/test"`, and relative file paths.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use boa_engine::module::{ModuleLoader, Referrer};
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsString, Module, Source};

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
    /// Cache: canonical path string → parsed Module (avoids double-parse within one context).
    module_cache: RefCell<HashMap<String, Module>>,
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
            module_cache: RefCell::new(HashMap::new()),
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
    /// During `resolve_relative`, paths in `overlay` short-circuit the SWC
    /// transpile and use the supplied JS as-is. Used by `zero mutate` to
    /// serve mutated source for one file while every other module is loaded
    /// normally.
    pub fn with_overlay(mut self, overlay: HashMap<PathBuf, String>) -> Self {
        self.overlay = overlay;
        self
    }

    /// Register an entry-point file path so the loader can resolve relative imports from it.
    ///
    /// Call this before evaluating the test file so the loader knows its directory.
    pub fn register_path(&self, key: &str, path: &Path) {
        self.path_map
            .borrow_mut()
            .insert(key.to_string(), path.to_path_buf());
    }

    /// Retrieve a cached module by cache key.
    pub fn get_cached(&self, key: &str) -> Option<Module> {
        self.module_cache.borrow().get(key).cloned()
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

    /// Resolve a relative specifier against the referrer's directory.
    ///
    /// Returns `Err` if the specifier escapes the project root or the file cannot be read.
    fn resolve_relative(
        &self,
        spec: &str,
        referrer: &Referrer,
        context: &mut Context,
    ) -> JsResult<Module> {
        let base_dir = self.referrer_dir(referrer);
        let candidate = base_dir.join(spec);

        let canonical = candidate.canonicalize().map_err(|e| {
            JsError::from_native(
                JsNativeError::error().with_message(format!("cannot resolve \"{spec}\": {e}")),
            )
        })?;

        if !canonical.starts_with(&self.root) {
            return Err(JsError::from_native(JsNativeError::error().with_message(
                format!("path escape: \"{spec}\" resolves outside the project root"),
            )));
        }

        let key = canonical.to_string_lossy().into_owned();

        // Return cached module if available.
        if let Some(m) = self.module_cache.borrow().get(&key).cloned() {
            return Ok(m);
        }

        // Overlay short-circuit: if this canonical path has pre-transpiled JS
        // attached (e.g. from `zero mutate`), use it as-is.
        if let Some(js) = self.overlay.get(&canonical) {
            let module = Module::parse(
                Source::from_bytes(js.as_bytes()).with_path(&canonical),
                None,
                context,
            )?;
            self.module_cache
                .borrow_mut()
                .insert(key.clone(), module.clone());
            self.path_map.borrow_mut().insert(key, canonical);
            return Ok(module);
        }

        let raw = fs::read_to_string(&canonical).map_err(|e| {
            JsError::from_native(
                JsNativeError::error()
                    .with_message(format!("cannot read \"{}\": {e}", canonical.display())),
            )
        })?;

        // Decide whether to instrument (coverage) or just transpile.
        let logical = canonical.to_string_lossy().into_owned();
        let instrument_cov = self
            .coverage
            .as_ref()
            .filter(|c| c.scope.covers(&canonical))
            .cloned();
        let t_start = crate::timing::start();
        let src = if let Some(cov) = instrument_cov {
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
                    return Err(JsError::from_native(JsNativeError::error().with_message(
                        format!("coverage instrument error in {}: {e}", canonical.display()),
                    )));
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
                    return Err(JsError::from_native(JsNativeError::error().with_message(
                        format!("transpile error in {}: {e}", canonical.display()),
                    )));
                }
            }
        } else {
            raw
        };
        crate::timing::record_since(crate::timing::Phase::Transpile, t_start);

        let module = Module::parse(
            Source::from_bytes(src.as_bytes()).with_path(&canonical),
            None,
            context,
        )?;

        self.module_cache
            .borrow_mut()
            .insert(key.clone(), module.clone());
        self.path_map.borrow_mut().insert(key, canonical);

        Ok(module)
    }

    /// Resolve the bare specifier `"zero/components"` to the user-facing
    /// component index at `<root>/.zero/components/index.ts`. The index file's
    /// own relative imports (e.g. `./Button.ts`) are resolved by Boa against
    /// the module's parsed path, so no synthetic referrer is needed.
    fn resolve_components_index(&self, context: &mut Context) -> JsResult<Module> {
        let path = self.root.join(".zero").join("components").join("index.ts");

        let canonical = path.canonicalize().map_err(|e| {
            JsError::from_native(
                JsNativeError::error()
                    .with_message(format!("cannot resolve \"zero/components\": {e}")),
            )
        })?;

        if !canonical.starts_with(&self.root) {
            return Err(JsError::from_native(JsNativeError::error().with_message(
                "path escape: \"zero/components\" resolves outside the project root".to_string(),
            )));
        }

        let key = "zero/components".to_string();
        if let Some(m) = self.module_cache.borrow().get(&key).cloned() {
            return Ok(m);
        }

        let raw = fs::read_to_string(&canonical).map_err(|e| {
            JsError::from_native(
                JsNativeError::error()
                    .with_message(format!("cannot read \"{}\": {e}", canonical.display())),
            )
        })?;

        let logical = canonical.to_string_lossy().into_owned();
        let src = match transpile_typescript(
            &raw,
            &TranspileOptions {
                filename: &logical,
                inline_source_map: false,
                emit_source_map: false,
            },
        ) {
            Ok(out) => out.code,
            Err(e) => {
                return Err(JsError::from_native(JsNativeError::error().with_message(
                    format!("transpile error in {}: {e}", canonical.display()),
                )));
            }
        };

        let module = Module::parse(
            Source::from_bytes(src.as_bytes()).with_path(&canonical),
            None,
            context,
        )?;

        self.module_cache
            .borrow_mut()
            .insert(key.clone(), module.clone());
        self.path_map.borrow_mut().insert(key, canonical);

        Ok(module)
    }

    /// Return the directory of the referrer (for resolving relative specifiers).
    fn referrer_dir(&self, referrer: &Referrer) -> PathBuf {
        match referrer {
            Referrer::Module(m) => {
                // Look up the module path we registered for this module.
                // Boa modules are GC objects; we match by path string stored in path_map
                // by comparing the module's path() if available.
                if let Some(path) = m.path()
                    && let Some(parent) = path.parent()
                {
                    return parent.to_path_buf();
                }
                self.root.clone()
            }
            _ => self.root.clone(),
        }
    }
}

impl ModuleLoader for ZeroModuleLoader {
    fn load_imported_module(
        self: std::rc::Rc<Self>,
        referrer: Referrer,
        specifier: JsString,
        context: &std::cell::RefCell<&mut Context>,
    ) -> impl std::future::Future<Output = JsResult<Module>> {
        let spec = specifier.to_std_string_escaped();

        let result: JsResult<Module> = (|| {
            if let Some(m) = self.module_cache.borrow().get(&spec).cloned() {
                return Ok(m);
            }

            match spec.as_str() {
                "zero" => {
                    let mut ctx = context.borrow_mut();
                    let m = Module::parse(
                        Source::from_bytes(self.runtime_src.as_bytes()),
                        None,
                        &mut ctx,
                    )?;
                    self.module_cache
                        .borrow_mut()
                        .insert("zero".to_string(), m.clone());
                    Ok(m)
                }
                "zero/test" => {
                    let mut ctx = context.borrow_mut();
                    let m = Module::parse(
                        Source::from_bytes(self.test_src.as_bytes()),
                        None,
                        &mut ctx,
                    )?;
                    self.module_cache
                        .borrow_mut()
                        .insert("zero/test".to_string(), m.clone());
                    Ok(m)
                }
                "zero/http" => {
                    let mut ctx = context.borrow_mut();
                    let m = Module::parse(
                        Source::from_bytes(self.http_src.as_bytes()),
                        None,
                        &mut ctx,
                    )?;
                    self.module_cache
                        .borrow_mut()
                        .insert("zero/http".to_string(), m.clone());
                    Ok(m)
                }
                "zero/components" => {
                    let mut ctx = context.borrow_mut();
                    self.resolve_components_index(&mut ctx)
                }
                s if s.starts_with("./") || s.starts_with("../") => {
                    let mut ctx = context.borrow_mut();
                    self.resolve_relative(s, &referrer, &mut ctx)
                }
                _ => Err(JsError::from_native(JsNativeError::error().with_message(
                    format!("unsupported module specifier: \"{spec}\""),
                ))),
            }
        })();

        async { result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_root() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn make_context_with_loader(
        loader: ZeroModuleLoader,
    ) -> (Context, std::rc::Rc<ZeroModuleLoader>) {
        use std::rc::Rc;
        let rc = Rc::new(loader);
        let ctx = Context::builder()
            .module_loader(rc.clone())
            .build()
            .expect("failed to build context");
        (ctx, rc)
    }

    #[test]
    fn resolves_zero_and_signal() {
        let dir = make_root();
        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, _loader) = make_context_with_loader(loader);

        // Parse "zero" module — should not error
        let m = Module::parse(
            Source::from_bytes(b"import { signal } from 'zero'; export { signal };"),
            None,
            &mut ctx,
        )
        .expect("failed to parse module");

        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();

        // Promise should be fulfilled (not rejected).
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "module evaluation rejected: {state:?}"
        );
    }

    #[test]
    fn resolves_zero_test_and_has_describe() {
        let dir = make_root();
        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, loader_rc) = make_context_with_loader(loader);

        let m = Module::parse(
            Source::from_bytes(b"import { describe } from 'zero/test';"),
            None,
            &mut ctx,
        )
        .expect("failed to parse");

        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "zero/test rejected: {state:?}"
        );

        // The zero/test module should be cached.
        assert!(
            loader_rc.get_cached("zero/test").is_some(),
            "zero/test not in cache after load"
        );
    }

    #[test]
    fn resolves_relative_file() {
        let dir = make_root();
        let foo_path = dir.path().join("foo.js");
        std::fs::write(&foo_path, b"export const x = 42;").unwrap();

        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, _) = make_context_with_loader(loader);

        // Entry module lives in the same dir; uses a relative `./foo.js` import.
        // We set the source path so the loader can resolve relative to this dir.
        let entry_path = dir.path().join("entry.js");
        let m = Module::parse(
            Source::from_bytes(b"import { x } from './foo.js';").with_path(&entry_path),
            None,
            &mut ctx,
        )
        .expect("failed to parse entry");
        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "relative import rejected: {state:?}"
        );
    }

    #[test]
    fn resolves_relative_ts_file() {
        let dir = make_root();
        let foo_path = dir.path().join("foo.ts");
        std::fs::write(&foo_path, b"export const x: number = 42;").unwrap();

        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, _) = make_context_with_loader(loader);

        let entry_path = dir.path().join("entry.js");
        let m = Module::parse(
            Source::from_bytes(
                b"import { x } from './foo.ts'; if (x !== 42) throw new Error('nope');",
            )
            .with_path(&entry_path),
            None,
            &mut ctx,
        )
        .expect("failed to parse entry");
        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "ts relative import rejected: {state:?}"
        );
    }

    #[test]
    fn parse_error_in_ts_dependency_surfaces() {
        let dir = make_root();
        let foo_path = dir.path().join("foo.ts");
        std::fs::write(&foo_path, b"const x: = ;").unwrap();

        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, _) = make_context_with_loader(loader);
        let entry_path = dir.path().join("entry.js");
        let m = Module::parse(
            Source::from_bytes(b"import { } from './foo.ts';").with_path(&entry_path),
            None,
            &mut ctx,
        )
        .expect("entry parse ok");
        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "expected rejection on bad TS, got: {state:?}"
        );
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

        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, loader_rc) = make_context_with_loader(loader);

        let m = Module::parse(
            Source::from_bytes(b"import { placeholder } from 'zero/components';"),
            None,
            &mut ctx,
        )
        .expect("failed to parse entry");

        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "zero/components rejected: {state:?}"
        );
        assert!(
            loader_rc.get_cached("zero/components").is_some(),
            "zero/components not in cache after load"
        );
    }

    #[test]
    fn resolves_zero_http_module() {
        let dir = make_root();
        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, loader_rc) = make_context_with_loader(loader);

        let m = Module::parse(
            Source::from_bytes(b"import { createHttp } from 'zero/http';"),
            None,
            &mut ctx,
        )
        .expect("failed to parse");

        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "zero/http rejected: {state:?}"
        );

        assert!(
            loader_rc.get_cached("zero/http").is_some(),
            "zero/http not in cache after load"
        );
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
        overlay.insert(canonical.clone(), "export const x = 99;".to_string());

        let loader = ZeroModuleLoader::new(dir.path()).with_overlay(overlay);
        let (mut ctx, _) = make_context_with_loader(loader);

        let entry_path = dir.path().join("entry.js");
        let m = Module::parse(
            Source::from_bytes(
                b"import { x } from './foo.js'; if (x !== 99) throw new Error('wanted 99, got ' + x);",
            )
            .with_path(&entry_path),
            None,
            &mut ctx,
        )
        .expect("entry parse");
        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            !matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "overlay did not override on-disk source: {state:?}"
        );
    }

    #[test]
    fn rejects_bare_unknown_specifier() {
        let dir = make_root();
        let loader = ZeroModuleLoader::new(dir.path());
        let (mut ctx, _) = make_context_with_loader(loader);

        let m = Module::parse(Source::from_bytes(b"import 'lodash';"), None, &mut ctx)
            .expect("parsed ok");
        let promise = m.load_link_evaluate(&mut ctx);
        let _ = ctx.run_jobs();
        let state = promise.state();
        assert!(
            matches!(
                state,
                boa_engine::builtins::promise::PromiseState::Rejected(_)
            ),
            "expected rejection for unknown specifier, got: {state:?}"
        );
    }
}
