# Test audit checklist

Use it before adding tests and, especially, before deleting them. It comes
from the koh test-removal audit (140 tests removed; 8 turned out to be real
losses, restored in one pull request) and the fux and koh audits.
`prompts/test-audit.md` runs it as an agent task.

## Delete or rewrite a test when it

1. Asserts behaviour that was deliberately replaced.
2. Compares with an older build, or pins bytes, pixels or screens produced by
   rendering. Exception: a hand-built encoding of a versioned contract.
3. Tests a dependency's job (the standard library, the async runtime, the
   network crate).
4. Can never run in CI: ignored, or gated on a variable no job sets.
5. Cannot fail: it returns early, its predicate is always true, it accepts any
   outcome, it never checks its precondition happened, or a missing tool
   counts as a pass.
6. Hard-codes incidental numbers: how many recordings exist, an exact error
   string that is not a contract.
7. Duplicates another test. Merge them into one table-driven test and keep
   every check.
8. Only prints.

## Before deleting, prove coverage

9. Name the remaining test that covers the same behaviour, and read it at HEAD
   rather than trusting its name.
10. Every verdict is one of: **covered** (by which test), **code gone**, or
    **lost**. Rank the losses and restore them.
11. Do not quietly shrink stress tests (100 rounds on 4 threads to 1 round on
    1 thread is a loss).
12. A round trip does not replace a pinned format: a changed layout still
    round-trips.

## Every new or kept test

13. Fails when its bug is put back. Break the code, watch the test fail,
    restore. `cargo mutants --in-diff` does this for a whole change.
14. Checks a property or an independent reference, not the current output.
15. Has a name that states the property in plain words.
16. Waits on conditions, never on guessed sleeps.
17. Is deterministic, or prints the seed that reproduces it.
18. Leaves the machine alone: no `/tmp`, real `HOME`, shell history,
    credentials, real accounts, or the person's own terminal session. Every
    process it starts is killed.
19. Takes seconds in the per-push run; longer variants run on a schedule with
    a budget.
20. Cannot pass for the wrong reason, such as matching typed echo or a skipped
    tool.
