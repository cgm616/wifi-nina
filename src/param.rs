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
        self.value = O::read_u16(&buf);
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
        self.value = O::read_u32(&buf);
        4
    }
}

impl SendParam for [u8] {
    fn len(&self) -> usize {
        self.len()
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        assert!(self.len() <= buf.len());
        (&mut buf[..self.len()]).copy_from_slice(self);
        self.len()
    }
}

impl<const CAP: usize> SendParam for ArrayVec<u8, CAP> {
    fn len(&self) -> usize {
        self.len()
    }

    fn serialize(&self, buf: &mut [u8]) -> usize {
        assert!(self.len() <= buf.len());
        (&mut buf[..self.len()]).copy_from_slice(self.as_slice());
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
