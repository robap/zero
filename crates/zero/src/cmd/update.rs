//! `zero update` — refresh framework files in `.zero/` from the embedded
//! binary. Never touches files outside `.zero/`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};

use zero_scaffold::{Operation, binary_manifest, framework_manifest};

/// Per-operation decision in interactive mode.
pub enum PerOpDecision {
    /// Apply this operation.
    Apply,
    /// Skip this operation; leave the path in its current state.
    Skip,
}

/// Top-level decision at the initial `Apply all? [Y/n/i]` prompt.
pub enum TopDecision {
    /// Apply every operation.
    ApplyAll,
    /// Apply nothing.
    Abort,
    /// Walk per-operation prompts.
    Interactive,
}

/// Abstraction over the interactive prompts so tests can stub them.
pub trait Confirmer {
    /// Top-level `Apply all? [Y/n/i]` prompt.
    ///
    /// # Parameters
    /// - `plan`: the planned operations, for display.
    ///
    /// # Returns
    /// The user's top-level decision.
    fn top_level(&mut self, plan: &[Operation]) -> anyhow::Result<TopDecision>;

    /// Per-operation `y/n` prompt asked once per planned op in interactive mode.
    ///
    /// # Parameters
    /// - `op`: the operation to display.
    ///
    /// # Returns
    /// The user's per-op decision.
    fn per_operation(&mut self, op: &Operation) -> anyhow::Result<PerOpDecision>;

    /// Final `Apply? [Y/n]` re-confirm after a per-op pass.
    ///
    /// # Parameters
    /// - `plan`: the filtered plan, for display.
    ///
    /// # Returns
    /// `Ok(true)` to apply, `Ok(false)` to abort.
    fn final_apply(&mut self, plan: &[Operation]) -> anyhow::Result<bool>;
}

/// Run the `zero update` subcommand. Constructs a stdin-driven confirmer
/// and delegates to [`run_with`].
///
/// # Parameters
/// - `yes`: when `true`, skip the top-level prompt and apply every operation.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub async fn run(yes: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let mut confirmer = StdinConfirmer;
    run_with(&cwd, yes, &mut confirmer)
}

/// Drive the full update flow: read `zero.toml` from `cwd` to find the
/// scaffolded project root, then detect preconditions, compute plan,
/// prompt, apply, and print summary.
///
/// # Parameters
/// - `cwd`: the directory containing `zero.toml` (typically the user's CWD).
/// - `yes`: short-circuits the top-level prompt and applies every operation.
/// - `confirmer`: prompt provider.
///
/// # Returns
/// `Ok(())` on success, an error otherwise.
pub fn run_with(cwd: &Path, yes: bool, confirmer: &mut dyn Confirmer) -> anyhow::Result<()> {
    let toml_path = cwd.join("zero.toml");
    if !toml_path.exists() {
        anyhow::bail!("zero update: no zero.toml found — run 'zero init' first");
    }
    let toml_text = fs::read_to_string(&toml_path)
        .map_err(|e| anyhow::anyhow!("zero update: failed to read zero.toml: {e}"))?;
    let config = zero_config::Config::from_toml_str(&toml_text)?;
    let project_root = cwd.join(&config.project.root);
    let bootstrapped = !project_root.join(".zero").is_dir();

    let plan = compute_plan(&project_root)?;
    if plan.is_empty() {
        println!("zero update: .zero/ is already up to date.");
        return Ok(());
    }

    print!("{}", render_plan(&plan));

    let to_apply: Vec<Operation> = if yes {
        plan.clone()
    } else {
        match confirmer.top_level(&plan)? {
            TopDecision::ApplyAll => plan.clone(),
            TopDecision::Abort => {
                println!("zero update: no changes applied");
                return Ok(());
            }
            TopDecision::Interactive => {
                let mut keep = Vec::new();
                for op in &plan {
                    match confirmer.per_operation(op)? {
                        PerOpDecision::Apply => keep.push(op.clone()),
                        PerOpDecision::Skip => {}
                    }
                }
                if keep.is_empty() {
                    println!("zero update: no changes applied");
                    return Ok(());
                }
                print!("{}", render_plan(&keep));
                if !confirmer.final_apply(&keep)? {
                    println!("zero update: no changes applied");
                    return Ok(());
                }
                keep
            }
        }
    };

    apply(&project_root, &to_apply)?;
    let (a, u, r) = count_kinds(&to_apply);
    if bootstrapped {
        println!(
            "zero update: bootstrapped .zero/ — applied {} operations ({a} added, {u} updated, {r} removed).",
            to_apply.len()
        );
    } else {
        println!(
            "zero update: applied {} operations ({a} added, {u} updated, {r} removed).",
            to_apply.len()
        );
    }
    Ok(())
}

