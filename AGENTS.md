# Creating a new project from this starter

When asked to create a project based on rust-starter, do not copy the
directory by hand. Run:

```sh
python3 scripts/new-project.py NAME ~/code/NAME --description "ONE LINE"
```

It renames everything, leaves out the example code, depends on this
repository's `testkit` by git (pinned to the current commit, which must be
pushed), writes the project's `AGENTS.md` and `docs/spec.md` from
`template/`, and checks the result is green. Then `cd` into the project and
follow **its** `AGENTS.md`: write the spec from the person's request, lay the
test groundwork, build in slices. The rest of this file is about changing
the starter itself.

# Working in this starter

- Keep the unsafe and panic-prevention lints at `forbid`, including tests.
  No test-only relaxations, `cfg_attr(not(test), ...)`, or blanket exemptions.
- Short idioms that pass these lints (indexing, arithmetic, casts, slicing)
  are in `template/AGENTS.md` under "Idioms that pass the lints"; use them
  here too.
- Tests return `Result`; propagate setup failures with `?` and return `Err`
  on a mismatch. Assertion macros are forbidden too.
- Prefer checked access, fallible conversions and arithmetic with explicit
  semantics. Do not hide errors with defaults or saturation just to pass Clippy.
- Preserve I/O errors. Write through `std::io::Write` instead of print macros.
- A `forbid` cannot be overridden. For a non-forbidden lint, use a narrow
  `#[expect(..., reason = "...")]` only when there is a concrete justification.
- Every new workspace member must contain `[lints] workspace = true`.
  Excluded packages need their own checks and lint policy.
- Do not claim Clippy proves the program or its dependencies cannot panic.
- Keep dependencies and automation proportional to the project. Avoid
  machine-specific linkers, CPU flags and runtime-specific rules in this base.
- Before finishing a Rust change, run `cargo fmt --all --check`, `cargo lint`,
  and `cargo test --workspace --all-features --locked`. After changing policy,
  also run `python3 scripts/check-policy.py`. Use `cargo test-iter` for changes
  that need optimized execution.

# Testing

`docs/testing/README.md` maps problems to techniques; `testkit/` holds the
helpers. In short:

- Check with `testkit::{ensure, ensure_eq, fail}` and return `Failure`.
- Randomized tests take a `testkit::Seed` and print `TEST_SEED=…` on failure.
  Reproduce a reported failure with that line before changing anything.
- Every new test must fail when its bug is put back: break the code, watch
  the test fail, restore. For a change, `cargo mutants --in-diff` with `-j 2`.
- Check properties or independent references. No whole-screen snapshots,
  pixel matches or comparisons with an older build; golden files only for
  formats that are a contract, updated with `UPDATE_GOLDEN=1` and reviewed.
- Wait for events (`testkit::sandbox::wait_until`, fux waits), never a guessed
  sleep. Wait for output the input cannot contain, not for typed echo.
- Leave the machine alone: scratch under `target/` (`Scratch`), never `/tmp`,
  the real `HOME`, shell history or real accounts. Every child process goes
  in a `ChildGuard`; stop every server and process you start.
- Drive terminal programs only through a private fux server
  (`testkit::fux::Fux`), never a person's own session or panes.
- Keep per-push tests to seconds. Larger `TEST_CASES`, fuzzing and full
  mutation runs belong in the scheduled `deep` workflow; never start a run
  over about ten minutes or on all cores without asking.
- A skip must not pass silently where the tool exists (`REQUIRE_FUX=1`).
- `fuzz/` copies the lint policy; `scripts/check-policy.py` keeps it equal.
