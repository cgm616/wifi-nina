use embedded_hal_async::delay::DelayUs;
use embedded_io::Error as EioError;

use core::fmt;

use crate::command;
use crate::params;

mod spi;

pub use spi::SpiError;
pub use spi::SpiTransport;

pub trait Transport {
    type Error: EioError;

    async fn reset<DELAY: DelayUs>(&mut self, delay: DELAY) -> Result<(), Self::Error>;

    async fn handle_cmd<SP, RP>(
        &mut self,
        command: command::Command,
        send_params: &SP,
        recv_params: &mut RP,
        long_send: bool,
        long_recv: bool,
    ) -> Result<(), Self::Error>
    where
        SP: params::SerializeParams + fmt::Debug,
        RP: params::ParseParams + fmt::Debug;
}

/// A source and sink for bytes while parsing and serializing
///
/// This trait abstracts a simple byte reader and writer, allowing the serialization
/// and parsing logic to operate on individual bytes while allowing the underlying
/// transport layers to send and receive in any way.
pub trait Transporter {
    /// The error type thrown by the functions
    type Error: EioError;

    /// Read a single byte
    async fn read(&mut self) -> Result<u8, Self::Error>;

    /// Read as many bytes as possible into a buffer
    ///
    /// Returns the number of bytes read.
    async fn read_into(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut count = 0;
        for e in buf {
            *e = self.read().await?;
            count += 1;
        }
        Ok(count)
    }

    /// Write a single byte
    async fn write(&mut self, byte: u8) -> Result<(), Self::Error>;

    /// Write the entire buffer
    async fn write_from(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        for e in bytes {
            self.write(*e).await?;
        }
        Ok(())
    }

    /// Flush the transport layer, ensuring any pending bytes are sent
    async fn flush(&mut self) -> Result<(), Self::Error>;
}
