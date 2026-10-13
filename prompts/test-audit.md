# Test audit: PROJECT

Fill in the CAPITALIZED parts.

Audit the tests of PROJECT (REPO_PATH) against `docs/testing/test-audit.md`.
Read-only unless told otherwise: do not run long suites; run single tests
only to confirm a suspicion, with capped threads.

For every test file, list each test and give it one verdict:

- **keep**: it checks a property or an independent reference, would fail if
  its bug came back, and leaves the machine alone;
- **rewrite**: name the checklist item it breaks and the smaller fix;
- **delete**: name the checklist item, and the remaining test that covers the
  same behaviour (read that test; do not trust its name), or say "code gone".

Then list behaviour in `src/` that no test would notice changing, ranked by
how much it matters. If cargo-mutants is installed, run
`cargo mutants --in-diff` on the most recent change with `-j 2` and use its
missed mutants as evidence.

Write the report to REPORT_PATH with a summary table first. If asked to apply
it, do it in one pull request, with each deletion's covering test named in
the description (no Co-Authored-By or any AI attribution in commits or the
pull request).
