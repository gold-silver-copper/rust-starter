#!/usr/bin/env python3
"""Exercise the actual lint configuration with accepted/rejected Rust programs."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]

# Each case must fail for the intended rule, not merely fail compilation.
CASES = {
    "unsafe": ("unsafe_code", "pub unsafe fn probe() {}"),
    "unwrap": ("clippy::unwrap_used", "pub fn probe(v: Option<u8>) -> u8 { v.unwrap() }"),
    "expect": ("clippy::expect_used", 'pub fn probe(v: Option<u8>) -> u8 { v.expect("value") }'),
    "panic": ("clippy::panic", 'pub fn probe() { panic!("failure") }'),
    "index": ("clippy::indexing_slicing", "pub fn probe(v: &[u8], i: usize) -> u8 { v[i] }"),
    "arithmetic": ("clippy::arithmetic_side_effects", "pub fn probe(a: u8, b: u8) -> u8 { a + b }"),
    "cast": ("clippy::cast_possible_truncation", "pub fn probe(v: u64) -> u8 { v as u8 }"),
    "todo": ("clippy::todo", "pub fn probe() { todo!() }"),
    "unreachable": ("clippy::unreachable", "pub fn probe() { unreachable!() }"),
    "unimplemented": ("clippy::unimplemented", "pub fn probe() { unimplemented!() }"),
    "print": ("clippy::print_stdout", 'pub fn probe() { println!("hello"); }'),
    "stderr": ("clippy::print_stderr", 'pub fn probe() { eprintln!("hello"); }'),
    "dbg": ("clippy::dbg_macro", "pub fn probe() { dbg!(); }"),
    "remove": ("clippy::disallowed_methods", "pub fn probe(v: &mut Vec<u8>) { v.remove(0); }"),
    "copy": ("clippy::disallowed_methods", "pub fn probe(a: &mut [u8], b: &[u8]) { a.copy_from_slice(b); }"),
    "chunks": ("clippy::disallowed_methods", "pub fn probe(v: &[u8], n: usize) { let _chunks = v.chunks(n); }"),
    "borrow": ("clippy::disallowed_methods", "pub fn probe(v: &std::cell::RefCell<u8>) { let _value = v.borrow(); }"),
    "abort": ("clippy::disallowed_methods", "pub fn probe() { std::process::abort() }"),
    "temp_dir": ("clippy::disallowed_methods", "pub fn probe() -> std::path::PathBuf { std::env::temp_dir() }"),
    "override": ("E0453", '#[expect(clippy::unwrap_used, reason = "test exemption")]\npub fn probe(v: Option<u8>) -> u8 { v.unwrap() }'),
}
for macro in ("assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne"):
    args = "v" if macro in ("assert", "debug_assert") else "v, false"
    CASES[macro] = ("clippy::disallowed_macros", f"pub fn probe(v: bool) {{ {macro}!({args}); }}")

ACCEPTED = '''pub fn probe() -> Result<(), &'static str> {
    let values = [1_u8, 2];
    let first = values.first().ok_or("empty input")?;
    let sum = first.checked_add(2).ok_or("overflow")?;
    if sum != 3 { return Err("unexpected sum"); }
    Ok(())
}
'''


def run_cargo(directory, *args):
    # Ambient flags must not weaken the fixtures or redirect their artifacts.
    env = os.environ.copy()
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CLIPPY_CONF_DIR"):
        env.pop(key, None)
    env["CARGO_TARGET_DIR"] = str(ROOT / "target" / "policy-build")
    return subprocess.run(
        ["cargo", *args], cwd=directory, env=env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )


# The fuzz crate is outside the workspace (nightly, sanitizers) and copies
# the policy. These are the only differences it may have.
FUZZ_EXCEPTIONS = {
    ("clippy", "panic"): ("forbid", "deny"),
    ("rust", "dead_code"): ("deny", "allow"),
}


def check_fuzz_lints(workspace_lints):
    path = ROOT / "fuzz" / "Cargo.toml"
    if not path.exists():
        return
    fuzz = tomllib.loads(path.read_text()).get("lints", {})
    for tool in sorted(set(workspace_lints) | set(fuzz)):
        ours, theirs = workspace_lints.get(tool, {}), fuzz.get(tool, {})
        for lint in sorted(set(ours) | set(theirs)):
            want, have = ours.get(lint), theirs.get(lint)
            if want != have and FUZZ_EXCEPTIONS.get((tool, lint)) != (want, have):
                raise SystemExit(f"fuzz/Cargo.toml lints.{tool}.{lint} is {have!r}; the workspace has {want!r}")


def main():
    metadata = run_cargo(ROOT, "metadata", "--no-deps", "--format-version=1", "--locked")
    if metadata.returncode:
        raise SystemExit(metadata.stderr)
    workspace = json.loads(metadata.stdout)
    for package in workspace["packages"]:
        if package["id"] in workspace["workspace_members"]:
            manifest = tomllib.loads(Path(package["manifest_path"]).read_text())
            if manifest.get("lints", {}).get("workspace") is not True:
                raise SystemExit(f"{package['name']} must inherit [lints] workspace = true")

    check_fuzz_lints(tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["lints"])

    scratch = ROOT / "target" / "policy-fixtures"
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch) as temporary:
        project = Path(temporary)
        for name in ("clippy.toml", "rust-toolchain.toml"):
            shutil.copyfile(ROOT / name, project / name)
        # Use the real workspace tables, independently of the app's dependencies.
        manifest = (ROOT / "Cargo.toml").read_text()
        policy = manifest[manifest.index("[workspace.lints.rust]"):]
        (project / "Cargo.toml").write_text(
            '[workspace]\n[package]\nname = "policy-probe"\nversion = "0.0.0"\n'
            'edition = "2024"\n[lints]\nworkspace = true\n' + policy
        )
        (project / "src").mkdir()
        (project / "tests").mkdir()
        checked = 0
        for context in ("production", "unit", "integration"):
            for name, (expected, source) in {"accepted": (None, ACCEPTED), **CASES}.items():
                # Referencing the probe also keeps dead_code from masking a failure.
                test = "\n#[test] fn fixture() { let _probe = probe; }\n"
                library = source if context == "production" else ""
                integration = ""
                if context == "unit":
                    library = "#[cfg(test)] mod tests {\n" + source + test + "}\n"
                elif context == "integration":
                    integration = source + test
                (project / "src/lib.rs").write_text(library)
                (project / "tests/policy.rs").write_text(integration)
                result = run_cargo(project, "clippy", "--offline", "--all-targets", "--message-format=json", "--", "-D", "warnings")
                codes = set()
                for line in result.stdout.splitlines():
                    message = json.loads(line)
                    if message.get("reason") == "compiler-message":
                        diagnostic = message["message"]
                        if diagnostic["level"] == "error" and diagnostic.get("code"):
                            codes.add(diagnostic["code"]["code"])
                passed = result.returncode == 0 if expected is None else result.returncode != 0 and expected in codes
                if not passed:
                    raise SystemExit(f"{context}/{name}: expected {expected or 'success'}\n{result.stdout}\n{result.stderr}")
                checked += 1
        print(f"Policy verified: {checked} probes across production, unit tests and integration tests.")


if __name__ == "__main__":
    main()
