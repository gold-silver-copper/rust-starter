# Testing playbook

Which technique fits which problem, what the starter already provides, and
how to run it. Every technique here works under the starter's lint policy:
tests return `Result`, nothing asserts or panics, and every random choice
comes from one printed seed.

## Pick by problem

| You have | Use | Starts from |
| --- | --- | --- |
| A function with rules any output must follow | Property checks over generated inputs | `tests/codec.rs`, [properties-and-fuzzing.md](properties-and-fuzzing.md) |
| A parser, decoder or protocol reading untrusted bytes | Coverage-guided fuzzing, same properties | `fuzz/`, `tests/oracles/rle.rs` |
| A format other programs read (wire, file, machine-readable CLI output) | Golden files | `testkit::golden`, `tests/golden/` |
| A second, independent implementation or a standard test suite | Differential tests | [properties-and-fuzzing.md](properties-and-fuzzing.md#differential-tests) |
| Distributed, concurrent or timed logic | Deterministic simulation with swarm faults | `tests/simulation.rs`, [simulation.md](simulation.md) |
| A terminal program | Drive the real binary through a private fux server | `tests/tui.rs`, [terminal-and-gui.md](terminal-and-gui.md) |
| A graphical program | Rule checks on rendered frames, offscreen | `testkit::frame`, [terminal-and-gui.md](terminal-and-gui.md#graphical-programs) |
| Doubt that the tests would notice a bug | Mutation testing | `.cargo/mutants.toml`, [mutation.md](mutation.md) |
| A tool or language that agents use | Agent tasks, held-out specs, playtests | `scripts/holdout.py`, [agent-evals.md](agent-evals.md) |
| A test suite that grew by accretion | The audit checklist | [test-audit.md](test-audit.md) |

Start with the cheapest row that can find the bug. Add the next technique when
there is an actual parser, protocol, invariant or UI to exercise, not before.

## What the starter provides

`testkit/` is a dependency-free workspace member used as a dev-dependency:

| Module | Gives |
| --- | --- |
| `check` | `ensure`, `ensure_eq` (with a line diff), `fail`, and `Failure`, which `?` converts any error into |
| `rng` | `Seed` (from `TEST_SEED`, else the CI commit, else fixed) and a `SplitMix64` `Rng` that is identical on every platform |
| `explore` | `Explore`: seeded cases, shrinking, a report with the replay command; `replay_dir` for fuzz corpora |
| `golden` | Golden-file comparison with a diff; `UPDATE_GOLDEN=1` rewrites |
| `sim` | Virtual time and a network that drops, duplicates, delays and reorders, with per-seed swarm faults and `heal` |
| `sandbox` | Private scratch directories under `target/`, kill-on-drop children, `wait_until`, an artifacts folder |
| `frame` | An RGBA frame with checked access, luma statistics and PPM output |
| `fux` | A private fux server per test, with `run`, `type_line`, `screen` and `wait_exit` |

`src/rle.rs` is an example target so every template runs. Delete it, its
tests, `tests/oracles/rle.rs`, `tests/golden/rle-encoding.txt` and the `rle`
fuzz target when you start; keep the shapes.

## Commands and budgets

| Command | Time | When |
| --- | --- | --- |
| `cargo test --workspace --all-features --locked` | seconds | Every change; CI on every push |
| `TEST_SEED=0x… cargo test --test NAME` | seconds | Replay a reported failure exactly |
| `TEST_CASES=100000 cargo test --release --test NAME` | minutes | Before merging risky logic; the nightly job |
| `cargo mutants --in-diff changes.diff -j 2` | minutes | On a pull request; CI does it |
| `cargo +nightly fuzz run TARGET -- -max_total_time=60` | 1 minute per target | The nightly job; locally after changing a parser |
| `python3 scripts/holdout.py selftest` | seconds | CI |

Never start runs over about ten minutes, runs on all cores, or large-memory
runs without asking the machine's owner. Cap parallelism (`-j 2`,
`--test-threads`). Long runs belong in the scheduled `deep` workflow.

## Seeds

Every randomized test takes a `Seed`. Locally it is fixed (`0x5eed`), so runs
repeat. In CI it is the first 16 hex digits of the commit, so every commit
explores new inputs and a failure still replays from the log's `TEST_SEED=…`
line. A failure report always carries that line.

## Artifacts

Anything a person or agent should look at after a run (captured screens,
rendered frames, simulation traces) goes through `testkit::sandbox::Artifacts`
into `target/tmp/artifacts/SUITE/`, and the failure message names the
absolute path. CI uploads that directory. Artifacts are for looking at, never
for comparing: pass or fail always comes from a rule.

## What does not belong

- Pixel-exact screenshots, whole-screen snapshots and comparisons with an
  older build of the same program. They freeze behaviour that should be free
  to improve. Pin only formats that are a contract.
- Tests that cannot fail: a predicate that is always true, a skip that counts
  as a pass, a check that never confirms its precondition happened.
- Guessed sleeps. Wait for the event (`wait_until`, fux waits).
- Anything touching the real machine: `/tmp`, the real `HOME`, shell history,
  credentials, a person's own terminal or fux server.
