//! Seeded property checks with shrinking, and corpus replay.
//!
//! A property is a function from a generated input to `Result<(), Failure>`.
//! Each case draws its own seed from the run's [`Seed`], a failure is shrunk
//! to a smaller input that still fails, and the report names the seed that
//! reproduces it. The same property can run over a fuzzer's corpus with
//! [`replay_dir`], so stable CI exercises what nightly fuzzing found.

use std::fmt::Debug;
use std::path::Path;
use std::{env, fs, io};

use crate::check::{Failure, fail};
use crate::rng::{Rng, Seed};

/// Shrink attempts are bounded so a pathological shrinker cannot hang a test.
const MAX_SHRINK_STEPS: u32 = 10_000;

/// A seeded run of `cases` generated inputs.
#[derive(Clone, Copy, Debug)]
pub struct Explore {
    seed: Seed,
    cases: u64,
}

impl Explore {
    /// `default_cases` cases from [`Seed::from_env`]; `TEST_CASES` overrides
    /// the count, so a scheduled CI job can run far more than a local test.
    ///
    /// # Errors
    /// Fails when `TEST_SEED` or `TEST_CASES` is set but not a number.
    pub fn from_env(default_cases: u64) -> Result<Self, Failure> {
        let cases = match env::var("TEST_CASES") {
            Ok(text) => text
                .trim()
                .parse()
                .map_err(|error| fail(format!("TEST_CASES={text:?}: {error}")))?,
            Err(_) => default_cases,
        };
        Ok(Self::new(Seed::from_env()?, cases))
    }

    /// Exactly `cases` cases from `seed`.
    #[must_use]
    pub const fn new(seed: Seed, cases: u64) -> Self {
        Self { seed, cases }
    }

    /// Check `property` on generated inputs, without shrinking.
    ///
    /// # Errors
    /// The first failing case, with its input and the seed that reproduces it.
    pub fn check<T: Debug>(
        &self,
        generate: impl FnMut(&mut Rng) -> T,
        property: impl FnMut(&T) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        self.check_shrinking(generate, |_| Vec::new(), property)
    }

    /// Check `property` on generated inputs; shrink a failure by repeatedly
    /// taking the first candidate from `shrink` that still fails.
    ///
    /// # Errors
    /// The first failing case, shrunk, with the seed that reproduces it.
    pub fn check_shrinking<T: Debug>(
        &self,
        mut generate: impl FnMut(&mut Rng) -> T,
        mut shrink: impl FnMut(&T) -> Vec<T>,
        mut property: impl FnMut(&T) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let mut root = self.seed.rng();
        for case in 0..self.cases {
            let mut rng = root.fork();
            let input = generate(&mut rng);
            let Err(error) = property(&input) else {
                continue;
            };
            let (input, error, steps) = minimize(input, error, &mut shrink, &mut property);
            return Err(fail(format!(
                "property failed on case {case} of {cases}\n\
                 input (after {steps} shrink steps): {input:?}\n\
                 error: {error}\n\
                 reproduce: rerun this test with {repro}",
                cases = self.cases,
                repro = self.seed.repro(),
            )));
        }
        Ok(())
    }

    /// Check `property` on random byte strings of up to `max_len` bytes,
    /// shrinking failures with [`shrink_bytes`].
    ///
    /// # Errors
    /// The first failing input, shrunk, with the seed that reproduces it.
    pub fn bytes(
        &self,
        max_len: usize,
        mut property: impl FnMut(&[u8]) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let bound = max_len
            .checked_add(1)
            .ok_or_else(|| fail("max_len is too large"))?;
        self.check_shrinking(
            |rng| {
                let len = rng.index(bound).unwrap_or_default();
                rng.bytes(len)
            },
            |input| shrink_bytes(input),
            |input| property(input),
        )
    }
}

fn minimize<T>(
    mut input: T,
    mut error: Failure,
    shrink: &mut impl FnMut(&T) -> Vec<T>,
    property: &mut impl FnMut(&T) -> Result<(), Failure>,
) -> (T, Failure, u32) {
    let mut steps = 0_u32;
    'smaller: while steps < MAX_SHRINK_STEPS {
        for candidate in shrink(&input) {
            if let Err(smaller) = property(&candidate) {
                (input, error) = (candidate, smaller);
                steps = steps.checked_add(1).unwrap_or(MAX_SHRINK_STEPS);
                continue 'smaller;
            }
        }
        break;
    }
    (input, error, steps)
}

/// Smaller variants of `input`: whole chunks removed (halves, quarters, …,
/// single bytes for short inputs), then single bytes set to zero.
#[must_use]
pub fn shrink_bytes(input: &[u8]) -> Vec<Vec<u8>> {
    let len = input.len();
    let mut out = Vec::new();
    let mut chunk = len;
    while chunk > 0 && (chunk > 1 || len <= 512) {
        let mut start = 0;
        while start < len {
            let end = start.checked_add(chunk).map_or(len, |end| end.min(len));
            out.push(
                input
                    .iter()
                    .take(start)
                    .chain(input.iter().skip(end))
                    .copied()
                    .collect(),
            );
            start = end;
        }
        let Some(half) = chunk.checked_div(2) else {
            break;
        };
        chunk = half;
    }
    if len <= 64 {
        for (position, _) in input.iter().enumerate().filter(|(_, byte)| **byte != 0) {
            let mut zeroed = input.to_vec();
            if let Some(byte) = zeroed.get_mut(position) {
                *byte = 0;
            }
            out.push(zeroed);
        }
    }
    out
}

