#![allow(clippy::type_complexity)]

use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::{
    delay::DelayUs,
    digital::Wait,
    spi::{SpiBus, SpiBusFlush},
};
use embedded_io::Error as EioError;

use core::{fmt, fmt::Debug, future::Future};

use crate::{
    command, params,
    transport::{Transport, Transporter},
};

#[derive(Debug)]
pub struct SpiTransport<SPI, CS, BUSY, RESET> {
    spi: SPI,
    cs: CS,
    busy: BUSY,
    reset: RESET,
}

pub struct BufTransporter<'a, const CAPACITY: usize, SPI: 'a, CS: 'a, BUSY: 'a, RESET: 'a> {
    buffer: [u8; CAPACITY],
    cursor: usize, // should never be more than CAPACITY or length
    spi: &'a mut SpiTransport<SPI, CS, BUSY, RESET>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SpiError<SPI, CS, BUSY, RESET> {
    Spi(SPI),
    Cs(CS),
    Busy(BUSY),
    Reset(RESET),
    Delay,
    Timeout,
    ErrorResponse,
    BufferFull,
    UnexpectedReplyByte(u8, u8),
}

impl<SPI, CS, BUSY, RESET> Debug for SpiError<SPI, CS, BUSY, RESET> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spi(_) => write!(f, "SPI"),
            Self::Cs(_) => write!(f, "CS"),
            Self::Busy(_) => write!(f, "BUSY"),
            Self::Reset(_) => write!(f, "WRITE"),
            Self::Delay => write!(f, "DELAY"),
            Self::Timeout => write!(f, "Timeout"),
            Self::ErrorResponse => write!(f, "ErrResp"),
            Self::BufferFull => write!(f, "BufferFull"),
            Self::UnexpectedReplyByte(b, loc) => write!(f, "URB: 0x{b:02x} at {loc}"),
        }
    }
}

