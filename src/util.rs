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
    pub(crate) enum MockError {
        BufferFull,
        NoMoreData,
    }

    impl<const CAPACITY: usize> Transporter for MockTransporter<CAPACITY> {
        type Error = MockError;

        async fn read(&mut self) -> Result<u8, Self::Error> {
            if cursor >= self.buffer.len() {
                return Err(MockError::NoMoreData);
            }

            cursor += 1;
            Ok(self.buffer[cursor - 1])
        }

        async fn write(&mut self, byte: u8) -> Result<(), Self::Error> {
            if cursor >= self.buffer.len() {
                return Err(MockError::BufferFull);
            }

            self.buffer[cursor] = byte;
            cursor += 1;

            Ok(())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            // nothing to do
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
}