/// Run `property` on every file in `dir` (for example a fuzz corpus), in name
/// order. A missing directory is an empty corpus.
///
/// # Errors
/// The first failing file, or the I/O error that stopped the replay.
pub fn replay_dir(
    dir: impl AsRef<Path>,
    mut property: impl FnMut(&[u8]) -> Result<(), Failure>,
) -> Result<usize, Failure> {
    let dir = dir.as_ref();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(fail(format!("{}: {error}", dir.display()))),
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.is_file());
    paths.sort();
    for path in &paths {
        let input = fs::read(path).map_err(|error| fail(format!("{}: {error}", path.display())))?;
        property(&input)
            .map_err(|error| error.context(format!("corpus file {}", path.display())))?;
    }
    Ok(paths.len())
}

#[cfg(test)]
mod tests {
    use super::{Explore, replay_dir, shrink_bytes};
    use crate::check::{Failure, ensure, ensure_eq};
    use crate::rng::Seed;
    use crate::sandbox::Scratch;

    #[test]
    fn shrinks_to_the_smallest_failing_input() -> Result<(), Failure> {
        // A deliberately false property: "no input contains 0xFF".
        let explore = Explore::new(Seed::new(1), 1_000);
        let Err(failure) = explore.bytes(64, |input| ensure(!input.contains(&0xFF), "found 0xFF"))
        else {
            return Err(Failure::new("a false property passed"));
        };
        ensure(
            failure.message().contains("input (after") && failure.message().contains("[255]"),
            format!("expected the input to shrink to [255]:\n{failure}"),
        )?;
        ensure(
            failure.message().contains("TEST_SEED=0x0000000000000001"),
            "report lacks the seed",
        )
    }

    #[test]
    fn a_true_property_passes() -> Result<(), Failure> {
        Explore::from_env(200)?.bytes(32, |input| ensure(input.len() <= 32, "too long"))
    }

    #[test]
    fn shrink_candidates_are_smaller_or_simpler() -> Result<(), Failure> {
        let input = [3_u8, 0, 7];
        for candidate in shrink_bytes(&input) {
            let simpler = candidate.len() < input.len()
                || (candidate != input
                    && candidate.iter().zip(&input).all(|(new, old)| new <= old));
            ensure(
                simpler,
                format!("{candidate:?} is not simpler than {input:?}"),
            )?;
        }
        ensure_eq(&shrink_bytes(&[]).len(), &0, "candidates for empty input")
    }

    #[test]
    fn check_without_shrinking_reports_the_case() -> Result<(), Failure> {
        let result = Explore::new(Seed::new(2), 50).check(
            |rng| rng.below(4),
            |value| ensure(*value != Some(3), "three"),
        );
        let Err(failure) = result else {
            return Err(Failure::new("a false property passed"));
        };
        ensure(failure.message().contains("Some(3)"), failure.to_string())
    }

    #[test]
    fn replay_counts_files_and_names_the_failing_one() -> Result<(), Failure> {
        let scratch = Scratch::new(crate::unit_base(), "replay")?;
        ensure_eq(
            &replay_dir(scratch.path().join("absent"), |_| Ok(()))?,
            &0,
            "files in a missing corpus",
        )?;
        std::fs::write(scratch.path().join("a"), b"fine")?;
        std::fs::write(scratch.path().join("b"), b"bad")?;
        ensure_eq(
            &replay_dir(scratch.path(), |_| Ok(()))?,
            &2,
            "files replayed",
        )?;
        let Err(failure) = replay_dir(scratch.path(), |input| ensure(input != b"bad", "bad input"))
        else {
            return Err(Failure::new("a failing corpus file passed"));
        };
        ensure(
            failure.message().contains("/b: bad input"),
            failure.to_string(),
        )
    }

    #[test]
    fn long_inputs_skip_the_finest_shrinks() -> Result<(), Failure> {
        // Single-byte removal and zeroing are quadratic; they stop at 512 and 64 bytes.
        let candidates = |len: usize| shrink_bytes(&vec![1; len]).len();
        // Removal of ceil(len / chunk) chunks per halving, plus one zeroing per byte.
        ensure_eq(
            &candidates(64),
            &(1 + 2 + 4 + 8 + 16 + 32 + 64 + 64),
            "candidates for 64 bytes",
        )?;
        ensure_eq(
            &candidates(65),
            &(1 + 3 + 5 + 9 + 17 + 33 + 65),
            "candidates for 65 bytes",
        )?;
        ensure_eq(
            &candidates(513),
            &(1 + 3 + 5 + 9 + 17 + 33 + 65 + 129 + 257),
            "candidates for 513 bytes",
        )
    }
}
