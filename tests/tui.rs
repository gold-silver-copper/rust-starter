//! Template: drive the real binary in a terminal through a private fux server.
//!
//! The test checks facts about the screen ("the greeting is shown", "the
//! program exited 0"), never the whole screen, so layout can improve without
//! rewriting tests. The capture is saved as an artifact for a person or agent
//! to read. Without a current fux the test returns early, unless
//! `REQUIRE_FUX=1` makes that a failure; `FUX_BIN` picks the binary.

use testkit::fux::Fux;
use testkit::sandbox::Artifacts;
use testkit::{Failure, ensure, ensure_eq};

const TARGET_TMP: &str = env!("CARGO_TARGET_TMPDIR");
const BINARY: &str = env!("CARGO_BIN_EXE_rust-starter");

#[test]
fn greets_in_a_real_terminal() -> Result<(), Failure> {
    let Some(fux) = Fux::start(TARGET_TMP, "tui")? else {
        return Ok(());
    };
    let pane = fux.run("greeting", 40, 6, BINARY, "Hello")?;
    ensure_eq(
        fux.wait_exit(&pane, 10)?.as_str(),
        "exited 0",
        "exit report",
    )?;
    let screen = fux.screen(&pane)?;
    let saved = Artifacts::new(TARGET_TMP, "tui")?.save("greeting.txt", screen.as_bytes())?;
    ensure(
        screen.lines().any(|line| line.trim() == "Hello, world!"),
        format!(
            "greeting missing from the screen; capture: {}",
            saved.display()
        ),
    )
}
