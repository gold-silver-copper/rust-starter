//! Application logic with explicit, fallible output.

pub mod rle;

use std::io::{self, Write};

/// Write the starter's greeting.
///
/// # Errors
/// Returns the output writer's error if writing fails.
pub fn run(output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "Hello, world!")
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::io;

    #[test]
    fn writes_greeting() -> io::Result<()> {
        let mut output = Vec::new();
        run(&mut output)?;
        if output != b"Hello, world!\n" {
            return Err(io::Error::other("unexpected greeting"));
        }
        Ok(())
    }
}