impl<SPI, CS, BUSY, RESET> EioError for SpiError<SPI, CS, BUSY, RESET> {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl<'a, const CAPACITY: usize, SPI, CS, BUSY, RESET> Transporter
    for BufTransporter<'a, CAPACITY, SPI, CS, BUSY, RESET>
where
    SPI: SpiBus + SpiBusFlush,
    CS: OutputPin,
    BUSY: Wait + InputPin,
    RESET: OutputPin,
{
    type Error = SpiError<SPI::Error, CS::Error, BUSY::Error, RESET::Error>;

    async fn read(&mut self) -> Result<u8, Self::Error> {
        if self.cursor >= self.buffer.len() {
            // We have consumed the buffer. Get more from the layer
            self.refill().await?;
        }

        // Return the next byte and increment the cursor
        let ret = self.buffer[self.cursor];
        self.cursor += 1;
        Ok(ret)
    }

    async fn write(&mut self, byte: u8) -> Result<(), Self::Error> {
        if self.cursor >= self.buffer.len() {
            // We have filled the buffer. Flush it to the layer
            self.flush().await?;
        }

        // Save the byte and increment the cursor
        self.buffer[self.cursor] = byte;
        self.cursor += 1;
        Ok(())
    }
}

impl<'a, const CAPACITY: usize, SPI, CS, BUSY, RESET>
    BufTransporter<'a, CAPACITY, SPI, CS, BUSY, RESET>
where
    SPI: SpiBus + SpiBusFlush,
    CS: OutputPin,
    BUSY: Wait + InputPin,
    RESET: OutputPin,
{
    // type Error = <Self as Transporter>::Error;

    async fn new(
        spi: &'a mut SpiTransport<SPI, CS, BUSY, RESET>,
    ) -> Result<Self, <Self as Transporter>::Error> {
        // Wait until the WifiNina is ready to receive
        spi.busy.wait_for_low().await.map_err(SpiError::Busy)?;

        // Assert chip select
        spi.cs.set_low().map_err(SpiError::Cs)?;

        // Wait until the WifiNina is ready to receive
        spi.busy.wait_for_high().await.map_err(SpiError::Busy)?;

        Ok(Self {
            buffer: [0; CAPACITY],
            cursor: 0,
            spi,
        })
    }

    async fn cleanup(self) -> Result<(), <Self as Transporter>::Error> {
        // Flush bus
        self.spi.spi.flush().await.map_err(SpiError::Spi)?;

        // Deassert chip select
        self.spi.cs.set_high().map_err(SpiError::Cs)?;

        Ok(())
    }

    fn clear(&mut self) {
        self.buffer = [0; CAPACITY];
        self.cursor = 0;
    }

    async fn flush(&mut self) -> Result<(), <Self as Transporter>::Error> {
        // Pad the buffer to a multiple of four
        while self.cursor % 4 != 0 {
            self.buffer[self.cursor] = 0xFF;
            self.cursor += 1;
        }

        // Send the data in the buffer
        self.spi
            .spi
            .transfer_in_place(&mut self.buffer[0..self.cursor])
            .await
            .map_err(SpiError::Spi)?;

        self.clear();
        Ok(())
    }

    async fn refill(&mut self) -> Result<(), <Self as Transporter>::Error> {
        self.clear();

        // Fill the buffer
        self.spi
            .spi
            .transfer_in_place(&mut self.buffer)
            .await
            .map_err(SpiError::Spi)?;

        Ok(())
    }
}

const START_CMD: u8 = 0xe0;
const END_CMD: u8 = 0xee;
const ERR_CMD: u8 = 0xef;
const REPLY_FLAG: u8 = 1 << 7;
// const WAIT_REPLY_TIMEOUT_BYTES: usize = 1000;

impl<SPI, CS, BUSY, RESET> Transport for SpiTransport<SPI, CS, BUSY, RESET>
where
    SPI: SpiBus + SpiBusFlush,
    CS: OutputPin,
    BUSY: Wait + InputPin,
    RESET: OutputPin,
{
    type Error = SpiError<SPI::Error, CS::Error, BUSY::Error, RESET::Error>;

    #[inline]
    async fn reset<DELAY: DelayUs>(&mut self, mut delay: DELAY) -> Result<(), Self::Error> {
        // self.cs.set_high().map_err(SpiError::Cs)?;

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
        SP: params::SerializeParams + fmt::Debug,
        RP: params::ParseParams + fmt::Debug,
    {
        // ----- FIRST PART: SENDING -----

        self.transaction::<'_, '_, 16, _, _>(|mut trans| async move {
            trans.write(START_CMD).await?;
            trans.write(u8::from(command) & !REPLY_FLAG).await?;

            send_params.serialize(&mut trans, long_send).await?;

            trans.write(END_CMD).await?;

            trans.flush().await?;

            Ok(trans)
        })
        .await?;

        // ----- SECOND PART: RECEIVING -----

        self.transaction::<'_, '_, 16, _, _>(|mut trans| async move {
            let mut first = [0; 2];
            trans.read_into(&mut first).await?;

            // Make sure the first byte doesn't indicate an error
            if first[0] == ERR_CMD {
                return Err(SpiError::ErrorResponse);
            } else if first[0] != START_CMD {
                return Err(SpiError::UnexpectedReplyByte(first[0], 0));
            }

            // Make sure the WifiNina is responding to the correct command
            if first[1] != u8::from(command) | REPLY_FLAG {
                return Err(SpiError::UnexpectedReplyByte(first[1], 1));
            }

            // Receive and parse the response
            recv_params.parse(&mut trans, long_recv).await?;

            // Ensure the WifiNina is finished
            let last = trans.read().await?;
            if last != END_CMD {
                return Err(SpiError::UnexpectedReplyByte(last, 2));
            }

            Ok(trans)
        })
        .await?;

        Ok(())
    }
}

impl<SPI, CS, BUSY, RESET> SpiTransport<SPI, CS, BUSY, RESET>
where
    SPI: SpiBus + SpiBusFlush,
    CS: OutputPin,
    BUSY: Wait + InputPin,
    RESET: OutputPin,
{
    #[inline]
    pub async fn start<DELAY: DelayUs>(
        spi: SPI,
        cs: CS,
        busy: BUSY,
        reset: RESET,
        delay: DELAY,
    ) -> Result<Self, <Self as Transport>::Error> {
        let mut this = Self {
            spi,
            cs,
            busy,
            reset,
        };

        super::Transport::reset(&mut this, delay).await?;

        Ok(this)
    }

    async fn transaction<'trans: 'inner, 'inner, const CAPACITY: usize, F, Fut>(
        &'trans mut self,
        f: F,
    ) -> Result<(), SpiError<SPI::Error, CS::Error, BUSY::Error, RESET::Error>>
    where
        F: (FnOnce(BufTransporter<'inner, CAPACITY, SPI, CS, BUSY, RESET>) -> Fut) + 'trans,
        Fut: Future<
                Output = Result<
                    BufTransporter<'inner, CAPACITY, SPI, CS, BUSY, RESET>,
                    SpiError<SPI::Error, CS::Error, BUSY::Error, RESET::Error>,
                >,
            > + 'inner,
    {
        let mut trans: BufTransporter<CAPACITY, _, _, _, _> = BufTransporter::new(self).await?;

        trans = f(trans).await?;

        trans.cleanup().await
    }
}
