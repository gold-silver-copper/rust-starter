# {{name}}

{{description}}

This project was created from rust-starter at commit `{{rev}}`. It is built by
agents working autonomously: the person says what they want, and the agent
plans, tests, implements and verifies it. These instructions are the
contract. Follow them without being reminded.

# How to work

## 1. Understand the request, then write the spec

Before writing code, turn the person's request into `docs/spec.md` (it starts
as a template). Fill in:

- **Goal**: what the program does and for whom, in a few sentences.
- **Interfaces**: the CLI, file formats, library API or UI, concretely.
- **Properties**: what must always be true, phrased so code can check it
  ("parsing what we print gives back the same value", "adding an item never
  removes another", "every request gets exactly one reply"). These become the
  tests. Aim for properties over examples.
- **Plan**: an ordered checklist of small slices, each one usable and
  testable on its own.
- **Decisions** and **Open questions**: record choices as you make them.

Choose testing techniques from the table in the testing playbook (below) by
what the project has: a parser gets fuzzing, distributed or timed logic gets
simulation, a format other programs read gets golden files, a terminal UI gets
fux-driven tests.

Ask the person only about decisions that change **what** gets built (scope,
behaviour they would notice, external services, licensing). Decide everything
else yourself and record it under Decisions. If they are not available,
choose the conservative option, record why, and continue.

## 2. Lay the test groundwork before the features

1. `tests/oracles/NAME.rs`: one function per area, `check(input) -> Result<(), Failure>`,
   checking every property in the spec. New behaviour means a new property
   here, not a new hand-picked example elsewhere.
2. `tests/support/gen.rs`: generators (`fn thing(rng: &mut Rng) -> Thing`)
   and shrinkers (`fn shrink(thing: &Thing) -> Vec<Thing>`, smaller variants)
   for the domain's values. Extend these; never write a second generator.
   Bias generators toward the interesting region (small sizes, repeats,
   boundaries); uniform randomness rarely hits it.
3. Tests that drive the oracle with `testkit::Explore`, include support code
   with `#[path = "support/gen.rs"] mod gen;` (or a `tests/support/mod.rs`).

## 3. Build in slices

For each slice in the plan:

1. Add or extend properties for the slice. Run them; they should fail or not
   compile yet.
2. Implement the smallest code that makes them pass.
3. **Break it on purpose**: put a plausible bug in the new code and confirm a
   test fails with a useful message, then restore. `cargo mutants --in-diff`
   does this systematically (see Verify).
4. Run the gate (below). Fix everything it reports.
5. Tick the slice in `docs/spec.md` and commit locally with a message that
   says what the slice does. Do not push, publish or open pull requests
   unless the person asked; if git has no identity, ask the person rather
   than inventing one.

Keep the program runnable after every slice. Prefer deleting code to adding
it, and the simplest design that passes the properties.

## 4. Verify

The gate, before every commit:

```sh
cargo fmt --all
cargo lint
cargo test --workspace --all-features --locked
python3 scripts/check-policy.py        # after changing Cargo.toml, clippy.toml or lints
```

Before declaring the work done, also:

```sh
# BASE: the commit before this task began (git log shows it)
git diff --src-prefix=a/ --dst-prefix=b/ BASE > changes.diff
TMPDIR="$HOME/.cache" cargo mutants --in-diff changes.diff -j 2
```

Every **missed** mutant gets a test that catches it or a written reason it
cannot be told apart (then add it to `exclude_re` in `.cargo/mutants.toml`).
Then run the program the way the person will, and confirm a real use works.

**Done means**: the spec's plan is ticked, the gate passes, no unexplained
missed mutants, and you have run the real program. Report what was built, how
you verified it, and the exact command (starting with `cd`) the person can
run to try it.

# Rules the code follows

These are enforced by `cargo lint`; work with them, never around them.

- No `unwrap`, `expect`, `panic!`, indexing, slicing, unchecked arithmetic,
  lossy casts or assertion macros, in tests too. Use `get`, `checked_*`,
  `try_from`, `?`, and choose wrapping or saturating arithmetic only when that
  is the intended meaning, never just to satisfy the lint.
- Tests return `Result<(), Failure>` and check with `testkit::{ensure,
  ensure_eq, fail}`.
- Write output through `std::io::Write` and propagate the error; no print
  macros.
- A `forbid` lint cannot be overridden. A `deny` lint may take a narrow
  `#[expect(lint, reason = "...")]` with a concrete reason.
- Before adopting a dependency that exports macros or derives, add it and run
  `cargo lint`: macros that emit `allow` attributes for forbidden lints break
  the build. `insta` and `proptest!` are banned because they panic.
- New workspace members need `[lints] workspace = true`.

# Idioms that pass the lints

Reach for these first; they are short and they compile. Every snippet here
passes `cargo lint` as written.

**Make functions return `Option` or `Result`**, so `?` absorbs every checked
step. Most lint friction comes from a function that returns a plain value and
has nowhere to send a failure.

| Instead of | Write |
| --- | --- |
| `v[i]` | `v.get(i)?` (or `.ok_or("why")?` in a `Result` fn) |
| `grid[r][c]` | `grid.get(r)?.get(c)` |
| `r - 1`, `c + 1` for neighbours | `r.checked_add_signed(dr)?` over `[(-1, 0), (1, 0), (0, -1), (0, 1)]` |
| `count += 1` in a loop | `.filter(...).count()`; otherwise `count = count.checked_add(1)?` |
| a sum or product | `.try_fold(0, \|s, x\| s.checked_add(*x))` |
| `v[i]` vs `v[i + 1]` | `v.iter().zip(v.iter().skip(1))` (`windows` is banned) |
| `a / b` | `a.checked_div(b)?` |
| `x as usize` | `usize::try_from(x).ok()?` / `usize::try_from(x)?` |
| `s[1..n - 1]` | `s.get(1..n.checked_sub(1)?)?`; or work on `.chars()` / `.bytes()` |
| `v[v.len() - 1]` | `v.last()?`; `match v { [.., a, b] => …, _ => … }` |
| `v.swap(i, j)` | `let [a, b] = v.get_disjoint_mut([i, j]).ok()?; std::mem::swap(a, b);` |
| `v.chunks(n)`, `"ab".repeat(n)` | `v.as_chunks::<N>()`, `std::iter::repeat_n("ab", n).collect::<String>()` |
| `x % 2 == 0` | `x.is_multiple_of(2)` |
| `v.iter().max().unwrap()` | `v.iter().max()?` |
| `.unwrap()` on a parse | `.parse().map_err(\|e\| format!("{text:?}: {e}"))?` |
| `println!(...)` | `writeln!(out, ...)?` with `out: &mut impl Write` |
| `assert_eq!(a, b)` in a test | `ensure_eq(&a, &b, "what")?` |

Traps:

- `checked_add(1).unwrap_or(MAX)` is rejected (`manual_saturating_arithmetic`):
  either propagate with `?` or write `saturating_add` when saturation is the
  real meaning. Never pick a default just to silence a lint.
- `wrapping_*` is for algorithms that wrap by definition (hashes, RNGs).
- `ensure(cond, format!(...))` formats even when it passes; in hot loops use
  `if !cond { return Err(fail(format!(...))); }`.
- Iterators avoid most arithmetic and indexing entirely, and read better.

# Working live with people

When people are pairing or watching in real time (a pairing session, an
interview, several people steering one agent), keep the loop short and
visible:

- Skip the written spec unless asked; state the plan in two or three lines in
  the conversation instead.
- After each change run `cargo test -q` (seconds); run `cargo lint` when a
  function is finished. Leave mutation testing for the end, if there is time.
- Show the property or example you are about to test before writing the code,
  so the people can correct the direction early.
- Prefer the idioms above over clever code; it has to be read aloud.
- The lint rules do not relax under time pressure. If one blocks a natural
  solution, use the idiom table and say why in one sentence.

# Testing rules

The playbook, with the reasoning and worked examples, lives upstream:
{{starter_docs}} (also `{{starter_path}}/docs/testing/` on the machine that
created this project). Read its README before choosing techniques. The rules:

- Randomness comes only from `testkit::Seed` / `testkit::Rng`. A failure
  prints `TEST_SEED=…`; reproduce with it **before** changing code, and add
  the shrunk input as a regression (a corpus file or a named test).
- Check properties or independent references. No whole-screen snapshots,
  pixel matches, or comparisons with an older build of this program. Golden
  files (`testkit::golden`, `UPDATE_GOLDEN=1`) only for formats that are a
  contract; review their diffs.
- Wait for events (`testkit::sandbox::wait_until`, fux waits), never a
  guessed sleep. Wait for output the input cannot contain.
- Leave the machine alone: scratch files via `testkit::sandbox::Scratch`
  under `target/`, never `/tmp`, the real `HOME`, shell history or real
  accounts. Child processes go in a `ChildGuard`; stop every process and
  server you start.
- Drive terminal programs only through a private fux server
  (`testkit::fux::Fux`), never a person's own terminal or session.
- Per-push tests take seconds. Bigger runs use `TEST_CASES`, fuzzing and full
  mutation runs, which belong in the scheduled `deep` workflow. Never start a
  run over about ten minutes, on all cores, or with large memory without
  asking; cap parallelism (`-j 2`).
- A test skipped because a tool is missing must not pass silently where the
  tool exists (`REQUIRE_FUX=1` and similar).
- A test that cannot fail is worse than none: check its precondition
  happened, and that it fails when its bug comes back.

# testkit

`testkit` is a dev-dependency from rust-starter, pinned in `Cargo.toml` by
`rev`. Do not copy it into this project. If it has a bug or lacks something
general, fix it in rust-starter, then bump the `rev` here. It provides:
`check` (ensure, ensure_eq, Failure), `rng` (Seed, Rng), `explore` (Explore,
shrink_bytes, replay_dir), `golden`, `sim` (Network, Faults), `sandbox`
(Scratch, ChildGuard, wait_until, Artifacts, run), `frame` (Frame, Stats) and
`fux` (Fux, Pane). Its source is in the cargo git checkout and upstream.

# Adding fuzzing

When the project has a parser or reads untrusted bytes, copy `fuzz/` from
rust-starter at the pinned commit, rename the package and target, point the
target at your oracle, add `seed-*` corpus files, and add a test that replays
the corpus with `testkit::explore::replay_dir`. `scripts/check-policy.py`
then keeps the fuzz crate's lints equal to the workspace's.
