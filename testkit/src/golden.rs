//! Golden files: compare text output with a reviewed file in the repository.
//!
//! Use them for formats that are a contract (a wire format, a file format, a
//! CLI's machine-readable output), not for whole screens or anything that is
//! expected to improve. `UPDATE_GOLDEN=1` rewrites the files; review the
//! change with `git diff` and commit it on its own or with its explanation.

use std::path::Path;
use std::{fs, io};

use crate::check::{Failure, line_diff};

/// Compare `actual` with `dir/name`, or write it when `UPDATE_GOLDEN=1`.
///
/// Pass `dir` from the calling crate, for example
/// `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden")`.
///
/// # Errors
/// A missing or different file, with a line diff and the update command, or
/// the I/O error that stopped the comparison.
pub fn check(dir: impl AsRef<Path>, name: &str, actual: &str) -> Result<(), Failure> {
    let update = crate::env_flag("UPDATE_GOLDEN");
    compare(&dir.as_ref().join(name), actual, update)
}

fn compare(path: &Path, actual: &str, update: bool) -> Result<(), Failure> {
    let shown = path.display();
    if update {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| Failure::new(format!("{}: {error}", parent.display())))?;
        }
        return fs::write(path, actual).map_err(|error| Failure::new(format!("{shown}: {error}")));
    }
    let expected = match fs::read_to_string(path) {
        Ok(expected) => expected,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(Failure::new(format!(
                "golden file {shown} is missing; create it with UPDATE_GOLDEN=1 and review it"
            )));
        }
        Err(error) => return Err(Failure::new(format!("{shown}: {error}"))),
    };
    if expected == actual {
        return Ok(());
    }
    Err(Failure::new(format!(
        "golden file {shown} differs (- file, + actual); if the change is intended, \
         rerun with UPDATE_GOLDEN=1 and review the diff\n{}",
        line_diff(&expected, actual)
    )))
}

#[cfg(test)]
mod tests {
    use super::{check, compare};
    use crate::check::{Failure, ensure};
    use crate::sandbox::Scratch;

    #[test]
    fn compares_creates_and_reports() -> Result<(), Failure> {
        let scratch = Scratch::new(crate::unit_base(), "golden")?;
        let path = scratch.path().join("nested/case.txt");
        let Err(missing) = compare(&path, "one\n", false) else {
            return Err(Failure::new("a missing golden file passed"));
        };
        ensure(missing.message().contains("missing"), missing.to_string())?;
        compare(&path, "one\ntwo\n", true)?;
        compare(&path, "one\ntwo\n", false)?;
        let Err(changed) = compare(&path, "one\nthree\n", false) else {
            return Err(Failure::new("a changed output passed"));
        };
        ensure(
            changed.message().contains("   2 - two\n   2 + three"),
            changed.to_string(),
        )?;
        let dir = scratch.path().join("nested");
        ensure(
            check(&dir, "case.txt", "other\n").is_err(),
            "check passed a mismatch",
        )
    }
}
