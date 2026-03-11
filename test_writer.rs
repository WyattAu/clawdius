// Minimal test to verify formatter compiles
use std::io::Write;

fn test_writer<W: Write>(writer: &mut W) -> std::io::Result<()> {
    writeln!(writer, "Test")?;

    // Create a wrapper that takes ownership of a mutable reference
    struct Wrapper<U: Write> {
        inner: U,
    }

    impl<U: Write> Wrapper<U> {
        fn new(inner: U) -> Self {
            Self { inner }
        }

        fn write(&mut self, data: &str) -> std::io::Result<()> {
            write!(self.inner, "{}", data)
        }
    }

    // This should work - we pass the mutable reference by value (moves the reference)
    let mut wrapper = Wrapper::new(writer.by_ref());
    wrapper.write("Hello")?;

    // After wrapper is dropped, we should be able to use writer again
    writeln!(writer, "After wrapper")?;

    Ok(())
}

fn main() {
    let mut output = Vec::new();
    test_writer(&mut output).unwrap();
    println!("{}", String::from_utf8_lossy(&output));
}
