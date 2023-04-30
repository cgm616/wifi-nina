use arrayvec::ArrayVec;

use core::marker;

use crate::{encoding, transport::Transporter};

/// A parameter for a WifiNina command
pub trait SerializeParam {
    /// Return the length, in bytes, of sending the parameter
    fn len(&self) -> usize;

    /// Return the length, in bytes, of sending the parameter if length-delimited
    fn len_length_delimited(&self, long: bool) -> usize {
        self.len() + if long { 2 } else { 1 }
    }

    /// Serialize the parameter into the provided `Transporter`
    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error>;

    /// Serialize the parameter into the provided `Transporter with its length first
    async fn serialize_length_delimited<T: Transporter>(
        &self,
        trans: &mut T,
        long: bool,
    ) -> Result<(), T::Error> {
        let len = self.len();
        encoding::serialize_len(trans, long, len).await?;
        self.serialize(trans).await
    }
}

/// A parameters that can be received from the WifiNina
pub trait ParseParam {
    /// Parse the parameter from a `Transporter` given a length
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error>;

    /// Parse the parameter from a `Transporter` without knowing its length
    async fn parse_length_delimited<T: Transporter>(
        &mut self,
        trans: &mut T,
        long: bool,
    ) -> Result<(), T::Error> {
        let len = encoding::parse_len(trans, long).await?;
        self.parse(trans, len).await
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

impl<A> SerializeParam for &A
where
    A: SerializeParam + ?Sized,
{
    fn len(&self) -> usize {
        (*self).len()
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        (*self).serialize(trans).await
    }
}

impl<A> ParseParam for &mut A
where
    A: ParseParam + ?Sized,
{
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        (*self).parse(trans, len).await
    }
}

impl SerializeParam for u8 {
    fn len(&self) -> usize {
        1
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        trans.write(*self).await
    }
}

impl ParseParam for u8 {
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        assert_eq!(1, len);

        *self = trans.read().await?;
        Ok(())
    }
}

impl<O> SerializeParam for Scalar<O, u16>
where
    O: byteorder::ByteOrder,
{
    fn len(&self) -> usize {
        2
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        let mut buf = [0_u8; 2];
        O::write_u16(&mut buf, self.value);
        trans.write_from(&buf).await
    }
}

impl<O> ParseParam for Scalar<O, u16>
where
    O: byteorder::ByteOrder,
{
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        assert_eq!(2, len);

        let mut buf = [0; 2];
        trans.read_into(&mut buf).await?;
        self.value = O::read_u16(&buf);
        Ok(())
    }
}

impl<O> SerializeParam for Scalar<O, u32>
where
    O: byteorder::ByteOrder,
{
    fn len(&self) -> usize {
        4
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        let mut buf = [0_u8; 4];
        O::write_u32(&mut buf, self.value);
        trans.write_from(&buf).await
    }
}

impl<O> ParseParam for Scalar<O, u32>
where
    O: byteorder::ByteOrder,
{
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        assert_eq!(4, len);

        let mut buf = [0; 4];
        trans.read_into(&mut buf).await?;
        self.value = O::read_u32(&buf);
        Ok(())
    }
}

impl SerializeParam for [u8] {
    fn len(&self) -> usize {
        self.len()
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        trans.write_from(&self).await
    }
}

impl<const CAP: usize> SerializeParam for ArrayVec<u8, CAP> {
    fn len(&self) -> usize {
        self.len()
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        self.as_slice().serialize(trans).await
    }
}

impl ParseParam for &mut [u8] {
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        assert!(len <= self.len());

        trans.read_into(&mut self[..len]).await?;
        Ok(())
    }
}

impl<const CAP: usize> ParseParam for ArrayVec<u8, CAP> {
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        if self.len() < len {
            // make space in the vector
            self.extend(core::iter::repeat(0).take(len - self.len()));
        }
        self.as_mut_slice().parse(trans, len).await // fill it up
    }
}

impl<A> SerializeParam for NullTerminated<A>
where
    A: SerializeParam,
{
    fn len(&self) -> usize {
        self.0.len() + 1
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T) -> Result<(), T::Error> {
        self.0.serialize(trans).await?;
        trans.write(0).await
    }
}

