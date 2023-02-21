#![allow(clippy::type_complexity)]

use crate::command;
use crate::params;

use core::{fmt, fmt::Debug};

use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::{
    delay::DelayUs,
    spi::{transaction, ErrorType, SpiBus, SpiBusRead, SpiBusWrite, SpiDevice},
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
pub enum SpiError<SPI, BUS, BUSY, RESET> {
    Spi(SPI),
    Bus(BUS),
    Busy(BUSY),
    Reset(RESET),
    Delay,
    Timeout,
    ErrorResponse,
    UnexpectedReplyByte(u8),
}

impl<SPI, BUS, BUSY, RESET> Debug for SpiError<SPI, BUS, BUSY, RESET> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spi(_) => write!(f, "SPI"),
            Self::Bus(_) => write!(f, "BUS"),
            Self::Busy(_) => write!(f, "BUSY"),
            Self::Reset(_) => write!(f, "WRITE"),
            Self::Delay => write!(f, "DELAY"),
            Self::Timeout => write!(f, "Timeout"),
            Self::ErrorResponse => write!(f, "ErrorResponse"),
            Self::UnexpectedReplyByte(b) => write!(f, "UnexpectedReplyByte: {b}"),
        }
    }
}

impl<SPI, BUS, BUSY, RESET> EioError for SpiError<SPI, BUS, BUSY, RESET> {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

const START_CMD: u8 = 0xe0;
const END_CMD: u8 = 0xee;
const ERR_CMD: u8 = 0xef;
const REPLY_FLAG: u8 = 1 << 7;
const WAIT_REPLY_TIMEOUT_BYTES: usize = 1000;

impl<SPI, BUSY, RESET> super::Transport for SpiTransport<SPI, BUSY, RESET>
where
    SPI: SpiDevice,
    SPI::Bus: SpiBus,
    BUSY: InputPin,
    RESET: OutputPin,
{
    type Error = SpiError<SPI::Error, <SPI::Bus as ErrorType>::Error, BUSY::Error, RESET::Error>;

    #[inline]
    async fn reset<DELAY: DelayUs>(&mut self, mut delay: DELAY) -> Result<(), Self::Error> {
        #[cfg(feature = "reset-high")]
        self.reset.set_high().map_err(SpiError::Reset)?;
        #[cfg(not(feature = "reset-high"))]
        self.reset.set_low().map_err(SpiError::Reset)?;

        delay.delay_ms(10).await.map_err(|_| SpiError::Delay)?;

        #[cfg(feature = "reset-high")]
        self.reset.set_low().map_err(SpiError::Reset)?;
        #[cfg(not(feature = "reset-high"))]
        self.reset.set_high().map_err(SpiError::Reset)?;

        delay.delay_ms(750).await.map_err(|_| SpiError::Delay)?;

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
        // TODO: check busy pin!

        // wait for busy pin to be low before transaction

        transaction!(&mut self.spi, move |bus| async move {
            // wait for busy pin to be high before sending

            // Write the byte indicating we're sending a command
            bus.write(&[START_CMD]).await?;
            // Write the command byte
            bus.write(&[u8::from(command) & !REPLY_FLAG]).await?;
            // Send the command parameters
            send_params.send(bus, long_send).await?;
            // Write the end command byte
            bus.write(&[END_CMD]).await?;

            // Make sure the whole message is padded
            let mut total_len = send_params.len(long_send) + 3;
            while 0 != total_len % 4 {
                bus.write(&[0xff]).await?;
                total_len += 1;
            }

            Result::<(), <SPI::Bus as ErrorType>::Error>::Ok(())
        })
        .await
        .map_err(SpiError::Spi)?;

        // wait for busy pin to be low before transaction

        transaction!(&mut self.spi, move |bus| async move {
            // wait for busy pin to be high before receiving

            // Wait until receiving the byte indicating a response. If it does
            // not come in time, throw an error and end the transaction.
            let mut i = 0;
            loop {
                if i > WAIT_REPLY_TIMEOUT_BYTES {
                    return Ok(Err(SpiError::Timeout));
                }

                let mut buf = [0];
                bus.read(&mut buf).await?;

                if buf[0] == ERR_CMD {
                    return Ok(Err(SpiError::ErrorResponse));
                } else if buf[0] == START_CMD {
                    break;
                }

                i += 1;
            }

            // Make sure the device is responding to the expected command
            let mut buf = [0];
            bus.read(&mut buf).await?;
            if buf[0] != u8::from(command) | REPLY_FLAG {
                return Ok(Err(SpiError::UnexpectedReplyByte(buf[0])));
            }

            // Receive the response
            recv_params.recv(bus, long_recv).await?;

            // The device should finish with the end byte
            bus.read(&mut buf).await?;
            if buf[0] != END_CMD {
                return Ok(Err(SpiError::UnexpectedReplyByte(buf[0])));
            }

            Ok(Ok(()))
        })
        .await
        .map_err(SpiError::Spi)??;

        Ok(())
    }
}

impl<SPI, BUSY, RESET> SpiTransport<SPI, BUSY, RESET>
where
    SPI: SpiDevice,
    SPI::Bus: SpiBus,
    BUSY: InputPin,
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
