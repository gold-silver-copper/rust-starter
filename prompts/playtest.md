# Playtest: PROGRAM

Fill in the CAPITALIZED parts, then give this to a fresh agent.

You are playtesting PROGRAM (WHAT IT IS, IN ONE LINE) as a real user would.
Build the release binary once into a private directory
(`cargo build --release --locked -j 2 --target-dir ~/.cache/claude-tmp/playtest-NAME`)
and use only that copy. Never build inside the repository while testing.

**Isolation, before anything else:** run the program in a private terminal
server, never in anyone's own terminal. With fux: `unset FUX_SOCKET FUX_PANE`,
make a 0700 directory under `~/.cache/claude-tmp/`, set `XDG_RUNTIME_DIR` to
a 0700 directory inside it and `FUX_SOCKET` to a socket inside it, then
`fux start`; read `fux help-agents` first. Give the program its own
`XDG_CONFIG_HOME`, `XDG_DATA_HOME` and `XDG_CACHE_HOME`. Never touch real
accounts, never post, never solve a captcha. At the end, stop every process
and server you started.

**Use it for a while.** Do the tasks a real user would: TASKS. Try sizes
40x12, 80x24 and 200x50, and resize while it runs. Wait for what is on screen;
never guess a sleep.

**Read earlier reports first** (REPORT_DIR) and do not re-report what they
found.

**Write as you go** to REPORT_DIR/playtest-DATE.md, appending each finding
when you find it:

- `[bug]` what happened, the exact steps, and file:line if you can find it.
- `[ux]` where you hesitated, guessed, or had to read the source.
- `[docs]` what the help or README left out.
- A **Worked** section: what behaved well, so fixes do not break it.

Report only. Do not change the code.
