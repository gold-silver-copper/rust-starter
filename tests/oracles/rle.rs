//! Properties of the codec, shared by the stable tests (`tests/codec.rs`) and
//! the nightly fuzz target (`fuzz/fuzz_targets/rle.rs`), so normal CI checks
//! exactly what fuzzing checks and replays what fuzzing found.

use rust_starter::rle::{DecodeError, decode, encode};
use testkit::{Failure, ensure, ensure_eq, fail};

/// Everything that must hold for `input`, read both as data to encode and
/// as untrusted bytes to decode.
pub fn check(input: &[u8]) -> Result<(), Failure> {
    let encoded = encode(input);
    ensure_eq(decode(&encoded)?.as_slice(), input, "decode(encode(input))")?;
    canonical(&encoded)?;
    match decode(input) {
        Ok(decoded) => ensure_eq(
            decode(&encode(&decoded))?.as_slice(),
            decoded.as_slice(),
            "re-encoded output",
        ),
        Err(DecodeError::Truncated) => ensure(
            !input.len().is_multiple_of(2),
            "an even-length input was called truncated",
        ),
        Err(DecodeError::ZeroCount(pair)) => {
            let count = input.get(
                pair.checked_mul(2)
                    .ok_or_else(|| Failure::new("pair index overflow"))?,
            );
            ensure_eq(&count, &Some(&0), "count of the reported pair")
        }
    }
}

/// The encoder never wastes space: no zero counts, and a run is split only
/// when it is full.
fn canonical(encoded: &[u8]) -> Result<(), Failure> {
    let (pairs, _) = encoded.as_chunks::<2>();
    for (pair, next) in pairs.iter().zip(pairs.iter().skip(1)) {
        let ([count, byte], [_, next_byte]) = (*pair, *next);
        if count == 0 || (next_byte == byte && count != u8::MAX) {
            return Err(fail(format!(
                "not canonical at {pair:?}, {next:?} in {encoded:?}"
            )));
        }
    }
    Ok(())
}
