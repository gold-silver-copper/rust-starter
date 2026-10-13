use std::io::{self, Write};

struct BrokenOutput;

impl Write for BrokenOutput {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn propagates_output_failure() -> io::Result<()> {
    match rust_starter::run(&mut BrokenOutput) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error),
        Ok(()) => Err(io::Error::other("output failure was silently ignored")),
    }
}
