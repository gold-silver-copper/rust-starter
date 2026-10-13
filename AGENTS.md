# Working in this starter

- Keep the unsafe and panic-prevention lints at `forbid`, including tests.
  No test-only relaxations, `cfg_attr(not(test), ...)`, or blanket exemptions.
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
