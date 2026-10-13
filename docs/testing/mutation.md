# Mutation testing

cargo-mutants changes the code (a `>` becomes `>=`, a function returns
`Default::default()`, a branch is deleted), runs the tests, and reports each
change the tests did not notice. A missed mutant is code whose behaviour no
test pins down. It is the most direct check that a test, including one an
agent wrote, actually tests something.

```sh
cargo install cargo-mutants --locked                 # once
cargo mutants -j 2                                   # the root package
cargo mutants -j 2 -p testkit                        # another member
cargo mutants -j 2 --file src/rle.rs                 # one file
```

Results go to `mutants.out/` (ignored by git): `missed.txt`, `caught.txt`,
`timeout.txt`, `unviable.txt`, and a log and diff per mutant.

## Settings (`.cargo/mutants.toml`)

- `cap_lints = true`: mutants build with plain rustc, and the starter's
  `dead_code = "deny"` would otherwise turn many mutants into unbuildable
  "unviable" ones, hiding real gaps.
- `test_workspace = true`: tests in every member count.
- `minimum_test_timeout = 20`: a mutant that makes a loop endless times out
  instead of hanging the run.

cargo-mutants copies the tree to the temporary directory. Where `/tmp` is
RAM-backed, point `TMPDIR` at a disk directory.

## On a pull request

```sh
git diff --src-prefix=a/ --dst-prefix=b/ origin/main... > changes.diff
cargo mutants --in-diff changes.diff -j 2
```

The explicit prefixes matter: with `diff.mnemonicPrefix=true` in git config,
`--in-diff` silently finds nothing to mutate. CI runs this on every pull
request.

## Reading the results

For each missed mutant, either add a test that fails on it or write down why
it is equivalent. Examples from this repository:

- **A real gap:** nothing checked a decode error's message; the
  `Display` impl could print nothing. Fixed by `decode_errors_say_where`.
- **A real gap in the helpers:** `golden::check` could be replaced by `Ok(())`
  and every golden test would still pass. Fixed by testing the comparison
  directly.
- **Equivalent:** in a stop-and-wait protocol the sender never gets ahead, so
  `seq == len` and `seq >= len` behave the same. No test can tell them apart;
  pick the clearer one.
- **Environment-only:** code that runs only where a tool such as fux is
  installed shows as missed elsewhere. Run mutants with the tool present.

Mutants that delete cleanup code (a `Drop` that kills a child or a server)
leak those processes for real. After a run, check for leftovers from the
mutants' temporary tree and kill only those.

Do not chase a score. Each surviving mutant is a question: is this behaviour
important enough to pin?
