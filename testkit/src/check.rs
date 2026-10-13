//! Result-returning checks. Assertion macros are forbidden, even in tests, so
//! a failed check is an error value that `?` carries out of the test.

use std::fmt::{self, Debug, Display};

/// A failed check or failed test setup.
///
/// `Debug` prints the message verbatim, so the report `cargo test` shows for
/// an `Err` is readable, multi-line text rather than an escaped string.
/// Like `anyhow::Error`, it does not implement `Error` itself; that keeps the
/// blanket conversion from every error type available to `?`.
pub struct Failure(String);

impl Failure {
    /// A failure carrying `message`.
    #[must_use]
    pub fn new(message: impl Display) -> Self {
        Self(message.to_string())
    }

    /// Prefix the message with what was being attempted.
    #[must_use]
    pub fn context(self, what: impl Display) -> Self {
        Self(format!("{what}: {}", self.0))
    }

    /// The message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl Debug for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<E: std::error::Error> From<E> for Failure {
    fn from(error: E) -> Self {
        Self(error.to_string())
    }
}

/// A failure carrying `message`, for `return Err(fail(...))` and `ok_or_else`.
#[must_use]
pub fn fail(message: impl Display) -> Failure {
    Failure::new(message)
}

/// `Ok` when `condition` holds, otherwise a failure carrying `message`.
///
/// The message is built before the call even when the check passes. In a
/// hot loop, prefer `if !condition { return Err(fail(format!(...))) }`.
///
/// # Errors
/// Returns the failure when `condition` is false.
pub fn ensure(condition: bool, message: impl Display) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(Failure::new(message))
    }
}

/// `Ok` when `actual == expected`, otherwise a failure naming `what` with a
/// line diff of the two `{:#?}` renderings.
///
/// # Errors
/// Returns the failure when the values differ.
pub fn ensure_eq<T: PartialEq + Debug + ?Sized>(
    actual: &T,
    expected: &T,
    what: impl Display,
) -> Result<(), Failure> {
    if actual == expected {
        return Ok(());
    }
    let (expected, actual) = (format!("{expected:#?}"), format!("{actual:#?}"));
    Err(Failure::new(format!(
        "{what} differs (- expected, + actual)\n{}",
        line_diff(&expected, &actual)
    )))
}

/// Differing lines shown before the rest is summarized.
const MAX_DIFF_LINES: usize = 40;

/// A minimal line-by-line diff, enough to locate a change in small texts.
#[must_use]
pub fn line_diff(expected: &str, actual: &str) -> String {
    let (mut old, mut new) = (expected.lines(), actual.lines());
    let mut diffs = Vec::new();
    for number in 1_usize.. {
        let lines = match (old.next(), new.next()) {
            (None, None) => break,
            (Some(left), Some(right)) if left == right => continue,
            (Some(left), Some(right)) => format!("{number:>4} - {left}\n{number:>4} + {right}\n"),
            (Some(left), None) => format!("{number:>4} - {left}\n"),
            (None, Some(right)) => format!("{number:>4} + {right}\n"),
        };
        diffs.push(lines);
    }
    let mut out: String = diffs
        .iter()
        .take(MAX_DIFF_LINES)
        .map(String::as_str)
        .collect();
    if let Some(hidden) = diffs
        .len()
        .checked_sub(MAX_DIFF_LINES)
        .filter(|hidden| *hidden > 0)
    {
        out.push_str(&format!("     ... and {hidden} more differing lines\n"));
    }
    if expected.ends_with('\n') != actual.ends_with('\n') {
        out.push_str("     (trailing newline differs)\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Failure, ensure, ensure_eq, line_diff};

    #[test]
    fn ensure_eq_reports_the_changed_line() -> Result<(), Failure> {
        let failure = match ensure_eq(&[1, 2, 3], &[1, 5, 3], "values") {
            Ok(()) => return Err(Failure::new("different values compared equal")),
            Err(failure) => failure,
        };
        ensure(
            failure.message().contains("-     5,\n") && failure.message().contains("+     2,\n"),
            format!("diff misses the changed element:\n{failure}"),
        )
    }

    #[test]
    fn diff_is_empty_for_equal_text() -> Result<(), Failure> {
        ensure_eq(
            line_diff("a\nb\n", "a\nb\n").as_str(),
            "",
            "diff of equal text",
        )
    }

    #[test]
    fn diff_notes_a_trailing_newline() -> Result<(), Failure> {
        ensure(
            line_diff("a\n", "a").contains("trailing newline"),
            "trailing newline change was not reported",
        )
    }

    #[test]
    fn failures_print_verbatim_and_long_diffs_are_cut() -> Result<(), Failure> {
        let failure = Failure::new("line one\n\"two\"");
        ensure_eq(
            format!("{failure:?}|{failure}").as_str(),
            "line one\n\"two\"|line one\n\"two\"",
            "rendering",
        )?;
        let lines = |text: &str, count| std::iter::repeat_n(text, count).collect::<String>();
        let new = lines("y\n", 41);
        let summary = "... and 1 more differing lines\n";
        ensure(
            line_diff(&lines("x\n", 41), &new).ends_with(summary),
            "41 changes not summarized",
        )?;
        let forty_changes = lines("x\n", 40) + "y\n";
        ensure(
            !line_diff(&forty_changes, &new).contains("more"),
            "40 changes were summarized",
        )
    }
}
