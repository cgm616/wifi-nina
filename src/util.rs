/// Utilities

#[cfg(test)]
pub(crate) mod test {
    use crate::transport::Transporter;

    /// A transporter for use in tests
    pub(crate) struct MockTransporter<const CAPACITY: usize> {
        pub buffer: [u8; CAPACITY],
        cursor: usize,
    }

    /// Errors produced by `MockTransporter`
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum MockError {
        BufferFull,
        NoMoreData,
    }

    impl std::fmt::Display for MockError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Self::BufferFull => write!(f, "the internal buffer is full"),
                Self::NoMoreData => write!(f, "there is no more data to read"),
            }
        }
    }

    impl embedded_io::Error for MockError {
        fn kind(&self) -> embedded_io::ErrorKind {
            embedded_io::ErrorKind::Other
        }
    }

    impl std::error::Error for MockError {}

    impl<const CAPACITY: usize> Transporter for MockTransporter<CAPACITY> {
        type Error = MockError;

        async fn read(&mut self) -> Result<u8, Self::Error> {
            if self.cursor >= self.buffer.len() {
                return Err(MockError::NoMoreData);
            }

            self.cursor += 1;
            Ok(self.buffer[self.cursor - 1])
        }

        async fn write(&mut self, byte: u8) -> Result<(), Self::Error> {
            if self.cursor >= self.buffer.len() {
                return Err(MockError::BufferFull);
            }

            self.buffer[self.cursor] = byte;
            self.cursor += 1;

            Ok(())
        }
    }

    impl<const CAPACITY: usize> MockTransporter<CAPACITY> {
        /// Create a new `MockTransporter`
        pub fn new() -> Self {
            Self {
                buffer: [0; CAPACITY],
                cursor: 0,
            }
        }

        /// Clear the buffer and internal state
        ///
        /// The `MockTransporter` will be in the same state as a brand-new instance.
        pub fn clear(&mut self) {
            self.buffer = [0; CAPACITY];
            self.cursor = 0;
        }

        /// Allow previously written bytes to be read as if off the wire
        pub fn to_reader(&mut self) {
            self.cursor = 0;
        }
    }

    /// Wrap a test in `futures::executor::block_on()`
    macro_rules! async_test {
        ( $($t:tt)* ) => {
            futures::executor::block_on(async move { $( $t )* })?;
        };
    }

    pub(crate) use async_test;
}
