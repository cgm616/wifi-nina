//! Utilities to receive and send over SPI.

use core::convert::TryFrom;

use byteorder::ByteOrder as _;

/// Read either a u8 or a u16 length from a buffer and return the length and how many bytes were read
pub(crate) fn parse_len(buf: &[u8], long: bool) -> (usize, usize) {
    if long {
        (byteorder::BigEndian::read_u16(&buf[..2]) as usize, 2)
    } else {
        (buf[0] as usize, 1)
    }
}

/// Serialize either a u8 or a u16 length to a buffer and return how many bytes were written
pub(crate) fn serialize_len(buf: &mut [u8], long: bool, len: usize) -> usize {
    if long {
        let len = u16::try_from(len).unwrap();
        byteorder::BigEndian::write_u16(&mut buf[..2], len);
        2
    } else {
        let len = u8::try_from(len).unwrap();
        buf[0] = len;
        1
    }
}
