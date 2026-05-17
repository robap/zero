# Floor exemptions

Two modules are exempted from the 70% line-coverage floor declared in spec R1.
The exemptions were chosen during execution of Step 4; see the answer to the
"floor gap" question in the session that produced this file.

## `src/prompts.rs` — 33% line coverage

`prompts.rs` is dominated by dialoguer DSL: `Input::with_theme(...).with_prompt(...).default(...).interact_text()`.
Every interactive function (`prompt_user`, `confirm_default_yes`) blocks on
stdin, and dialoguer offers no in-process injection seam in this version.

Existing coverage covers `validate_path_segment` (the only pure helper) and
exercises the prompts indirectly through integration tests in
`tests/init_confirm.rs` and `tests/init_existing_toml.rs` (which spawn the
`zero` binary as a subprocess — invisible to llvm-cov).

The script gate (`scripts/check-coverage.sh`, Step 21) should skip this file or
add it to its allow-list.

## `src/cmd/mutate.rs` — 51% line coverage

`cmd/mutate.rs` is a 1000-line orchestrator. ~340 lines live inside `run_inner`
(scheduled for the Step 20 refactor) and inside `dispatch_parallel` /
`run_one_mutant_subprocess`, both of which `fork+exec` a child `zero` binary —
the subprocess code is genuinely not exercisable by llvm-cov.

Existing in-file tests already cover `parse_operators`, the baseline-failure
path, killed/survived mutant accounting, and the operator filter (see
`#[cfg(test)] mod tests` at the end of the file). The Step 20 refactor will
split `run_inner` into named helpers and bring more of it under unit test, at
which point this exemption can be reconsidered.

The script gate (`scripts/check-coverage.sh`, Step 21) should skip this file or
add it to its allow-list.
