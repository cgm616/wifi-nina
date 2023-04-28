use arrayvec::ArrayVec;

use core::marker;

use crate::encoding;

/// A parameter for a WifiNina command
pub trait SendParam {
    /// Return the length, in bytes, of sending the parameter
    fn len(&self) -> usize;

    /// Return the length, in bytes, of sending the parameter if length-delimited
    fn len_length_delimited(&self, long: bool) -> usize {
        self.len() + if long { 2 } else { 1 }
    }

    /// Serialize the parameter into the provided buffer and return the length written
    ///
    /// Panics if the parameter is too long to be serialized into the buffer
    fn serialize(&self, buf: &mut [u8]) -> usize;

    /// Serialize the parameter into the provided buffer with its length first and return the total length written
    fn serialize_length_delimited(&self, buf: &mut [u8], long: bool) -> usize {
        let len = self.len();
        let written = encoding::serialize_len(buf, long, len);
        written + self.serialize(&mut buf[written..])
    }
}

/// A parameters that can be received from the WifiNina
pub trait RecvParam {
    /// Parse the parameter from the contents of a buffer, knowing the length ahead of time
    ///
    /// Returns the length parsed from the buffer
    fn parse(&mut self, buf: &[u8], len: usize) -> usize;

    /// Parse the parameter from a buffer without knowing its length
    ///
    /// Returns the length parsed from the buffer
    fn parse_length_delimited(&mut self, buf: &[u8], long: bool) -> usize {
        let (len, read) = encoding::parse_len(buf, long);
        read + self.parse(&buf[read..], len)
    }
}

/// A wrapper type to null-terminate any parameter
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct NullTerminated<A>(A)
where
    A: ?Sized;

/// A scalar value with a certain endian-ness and length
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct Scalar<O, A>
where
    A: ?Sized,
{
    phantom: marker::PhantomData<O>,
    value: A,
}

impl<A> SendParam for &A
where
    A: SendParam + ?Sized,
{
    fn len(&self) -> usize {
        (*self).len()
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        (*self).serialize(buf)
    }
}

impl<A> RecvParam for &mut A
where
    A: RecvParam + ?Sized,
{
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        (*self).parse(buf, len)
    }
}

impl SendParam for u8 {
    fn len(&self) -> usize {
        1
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        buf[0] = *self;
        1
    }
}

impl RecvParam for u8 {
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        assert_eq!(1, len);
        *self = buf[0];
        1
    }
}

impl<O> SendParam for Scalar<O, u16>
where
    O: byteorder::ByteOrder,
{
    fn len(&self) -> usize {
        2
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        O::write_u16(buf, self.value);
        2
    }
}

impl<O> RecvParam for Scalar<O, u16>
where
    O: byteorder::ByteOrder,
{
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        assert_eq!(2, len);
        self.value = O::read_u16(buf);
        2
    }
}

impl<O> SendParam for Scalar<O, u32>
where
    O: byteorder::ByteOrder,
{
    fn len(&self) -> usize {
        4
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        O::write_u32(buf, self.value);
        4
    }
}

impl<O> RecvParam for Scalar<O, u32>
where
    O: byteorder::ByteOrder,
{
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        assert_eq!(4, len);
        self.value = O::read_u32(buf);
        4
    }
}

impl SendParam for [u8] {
    fn len(&self) -> usize {
        self.len()
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        assert!(self.len() <= buf.len());
        buf[..self.len()].copy_from_slice(self);
        self.len()
    }
}

impl<const CAP: usize> SendParam for ArrayVec<u8, CAP> {
    fn len(&self) -> usize {
        self.len()
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        assert!(self.len() <= buf.len());
        buf[..self.len()].copy_from_slice(self.as_slice());
        self.len()
    }
}

impl RecvParam for &mut [u8] {
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        self.copy_from_slice(&buf[..len]);
        len
    }
}

impl<const CAP: usize> RecvParam for ArrayVec<u8, CAP> {
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        self.try_extend_from_slice(&buf[..len])
            .expect("ArrayVec should have enough capacity"); // TODO
        len
    }
}

impl<A> SendParam for NullTerminated<A>
where
    A: SendParam,
{
    fn len(&self) -> usize {
        self.0.len() + 1
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        let len = self.0.serialize(buf);
        buf[len] = 0;
        len + 1
    }
}

impl<A> RecvParam for NullTerminated<A>
where
    A: RecvParam,
{
    fn parse(&mut self, buf: &[u8], len: usize) -> usize {
        let read = self.0.parse(buf, len - 1);
        assert_eq!(read, len - 1);
        assert_eq!(0, buf[read]);
        read + 1
    }
}

impl<A> NullTerminated<A> {
    pub fn new(value: A) -> Self {
        Self(value)
    }

    pub fn into_inner(self) -> A {
        self.0
    }
}

impl<A> Scalar<byteorder::BigEndian, A> {
    pub fn be(value: A) -> Self {
        let phantom = marker::PhantomData;
        Self { value, phantom }
    }
}

impl<A> Scalar<byteorder::LittleEndian, A> {
    pub fn le(value: A) -> Self {
        let phantom = marker::PhantomData;
        Self { value, phantom }
    }
}

impl<O, A> Scalar<O, A> {
    pub fn into_inner(self) -> A {
        self.value
    }
}