/// Compute the set of `Operation`s by comparing `<root>/.zero/` against
/// the framework manifest.
///
/// # Parameters
/// - `root`: the project root.
///
/// # Returns
/// The ordered plan (adds and updates first, then removes).
pub fn compute_plan(root: &Path) -> anyhow::Result<Vec<Operation>> {
    let text_manifest = framework_manifest();
    let bin_manifest = binary_manifest();
    let manifest_paths: BTreeSet<PathBuf> = text_manifest
        .iter()
        .map(|(p, _)| PathBuf::from(p))
        .chain(bin_manifest.iter().map(|(p, _)| PathBuf::from(p)))
        .collect();

    let mut ops = Vec::new();

    for (rel, content) in &text_manifest {
        let abs = root.join(rel);
        if !abs.exists() {
            ops.push(Operation::Add(PathBuf::from(rel)));
        } else {
            let on_disk = fs::read(&abs)?;
            if on_disk != content.as_bytes() {
                ops.push(Operation::Update(PathBuf::from(rel)));
            }
        }
    }
    for (rel, bytes) in &bin_manifest {
        let abs = root.join(rel);
        if !abs.exists() {
            ops.push(Operation::Add(PathBuf::from(rel)));
        } else {
            let on_disk = fs::read(&abs)?;
            if on_disk != *bytes {
                ops.push(Operation::Update(PathBuf::from(rel)));
            }
        }
    }

    let dot_zero = root.join(".zero");
    if dot_zero.is_dir() {
        let mut extras: Vec<PathBuf> = Vec::new();
        walk_files(&dot_zero, &mut |abs| {
            if let Ok(rel) = abs.strip_prefix(root) {
                let rel_buf = rel.to_path_buf();
                if !manifest_paths.contains(&rel_buf) {
                    extras.push(rel_buf);
                }
            }
        })?;
        extras.sort();
        for rel in extras {
            ops.push(Operation::Remove(rel));
        }
    }
    Ok(ops)
}

/// Recursively visit every file under `dir`, calling `f` on each.
///
/// # Parameters
/// - `dir`: directory to walk.
/// - `f`: callback invoked once per file.
///
/// # Returns
/// `Ok(())` on success.
fn walk_files(dir: &Path, f: &mut dyn FnMut(&Path)) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ty = entry.file_type()?;
        if ty.is_dir() {
            walk_files(&path, f)?;
        } else if ty.is_file() {
            f(&path);
        }
    }
    Ok(())
}

/// Render the operation plan as a multi-line string, grouped by Add /
/// Update / Remove. Pure; no I/O.
///
/// # Parameters
/// - `ops`: the operations to render.
///
/// # Returns
/// The rendered plan, ending with `"Apply all? [Y/n/i]\n"`.
pub fn render_plan(ops: &[Operation]) -> String {
    let mut adds = Vec::new();
    let mut updates = Vec::new();
    let mut removes = Vec::new();
    for op in ops {
        match op {
            Operation::Add(p) => adds.push(p),
            Operation::Update(p) => updates.push(p),
            Operation::Remove(p) => removes.push(p),
        }
    }

    let mut out = String::new();
    out.push_str("zero update will perform these operations in .zero/:\n\n");
    if !adds.is_empty() {
        out.push_str("  add:\n");
        for p in &adds {
            let _ = writeln!(out, "    {}", p.display());
        }
        out.push('\n');
    }
    if !updates.is_empty() {
        out.push_str("  update:\n");
        for p in &updates {
            let _ = writeln!(out, "    {}", p.display());
        }
        out.push('\n');
    }
    if !removes.is_empty() {
        out.push_str("  remove:\n");
        for p in &removes {
            let _ = writeln!(out, "    {}", p.display());
        }
        out.push('\n');
    }
    out.push_str("Apply all? [Y/n/i]\n");
    out
}

