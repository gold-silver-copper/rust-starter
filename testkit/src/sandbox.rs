//! Isolation for tests that touch the outside world: private scratch
//! directories under `target/`, child processes that are always reaped,
//! waits on a condition instead of sleeps, and a place for artifacts.
//!
//! Pass the base directory from the calling test crate, normally
//! `env!("CARGO_TARGET_TMPDIR")` (set for integration tests), so nothing is
//! written to the system temporary directory.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::{fs, thread};

use crate::check::{Failure, fail};

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A private directory, removed on drop unless `KEEP_SCRATCH=1`.
#[derive(Debug)]
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// A fresh owner-only directory `base/name-PID-N`.
    ///
    /// # Errors
    /// The I/O error that prevented creating it.
    pub fn new(base: impl AsRef<Path>, name: &str) -> Result<Self, Failure> {
        let number = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let path = base
            .as_ref()
            .join(format!("{name}-{}-{number}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|error| fail(format!("{}: {error}", path.display())))?;
        }
        fs::create_dir_all(&path).map_err(|error| fail(format!("{}: {error}", path.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { path })
    }

    /// The directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !crate::env_flag("KEEP_SCRATCH") {
            // Nothing useful can be done with a failure during unwinding or drop.
            drop(fs::remove_dir_all(&self.path));
        }
    }
}

/// A child process that is killed and reaped when dropped, so a failing
/// test cannot leak it.
#[derive(Debug)]
pub struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    /// Spawn `command` with null stdin.
    ///
    /// # Errors
    /// The spawn failure, naming the program.
    pub fn spawn(command: &mut Command) -> Result<Self, Failure> {
        let program = command.get_program().to_string_lossy().into_owned();
        let child = command
            .stdin(Stdio::null())
            .spawn()
            .map_err(|error| fail(format!("spawning {program}: {error}")))?;
        Ok(Self { child })
    }

    /// The process id.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        // Kill fails only if it already exited; wait reaps it either way.
        drop(self.child.kill());
        drop(self.child.wait());
    }
}

/// Run `command` to completion and return its output, failing with its
/// stderr when it exits unsuccessfully.
///
/// # Errors
/// The spawn failure, or a nonzero exit with the captured stderr.
pub fn run(command: &mut Command) -> Result<Output, Failure> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| fail(format!("running {program}: {error}")))?;
    if output.status.success() {
        return Ok(output);
    }
    Err(fail(format!(
        "{program} exited with {}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    )))
}

/// Poll `condition` every 20 ms until it returns `true`, failing after
/// `timeout` with `what`. Wait for the event itself, never a guessed sleep.
///
/// # Errors
/// The condition's own error, or a timeout naming `what`.
pub fn wait_until(
    timeout: Duration,
    what: &str,
    mut condition: impl FnMut() -> Result<bool, Failure>,
) -> Result<(), Failure> {
    let start = Instant::now();
    loop {
        if condition()? {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(fail(format!(
                "timed out after {timeout:?} waiting for {what}"
            )));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Files kept after a run for a person or an agent to look at: screenshots,
/// captures, traces. Saved to `base/artifacts/SUITE/` and never compared.
#[derive(Debug)]
pub struct Artifacts {
    dir: PathBuf,
}

impl Artifacts {
    /// The artifact directory for `suite`, created if needed.
    ///
    /// # Errors
    /// The I/O error that prevented creating it.
    pub fn new(base: impl AsRef<Path>, suite: &str) -> Result<Self, Failure> {
        let dir = base.as_ref().join("artifacts").join(suite);
        fs::create_dir_all(&dir).map_err(|error| fail(format!("{}: {error}", dir.display())))?;
        Ok(Self {
            dir: fs::canonicalize(&dir)?,
        })
    }

    /// Write `bytes` to `name` and return its absolute path, for the report.
    ///
    /// # Errors
    /// The I/O error that prevented writing it.
    pub fn save(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, Failure> {
        let path = self.dir.join(name);
        fs::write(&path, bytes).map_err(|error| fail(format!("{}: {error}", path.display())))?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::{Artifacts, ChildGuard, Scratch, run, wait_until};
    use crate::check::{Failure, ensure, ensure_eq};
    use crate::unit_base as base;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn scratch_is_private_and_removed() -> Result<(), Failure> {
        let scratch = Scratch::new(base(), "unit")?;
        let path = scratch.path().to_path_buf();
        ensure(path.is_dir(), "scratch directory was not created")?;
        drop(scratch);
        ensure(!path.exists(), "scratch directory survived its guard")
    }

    #[test]
    fn wait_until_times_out_with_the_reason() -> Result<(), Failure> {
        let Err(failure) = wait_until(Duration::from_millis(50), "the impossible", || Ok(false))
        else {
            return Err(Failure::new("a false condition was reported as met"));
        };
        ensure(
            failure.message().contains("the impossible"),
            failure.to_string(),
        )
    }

    #[test]
    fn wait_until_returns_once_the_condition_holds() -> Result<(), Failure> {
        let mut calls = 0_u32;
        wait_until(Duration::from_secs(10), "the third poll", || {
            calls = calls
                .checked_add(1)
                .ok_or_else(|| Failure::new("overflow"))?;
            Ok(calls == 3)
        })?;
        ensure_eq(&calls, &3, "polls")
    }

    #[test]
    fn artifacts_are_saved_with_absolute_paths() -> Result<(), Failure> {
        let scratch = Scratch::new(base(), "artifacts")?;
        let path = Artifacts::new(scratch.path(), "suite")?.save("note.txt", b"seen")?;
        ensure(
            path.is_absolute(),
            format!("{} is relative", path.display()),
        )?;
        ensure_eq(
            std::fs::read(&path)?.as_slice(),
            b"seen".as_slice(),
            "saved bytes",
        )
    }

    #[cfg(unix)]
    #[test]
    fn a_dropped_child_is_killed() -> Result<(), Failure> {
        let child = ChildGuard::spawn(Command::new("sleep").arg("30"))?;
        let pid = child.id().to_string();
        let alive = |pid: &str| {
            Command::new("kill")
                .args(["-0", pid])
                .status()
                .is_ok_and(|status| status.success())
        };
        ensure(alive(&pid), "the child never started")?;
        drop(child);
        ensure(!alive(&pid), "the child outlived its guard")
    }

    #[test]
    fn run_reports_stderr_of_a_failed_command() -> Result<(), Failure> {
        let Err(failure) = run(Command::new("sh").args(["-c", "echo broken >&2; exit 3"])) else {
            return Err(Failure::new("a failing command was reported as success"));
        };
        ensure(failure.message().contains("broken"), failure.to_string())
    }
}
