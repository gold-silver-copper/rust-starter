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

/// Unit tests have no `CARGO_TARGET_TMPDIR`; they use the workspace target.
#[cfg(test)]
fn unit_base() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/testkit-unit")
}
