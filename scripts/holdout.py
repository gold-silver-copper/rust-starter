#!/usr/bin/env python3
"""Held-out evaluation sets: seal, unseal and score them.

A held-out set measures whether a change helps on work nobody tuned against.
It is written before the change, sealed so neither people nor agents read it
while iterating, and opened once, against a build locked at a recorded commit.
See docs/testing/agent-evals.md for the whole method.

    holdout.py seal DIR --ledger LEDGER [--seed N] [--commit SHA]
        Rename every file in DIR to an opaque id in shuffled order (names leak
        content), record each id's SHA-256 in LEDGER, then remove all
        permissions from DIR. Keep LEDGER outside the repository.
    holdout.py unseal DIR --ledger LEDGER
        Restore permissions and verify every hash. If the ledger names a
        commit, refuse when the working tree differs from it: the result would
        describe an unlocked design.
    holdout.py score RESULTS.jsonl
        Pass rate with a 95% Wilson interval. One JSON object per line:
        {"id": "...", "outcome": "pass" | anything else}.
    holdout.py compare BEFORE.jsonl AFTER.jsonl
        Both rates, and an exact McNemar test on the ids both runs share:
        did the change flip more failures to passes than the reverse?
    holdout.py selftest
"""

import argparse
import hashlib
import json
import math
import os
import random
import shutil
import stat
import subprocess
import sys
from pathlib import Path


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def seal(directory, ledger, seed, commit):
    files = sorted(p for p in directory.iterdir() if p.is_file())
    if not files:
        raise SystemExit(f"{directory} has no files to seal")
    if ledger.exists():
        raise SystemExit(f"{ledger} exists; a set is sealed once")
    random.Random(seed).shuffle(files)
    width = len(str(len(files)))
    staged = [(path, directory / f".sealing-{index}") for index, path in enumerate(files)]
    for path, temporary in staged:
        path.rename(temporary)
    hashes = {}
    for index, (_, temporary) in enumerate(staged, start=1):
        opaque = directory / f"h{index:0{width}d}"
        temporary.rename(opaque)
        hashes[opaque.name] = sha256(opaque)
    record = {"directory": str(directory.resolve()), "commit": commit, "files": hashes}
    ledger.parent.mkdir(parents=True, exist_ok=True)
    ledger.write_text(json.dumps(record, indent=2) + "\n")
    for path in directory.iterdir():
        path.chmod(0)
    directory.chmod(0)
    print(f"sealed {len(hashes)} files in {directory}; ledger {ledger}")


def unseal(directory, ledger, force):
    record = json.loads(ledger.read_text())
    commit = record.get("commit")
    if commit and not force:
        changed = subprocess.run(["git", "diff", "--quiet", commit, "--"], check=False).returncode
        if changed:
            raise SystemExit(
                f"the working tree differs from the locked commit {commit}; "
                "check out the lock to measure it, or pass --force to record an unlocked run"
            )
    directory.chmod(stat.S_IRWXU)
    for path in directory.iterdir():
        path.chmod(stat.S_IRUSR | stat.S_IWUSR)
    present = {path.name: sha256(path) for path in directory.iterdir() if path.is_file()}
    expected = record["files"]
    problems = [f"missing {name}" for name in expected if name not in present]
    problems += [f"unexpected {name}" for name in present if name not in expected]
    problems += [f"changed {name}" for name in expected if name in present and present[name] != expected[name]]
    if problems:
        raise SystemExit("the sealed set was altered:\n  " + "\n  ".join(problems))
    print(f"unsealed {len(present)} files; all hashes match")


def outcomes(path):
    results = {}
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        if line.strip():
            entry = json.loads(line)
            if "id" not in entry or "outcome" not in entry:
                raise SystemExit(f"{path}:{number}: each line needs an id and an outcome")
            results[str(entry["id"])] = entry["outcome"] == "pass"
    return results


def wilson(passes, total, z=1.959964):
    if total == 0:
        return (0.0, 1.0)
    rate = passes / total
    centre = rate + z * z / (2 * total)
    spread = z * math.sqrt(rate * (1 - rate) / total + z * z / (4 * total * total))
    scale = 1 + z * z / total
    return ((centre - spread) / scale, (centre + spread) / scale)


