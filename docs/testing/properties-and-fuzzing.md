# Properties, fuzzing, golden files and differential tests

## One oracle, three drivers

Write the properties once, as a function from input to `Result<(), Failure>`
(`tests/oracles/rle.rs`). Then drive it three ways:

1. **Seeded cases in `cargo test`** (`Explore`): fast, deterministic, every push.
2. **Corpus replay in `cargo test`** (`explore::replay_dir`): every input the
   fuzzer kept, on stable, every push.
3. **Coverage-guided fuzzing** (`fuzz/`, nightly): finds inputs a random
   generator would take years to hit.

Because the fuzz target includes the oracle by `#[path]`, there is no second
copy of the rules to drift.

## Good properties

Pick checks that any correct output satisfies, never "equals what it printed
last time":

- **Round trip:** `decode(encode(x)) == x`; `parse(print(x)) == x`.
- **Rejection is precise:** invalid input gives an error that names the right
  place, and never a panic, a hang or a huge allocation.
- **Canonical output:** the encoder never produces what the decoder would
  re-encode differently.
- **Split invariance:** feeding input whole, in pieces or byte by byte gives
  the same result (stream parsers, terminal emulators).
- **Metamorphic rewrites:** a transformation that must not change meaning
  (`swap swap`, inlining a definition, reordering independent operations)
  does not.
- **Pause and resume:** stopping after any number of steps and continuing
  gives the same result as one run.
- **Model agreement:** a deliberately simple second implementation (a `Vec`
  for a ring buffer, a naive interpreter) agrees on every generated input.

Make generators reach the interesting region. Uniform random bytes rarely
repeat, so `tests/codec.rs` adds a second generator biased toward long runs:
that is the one that finds the 255-count boundary.

Count outcome kinds where it matters: a property that only ever saw the error
path proves little. fux's differential test fails if any kind of outcome was
never reached.

## `Explore`

```rust
#[test]
fn properties_hold() -> Result<(), Failure> {
    Explore::from_env(2_000)?.bytes(600, oracle::check)
}
```

- `from_env(n)`: `n` cases, `TEST_CASES` overrides, seed from `TEST_SEED` or
  the CI commit.
- `bytes(max_len, property)`: random byte strings, shrunk on failure.
- `check(generate, property)`: any input type, no shrinking.
- `check_shrinking(generate, shrink, property)`: your own shrinker.

A failure reads:

```text
property failed on case 1 of 300
input (after 6 shrink steps): [0, 0, …]
error: pair 0 has a zero count
reproduce: rerun this test with TEST_SEED=0x0000000000005eed
```

`ensure` builds its message even when the check passes; in a hot loop use
`if !ok { return Err(fail(format!(…))) }`.

## proptest, when you need richer strategies

proptest's `proptest!` macro fails by panicking inside the dependency, where
Clippy cannot see it, so `clippy.toml` bans it. Drive `TestRunner` from a
`Result` test instead (verified against the starter's lints, proptest 1.12):

```rust
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};

#[test]
fn round_trips() -> Result<(), String> {
    // with_source_file makes failures persist to proptest-regressions/.
    let mut runner = TestRunner::new(Config::with_source_file(file!()));
    runner
        .run(&vec(any::<u8>(), 0..256), |input| {
            let decoded = decode(&encode(&input)).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(decoded, input); // returns Err; does not panic
            Ok(())
        })
        .map_err(|error| error.to_string())
}
```

- Use `proptest::strategy::Union::new([a.boxed(), b.boxed()])`, not
  `prop_oneof!`, which emits `allow` attributes for a forbidden lint group.
- For stateful systems, use `proptest-state-machine`'s
  `ReferenceStateMachine::sequential_strategy` inside `TestRunner::run` and
  drive the real code yourself, returning `Err` on divergence. Its
  `StateMachineTest::apply` can only fail by panicking.
- Commit `proptest-regressions/`.

## Fuzzing with cargo-fuzz

`fuzz/` is its own workspace: cargo-fuzz needs nightly and sanitizer flags
the main build must not inherit. Its lint table copies the root's;
`scripts/check-policy.py` fails if they drift. The one difference is
`clippy::panic = "deny"`: libFuzzer records a failure only on a panic or
abort, so the target's single `fail()` carries a justified `#[expect]`.

```sh
cargo install cargo-fuzz --locked                      # once
cd fuzz && cargo +nightly fuzz run rle -- -max_total_time=60
```

Corpus convention (`fuzz/.gitignore`):

- `corpus/TARGET/seed-*`: hand-written starting inputs. Commit them. Seeding
  with valid examples gives the fuzzer far more coverage at the start (koh: 24
  to 984 coverage points).
- `corpus/TARGET/regression-*`: a minimized crash, committed with its fix.
- Everything else libFuzzer writes is ignored.

When the fuzzer finds a crash:

```sh
cargo +nightly fuzz tmin rle fuzz/artifacts/rle/crash-…   # minimize
cp fuzz/artifacts/rle/minimized-from-… fuzz/corpus/rle/regression-short-name
cargo test --test codec fuzz_corpus_replays               # now fails on stable
```

Fix the bug; the replay test keeps it fixed in every CI run.

Adding a target: write `fuzz/fuzz_targets/NAME.rs` and a `[[bin]]` entry, add
`corpus/NAME/seed-*` files, and add a `replay_dir` test for it. A target with
a dictionary (`-dict=fuzz/NAME.dict`) of the format's keywords reaches deeper
into text formats.

## Golden files

`testkit::golden::check(dir, name, actual)` compares with a committed file
and fails with a line diff. `UPDATE_GOLDEN=1 cargo test` rewrites; review with
`git diff` and commit the change with its reason.

Use them for contracts: a wire format, a file format, a CLI's JSON. Keep each
one small and targeted (`tests/codec.rs` pins five encodings, not a dump).
Do not use them for screens, rendered images, or output that should be free
to improve: ghtui's 115 whole-screen snapshots churned on every key-hint
change and once hid seven dropped tests in a merge.

insta is banned by `clippy.toml` because its assertion macros panic.

## Differential tests

Compare against something **independent**:

- a deliberately simple second implementation in the test tree;
- an external implementation (gforth for a Forth, other terminal emulators
  for a terminal), skipped cleanly when not installed, with `REQUIRE_…=1`
  turning the skip into a failure where it is installed;
- a standard conformance suite (Forth 2012, Exercism cases) ported as data;
- an examples directory whose files carry their expected output in a header
  (`# expect: …`), run by one test that fails if it finds no examples.

Never compare against an older build of the same program. koh deleted such an
oracle because it "failed any change to what koh paints or sends, freezing
behaviour". When several references disagree, fux-vt lets a majority vote
decide rather than trusting any one.
