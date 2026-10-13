# Testing with agents

When the product is used by agents (a CLI, a language, a library's API, its
docs), or when a person's judgement is the test (UX, discoverability), agents
can run the evaluation. These methods come from fux's agent benchmark and its
command-language redesign, ck's playtest loop, and the koh audits. They cost
tokens and machine time, so each one is a deliberate run, not part of `cargo
test`.

## Principles

- **A checker decides, never the agent's claim.** The task writes its outcome
  to a file (the chosen menu item, the exit status, an answer), and code
  compares it. Agents report success they did not achieve.
- **Every grade is a claim.** Re-run what the agent produced mechanically,
  then have a stronger model confirm or overrule grades. In the fuxth corpus,
  review changed 52 of 300 grades.
- **Writers see only what a user sees**: README, help, examples, never the
  implementation or the fixtures' source. That tests the docs too. In one fux
  round, 10 of 100 agents read the fixture source because the goal printed its
  path.
- **Isolate every run**: a private server or socket, own `XDG_*` directories,
  no access to the person's session, a frozen copy of the binary.
- **Ask for friction**: end every run with one `FRICTION:` line naming the
  hardest moment. Those lines, counted across runs, find the fixes.
- **Measure cost, not just success**: commands, polls and seconds slept per
  task show whether a tool is getting easier to use.
- **Expect noise**: identical agent runs differ (fux saw "no friction" counts
  swing between 2 and 19). Repeat, report intervals, and compare paired runs.

## Task suites

A task is a goal in plain words plus a checker. fux's benchmark uses one TOML
table per task:

```toml
[[task]]
id = "menu-delta"
category = "tui"
goal = "Start `{fixture} menu`. Choose `delta` (j/k move, Enter chooses)."
check = { file = "chosen.txt", equals = "delta" }
```

- **Fixtures** are small deterministic programs that stand in for real ones
  (a menu, a form, a spinner that asks permission, a process that ignores
  TERM). They write outcomes to files and exit when their terminal closes;
  leaked fixtures once took every core of the machine.
- **Two tiers.** A scripted tier solves every task with a recipe and runs in
  CI (`--strict`): it proves the tasks are solvable and catches regressions
  cheaply. An agent tier gives a fresh, cheap model only the goal and how to
  learn the tool.
- **Compare tools on the same tasks** (fux was measured against tmux, zellij
  and herdr), all of them on all tasks.
- **Check the docs cover the tasks**: fux's `guide_check.py` fails if the
  first 60 lines of the agent guide do not name every command the scripted
  tier uses.
- Log real tool calls with wrapper scripts on `PATH` rather than trusting the
  transcript.

## Held-out specs

To change a design and know whether it helped, measure on work the design was
not tuned against.

1. **Write specs before changing anything.** Each spec: an id, a statement in
   plain words, fixed inputs, and an expected output **computed outside the
   system under test** (for fuxth, a Python script).
2. **Pilot** about 20 specs to tune the prompt, the report format and timing.
3. **Split.** A tuning set (fuxth: 300) informs the design. A held-out set
   (fuxth: 1,000), written by a different agent that never saw the tuning set,
   is sealed:

   ```sh
   python3 scripts/holdout.py seal evals/holdout --ledger ~/notes/PROJECT/holdout-ledger.json
   ```

   Files get opaque, shuffled ids (names leak content), their SHA-256 hashes
   go into a ledger kept outside the repository, and the directory loses all
   permissions so neither people nor agents read it while iterating.
4. **Lock the design** at a commit and record it in the ledger
   (`seal --commit SHA`, or edit the ledger before unsealing).
5. **Unseal once**, on the locked build. `holdout.py unseal` refuses if the
   working tree differs from the locked commit and verifies every hash.
6. **Run** one fresh writer per spec. Each report grades **worked / hack /
   impossible**, names the one missing piece and which layer it belongs to
   (core, library, checker, docs), and gives minimal repros of bugs. Re-run
   each program mechanically; have reviewers confirm grades.
7. **Score** with intervals and compare paired runs:

   ```sh
   python3 scripts/holdout.py score after.jsonl
   python3 scripts/holdout.py compare before.jsonl after.jsonl
   ```

   `compare` prints both pass rates with 95% Wilson intervals and an exact
   McNemar test over the specs both runs share. Tuning-set gain minus
   held-out gain estimates how much the design overfit the tuning set.

Every spec program is also free input for the mechanical tests: feed them to
the differential and fuzz harnesses as seeds.

## QA waves

Many cheap runs test the product against its documentation and file bug
claims; a few strong-model judges confirm or reject each claim against the
newest build; confirmed bugs are fixed with a test that fails before the fix;
the next wave runs on the new build. fuxth's claims fell 980, 695, 266, …, 163
over successive waves. Word the task as quality assurance against the
documentation: one provider's filter flagged "bug hunter" as a security risk.

## Stress testing

Several strong agents, one per area (parsing, waiting, shells, full-screen
programs, lifecycle), each with a frozen copy of the binary and a private
server, try to break it and **only report**. A coordinator fixes everything
in one pull request; the agents re-check on the new build.
`prompts/stress-test.md` is a template.

## Playtests

An agent runs the release build in a private terminal at realistic sizes,
uses it for a while as a person would, and writes a dated report: bugs with
file and line, rough edges, and an explicit "what worked" list. Then fix in
one pull request and repeat (ck went through seven rounds in a day). Testers
read earlier reports and do not redo them, append findings as they go, and
follow hard rules (never post, never solve a captcha, never touch real
accounts). `prompts/playtest.md` is a template.

## Mining transcripts

Past agent sessions are data. Count sleeps, hand-written helper scripts and
polling loops in transcripts of agents using the tool; each pattern is a
missing feature (fux's `wait` command came from ~450 sleeps in 290 calls).
