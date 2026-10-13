//! The fux driver against a real fux. Skipped where none is installed, unless
//! `REQUIRE_FUX=1`; `FUX_BIN` picks the binary.

use testkit::fux::Fux;
use testkit::{Failure, ensure, ensure_eq};

#[test]
fn types_and_waits_for_output_the_input_cannot_match() -> Result<(), Failure> {
    let Some(fux) = Fux::start(env!("CARGO_TARGET_TMPDIR"), "fux-driver")? else {
        return Ok(());
    };
    // The reply is upper case, so the typed "abc" can never satisfy the wait.
    let program = r#"sh -c 'echo ready; read line; echo "got $line" | tr a-z A-Z'"#;
    let pane = fux.run("echo", 40, 6, program, "ready")?;
    fux.type_line(&pane, "abc", "GOT ABC")?;
    ensure(
        fux.screen(&pane)?.contains("GOT ABC"),
        "reply missing from the screen",
    )?;
    ensure_eq(
        fux.wait_exit(&pane, 10)?.as_str(),
        "exited 0",
        "exit report",
    )?;
    ensure(
        fux.type_line(&pane, "more", "NEVER").is_err(),
        "typing into an exited program succeeded",
    )
}
