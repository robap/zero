//! Fingerprint cache for `zero mutate` (`mutation/cache.json`).
//!
//! Records, per source file, the mutant verdicts of the previous run together
//! with a fingerprint over the file's full behavioral closure — the file
//! itself, every test that exercises it, and every module those tests load.
//! A later run reuses the verdicts when the fingerprint still matches, and
//! skips the baseline entirely when the whole recorded universe is
//! byte-identical. See `issues/mutate-cache/spec.md`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zero_test_runner::mutate::Operator;

use super::mutate::PerOperatorSummary;

/// Version of the `mutation/cache.json` schema. Any mismatch on read makes
/// the cache count as absent.
pub const CACHE_SCHEMA_VERSION: u64 = 2;

/// How `run_inner` interacts with `mutation/cache.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// Read (reuse + fast path) and write. Default `zero mutate`.
    ReadWrite,
    /// Ignore any existing cache, run everything, rewrite. `--no-cache`.
    Fresh,
    /// Neither read nor write. `--operators` / `--max-mutants` runs.
    Bypass,
}

/// In-memory form of `mutation/cache.json`. All path keys root-relative,
/// `/`-separated, sorted.
#[derive(Debug, Default)]
pub struct MutateCache {
    /// Root-relative paths of every discovered test file.
    pub test_files: Vec<String>,
    /// Root-relative paths of every in-scope source file.
    pub src_files: Vec<String>,
    /// Every file in any entry's closure (∪ tests ∪ srcs) → sha256 hex.
    pub files: BTreeMap<String, String>,
    /// Per-source-file cached verdicts, keyed root-relative.
    pub entries: BTreeMap<String, CacheEntry>,
}

/// Cached verdicts and accounting for one source file.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Sha256 over the file's closure (see [`fingerprint`]).
    pub fingerprint: String,
    /// Executed verdicts, in recorded order (includes apply-errored sites).
    pub sites: Vec<CachedSite>,
    /// This file's contribution to the per-operator summary.
    pub per_operator: PerOperatorSummary,
}

/// One recorded mutant verdict.
#[derive(Debug, Clone)]
pub struct CachedSite {
    pub line: u32,
    pub column: u32,
    /// `Operator::id()` string.
    pub operator: String,
    pub original: String,
    pub replacement: String,
    /// `"killed" | "survived" | "errored"`.
    pub status: String,
}

/// Sha256 over `path`'s raw bytes as lowercase hex, memoized per run.
/// Unreadable file ⇒ `None` (callers treat as a fingerprint miss).
pub fn hash_file(path: &Path, memo: &mut HashMap<PathBuf, Option<String>>) -> Option<String> {
    if let Some(cached) = memo.get(path) {
        return cached.clone();
    }
    let hashed = std::fs::read(path)
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes)));
    memo.insert(path.to_path_buf(), hashed.clone());
    hashed
}

