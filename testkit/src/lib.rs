//! Test helpers that keep the starter's no-panic policy: every check returns
//! a [`Failure`] instead of asserting, every random choice comes from one
//! printed [`Seed`], and every outside resource is private and cleaned up.
//!
//! See `docs/testing/README.md` for which technique fits which problem.

pub mod check;
pub mod explore;
pub mod frame;
pub mod fux;
pub mod golden;
pub mod rng;
pub mod sandbox;
pub mod sim;

pub use check::{Failure, ensure, ensure_eq, fail};
pub use explore::Explore;
pub use rng::{Rng, Seed};

/// Whether the opt-in environment variable `name` is set to `1`.
fn env_flag(name: &str) -> bool {
    is_on(std::env::var_os(name).as_deref())
}

fn is_on(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

/// Unit tests have no `CARGO_TARGET_TMPDIR`; they use the workspace target.
#[cfg(test)]
fn unit_base() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/testkit-unit")
}

#[cfg(test)]
mod tests {
    use super::is_on;
    use crate::check::{Failure, ensure_eq};

    #[test]
    fn only_one_turns_a_flag_on() -> Result<(), Failure> {
        let values = [None, Some("1"), Some("0"), Some(""), Some("true")];
        let on: Vec<bool> = values
            .iter()
            .map(|value| is_on(value.map(std::ffi::OsStr::new)))
            .collect();
        ensure_eq(&on, &vec![false, true, false, false, false], "flags")
    }
}
