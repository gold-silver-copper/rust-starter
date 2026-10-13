//! Example target for the testing templates: a run-length codec.
//!
//! A decoder for untrusted bytes is the classic shape for fuzzing, property
//! tests and golden files; `tests/codec.rs`, `tests/oracles/rle.rs` and
//! `fuzz/` exercise it. Replace it with your own code and keep the patterns.
//!
//! Format: a sequence of `(count, byte)` pairs, `count` in `1..=255`.

use std::fmt::{self, Display};

/// Why bytes are not a valid encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ends in the middle of a pair.
    Truncated,
    /// The pair with this index (from 0) has a count of zero.
    ZeroCount(usize),
}

impl Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("input ends in the middle of a pair"),
            Self::ZeroCount(pair) => write!(formatter, "pair {pair} has a zero count"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Encode `data` as `(count, byte)` pairs.
#[must_use]
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut run: Option<(u8, u8)> = None;
    for &byte in data {
        run = match run {
            // A full run (255) ends like a change of byte does.
            Some((count, current)) => match count.checked_add(1) {
                Some(longer) if current == byte => Some((longer, current)),
                _ => {
                    out.extend([count, current]);
                    Some((1, byte))
                }
            },
            None => Some((1, byte)),
        };
    }
    if let Some((count, current)) = run {
        out.extend([count, current]);
    }
    out
}

/// Decode `(count, byte)` pairs.
///
/// # Errors
/// [`DecodeError`] for a trailing half pair or a zero count.
pub fn decode(encoded: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let (pairs, rest) = encoded.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(DecodeError::Truncated);
    }
    let mut out = Vec::new();
    for (index, &[count, byte]) in pairs.iter().enumerate() {
        if count == 0 {
            return Err(DecodeError::ZeroCount(index));
        }
        out.extend(std::iter::repeat_n(byte, usize::from(count)));
    }
    Ok(out)
}