/// Apply a slice of operations to `<root>/.zero/`. Refuses to write to any
/// path that does not have `<root>/.zero/` as its prefix.
///
/// # Parameters
/// - `root`: the project root.
/// - `ops`: operations to apply.
///
/// # Returns
/// `Ok(())` on success.
pub fn apply(root: &Path, ops: &[Operation]) -> anyhow::Result<()> {
    let text_manifest = framework_manifest();
    let bin_manifest = binary_manifest();
    let dot_zero = root.join(".zero");
    for op in ops {
        let rel = match op {
            Operation::Add(p) | Operation::Update(p) | Operation::Remove(p) => p,
        };
        let abs = root.join(rel);
        if !abs.starts_with(&dot_zero) {
            anyhow::bail!(
                "zero update: refusing to touch path outside .zero/: {}",
                abs.display()
            );
        }
        match op {
            Operation::Add(_) | Operation::Update(_) => {
                let content_bytes: Vec<u8> = if let Some((_, txt)) = text_manifest
                    .iter()
                    .find(|(p, _)| Path::new(p) == rel.as_path())
                {
                    txt.as_bytes().to_vec()
                } else if let Some((_, bytes)) = bin_manifest
                    .iter()
                    .find(|(p, _)| Path::new(p) == rel.as_path())
                {
                    bytes.to_vec()
                } else {
                    anyhow::bail!("internal: no manifest entry for {}", rel.display());
                };
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&abs, &content_bytes)?;
            }
            Operation::Remove(_) => {
                fs::remove_file(&abs)?;
            }
        }
    }
    Ok(())
}

/// Count operations by kind.
///
/// # Parameters
/// - `ops`: operations to count.
///
/// # Returns
/// A tuple `(adds, updates, removes)`.
fn count_kinds(ops: &[Operation]) -> (usize, usize, usize) {
    let mut a = 0;
    let mut u = 0;
    let mut r = 0;
    for op in ops {
        match op {
            Operation::Add(_) => a += 1,
            Operation::Update(_) => u += 1,
            Operation::Remove(_) => r += 1,
        }
    }
    (a, u, r)
}

/// Default `Confirmer` that reads from stdin via `dialoguer`.
pub struct StdinConfirmer;

impl Confirmer for StdinConfirmer {
    fn top_level(&mut self, _plan: &[Operation]) -> anyhow::Result<TopDecision> {
        let theme = ColorfulTheme::default();
        let choice = Select::with_theme(&theme)
            .with_prompt("Apply all?")
            .default(0)
            .items(&["yes (apply all)", "no (abort)", "interactive"])
            .interact()?;
        Ok(match choice {
            0 => TopDecision::ApplyAll,
            1 => TopDecision::Abort,
            _ => TopDecision::Interactive,
        })
    }

    fn per_operation(&mut self, op: &Operation) -> anyhow::Result<PerOpDecision> {
        let theme = ColorfulTheme::default();
        let (kind, path) = match op {
            Operation::Add(p) => ("add", p),
            Operation::Update(p) => ("update", p),
            Operation::Remove(p) => ("remove", p),
        };
        let prompt = format!("{kind} {}?", path.display());
        let yes = Confirm::with_theme(&theme)
            .with_prompt(prompt)
            .default(true)
            .interact()?;
        Ok(if yes {
            PerOpDecision::Apply
        } else {
            PerOpDecision::Skip
        })
    }

