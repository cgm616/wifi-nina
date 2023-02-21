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
        SP: params::SendParams + fmt::Debug,
        RP: params::RecvParams + fmt::Debug;
}
