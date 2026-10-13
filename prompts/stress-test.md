# Stress test: PROGRAM, area AREA

Fill in the CAPITALIZED parts. Run one agent per area (for example: input
parsing, waiting and timing, lifecycle and cleanup, large or hostile input,
odd sizes and resizes), in parallel.

You are trying to break PROGRAM in the area AREA. Use the frozen binary at
BINARY_PATH only; it will not change while you work.

**Isolation:** a private server or socket in a 0700 directory under
`~/.cache/claude-tmp/`, your own `XDG_*` directories, no access to anyone's
own session or files. Cap parallel work to two cores. Never run anything over
about ten minutes. Stop every process you started before you finish.

**Method:** read the user-facing docs (DOCS) as a user would, then attack the
area: boundaries, empty and huge inputs, Unicode, malformed input, interrupted
operations, races between two clients, programs that ignore signals. Prefer
small reproductions.

**Report only**, to REPORT_PATH. For each finding: a minimal reproduction
(exact commands), what happened, what the docs say should happen, and how
sure you are. Mark each as `confirmed` (reproduced twice) or `suspected`.
Do not change the code; a coordinator fixes everything in one pull request
and you will be asked to re-check on the new build.