impl<A> ParseParam for NullTerminated<A>
where
    A: ParseParam,
{
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        self.0.parse(trans, len - 1).await?;
        assert_eq!(trans.read().await?, 0);
        Ok(())
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
    use crate::util::test::MockTransporter;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn serialize_and_parse_u8(byte: u8) {
            let mut trans: MockTransporter<2> = MockTransporter::new();

            byte.serialize(&mut trans).await.unwrap();

            prop_assert_eq!(trans.buffer[0], byte);

            let mut parsed = 0_u8;
            parsed.parse(&mut trans, 1).await.unwrap();

            prop_assert_eq!(parsed, byte);
        }

        #[test]
        fn serialize_and_parse_u8_with_length(byte: u8) {
            let mut buf: [u8; 2] = [0; 2];

            let len_serialized = byte.serialize_length_delimited(&mut buf, false, None).unwrap();
            prop_assert_eq!(2, len_serialized);

            prop_assert_eq!(buf[0], 1);
            prop_assert_eq!(buf[1], byte);

            let mut parsed = 0_u8;
            let len_parsed = parsed.parse_length_delimited(&buf, false, None).unwrap();
            prop_assert_eq!(2, len_parsed);

            prop_assert_eq!(parsed, byte);
        }

        #[test]
        fn serialize_and_parse_bytes_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=127)) {
            let mut buf: [u8; 128] = [0; 128];

            let len_serialized = bytes.as_slice().serialize_length_delimited(&mut buf, false, None).unwrap();
            prop_assert_eq!(bytes.len() + 1, len_serialized);

            prop_assert_eq!(buf[0] as usize, bytes.len());

            let mut parsed = ArrayVec::<u8, 127>::new();
            let len_parsed = parsed.parse_length_delimited(&buf, false, None).unwrap();
            prop_assert_eq!(len_serialized, len_parsed);

            prop_assert_eq!(parsed.as_slice(), bytes.as_slice());
        }

        #[test]
        fn serialize_and_parse_arrayvec_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=127)) {
            let mut buf: [u8; 128] = [0; 128];

            let mut arrayvec = ArrayVec::<u8, 127>::new();
            arrayvec.try_extend_from_slice(bytes.as_slice()).unwrap();
            let len_serialized = arrayvec.serialize_length_delimited(&mut buf, false, None).unwrap();
            prop_assert_eq!(bytes.len() + 1, len_serialized);

            prop_assert_eq!(buf[0] as usize, bytes.len());

            let mut parsed = ArrayVec::<u8, 127>::new();
            let len_parsed = parsed.parse_length_delimited(&buf, false, None).unwrap();
            prop_assert_eq!(len_serialized, len_parsed);

            prop_assert_eq!(parsed.as_slice(), arrayvec.as_slice());
        }

        #[test]
        fn serialize_and_parse_nullterminated_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=8)) {
            let mut buf: [u8; 10] = [0; 10];

            let mut arrayvec = ArrayVec::<u8, 8>::new();
            arrayvec.try_extend_from_slice(bytes.as_slice()).unwrap();
            let null_terminated = NullTerminated(arrayvec);

            let len_serialized = null_terminated.serialize_length_delimited(&mut buf, false, None).unwrap();
            prop_assert_eq!(len_serialized, bytes.len() + 2);

            prop_assert_eq!(buf[0] as usize, bytes.len() + 1);
            prop_assert_eq!(buf[9], 0);

            let mut parsed = NullTerminated(ArrayVec::<u8, 8>::new());
            let len_parsed = parsed.parse_length_delimited(&buf, false, None).unwrap();
            prop_assert_eq!(len_serialized, len_parsed);

            prop_assert_eq!(parsed.as_slice(), null_terminated.as_slice());
        }

        #[test]
        fn serialize_and_parse_scalar_u16(scalar: u16) {
            let mut buf: [u8; 2] = [0; 2];

            let be = Scalar::be(scalar);
            let len_serialized = be.serialize(&mut buf, None).unwrap();
            prop_assert_eq!(2, len_serialized);

            let mut parsed = Scalar::be(0);
            let len_parsed = parsed.parse(&buf, len_serialized, None).unwrap();
            prop_assert_eq!(2, len_parsed);

            prop_assert_eq!(*parsed, scalar);

            let le = Scalar::le(scalar);
            let len_serialized = le.serialize(&mut buf, None).unwrap();
            prop_assert_eq!(2, len_serialized);

            let mut parsed = Scalar::le(0);
            let len_parsed = parsed.parse(&buf, len_serialized, None).unwrap();
            prop_assert_eq!(2, len_parsed);

            prop_assert_eq!(*parsed, scalar);
        }

        #[test]
        fn serialize_and_parse_scalar_u32(scalar: u32) {
            let mut buf: [u8; 4] = [0; 4];

            let be = Scalar::be(scalar);
            let len_serialized = be.serialize(&mut buf, None).unwrap();
            prop_assert_eq!(4, len_serialized);

            let mut parsed = Scalar::be(0);
            let len_parsed = parsed.parse(&buf, len_serialized, None).unwrap();
            prop_assert_eq!(4, len_parsed);

            prop_assert_eq!(*parsed, scalar);

            let le = Scalar::le(scalar);
            let len_serialized = le.serialize(&mut buf, None).unwrap();
            prop_assert_eq!(4, len_serialized);

            let mut parsed = Scalar::le(0);
            let len_parsed = parsed.parse(&buf, len_serialized, None).unwrap();
            prop_assert_eq!(4, len_parsed);

            prop_assert_eq!(*parsed, scalar);
        }

        #[test]
        fn serialize_partial_bytes(ref bytes in proptest::collection::vec(any::<u8>(), 10..=16)) {
            let mut buf: [u8; 10] = [0; 10]; // not long enough for the input

            let res = bytes.as_slice().serialize_length_delimited(&mut buf, false, None);
            prop_assert!(res.is_err());
            let len_serialized = res.unwrap_err();
            prop_assert_eq!(len_serialized, 10); // should have filled the buffer

            prop_assert_eq!(buf[0] as usize, bytes.len());
            prop_assert_eq!(buf[1..], bytes[1..10]);

            buf = [0; 10]; // do something with the buffer, i.e. send it

            let res = bytes.as_slice().serialize_length_delimited(&mut buf, false, Some(len_serialized));
            prop_assert!(res.is_ok()); // should succeed!
            let len_serialized = res.unwrap();
            prop_assert_eq!(len_serialized, bytes.len() + 1 - 10); // should report partial length: only what's put in the buffer in this call

            prop_assert_eq!(buf[..len_serialized], bytes[10..10 + len_serialized]);
        }
    }
}
