//! Templates for property tests, corpus replay and golden files, applied to
//! the example codec. Copy the shape; replace the codec.

#[path = "oracles/rle.rs"]
mod oracle;

use rust_starter::rle::{decode, encode};
use testkit::{Explore, Failure, ensure_eq, explore, golden};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");
const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fuzz/corpus/rle");

/// Random bytes, shrunk on failure. `TEST_SEED` replays a run and
/// `TEST_CASES` raises the count (the scheduled CI job runs far more).
#[test]
fn properties_hold_for_random_bytes() -> Result<(), Failure> {
    Explore::from_env(2_000)?.bytes(600, oracle::check)
}

/// Long runs matter here (counts saturate at 255) and uniform random bytes
/// rarely repeat, so a second generator biases toward runs.
#[test]
fn properties_hold_for_long_runs() -> Result<(), Failure> {
    Explore::from_env(300)?.check_shrinking(
        |rng| {
            let mut data = Vec::new();
            for _ in 0..rng.below(6).unwrap_or_default() {
                let len = rng.index(700).unwrap_or_default();
                let byte = *rng.pick(b"ab\0").unwrap_or(&b'a');
                data.extend(std::iter::repeat_n(byte, len));
            }
            data
        },
        |data| explore::shrink_bytes(data),
        |data| oracle::check(data),
    )
}

/// Everything the fuzzer kept (committed `seed-*` and `regression-*` files)
/// runs on stable in every CI build.
#[test]
fn fuzz_corpus_replays() -> Result<(), Failure> {
    let replayed = explore::replay_dir(CORPUS, oracle::check)?;
    // A corpus that silently went missing would make this test vacuous.
    ensure_eq(&(replayed > 0), &true, "corpus files found")
}

/// The encoding is a format other programs may read, so it is pinned. Small
/// targeted cases, never a dump of everything.
#[test]
fn encoding_is_stable() -> Result<(), Failure> {
    let mut report = String::new();
    for input in [&b""[..], b"a", b"aaab", b"abab", &[7; 300]] {
        let encoded = encode(input);
        ensure_eq(decode(&encoded)?.as_slice(), input, "round trip")?;
        report.push_str(&format!("{} bytes -> {:02x?}\n", input.len(), encoded));
    }
    golden::check(GOLDEN, "rle-encoding.txt", &report)
}

/// Error messages are part of the interface: they must say where input broke.
#[test]
fn decode_errors_say_where() -> Result<(), Failure> {
    let error = decode(&[1, b'a', 0, b'b'])
        .err()
        .ok_or_else(|| Failure::new("zero count accepted"))?;
    ensure_eq(
        error.to_string().as_str(),
        "pair 1 has a zero count",
        "message",
    )
}