    fn final_apply(&mut self, _plan: &[Operation]) -> anyhow::Result<bool> {
        let theme = ColorfulTheme::default();
        let yes = Confirm::with_theme(&theme)
            .with_prompt("Apply?")
            .default(true)
            .interact()?;
        Ok(yes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zero_scaffold::{ScaffoldContext, write_initial_project, write_user_files};

    fn scaffold() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("zero.toml"),
            "[project]\nroot = \".\"\n\n[build]\nout = \"dist\"\n",
        )
        .unwrap();
        write_initial_project(
            &root,
            &ScaffoldContext {
                title: "T".to_string(),
            },
        )
        .unwrap();
        (dir, root)
    }

    struct StubConfirmer {
        top: TopDecision,
        per_op: Vec<PerOpDecision>,
        final_apply: bool,
    }

    impl Confirmer for StubConfirmer {
        fn top_level(&mut self, _plan: &[Operation]) -> anyhow::Result<TopDecision> {
            Ok(match self.top {
                TopDecision::ApplyAll => TopDecision::ApplyAll,
                TopDecision::Abort => TopDecision::Abort,
                TopDecision::Interactive => TopDecision::Interactive,
            })
        }
        fn per_operation(&mut self, _op: &Operation) -> anyhow::Result<PerOpDecision> {
            Ok(match self.per_op.remove(0) {
                PerOpDecision::Apply => PerOpDecision::Apply,
                PerOpDecision::Skip => PerOpDecision::Skip,
            })
        }
        fn final_apply(&mut self, _plan: &[Operation]) -> anyhow::Result<bool> {
            Ok(self.final_apply)
        }
    }