impl<A> core::ops::Deref for NullTerminated<A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<A> core::ops::DerefMut for NullTerminated<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<O, A> core::ops::Deref for Scalar<O, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<O, A> core::ops::DerefMut for Scalar<O, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn serialize_and_parse_u8(byte: u8) {
            let mut buf: [u8; 1] = [0; 1];

            let len_serialized = byte.serialize(&mut buf);
            prop_assert_eq!(1, len_serialized);

            prop_assert_eq!(buf[0], byte);

            let mut parsed = 0_u8;
            let len_parsed = parsed.parse(&buf, len_serialized);
            prop_assert_eq!(1, len_parsed);

            prop_assert_eq!(parsed, byte);
        }

        #[test]
        fn serialize_and_parse_u8_with_length(byte: u8) {
            let mut buf: [u8; 2] = [0; 2];

            let len_serialized = byte.serialize_length_delimited(&mut buf, false);
            prop_assert_eq!(2, len_serialized);

            prop_assert_eq!(buf[0], 1);
            prop_assert_eq!(buf[1], byte);

            let mut parsed = 0_u8;
            let len_parsed = parsed.parse_length_delimited(&buf, false);
            prop_assert_eq!(2, len_parsed);

            prop_assert_eq!(parsed, byte);
        }

        #[test]
        fn serialize_and_parse_bytes_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=127)) {
            let mut buf: [u8; 128] = [0; 128];

            let len_serialized = bytes.as_slice().serialize_length_delimited(&mut buf, false);
            prop_assert_eq!(bytes.len() + 1, len_serialized);

            prop_assert_eq!(buf[0] as usize, bytes.len());

            let mut parsed = ArrayVec::<u8, 127>::new();
            let len_parsed = parsed.parse_length_delimited(&buf, false);
            prop_assert_eq!(len_serialized, len_parsed);

            prop_assert_eq!(parsed.as_slice(), bytes.as_slice());
        }

        #[test]
        fn serialize_and_parse_arrayvec_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=127)) {
            let mut buf: [u8; 128] = [0; 128];

            let mut arrayvec = ArrayVec::<u8, 127>::new();
            arrayvec.try_extend_from_slice(bytes.as_slice()).unwrap();
            let len_serialized = arrayvec.serialize_length_delimited(&mut buf, false);
            prop_assert_eq!(bytes.len() + 1, len_serialized);

            prop_assert_eq!(buf[0] as usize, bytes.len());

            let mut parsed = ArrayVec::<u8, 127>::new();
            let len_parsed = parsed.parse_length_delimited(&buf, false);
            prop_assert_eq!(len_serialized, len_parsed);

            prop_assert_eq!(parsed.as_slice(), arrayvec.as_slice());
        }

        #[test]
        fn serialize_and_parse_nullterminated_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=8)) {
            let mut buf: [u8; 10] = [0; 10];

            let mut arrayvec = ArrayVec::<u8, 8>::new();
            arrayvec.try_extend_from_slice(bytes.as_slice()).unwrap();
            let null_terminated = NullTerminated(arrayvec);

            let len_serialized = null_terminated.serialize_length_delimited(&mut buf, false);
            prop_assert_eq!(len_serialized, bytes.len() + 2);

            prop_assert_eq!(buf[0] as usize, bytes.len() + 1);
            prop_assert_eq!(buf[9], 0);

            let mut parsed = NullTerminated(ArrayVec::<u8, 8>::new());
            let len_parsed = parsed.parse_length_delimited(&buf, false);
            prop_assert_eq!(len_serialized, len_parsed);

            prop_assert_eq!(parsed.as_slice(), null_terminated.as_slice());
        }

        #[test]
        fn serialize_and_parse_scalar_u16(scalar: u16) {
            let mut buf: [u8; 2] = [0; 2];

            let be = Scalar::be(scalar);
            let len_serialized = be.serialize(&mut buf);
            prop_assert_eq!(2, len_serialized);

            let mut parsed = Scalar::be(0);
            let len_parsed = parsed.parse(&buf, len_serialized);
            prop_assert_eq!(2, len_parsed);

            prop_assert_eq!(*parsed, scalar);

            let le = Scalar::le(scalar);
            let len_serialized = le.serialize(&mut buf);
            prop_assert_eq!(2, len_serialized);

            let mut parsed = Scalar::le(0);
            let len_parsed = parsed.parse(&buf, len_serialized);
            prop_assert_eq!(2, len_parsed);

            prop_assert_eq!(*parsed, scalar);
        }

        #[test]
        fn serialize_and_parse_scalar_u32(scalar: u32) {
            let mut buf: [u8; 4] = [0; 4];

            let be = Scalar::be(scalar);
            let len_serialized = be.serialize(&mut buf);
            prop_assert_eq!(4, len_serialized);

            let mut parsed = Scalar::be(0);
            let len_parsed = parsed.parse(&buf, len_serialized);
            prop_assert_eq!(4, len_parsed);

            prop_assert_eq!(*parsed, scalar);

            let le = Scalar::le(scalar);
            let len_serialized = le.serialize(&mut buf);
            prop_assert_eq!(4, len_serialized);

            let mut parsed = Scalar::le(0);
            let len_parsed = parsed.parse(&buf, len_serialized);
            prop_assert_eq!(4, len_parsed);

            prop_assert_eq!(*parsed, scalar);
        }
    }
}
