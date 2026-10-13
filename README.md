# rust-starter

A dependency-free Rust starting point with strict lints in **all first-party
targets, including tests**, a fast optimized `iter` profile, and groundwork
for property, fuzz, golden-file, simulation, mutation, terminal, graphical
and agent-driven testing that obeys the same lints.

## New project

```sh
python3 scripts/new-project.py NAME ~/code/NAME --description "ONE LINE"
```

creates a project with this policy, CI and testing setup, depending on this
repository's `testkit` by git. Then tell an agent what the project should be:
the generated `AGENTS.md` makes it write a spec with checkable properties,
build the tests first, and develop in verified slices. `template/` holds the
files a new project gets.

## Start here

Rustup installs the pinned toolchain, Clippy and rustfmt from
`rust-toolchain.toml`. The policy regression check additionally needs Python 3.11+.

```sh
cargo run
cargo lint
cargo test --workspace --all-features --locked
cargo iter
cargo test-iter
python3 scripts/check-policy.py
```

Rename the package in `Cargo.toml` and `fuzz/Cargo.toml`, and the
`rust_starter` / `rust-starter` references (`grep -rn 'rust.starter' src tests
fuzz`), then run `cargo generate-lockfile` in the root and in `fuzz/`. Delete
the example codec (`src/rle.rs`, `tests/codec.rs`, `tests/oracles/rle.rs`,
`tests/golden/`, the `rle` fuzz target and corpus) once you have your own
targets. Keep both `Cargo.lock` files in version control. The starter has `publish = false`; choose a
license and fill in package metadata before enabling publication.

## The policy

`Cargo.toml` holds the shared lint levels. `clippy.toml` holds the API bans and
explicitly disables every test relaxation used by the source projects.

| Area | Enforced approach |
| --- | --- |
| Unsafe code | Rust's `unsafe_code = "forbid"`, including tests |
| Explicit panic paths | Forbid unwrap, expect, panic, unreachable, todo and unimplemented |
| Access | Checked access instead of indexing, string slicing and listed panic-prone collection methods |
| Arithmetic | Checked operations or deliberately chosen wrapping/saturating semantics; fallible conversions for narrowing |
| Assertions | Ban assert and debug-assert macros too; tests return errors |
| Output | Use `Write` and handle errors; printing/debug macros are forbidden |
| Ignored failures | Forbid discarded must-use values, unused `.ok()` results and discarded futures |
| Suppressions | `forbid` rules cannot be locally relaxed; narrower `deny`/`warn` rules may use justified `#[expect]` |

This is stricter than fux in two respects: assertion macros are banned even in
tests, and its measured hot-path exceptions for slice copying/rotation and
character encoding are absent. The starter has no workload that justifies them.

Use `?` for setup and I/O failures, and return an error on a failed check:

```rust
#[test]
fn checked_addition() -> Result<(), &'static str> {
    let actual = 2_u32.checked_add(2).ok_or("addition overflowed")?;
    if actual != 4 {
        return Err("expected four");
    }
    Ok(())
}
```

A test returning `Err` fails normally. Do not replace failed operations with
defaults merely to satisfy a lint. Choose saturation only when saturation is
the intended behavior.

Some proc macros emit lint relaxations that conflict with `forbid`; this already
appears in koh/fux/ghtui's configuration comments. Check macro compatibility
before adopting derives or test runtimes. Prefer a builder or a manual entry
point when needed, while preserving the core restrictions.

### What this does and does not guarantee

Unsafe code is rejected by normal Rust compilation. The Clippy restrictions
require **`cargo lint`**; `cargo build` and `cargo test` do not run Clippy.

These are strong guardrails, not a proof that the whole program cannot panic.
Clippy lints a third-party macro's expansion as that crate's code, so an
`unwrap` or `panic!` hidden inside a dependency's macro passes every lint;
`clippy.toml` therefore bans the known panicking test macros by path.
The standard library, dependencies, callbacks, macro expansions, allocation
failures and APIs outside the curated ban list can still panic or abort.
Only compiled configurations are checked; Clippy does not check Rust code in
documentation examples. Feature and target-specific code needs corresponding
CI builds as it is added. Test harness internals and dependencies are outside
the first-party lint policy.

The regression script compiles accepted code and tries forbidden operations in
production code, unit tests and integration tests. It checks the specific
diagnostic, including refusal of a local `#[expect]` override. It also checks
that every workspace member opts into the shared lints. Fixtures and artifacts
stay under `target/`.

## Testing

[docs/testing/README.md](docs/testing/README.md) is the playbook: which
technique fits which problem, and how to run it. The pieces:

| Piece | What it is |
| --- | --- |
| `testkit/` | Dependency-free dev-dependency: `ensure`/`ensure_eq` returning `Failure`, seeds and a portable RNG, property exploration with shrinking, golden files, a deterministic network simulator, private scratch dirs and kill-on-drop processes, image frames, and a private fux server for driving terminal programs |
| `tests/codec.rs`, `tests/oracles/` | Property, corpus-replay and golden-file templates sharing one oracle with the fuzzer |
| `tests/simulation.rs` | TigerBeetle-style simulation: swarm faults, safety every step, liveness after healing |
| `tests/tui.rs` | The real binary driven through a private fux server |
| `fuzz/` | cargo-fuzz crate (nightly), its own workspace and a policy-checked copy of the lints |
| `.cargo/mutants.toml` | cargo-mutants settings that keep the strict lints from hiding mutants |
| `scripts/holdout.py` | Seal, unseal and score held-out evaluation sets |
| `prompts/` | Agent templates: playtest, stress test, test audit |

