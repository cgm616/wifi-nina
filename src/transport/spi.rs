#![allow(clippy::type_complexity)]

use crate::command;
use crate::params;

use core::{fmt, fmt::Debug};

use embedded_hal::digital::OutputPin;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::{
    delay::DelayUs,
    spi::{Operation, SpiDevice},
};
use embedded_io::Error as EioError;

use super::Transport;

#[derive(Debug)]
pub struct SpiTransport<SPI, BUSY, RESET> {
    spi: SPI,
    busy: BUSY,
    reset: RESET,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SpiError<SPI, BUSY, RESET> {
    Spi(SPI),
    Busy(BUSY),
    Reset(RESET),
    Delay,
    Timeout,
    ErrorResponse,
    UnexpectedReplyByte(u8, u8),
}

impl<SPI, BUSY, RESET> Debug for SpiError<SPI, BUSY, RESET> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spi(_) => write!(f, "SPI"),
            Self::Busy(_) => write!(f, "BUSY"),
            Self::Reset(_) => write!(f, "WRITE"),
            Self::Delay => write!(f, "DELAY"),
            Self::Timeout => write!(f, "Timeout"),
            Self::ErrorResponse => write!(f, "ErrResp"),
            Self::UnexpectedReplyByte(b, loc) => write!(f, "URB: 0x{b:02x} at {loc}"),
        }
    }
}

impl<SPI, BUSY, RESET> EioError for SpiError<SPI, BUSY, RESET> {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

const START_CMD: u8 = 0xe0;
const END_CMD: u8 = 0xee;
const ERR_CMD: u8 = 0xef;
const REPLY_FLAG: u8 = 1 << 7;
// const WAIT_REPLY_TIMEOUT_BYTES: usize = 1000;

impl<SPI, BUSY, RESET> super::Transport for SpiTransport<SPI, BUSY, RESET>
where
    SPI: SpiDevice,
    BUSY: Wait,
    RESET: OutputPin,
{
    type Error = SpiError<SPI::Error, BUSY::Error, RESET::Error>;

    #[inline]
    async fn reset<DELAY: DelayUs>(&mut self, mut delay: DELAY) -> Result<(), Self::Error> {
        #[cfg(feature = "reset-high")]
        self.reset.set_high().map_err(SpiError::Reset)?;
        #[cfg(not(feature = "reset-high"))]
        self.reset.set_low().map_err(SpiError::Reset)?;

        delay.delay_ms(100).await;

        #[cfg(feature = "reset-high")]
        self.reset.set_low().map_err(SpiError::Reset)?;
        #[cfg(not(feature = "reset-high"))]
        self.reset.set_high().map_err(SpiError::Reset)?;

        delay.delay_ms(750).await;

        Ok(())
    }

    #[inline]
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
        RP: params::RecvParams + fmt::Debug,
    {
        // Set up buffer to hold data
        let mut buf: [u8; 1024] = [0; 1024];

        // Wait until the WifiNina is ready to receive
        let busy = &mut self.busy;
        busy.wait_for_low().await.map_err(SpiError::Busy)?;

        // Set up command to send
        buf[0] = START_CMD;
        buf[1] = u8::from(command) & !REPLY_FLAG;
        let len = send_params.serialize(&mut buf[2..], long_send);
        buf[2 + len] = END_CMD;

        // Pad the buffer to the nearest multiple of four
        let mut len = 2 + len + 1; // the number of current bytes in the buffer
        while len % 4 != 0 {
            buf[len] = 0xFF;
            len += 1;
        }

        // Send the command in the buffer
        self.spi
            .transaction(&mut [Operation::Write(&buf[0..len])])
            .await
            .map_err(SpiError::Spi)?;

        // Wait until the WifiNina is ready to respond
        let busy = &mut self.busy;
        busy.wait_for_low().await.map_err(SpiError::Busy)?;

        // Receive data into the buffer
        self.spi
            .transaction(&mut [Operation::Read(&mut buf)])
            .await
            .map_err(SpiError::Spi)?;

        // Make sure the first byte doesn't indicate an error
        if buf[0] == ERR_CMD {
            return Err(SpiError::ErrorResponse);
        } else if buf[0] != START_CMD {
            return Err(SpiError::UnexpectedReplyByte(buf[0], 0));
        }

        // Make sure the WifiNina is responding to the correct command
        if buf[1] != u8::from(command) | REPLY_FLAG {
            return Err(SpiError::UnexpectedReplyByte(buf[1], 1));
        }

        // Parse the response
        let len = recv_params.parse(&buf[2..], long_recv);

        // Ensure the WifiNina is finished
        if buf[2 + len] != END_CMD {
            return Err(SpiError::UnexpectedReplyByte(buf[2 + len], 2));
        }

        Ok(())
    }
}

impl<SPI, BUSY, RESET> SpiTransport<SPI, BUSY, RESET>
where
    SPI: SpiDevice,
    BUSY: Wait,
    RESET: OutputPin,
{
    #[inline]
    pub async fn start<DELAY: DelayUs>(
        spi: SPI,
        busy: BUSY,
        reset: RESET,
        delay: DELAY,
    ) -> Result<Self, <Self as Transport>::Error> {
        let mut this = Self { spi, busy, reset };

        super::Transport::reset(&mut this, delay).await?;

        Ok(this)
    }
}