def describe(name, results):
    passes, total = sum(results.values()), len(results)
    low, high = wilson(passes, total)
    rate = passes / total if total else 0.0
    return f"{name}: {passes}/{total} pass = {rate:.1%} (95% CI {low:.1%} to {high:.1%})"


def mcnemar(gained, lost):
    """Exact two-sided p-value for `gained` vs `lost` discordant pairs."""
    pairs = gained + lost
    if pairs == 0:
        return 1.0
    tail = sum(math.comb(pairs, k) for k in range(min(gained, lost) + 1)) / 2**pairs
    return min(1.0, 2 * tail)


def compare(before, after):
    shared = sorted(set(before) & set(after))
    gained = sum(1 for key in shared if after[key] and not before[key])
    lost = sum(1 for key in shared if before[key] and not after[key])
    lines = [
        describe("before", before),
        describe("after", after),
        f"paired on {len(shared)} ids: {gained} newly pass, {lost} newly fail, "
        f"exact McNemar p = {mcnemar(gained, lost):.3g}",
    ]
    only = len(set(before) ^ set(after))
    if only:
        lines.append(f"{only} ids appear in only one run and are not paired")
    return "\n".join(lines)


def selftest():
    import tempfile

    base = Path(__file__).resolve().parents[1] / "target" / "holdout-selftest"
    base.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=base) as temporary:
        root = Path(temporary)
        specs, ledger = root / "specs", root / "notes" / "ledger.json"
        specs.mkdir()
        for name in ("parse-dates.md", "retry-backoff.md", "unicode-width.md"):
            (specs / name).write_text(f"spec {name}\n")
        seal(specs, ledger, seed=1, commit=None)
        if os.access(specs, os.R_OK) and os.geteuid() != 0:
            raise SystemExit("selftest: a sealed directory is still readable")
        unseal(specs, ledger, force=False)
        names = sorted(path.name for path in specs.iterdir())
        if names != ["h1", "h2", "h3"]:
            raise SystemExit(f"selftest: unexpected opaque names {names}")
        (specs / "h2").write_text("tampered\n")
        specs.chmod(0)
        try:
            unseal(specs, ledger, force=False)
        except SystemExit as error:
            if "changed h2" not in str(error):
                raise
        else:
            raise SystemExit("selftest: tampering was not detected")
        finally:
            specs.chmod(stat.S_IRWXU)
        low, high = wilson(41, 100)
        if not (0.31 < low < 0.32 and 0.50 < high < 0.52):
            raise SystemExit(f"selftest: Wilson interval {low:.4f}..{high:.4f}")
        if not (0.0117 < mcnemar(10, 1) < 0.0118) or mcnemar(5, 5) != 1.0:
            raise SystemExit("selftest: McNemar p-values")
        shutil.rmtree(specs, ignore_errors=True)
    print("holdout selftest passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    sealing = commands.add_parser("seal")
    sealing.add_argument("directory", type=Path)
    sealing.add_argument("--ledger", type=Path, required=True)
    sealing.add_argument("--seed", type=int, default=random.SystemRandom().randrange(2**32))
    sealing.add_argument("--commit")
    unsealing = commands.add_parser("unseal")
    unsealing.add_argument("directory", type=Path)
    unsealing.add_argument("--ledger", type=Path, required=True)
    unsealing.add_argument("--force", action="store_true")
    scoring = commands.add_parser("score")
    scoring.add_argument("results", type=Path)
    comparing = commands.add_parser("compare")
    comparing.add_argument("before", type=Path)
    comparing.add_argument("after", type=Path)
    commands.add_parser("selftest")
    arguments = parser.parse_args()
    if arguments.command == "seal":
        seal(arguments.directory, arguments.ledger, arguments.seed, arguments.commit)
    elif arguments.command == "unseal":
        unseal(arguments.directory, arguments.ledger, arguments.force)
    elif arguments.command == "score":
        print(describe("run", outcomes(arguments.results)))
    elif arguments.command == "compare":
        print(compare(outcomes(arguments.before), outcomes(arguments.after)))
    else:
        selftest()
    return 0


if __name__ == "__main__":
    sys.exit(main())