/// Root-relative, `/`-separated key for `p`. Falls back to the lossy
/// absolute string if `p` is not under `root`.
pub fn rel_key(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Behavioral closure of `src`: the file itself, the tests that exercise it
/// (or *all* tests when `src_to_tests` lacks it, mirroring dispatch's
/// fallback), and every file those tests loaded.
pub fn closure_for(
    src: &Path,
    src_to_tests: &HashMap<PathBuf, Vec<PathBuf>>,
    test_loaded: &HashMap<PathBuf, Vec<PathBuf>>,
    all_tests: &[PathBuf],
) -> BTreeSet<PathBuf> {
    let relevant: &[PathBuf] = src_to_tests
        .get(src)
        .map(|v| v.as_slice())
        .unwrap_or(all_tests);
    let mut closure: BTreeSet<PathBuf> = BTreeSet::new();
    closure.insert(src.to_path_buf());
    for t in relevant {
        closure.insert(t.clone());
        if let Some(loaded) = test_loaded.get(t) {
            closure.extend(loaded.iter().cloned());
        }
    }
    closure
}

/// Sha256 over the sorted `rel_key\0hash\n` concatenation of `closure`.
/// `None` if any member fails to hash.
pub fn fingerprint(
    root: &Path,
    closure: &BTreeSet<PathBuf>,
    memo: &mut HashMap<PathBuf, Option<String>>,
) -> Option<String> {
    // BTreeSet iterates in path order; rel_key preserves that order well
    // enough for determinism (same closure ⇒ same concatenation).
    let mut blob = String::new();
    for p in closure {
        let h = hash_file(p, memo)?;
        blob.push_str(&rel_key(root, p));
        blob.push('\0');
        blob.push_str(&h);
        blob.push('\n');
    }
    Some(format!("{:x}", Sha256::digest(blob.as_bytes())))
}

/// Read `root/mutation/cache.json`. Any parse error, missing field, schema
/// version mismatch, or `cli_version` mismatch ⇒ `None`. Never errors,
/// never logs.
pub fn load(root: &Path, cli_version: &str) -> Option<MutateCache> {
    let raw = std::fs::read_to_string(root.join("mutation/cache.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    if v.get("schema_version")?.as_u64()? != CACHE_SCHEMA_VERSION {
        return None;
    }
    if v.get("cli_version")?.as_str()? != cli_version {
        return None;
    }
    let str_list = |key: &str| -> Option<Vec<String>> {
        v.get(key)?
            .as_array()?
            .iter()
            .map(|s| s.as_str().map(str::to_string))
            .collect()
    };
    let mut files = BTreeMap::new();
    for (k, h) in v.get("files")?.as_object()? {
        files.insert(k.clone(), h.as_str()?.to_string());
    }
    let mut entries = BTreeMap::new();
    for (k, e) in v.get("entries")?.as_object()? {
        entries.insert(k.clone(), entry_from_json(e)?);
    }
    Some(MutateCache {
        test_files: str_list("test_files")?,
        src_files: str_list("src_files")?,
        files,
        entries,
    })
}

/// Parse one cache entry; any missing/mistyped field ⇒ `None`.
fn entry_from_json(e: &serde_json::Value) -> Option<CacheEntry> {
    let mut sites = Vec::new();
    for s in e.get("sites")?.as_array()? {
        sites.push(CachedSite {
            line: u32::try_from(s.get("line")?.as_u64()?).ok()?,
            column: u32::try_from(s.get("column")?.as_u64()?).ok()?,
            operator: s.get("operator")?.as_str()?.to_string(),
            original: s.get("original")?.as_str()?.to_string(),
            replacement: s.get("replacement")?.as_str()?.to_string(),
            status: s.get("status")?.as_str()?.to_string(),
        });
    }
    Some(CacheEntry {
        fingerprint: e.get("fingerprint")?.as_str()?.to_string(),
        sites,
        per_operator: per_operator_from_json(e.get("per_operator")?)?,
    })
}

/// Parse the per-operator block (keyed by operator id, seven count fields).
fn per_operator_from_json(v: &serde_json::Value) -> Option<PerOperatorSummary> {
    let obj = v.as_object()?;
    let mut out = PerOperatorSummary::default();
    for op in Operator::ALL {
        let row = obj.get(op.id())?;
        let i = op.index();
        let count = |key: &str| -> Option<usize> { usize::try_from(row.get(key)?.as_u64()?).ok() };
        out.matched[i] = count("matched")?;
        out.unreachable[i] = count("unreachable")?;
        out.equivalent_byte[i] = count("equivalent_byte")?;
        out.equivalent_static[i] = count("equivalent_static")?;
        out.killed[i] = count("killed")?;
        // Additive field (cache schema v2): tolerate its absence so a
        // half-written or hand-edited cache still loads.
        out.timed_out[i] = count("timed_out").unwrap_or(0);
        out.survived[i] = count("survived")?;
        out.errored[i] = count("errored")?;
    }
    Some(out)
}

/// Serialize `cache` and write `root/mutation/cache.json` atomically-ish
/// (write `cache.json.tmp`, rename over).
pub fn save(root: &Path, cli_version: &str, cache: &MutateCache) -> std::io::Result<()> {
    let mut entries = serde_json::Map::new();
    for (k, e) in &cache.entries {
        entries.insert(k.clone(), entry_to_json(e));
    }
    let value = serde_json::json!({
        "schema_version": CACHE_SCHEMA_VERSION,
        "cli_version": cli_version,
        "test_files": cache.test_files,
        "src_files": cache.src_files,
        "files": cache.files,
        "entries": entries,
    });
    let dir = root.join("mutation");
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join("cache.json.tmp");
    let s = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into());
    std::fs::write(&tmp, s)?;
    std::fs::rename(&tmp, dir.join("cache.json"))
}

/// Serialize one cache entry.
fn entry_to_json(e: &CacheEntry) -> serde_json::Value {
    let sites: Vec<_> = e
        .sites
        .iter()
        .map(|s| {
            serde_json::json!({
                "line": s.line,
                "column": s.column,
                "operator": s.operator,
                "original": s.original,
                "replacement": s.replacement,
                "status": s.status,
            })
        })
        .collect();
    let mut per_op = serde_json::Map::new();
    for op in Operator::ALL {
        let i = op.index();
        per_op.insert(
            op.id().to_string(),
            serde_json::json!({
                "matched":           e.per_operator.matched[i],
                "unreachable":       e.per_operator.unreachable[i],
                "equivalent_byte":   e.per_operator.equivalent_byte[i],
                "equivalent_static": e.per_operator.equivalent_static[i],
                "killed":            e.per_operator.killed[i],
                "timed_out":         e.per_operator.timed_out[i],
                "survived":          e.per_operator.survived[i],
                "errored":           e.per_operator.errored[i],
            }),
        );
    }
    serde_json::json!({
        "fingerprint": e.fingerprint,
        "sites": sites,
        "per_operator": per_op,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_file_is_deterministic_and_memoized() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, "hello").unwrap();
        let mut memo = HashMap::new();
        let h1 = hash_file(&p, &mut memo).expect("hash");
        let h2 = hash_file(&p, &mut memo).expect("hash again");
        assert_eq!(h1, h2);
        // sha256("hello")
        assert_eq!(
            h1,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(memo.len(), 1, "second call must hit the memo");
    }

    #[test]
    fn rel_key_strips_root_and_normalizes_separators() {
        let root = Path::new("/proj");
        assert_eq!(rel_key(root, Path::new("/proj/src/a.ts")), "src/a.ts");
        // Path outside the root falls back to the lossy absolute string.
        assert_eq!(
            rel_key(root, Path::new("/elsewhere/b.ts")),
            "/elsewhere/b.ts"
        );
    }

    #[test]
    fn closure_includes_src_tests_and_loaded_modules() {
        let src = PathBuf::from("/proj/src/a.ts");
        let test = PathBuf::from("/proj/a.test.ts");
        let helper = PathBuf::from("/proj/src/helper.ts");
        let mut src_to_tests = HashMap::new();
        src_to_tests.insert(src.clone(), vec![test.clone()]);
        let mut test_loaded = HashMap::new();
        test_loaded.insert(
            test.clone(),
            vec![test.clone(), src.clone(), helper.clone()],
        );
        let all_tests = vec![test.clone()];

        let closure = closure_for(&src, &src_to_tests, &test_loaded, &all_tests);
        let want: BTreeSet<PathBuf> = [src, test, helper].into_iter().collect();
        assert_eq!(closure, want);
    }

    #[test]
    fn closure_falls_back_to_all_tests_when_unmapped() {
        // Mirrors dispatch's `unwrap_or(test_files)`: a source file no test
        // loaded is exercised by *every* test, so its closure must include
        // them all.
        let src = PathBuf::from("/proj/src/orphan.ts");
        let t1 = PathBuf::from("/proj/one.test.ts");
        let t2 = PathBuf::from("/proj/two.test.ts");
        let src_to_tests: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        let mut test_loaded = HashMap::new();
        test_loaded.insert(t1.clone(), vec![t1.clone()]);
        test_loaded.insert(t2.clone(), vec![t2.clone()]);
        let all_tests = vec![t1.clone(), t2.clone()];

        let closure = closure_for(&src, &src_to_tests, &test_loaded, &all_tests);
        let want: BTreeSet<PathBuf> = [src, t1, t2].into_iter().collect();
        assert_eq!(closure, want);
    }

    #[test]
    fn fingerprint_changes_on_content_and_membership() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let a = root.join("a.ts");
        let b = root.join("b.ts");
        std::fs::write(&a, "let x = 1;").unwrap();
        std::fs::write(&b, "let y = 2;").unwrap();

        let one: BTreeSet<PathBuf> = [a.clone()].into_iter().collect();
        let both: BTreeSet<PathBuf> = [a.clone(), b.clone()].into_iter().collect();

        let mut memo = HashMap::new();
        let fp_one = fingerprint(root, &one, &mut memo).expect("fp one");
        let fp_both = fingerprint(root, &both, &mut memo).expect("fp both");
        assert_ne!(fp_one, fp_both, "membership change must change the print");

        // Same closure, fresh memo => same fingerprint (deterministic).
        let mut memo2 = HashMap::new();
        assert_eq!(fingerprint(root, &both, &mut memo2).unwrap(), fp_both);

        // Content change to any member changes the print.
        std::fs::write(&b, "let y = 3;").unwrap();
        let mut memo3 = HashMap::new();
        let fp_edited = fingerprint(root, &both, &mut memo3).expect("fp edited");
        assert_ne!(fp_both, fp_edited, "content change must change the print");
    }

    #[test]
    fn fingerprint_unreadable_member_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let a = root.join("a.ts");
        std::fs::write(&a, "let x = 1;").unwrap();
        let closure: BTreeSet<PathBuf> = [a, root.join("missing.ts")].into_iter().collect();
        let mut memo = HashMap::new();
        assert_eq!(fingerprint(root, &closure, &mut memo), None);
    }

    /// A two-entry cache exercising every field.
    fn sample_cache() -> MutateCache {
        let mut per_op = PerOperatorSummary::default();
        per_op.matched[0] = 3;
        per_op.unreachable[0] = 1;
        per_op.equivalent_byte[1] = 2;
        per_op.equivalent_static[2] = 4;
        per_op.killed[0] = 2;
        per_op.timed_out[0] = 1;
        per_op.survived[3] = 1;
        per_op.errored[7] = 5;
        let mut entries = BTreeMap::new();
        entries.insert(
            "src/a.ts".to_string(),
            CacheEntry {
                fingerprint: "f".repeat(64),
                sites: vec![
                    CachedSite {
                        line: 3,
                        column: 14,
                        operator: "arith".into(),
                        original: "a + b".into(),
                        replacement: "a - b".into(),
                        status: "killed".into(),
                    },
                    CachedSite {
                        line: 4,
                        column: 5,
                        operator: "arith".into(),
                        original: "i += 1".into(),
                        replacement: "i -= 1".into(),
                        status: "timed-out".into(),
                    },
                ],
                per_operator: per_op,
            },
        );
        entries.insert(
            "src/b.ts".to_string(),
            CacheEntry {
                fingerprint: "0".repeat(64),
                sites: vec![],
                per_operator: PerOperatorSummary::default(),
            },
        );
        let mut files = BTreeMap::new();
        files.insert("a.test.ts".to_string(), "1".repeat(64));
        files.insert("src/a.ts".to_string(), "2".repeat(64));
        files.insert("src/b.ts".to_string(), "3".repeat(64));
        MutateCache {
            test_files: vec!["a.test.ts".into()],
            src_files: vec!["src/a.ts".into(), "src/b.ts".into()],
            files,
            entries,
        }
    }

    #[test]
    fn save_load_roundtrip_preserves_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cache = sample_cache();
        save(root, "0.1.0", &cache).expect("save");
        assert!(root.join("mutation/cache.json").is_file());

        let loaded = load(root, "0.1.0").expect("load");
        assert_eq!(loaded.test_files, cache.test_files);
        assert_eq!(loaded.src_files, cache.src_files);
        assert_eq!(loaded.files, cache.files);
        assert_eq!(loaded.entries.len(), 2);

        let a = &loaded.entries["src/a.ts"];
        let orig_a = &cache.entries["src/a.ts"];
        assert_eq!(a.fingerprint, orig_a.fingerprint);
        assert_eq!(a.sites.len(), 2);
        let s = &a.sites[0];
        assert_eq!((s.line, s.column, s.operator.as_str()), (3, 14, "arith"));
        assert_eq!(
            (
                s.original.as_str(),
                s.replacement.as_str(),
                s.status.as_str()
            ),
            ("a + b", "a - b", "killed")
        );
        // A timed-out site survives save → load.
        assert_eq!(a.sites[1].status.as_str(), "timed-out");
        for i in 0..8 {
            assert_eq!(a.per_operator.matched[i], orig_a.per_operator.matched[i]);
            assert_eq!(
                a.per_operator.unreachable[i],
                orig_a.per_operator.unreachable[i]
            );
            assert_eq!(
                a.per_operator.equivalent_byte[i],
                orig_a.per_operator.equivalent_byte[i]
            );
            assert_eq!(
                a.per_operator.equivalent_static[i],
                orig_a.per_operator.equivalent_static[i]
            );
            assert_eq!(a.per_operator.killed[i], orig_a.per_operator.killed[i]);
            assert_eq!(
                a.per_operator.timed_out[i],
                orig_a.per_operator.timed_out[i]
            );
            assert_eq!(a.per_operator.survived[i], orig_a.per_operator.survived[i]);
            assert_eq!(a.per_operator.errored[i], orig_a.per_operator.errored[i]);
        }
        let b = &loaded.entries["src/b.ts"];
        assert!(b.sites.is_empty());
    }

    #[test]
    fn load_missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path(), "0.1.0").is_none());
    }

    #[test]
    fn load_corrupt_json_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("mutation")).unwrap();
        std::fs::write(root.join("mutation/cache.json"), "{not json!").unwrap();
        assert!(load(root, "0.1.0").is_none());
    }

    #[test]
    fn load_wrong_schema_version_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        save(root, "0.1.0", &sample_cache()).expect("save");
        // Doctor the schema version in place.
        let raw = std::fs::read_to_string(root.join("mutation/cache.json")).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        v["schema_version"] = serde_json::json!(CACHE_SCHEMA_VERSION + 1);
        std::fs::write(root.join("mutation/cache.json"), v.to_string()).unwrap();
        assert!(load(root, "0.1.0").is_none());
    }

    #[test]
    fn load_wrong_cli_version_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        save(root, "0.1.0", &sample_cache()).expect("save");
        assert!(load(root, "9.9.9").is_none());
        assert!(load(root, "0.1.0").is_some(), "matching version loads");
    }

    #[test]
    fn load_missing_field_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        save(root, "0.1.0", &sample_cache()).expect("save");
        let raw = std::fs::read_to_string(root.join("mutation/cache.json")).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        v.as_object_mut().unwrap().remove("files");
        std::fs::write(root.join("mutation/cache.json"), v.to_string()).unwrap();
        assert!(load(root, "0.1.0").is_none());
    }

    #[test]
    fn hash_file_unreadable_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.txt");
        let mut memo = HashMap::new();
        assert_eq!(hash_file(&missing, &mut memo), None);
    }
}
