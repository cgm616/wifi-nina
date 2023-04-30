//! Utilities to receive and send over SPI.

use byteorder::ByteOrder as _;

use core::convert::TryFrom;

use crate::transport::Transporter;

/// Read either a u8 or a u16 length from a `Transporter` and return the length
pub(crate) async fn parse_len<T: Transporter>(
    trans: &mut T,
    long: bool,
) -> Result<usize, T::Error> {
    if long {
        let mut buf = [0; 2];
        trans.read_into(&mut buf).await?;
        Ok(byteorder::BigEndian::read_u16(&buf) as usize)
    } else {
        Ok(trans.read().await?.into())
    }
}

/// Serialize either a u8 or a u16 length to a `Transporter`
pub(crate) async fn serialize_len<T: Transporter>(
    trans: &mut T,
    long: bool,
    len: usize,
) -> Result<(), T::Error> {
    if long {
        let len = u16::try_from(len).unwrap();
        let mut buf = [0; 2];
        byteorder::BigEndian::write_u16(&mut buf, len);
        trans.write_from(&buf).await?;
    } else {
        let len = u8::try_from(len).unwrap();
        trans.write(len).await?;
    }

    Ok(())
}
