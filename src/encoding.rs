//! Utilities to receive and send over SPI.

use core::convert::TryFrom;

use byteorder::ByteOrder as _;
use embedded_hal_async::spi::{SpiBusRead, SpiBusWrite};

/// Read either a u8 or a u16 length over SPI.
pub async fn recv_len<S>(spi: &mut S, long: bool) -> Result<usize, S::Error>
where
    S: SpiBusRead,
{
    let len = if long {
        let mut buf = [0; 2];
        spi.read(&mut buf).await?;
        byteorder::BigEndian::read_u16(&buf) as usize
    } else {
        let mut buf = [0; 1];
        spi.read(&mut buf).await?;
        buf[0] as usize
    };

    Ok(len)
}

/// Send either a u8 or u16 length over SPI.
pub async fn send_len<S>(spi: &mut S, long: bool, len: usize) -> Result<(), S::Error>
where
    S: SpiBusWrite,
{
    if long {
        let len = u16::try_from(len).unwrap();
        let mut buf = [0; 2];
        byteorder::BigEndian::write_u16(&mut buf, len);
        spi.write(&buf).await
    } else {
        let len = u8::try_from(len).unwrap();
        spi.write(&[len]).await
    }
}
