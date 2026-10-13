use std::io;

fn main() -> io::Result<()> {
    rust_starter::run(&mut io::stdout().lock())
}
