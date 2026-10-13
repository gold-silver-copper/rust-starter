//! Drive terminal programs through a private fux server.
//!
//! Each [`Fux`] starts its own server on a socket inside a private scratch
//! directory, so a test can never reach the server of the person running it,
//! and the server is force-killed on drop. Steps are fux command lines
//! (`fux help-agents` documents them); [`Fux::words`] runs one.
//!
//! When no fux with `help-agents` is installed, [`Fux::start`] returns
//! `Ok(None)` and the test should return early. Set `REQUIRE_FUX=1` where fux
//! is installed (CI, a dev machine) to turn that skip into a failure, so a
//! missing tool cannot silently pass a test. `FUX_BIN` picks the binary.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::check::{Failure, fail};
use crate::sandbox::{Scratch, run};

/// Unix socket paths are limited to about 104–108 bytes.
const MAX_SOCKET_PATH: usize = 100;

/// A private fux server, killed on drop.
#[derive(Debug)]
pub struct Fux {
    bin: OsString,
    socket: PathBuf,
    runtime: PathBuf,
    scratch: Scratch,
}

/// A pane id such as `%2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pane(String);

impl Pane {
    /// The id as fux prints it.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.0
    }
}

impl Fux {
    /// Start a private server under `base`, or `Ok(None)` when fux is not
    /// installed and `REQUIRE_FUX` is not `1`.
    ///
    /// # Errors
    /// The server failed to start, the socket path is too long, or fux is
    /// missing while `REQUIRE_FUX=1`.
    pub fn start(base: impl AsRef<Path>, name: &str) -> Result<Option<Self>, Failure> {
        let bin = env::var_os("FUX_BIN").unwrap_or_else(|| OsString::from("fux"));
        let usable = Command::new(&bin)
            .arg("help-agents")
            .output()
            .is_ok_and(|output| output.status.success());
        if !usable {
            if env::var_os("REQUIRE_FUX").is_some_and(|value| value == "1") {
                return Err(fail(format!(
                    "REQUIRE_FUX=1 but {} has no `help-agents`; install a current fux or set FUX_BIN",
                    bin.to_string_lossy()
                )));
            }
            return Ok(None);
        }
        let scratch = Scratch::new(base, name)?;
        let socket = scratch.path().join("fux.sock");
        let runtime = scratch.path().join("run");
        if socket.as_os_str().len() > MAX_SOCKET_PATH {
            return Err(fail(format!(
                "socket path {} is too long for a unix socket; use a shorter target directory",
                socket.display()
            )));
        }
        std::fs::create_dir(&runtime)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700))?;
        }
        let fux = Self {
            bin,
            socket,
            runtime,
            scratch,
        };
        fux.words(&["start"])?;
        Ok(Some(fux))
    }

    /// A `fux` command bound to this server only.
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.bin);
        command
            .env_remove("FUX_PANE")
            .env_remove("FUX_SERVER")
            .env("FUX_SOCKET", &self.socket)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .current_dir(self.scratch.path());
        command
    }

    /// Run one fux command line and return its trimmed stdout.
    ///
    /// # Errors
    /// A nonzero exit, with fux's stderr (which says what the pane was doing).
    pub fn words(&self, words: &[&str]) -> Result<String, Failure> {
        let output = run(self.command().args(words))
            .map_err(|error| error.context(format!("fux {}", words.join(" "))))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Start `command` in a `columns x rows` pane named `name`, returning once
    /// `ready` shows on its screen.
    ///
    /// # Errors
    /// The program failed to start or `ready` never showed.
    pub fn run(
        &self,
        name: &str,
        columns: u16,
        rows: u16,
        command: &str,
        ready: &str,
    ) -> Result<Pane, Failure> {
        let (columns, rows) = (columns.to_string(), rows.to_string());
        let pane = self.words(&[
            "opts",
            "str",
            name,
            "name",
            &columns,
            &rows,
            "size",
            "str",
            command,
            "command",
            "str",
            ready,
            "wait-text",
            "run",
        ])?;
        if !pane.starts_with('%') {
            return Err(fail(format!("fux run printed {pane:?}, not a pane id")));
        }
        Ok(Pane(pane))
    }

    /// Type `text`, press Enter, and wait until `then` shows. `then` must be
    /// text only the program's output contains, never part of what was typed.
    ///
    /// # Errors
    /// `then` never showed, or the program exited first.
    pub fn type_line(&self, pane: &Pane, text: &str, then: &str) -> Result<(), Failure> {
        self.words(&[
            pane.id(),
            "keys",
            "str",
            text,
            "text",
            "str",
            "Enter",
            "key",
            "str",
            then,
            "wait-text",
            "send-keys",
        ])
        .map(drop)
    }

    /// The pane's current screen as text.
    ///
    /// # Errors
    /// The pane does not exist.
    pub fn screen(&self, pane: &Pane) -> Result<String, Failure> {
        self.words(&[pane.id(), "opts", "capture"])
    }

    /// Wait up to `seconds` for the pane's program to exit; returns fux's
    /// report, such as `exited 0`.
    ///
    /// # Errors
    /// The program did not exit in time.
    pub fn wait_exit(&self, pane: &Pane, seconds: u32) -> Result<String, Failure> {
        self.words(&[
            pane.id(),
            "opts",
            "wait-exit",
            &seconds.to_string(),
            "timeout",
            "wait",
        ])
    }
}

impl Drop for Fux {
    fn drop(&mut self) {
        // The server is private to this test; nothing else can be reached.
        drop(self.command().arg("force-kill-server").output());
    }
}
