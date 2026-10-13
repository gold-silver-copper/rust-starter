//! Seeds and a dependency-free deterministic generator.
//!
//! Every randomized test takes one [`Seed`]. Locally it is fixed, so a run is
//! repeatable; in CI it comes from the commit, so each commit explores new
//! cases yet any failure replays from the printed `TEST_SEED`.

use std::env;
use std::fmt::{self, Display};

use crate::check::{Failure, fail};

/// The seed used when neither `TEST_SEED` nor `GITHUB_SHA` is set.
pub const DEFAULT_SEED: u64 = 0x5EED;

/// The root of one randomized run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seed(u64);

impl Seed {
    /// A seed with this exact value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// `TEST_SEED` (decimal or `0x` hex), else the first 16 hex digits of
    /// `GITHUB_SHA`, else [`DEFAULT_SEED`].
    ///
    /// # Errors
    /// Fails when `TEST_SEED` is set but is not a number.
    pub fn from_env() -> Result<Self, Failure> {
        if let Ok(text) = env::var("TEST_SEED") {
            return parse(&text)
                .map(Self)
                .ok_or_else(|| fail(format!("TEST_SEED={text:?} is not a decimal or 0x-hex u64")));
        }
        let commit = env::var("GITHUB_SHA").ok();
        let from_commit = commit
            .as_deref()
            .and_then(|sha| sha.get(..16))
            .and_then(|digits| u64::from_str_radix(digits, 16).ok());
        Ok(Self(from_commit.unwrap_or(DEFAULT_SEED)))
    }

    /// A generator for this seed.
    #[must_use]
    pub const fn rng(self) -> Rng {
        Rng::new(self.0)
    }

    /// The environment prefix that reproduces this run: `TEST_SEED=0x…`.
    #[must_use]
    pub fn repro(self) -> String {
        format!("TEST_SEED={self}")
    }
}

impl Display for Seed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#018x}", self.0)
    }
}

fn parse(text: &str) -> Option<u64> {
    let text = text.trim();
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(&hex.replace('_', ""), 16).ok(),
        None => text.replace('_', "").parse().ok(),
    }
}

/// `SplitMix64`: tiny, fast, and fully determined by its seed on every
/// platform and Rust version. Not for cryptography.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator starting from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 random bits. Wrapping arithmetic is the algorithm here.
    pub const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    /// An independent generator, for a sub-system or a single case.
    #[must_use]
    pub const fn fork(&mut self) -> Self {
        Self::new(self.next_u64())
    }

    /// A value in `0..bound` without modulo bias, or `None` when `bound` is 0.
    pub fn below(&mut self, bound: u64) -> Option<u64> {
        let threshold = bound.wrapping_neg().checked_rem(bound)?;
        loop {
            let wide = u128::from(self.next_u64()).checked_mul(u128::from(bound))?;
            if u128::from(threshold) <= wide & u128::from(u64::MAX) {
                return u64::try_from(wide >> 64).ok();
            }
        }
    }

    /// A value in `0..bound` as an index, or `None` when `bound` is 0.
    pub fn index(&mut self, bound: usize) -> Option<usize> {
        let value = self.below(u64::try_from(bound).ok()?)?;
        usize::try_from(value).ok()
    }

    /// True with probability `numerator / denominator` (false for a zero denominator).
    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator)
            .is_some_and(|value| value < numerator)
    }

    /// A random element, or `None` for an empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.index(items.len())?)
    }

    /// `len` random bytes.
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        std::iter::repeat_with(|| self.next_u64())
            .flat_map(u64::to_le_bytes)
            .take(len)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Rng, Seed, parse};
    use crate::check::{Failure, ensure, ensure_eq};

    #[test]
    fn matches_the_reference_stream() -> Result<(), Failure> {
        // SplitMix64's published outputs for seed 1234567.
        let mut rng = Rng::new(1_234_567);
        let actual = [rng.next_u64(), rng.next_u64()];
        ensure_eq(
            &actual,
            &[6_457_827_717_110_365_317, 3_203_168_211_198_807_973],
            "SplitMix64 stream",
        )
    }

    #[test]
    fn below_stays_in_range() -> Result<(), Failure> {
        let mut rng = Seed::from_env()?.rng();
        for _ in 0..10_000 {
            let value = rng
                .below(10)
                .ok_or_else(|| Failure::new("nonzero bound gave None"))?;
            ensure(value < 10, format!("{value} is outside 0..10"))?;
        }
        ensure(rng.below(0).is_none(), "a zero bound must give None")
    }

    #[test]
    fn bytes_has_the_requested_length() -> Result<(), Failure> {
        let mut rng = Rng::new(3);
        for len in [0, 1, 7, 8, 9, 100] {
            ensure_eq(&rng.bytes(len).len(), &len, "byte count")?;
        }
        Ok(())
    }

    #[test]
    fn parses_decimal_and_hex() -> Result<(), Failure> {
        ensure_eq(&parse("42"), &Some(42), "decimal")?;
        ensure_eq(&parse("0x5eed"), &Some(0x5EED), "hex")?;
        ensure_eq(
            &parse("0xffff_ffff"),
            &Some(0xFFFF_FFFF),
            "hex with separators",
        )?;
        ensure_eq(&parse("seed"), &None, "garbage")?;
        ensure_eq(
            Seed::new(0x5EED).repro().as_str(),
            "TEST_SEED=0x0000000000005eed",
            "repro",
        )
    }

    #[test]
    fn index_chance_and_pick_cover_their_ranges() -> Result<(), Failure> {
        let mut rng = Rng::new(11);
        let mut seen = [0_u32; 5];
        let mut heads = 0_u32;
        for _ in 0..5_000 {
            let slot = rng.index(5).and_then(|index| seen.get_mut(index));
            let slot = slot.ok_or_else(|| Failure::new("index out of range"))?;
            *slot = slot
                .checked_add(1)
                .ok_or_else(|| Failure::new("overflow"))?;
            heads = heads
                .checked_add(u32::from(rng.chance(500, 1000)))
                .ok_or_else(|| Failure::new("overflow"))?;
            ensure(
                !rng.chance(0, 1000) && rng.chance(1000, 1000),
                "certain outcomes varied",
            )?;
        }
        ensure(
            seen.iter().all(|count| (800..1200).contains(count)),
            format!("index is skewed: {seen:?}"),
        )?;
        ensure(
            (2_300..2_700).contains(&heads),
            format!("{heads} heads in 5000 fair flips"),
        )?;
        let items = ["a", "b", "c"];
        let picked: Vec<_> = (0..60).filter_map(|_| rng.pick(&items)).collect();
        ensure(
            items.iter().all(|item| picked.contains(&item)),
            "pick never chose some items",
        )?;
        ensure(rng.pick::<u8>(&[]).is_none(), "pick from nothing")
    }
}