Randomized tests use a fixed seed locally and the commit in CI; every failure
prints the `TEST_SEED=…` that replays it. `REQUIRE_FUX=1` turns the fux test's
skip into a failure wherever fux is installed.

## Build profiles

| Profile | Intended use | Settings |
| --- | --- | --- |
| `dev` / `test` | Quick compilation, ordinary tests | Unoptimized; line-table backtraces for local code; no dependency debuginfo |
| `iter` | Optimized edit/run/test loop | Optimization level 3; LTO off; 16 codegen units; incremental compilation; line tables; debug assertions and overflow checks |
| `release` | Shipping | Optimization level 3; thin LTO; one codegen unit; overflow checks retained |

`cargo iter` is an alias for `cargo run --profile iter --locked`; use
`cargo build --profile iter --locked` to build without running. `iter` trades
some optimization across compilation units for cheaper rebuilds. Measure it on
the actual application before tuning further; this starter has no meaningful
performance workload.

`panic = "unwind"` remains enabled. Aborting would change what happens after a
panic, not prevent one. Overflow checks stay enabled as defense against silent
wrapping in code that escapes the lint restrictions. Full variable inspection
in a debugger needs full debuginfo, for example
`CARGO_PROFILE_DEV_DEBUG=2 cargo build`.

Cargo's [profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html)
documents these tradeoffs. In particular, `lto = "off"` fully disables LTO;
`lto = false` can still perform local thin LTO.

## Growing the project

The root is both a package and a workspace. Add members to `[workspace].members`
and put this in **each member's** manifest:

```toml
[lints]
workspace = true
```

Use `edition.workspace = true` and `rust-version.workspace = true` there too.
Run checks with `--workspace` so the root package does not hide member tests.
Standalone packages excluded from the workspace need their own lint tables and
CI checks; adding them requires updating the workflow explicitly.

CI runs formatting, Clippy for all targets/features (and for `fuzz/`), the
policy regression checks, ordinary and `iter` tests, the holdout script's
self-test, documentation and a release build, plus `cargo mutants --in-diff`
on pull requests. The scheduled `deep` workflow fuzzes each target, runs
property and simulation tests with far more cases, and mutates the whole
workspace. It starts
with Linux; add platforms and feature combinations as the application needs
them. Both declared minimum Rust and the pinned toolchain are 1.98.1, so there
is no separate, untested older-version claim. Update both together, or introduce
a separate minimum-version job when they diverge.

No async runtime, CLI framework, error crate, linker override, native CPU flag,
dependency optimization override or test framework is preselected; `testkit`
covers the common cases without dependencies. Add dependency advisory checks
when adding dependencies.

## Where the defaults came from

Reviewed local project configuration and retained Claude Code sessions on
2026-10-12. The useful pieces were:

| Project | Adopted or considered |
| --- | --- |
| fux | Non-overridable unsafe/panic/arithmetic/cast restrictions, fallible output, and a curated panic-prone std API list |
| koh | Release overflow checks, unwinding for cleanup, and narrow justified expectations; its test exemptions and application-specific spawn/queue rules were deliberately omitted |
| rig | Shared workspace settings, explicit workspace-wide checks, line-table debuginfo and no dependency debuginfo; async lock lints |
| ghtui | Selected readability lints and the documented proc-macro conflict with `forbid` |
| ratty / bevy_terminal | `release-fast`: optimized incremental builds with multiple codegen units |
| hyper-surf | `gen-test`: optimized test execution without release's expensive linking |
| bevy_noodle | API-specific method bans illustrate how to extend the policy after adding a framework |

The retained koh sessions specifically exposed a `debug_assert_eq!` hole in the
panic lints and overly broad test exemptions. Those findings informed the
assertion bans and regression checks here. The subsequent request for no test
exemptions takes precedence over the more permissive setups in other projects.

No `iter` profile was found in the retained sessions or inspected manifests;
that name is new here, built from the existing `release-fast` / `gen-test`
pattern. No retained Claude session for rig was found; its contribution comes
from the local checkout at `.rig-desktop/source/rig`. Session retention was
partial, so this is a synthesis of available evidence, not a claim to have
recovered all project history.

The testing groundwork came from a second review on 2026-10-12 of the same
projects' tests, notes and sessions, plus TigerBeetle and Trustfall:

| Source | Adopted |
| --- | --- |
| fux | Private-server terminal driving, waits instead of sleeps, fuzz targets that check properties, corpus naming, the agent benchmark and held-out spec method, mutation testing of a redesign |
| koh | Sans-I/O cores with injected time, a seeded fake network, fuzz corpora seeded from valid messages, a single test-support crate, the test-removal audit checklist |
| ck | A fuzzer of fake sites on a virtual clock with shrinking and seed replay; the playtest loop |
| bevy_terminal, hyper-surf, ratty, ghtui | Offscreen rendering and rule-based image checks instead of pixel matches; artifacts for agents; the cost of whole-screen snapshots |
| TigerBeetle | One seed per world, swarm faults, safety every step and liveness after healing, the commit as the CI seed |
| Trustfall | Golden files for contracts only, fuzz corpora seeded from test fixtures |

A compile spike confirmed which tools fit the lint policy: proptest through
`TestRunner`, cargo-fuzz in its own workspace, cargo-mutants with
`cap_lints`; insta's and `proptest!`'s panicking macros do not.
