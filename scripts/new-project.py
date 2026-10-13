#!/usr/bin/env python3
"""Create a new project from rust-starter, ready for an agent to build.

    python3 scripts/new-project.py NAME DEST [--description TEXT]
        [--testkit git|path] [--no-check]

NAME is the package name (lowercase, digits, - or _). DEST must not exist.
The project gets the lint policy, CI, the cargo aliases and a minimal
program, plus an AGENTS.md that tells an agent how to build it. testkit is
not copied: the project depends on this repository's testkit by git, pinned
to the current commit, so fixes land in one place (`--testkit path` points
at this checkout instead, for testing the template). The example codec,
its tests and the fuzz crate stay here; AGENTS.md says how to add fuzzing.

Unless --no-check, it then runs the project's format, lint, test and policy
checks, so the agent starts from a green baseline.
"""

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
URL = "https://github.com/gold-silver-copper/rust-starter"

# Starter files a new project does not get: the shared testkit (a
# dependency instead), the example target and its tests, the fuzz crate
# (added when there is a parser), and this repository's own docs and tools.
SKIP = (
    "testkit/", "fuzz/", "template/", "docs/", "prompts/",
    "src/rle.rs", "tests/codec.rs", "tests/oracles/", "tests/golden/",
    "tests/simulation.rs", "tests/tui.rs",
    "scripts/new-project.py", "scripts/holdout.py",
    "README.md", "AGENTS.md",
)


def run(*command, cwd=ROOT, check=True):
    return subprocess.run(command, cwd=cwd, text=True, capture_output=True, check=check)


def sub(path, old, new):
    """Replace exactly one occurrence, so template drift fails loudly."""
    text = path.read_text()
    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected one match for {old!r}, found {text.count(old)}")
    path.write_text(text.replace(old, new))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("name")
    parser.add_argument("dest", type=Path)
    parser.add_argument("--description", default="A Rust project.")
    parser.add_argument("--testkit", choices=("git", "path"), default="git")
    parser.add_argument("--no-check", action="store_true")
    arguments = parser.parse_args()

    name, dest = arguments.name, arguments.dest.expanduser().resolve()
    if not re.fullmatch(r"[a-z][a-z0-9_-]*", name):
        raise SystemExit(f"{name!r}: use lowercase letters, digits, - and _, starting with a letter")
    if dest.exists():
        raise SystemExit(f"{dest} already exists")
    crate = name.replace("-", "_")
    rev = run("git", "rev-parse", "HEAD").stdout.strip()
    if arguments.testkit == "git":
        if run("git", "status", "--porcelain").stdout.strip():
            raise SystemExit("rust-starter has uncommitted changes; commit and push them, or use --testkit path")
        if not run("git", "branch", "-r", "--contains", rev).stdout.strip():
            raise SystemExit(f"commit {rev[:12]} is not on the remote yet; push it, or use --testkit path")
        testkit = f'testkit = {{ git = "{URL}", rev = "{rev}" }}'
    else:
        testkit = f'testkit = {{ path = "{ROOT / "testkit"}" }}'

    for relative in run("git", "ls-files").stdout.splitlines():
        if relative.startswith(SKIP) or relative in SKIP:
            continue
        target = dest / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, target)

    values = {
        "name": name,
        "description": arguments.description,
        "rev": rev[:12],
        "starter_url": URL,
        "starter_docs": f"{URL}/blob/{rev}/docs/testing/README.md",
        "starter_path": str(ROOT),
    }
    for template in (ROOT / "template").rglob("*"):
        if template.is_file():
            text = template.read_text()
            for key, value in values.items():
                text = text.replace("{{" + key + "}}", value)
            if "{{" in text:
                raise SystemExit(f"{template}: unfilled placeholder")
            target = dest / template.relative_to(ROOT / "template")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text)

    manifest = dest / "Cargo.toml"
    sub(manifest, 'members = [".", "testkit"]', 'members = ["."]')
    sub(manifest, 'name = "rust-starter"', f'name = "{name}"')
    sub(manifest, 'description = "A strict Rust starter with checked operations and fast iteration builds"',
        f'description = "{arguments.description}"')
    sub(manifest, 'testkit = { path = "testkit" }', testkit)
    sub(dest / "src/lib.rs", "pub mod rle;\n\n", "")
    sub(dest / "src/main.rs", "rust_starter::", f"{crate}::")
    sub(dest / "tests/output.rs", "rust_starter::", f"{crate}::")

    mutants = dest / ".cargo/mutants.toml"
    text = mutants.read_text()
    mutants.write_text(text[: text.index("\n# Mutants no test can tell apart")] + "\n")

    ci = dest / ".github/workflows/ci.yml"
    sub(ci, """      - name: Lints for the fuzz crate (its own workspace)
        run: cargo clippy --manifest-path fuzz/Cargo.toml --all-targets --locked -- -D warnings
""", """      - name: Lints for the fuzz crate (its own workspace)
        if: hashFiles('fuzz/Cargo.toml') != ''
        run: cargo clippy --manifest-path fuzz/Cargo.toml --all-targets --locked -- -D warnings
""")
    sub(ci, """      - name: Held-out evaluation script self-test
        run: python3 scripts/holdout.py selftest
""", "")
    sub(ci, """ \\
            --exclude testkit/src/fux.rs || status=$?""", " || status=$?")
    sub(ci, """      # Fails on missed mutants only: a mutant that hangs the tests (timeout)
      # was noticed. fux.rs needs a current fux, which this runner lacks; run
      # it locally with FUX_BIN and REQUIRE_FUX=1.
""", """      # Fails on missed mutants only: a mutant that hangs the tests (timeout)
      # was noticed.
""")
    deep = dest / ".github/workflows/deep.yml"
    sub(deep, """  fuzz:
    runs-on: ubuntu-latest""", """  fuzz:
    if: hashFiles('fuzz/Cargo.toml') != ''
    runs-on: ubuntu-latest""")
    strip_new_project_job(ci)

    run("git", "init", "-q", "-b", "main", cwd=dest)
    run("cargo", "generate-lockfile", cwd=dest)
    print(f"created {dest} ({name}) with testkit from {'git ' + rev[:12] if arguments.testkit == 'git' else 'this checkout'}")

    if not arguments.no_check:
        checks = (
            ("cargo", "fmt", "--all", "--check"),
            ("cargo", "lint"),
            ("cargo", "test", "--workspace", "--all-features", "--locked"),
            ("python3", "scripts/check-policy.py"),
        )
        for command in checks:
            result = run(*command, cwd=dest, check=False)
            if result.returncode:
                raise SystemExit(f"{' '.join(command)} failed in {dest}:\n{result.stdout}\n{result.stderr}")
        print("baseline green: fmt, lint, tests, policy")
    print(f"next: cd {dest} and follow AGENTS.md, starting with docs/spec.md")
    return 0


def strip_new_project_job(ci):
    """Drop this repository's template self-test job, the workflow's last."""
    text = ci.read_text()
    marker = "\n  # The template itself"
    if text.count(marker) != 1:
        raise SystemExit(f"{ci}: expected one template self-test job")
    ci.write_text(text[: text.index(marker)] + "\n")


if __name__ == "__main__":
    sys.exit(main())
