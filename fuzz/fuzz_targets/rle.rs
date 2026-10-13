//! `cargo +nightly fuzz run rle`: the same properties `tests/codec.rs` checks,
//! explored by libFuzzer's coverage feedback instead of a seeded generator.

#![no_main]

#[path = "../../tests/oracles/rle.rs"]
mod oracle;

use libfuzzer_sys::fuzz_target;

/// Report a property failure the only way libFuzzer understands.
#[expect(clippy::panic, reason = "libFuzzer records a failure only on panic or abort")]
fn fail(message: &testkit::Failure) -> ! {
    panic!("{message}")
}

fuzz_target!(|input: &[u8]| {
    if let Err(failure) = oracle::check(input) {
        fail(&failure);
    }
});