    #[test]
    fn update_with_no_drift_reports_up_to_date() {
        let (_dir, root) = scaffold();
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.is_empty(),
            "expected empty plan on clean scaffold, got {plan:?}"
        );
    }

    #[test]
    fn update_with_missing_file_proposes_add() {
        let (_dir, root) = scaffold();
        fs::remove_file(root.join(".zero/styles/_utilities.scss")).unwrap();
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.contains(&Operation::Add(PathBuf::from(
                ".zero/styles/_utilities.scss"
            ))),
            "plan missing Add for _utilities.scss: {plan:?}"
        );
    }

    #[test]
    fn update_with_modified_file_proposes_update() {
        let (_dir, root) = scaffold();
        let mut content = fs::read(root.join(".zero/zero.d.ts")).unwrap();
        content.extend_from_slice(b"\n// x\n");
        fs::write(root.join(".zero/zero.d.ts"), content).unwrap();
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.contains(&Operation::Update(PathBuf::from(".zero/zero.d.ts"))),
            "plan missing Update for zero.d.ts: {plan:?}"
        );
    }

    #[test]
    fn update_with_extra_file_proposes_remove() {
        let (_dir, root) = scaffold();
        fs::write(root.join(".zero/styles/_extra.scss"), "// stray\n").unwrap();
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.contains(&Operation::Remove(PathBuf::from(
                ".zero/styles/_extra.scss"
            ))),
            "plan missing Remove for _extra.scss: {plan:?}"
        );
    }

    #[test]
    fn update_refuses_when_no_zero_toml() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let mut stub = StubConfirmer {
            top: TopDecision::ApplyAll,
            per_op: Vec::new(),
            final_apply: true,
        };
        let err = run_with(&root, true, &mut stub).expect_err("should fail without zero.toml");
        assert!(
            err.to_string().contains("no zero.toml found"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn update_bootstraps_missing_dot_zero() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        fs::write(
            root.join("zero.toml"),
            "[project]\nroot = \".\"\n\n[build]\nout = \"dist\"\n",
        )
        .unwrap();
        // Write user files only — no `.zero/` on entry.
        write_user_files(
            &root,
            &ScaffoldContext {
                title: "T".to_string(),
            },
        )
        .unwrap();
        assert!(
            !root.join(".zero").is_dir(),
            "precondition: .zero/ must not exist before run_with"
        );

        let mut stub = StubConfirmer {
            top: TopDecision::ApplyAll,
            per_op: Vec::new(),
            final_apply: true,
        };
        run_with(&root, true, &mut stub).expect("update should bootstrap a missing .zero/");

        assert!(
            root.join(".zero").is_dir(),
            ".zero/ should exist after bootstrap"
        );
        for (rel, _) in framework_manifest() {
            assert!(
                root.join(rel).exists(),
                "expected manifest path {rel} to be materialized on disk"
            );
        }
    }

    #[test]
    fn update_yes_flag_applies_all_operations() {
        let (_dir, root) = scaffold();
        fs::write(root.join(".zero/zero.d.ts"), b"// MUTATED\n").unwrap();
        fs::remove_file(root.join(".zero/styles/_utilities.scss")).unwrap();
        fs::write(root.join(".zero/styles/_extra.scss"), b"// stray\n").unwrap();

        let mut stub = StubConfirmer {
            top: TopDecision::ApplyAll,
            per_op: Vec::new(),
            final_apply: true,
        };
        run_with(&root, true, &mut stub).unwrap();

        // Converged: plan is empty after apply.
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.is_empty(),
            "expected empty plan after --yes apply, got {plan:?}"
        );
        assert!(
            !root.join(".zero/styles/_extra.scss").exists(),
            "stray file not removed"
        );
        assert!(
            root.join(".zero/styles/_utilities.scss").exists(),
            "missing file not restored"
        );
    }

    #[test]
    fn apply_refuses_path_outside_dot_zero() {
        let (_dir, root) = scaffold();
        let bad = vec![Operation::Add(PathBuf::from("outside.txt"))];
        let err = apply(&root, &bad).expect_err("should refuse outside-.zero/ path");
        assert!(
            err.to_string().contains("outside .zero/"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn update_with_missing_font_proposes_add() {
        let (_dir, root) = scaffold();
        fs::remove_file(root.join(".zero/fonts/Geist-VariableFont_wght.woff2")).unwrap();
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.contains(&Operation::Add(PathBuf::from(
                ".zero/fonts/Geist-VariableFont_wght.woff2"
            ))),
            "plan missing Add for woff2 font: {plan:?}"
        );
    }

    #[test]
    fn update_with_modified_font_proposes_update() {
        let (_dir, root) = scaffold();
        fs::write(
            root.join(".zero/fonts/Geist-VariableFont_wght.woff2"),
            b"garbage bytes",
        )
        .unwrap();
        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.contains(&Operation::Update(PathBuf::from(
                ".zero/fonts/Geist-VariableFont_wght.woff2"
            ))),
            "plan missing Update for woff2 font: {plan:?}"
        );
    }

    #[test]
    fn update_yes_flag_restores_binary_drift() {
        let (_dir, root) = scaffold();
        fs::remove_file(root.join(".zero/fonts/Geist-VariableFont_wght.woff2")).unwrap();
        fs::write(root.join(".zero/fonts/OFL.txt"), b"// mutated\n").unwrap();

        let mut stub = StubConfirmer {
            top: TopDecision::ApplyAll,
            per_op: Vec::new(),
            final_apply: true,
        };
        run_with(&root, true, &mut stub).unwrap();

        let plan = compute_plan(&root).unwrap();
        assert!(
            plan.is_empty(),
            "expected empty plan after --yes apply, got {plan:?}"
        );
        // Confirm bytes match the embedded manifest entry.
        let on_disk = fs::read(root.join(".zero/fonts/Geist-VariableFont_wght.woff2")).unwrap();
        let expected = zero_scaffold::binary_manifest()
            .into_iter()
            .find(|(p, _)| *p == ".zero/fonts/Geist-VariableFont_wght.woff2")
            .map(|(_, b)| b)
            .unwrap();
        assert_eq!(on_disk, expected);
    }

    #[test]
    fn update_with_empty_dot_zero_dir_proposes_only_adds() {
        let (_dir, root) = scaffold();
        for entry in fs::read_dir(root.join(".zero")).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path).unwrap();
            } else {
                fs::remove_file(&path).unwrap();
            }
        }
        let plan = compute_plan(&root).unwrap();
        assert_eq!(
            plan.len(),
            framework_manifest().len() + zero_scaffold::binary_manifest().len(),
            "expected one Add per manifest entry, got {plan:?}"
        );
        for op in &plan {
            assert!(
                matches!(op, Operation::Add(_)),
                "expected only Adds, got {op:?}"
            );
        }
    }
}
