use heapless::Vec;

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
        trans.write_from(self).await
    }
}

impl<const CAP: usize> SerializeParam for Vec<u8, CAP> {
    fn len(&self) -> usize {
        self.as_slice().len()
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

impl<const CAP: usize> ParseParam for Vec<u8, CAP> {
    async fn parse<T: Transporter>(&mut self, trans: &mut T, len: usize) -> Result<(), T::Error> {
        if self.len() < len {
            // make space in the vector
            self.extend(core::iter::repeat(0).take(len - self.len()));
        }
        core::ops::DerefMut::deref_mut(self).parse(trans, len).await // fill it up
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
    use proptest::prelude::*;

    use super::*;
    use crate::util::test::{async_test, MockTransporter};

    proptest! {
        #[test]
        fn serialize_and_parse_u8(byte: u8) {
            async_test! {
                let mut trans: MockTransporter<2> = MockTransporter::new();

                byte.serialize(&mut trans).await?;

                prop_assert_eq!(trans.buffer[0], byte);

                trans.to_reader();

                let mut parsed = 0_u8;
                parsed.parse(&mut trans, 1).await?;

                prop_assert_eq!(parsed, byte);
                Ok(())
            }
        }

        #[test]
        fn serialize_and_parse_u8_with_length(byte: u8) {
            async_test! {
                let mut trans: MockTransporter<2> = MockTransporter::new();

                byte.serialize_length_delimited(&mut trans, false).await?;

                prop_assert_eq!(trans.buffer[0], 1);
                prop_assert_eq!(trans.buffer[1], byte);

                trans.to_reader();

                let mut parsed = 0_u8;
                parsed.parse_length_delimited(&mut trans, false).await?;

                prop_assert_eq!(parsed, byte);
                Ok(())
            }
        }

        #[test]
        fn serialize_and_parse_bytes_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=127)) {
            async_test! {
                let mut trans: MockTransporter<128> = MockTransporter::new();

                bytes.as_slice().serialize_length_delimited(&mut trans, false).await?;

                prop_assert_eq!(trans.buffer[0] as usize, bytes.len());

                trans.to_reader();

                let mut parsed = Vec::<u8, 127>::new();
                parsed.parse_length_delimited(&mut trans, false).await?;

                prop_assert_eq!(parsed.as_slice(), bytes.as_slice());
                Ok(())
            }
        }

        #[test]
        fn serialize_and_parse_vec_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=127)) {
            async_test! {
                let mut trans: MockTransporter<128> = MockTransporter::new();

                let mut vec = Vec::<u8, 127>::new();
                vec.extend_from_slice(bytes.as_slice()).unwrap();
                vec.serialize_length_delimited(&mut trans, false).await.unwrap();

                prop_assert_eq!(trans.buffer[0] as usize, bytes.len());

                trans.to_reader();

                let mut parsed = Vec::<u8, 127>::new();
                parsed.parse_length_delimited(&mut trans, false).await.unwrap();

                prop_assert_eq!(parsed.as_slice(), vec.as_slice());
                Ok(())
            }


        }

        #[test]
        fn serialize_and_parse_nullterminated_with_length(ref bytes in proptest::collection::vec(any::<u8>(), 0..=8)) {
            async_test! {
                let mut trans: MockTransporter<10> = MockTransporter::new();

                let mut vec = Vec::<u8, 8>::new();
                vec.extend_from_slice(bytes.as_slice()).unwrap();
                let null_terminated = NullTerminated(vec);

                null_terminated.serialize_length_delimited(&mut trans, false).await?;

                prop_assert_eq!(trans.buffer[0] as usize, bytes.len() + 1);
                prop_assert_eq!(trans.buffer[9], 0);

                trans.to_reader();

                let mut parsed = NullTerminated(Vec::<u8, 8>::new());
                parsed.parse_length_delimited(&mut trans, false).await?;

                prop_assert_eq!(parsed.as_slice(), null_terminated.as_slice());
                Ok(())
            }
        }

        #[test]
        fn serialize_and_parse_scalar_u16(scalar: u16) {
            async_test! {
                let mut trans: MockTransporter<2> = MockTransporter::new();

                let be = Scalar::be(scalar);
                be.serialize(&mut trans).await?;

                trans.to_reader();

                let mut parsed = Scalar::be(0);
                parsed.parse(&mut trans, 2).await?;

                prop_assert_eq!(*parsed, scalar);

                trans.clear();

                let le = Scalar::le(scalar);
                le.serialize(&mut trans).await?;

                trans.to_reader();

                let mut parsed = Scalar::le(0);
                parsed.parse(&mut trans, 2).await?;

                prop_assert_eq!(*parsed, scalar);
                Ok(())
            }
        }

        #[test]
        fn serialize_and_parse_scalar_u32(scalar: u32) {
            async_test! {
                let mut trans: MockTransporter<4> = MockTransporter::new();

                let be = Scalar::be(scalar);
                be.serialize(&mut trans).await?;

                trans.to_reader();

                let mut parsed = Scalar::be(0);
                parsed.parse(&mut trans, 4).await?;

                prop_assert_eq!(*parsed, scalar);

                trans.clear();

                let le = Scalar::le(scalar);
                le.serialize(&mut trans).await?;

                trans.to_reader();

                let mut parsed = Scalar::le(0);
                parsed.parse(&mut trans, 4).await?;

                prop_assert_eq!(*parsed, scalar);
                Ok(())
            }

        }
    }
}
