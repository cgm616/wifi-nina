use embedded_hal_async::spi::{SpiBusRead, SpiBusWrite};

use core::marker;

use crate::encoding;

pub trait SendParam {
    fn len(&self) -> usize;

    fn len_length_delimited(&self, long: bool) -> usize {
        self.len() + if long { 2 } else { 1 }
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite;

    async fn send_length_delimited<S>(&self, spi: &mut S, long: bool) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        encoding::send_len(spi, long, self.len()).await?;
        self.send(spi).await
    }
}

pub trait RecvParam {
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead;

    async fn recv_length_delimited<S>(&mut self, spi: &mut S, long: bool) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        let len = encoding::recv_len(spi, long).await?;
        self.recv(spi, len).await
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct NullTerminated<A>(A)
where
    A: ?Sized;

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

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        (*self).send(spi).await
    }
}

impl<A> RecvParam for &mut A
where
    A: RecvParam + ?Sized,
{
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        (*self).recv(spi, len).await
    }
}

impl SendParam for u8 {
    fn len(&self) -> usize {
        1
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        let buf = [*self];
        spi.write(&buf).await?;
        Ok(())
    }
}

impl RecvParam for u8 {
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        assert_eq!(1, len);
        let mut buf = [0; 1];
        spi.read(&mut buf).await?;
        *self = buf[0];
        Ok(())
    }
}

impl<O> SendParam for Scalar<O, u16>
where
    O: byteorder::ByteOrder,
{
    fn len(&self) -> usize {
        2
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        let mut buf = [0; 2];
        O::write_u16(&mut buf, self.value);
        spi.write(&buf).await
    }
}

impl<O> RecvParam for Scalar<O, u16>
where
    O: byteorder::ByteOrder,
{
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        assert_eq!(2, len);
        let mut buf = [0; 2];
        spi.read(&mut buf).await?;
        self.value = O::read_u16(&buf);
        Ok(())
    }
}

impl<O> SendParam for Scalar<O, u32>
where
    O: byteorder::ByteOrder,
{
    fn len(&self) -> usize {
        4
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        let mut buf = [0; 4];
        O::write_u32(&mut buf, self.value);
        spi.write(&buf).await
    }
}

impl<O> RecvParam for Scalar<O, u32>
where
    O: byteorder::ByteOrder,
{
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        assert_eq!(4, len);
        let mut buf = [0; 4];
        spi.read(&mut buf).await?;
        self.value = O::read_u32(&buf);
        Ok(())
    }
}

impl SendParam for [u8] {
    fn len(&self) -> usize {
        self.len()
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        spi.write(self).await
    }
}

impl<const CAP: usize> SendParam for arrayvec::ArrayVec<u8, CAP> {
    fn len(&self) -> usize {
        self.len()
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        SendParam::send(self.as_slice(), spi).await
    }
}

impl RecvParam for &mut [u8] {
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        use core::mem;

        spi.read(self).await?;

        let slice = mem::take(self);
        *self = &mut slice[..len];

        Ok(())
    }
}

impl<const CAP: usize> RecvParam for arrayvec::ArrayVec<u8, CAP> {
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        let start_index = self.len();
        self.extend(core::iter::repeat(0).take(len));
        spi.read(&mut self[start_index..]).await?;

        Ok(())
    }
}

impl<A> SendParam for NullTerminated<A>
where
    A: SendParam,
{
    fn len(&self) -> usize {
        self.0.len() + 1
    }

    async fn send<S>(&self, spi: &mut S) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        self.0.send(spi).await?;
        let buf = [0; 1];
        spi.write(&buf).await
    }
}

impl<A> RecvParam for NullTerminated<A>
where
    A: RecvParam,
{
    async fn recv<S>(&mut self, spi: &mut S, len: usize) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        self.0.recv(spi, len - 1).await?;
        let mut buf = [1; 1];
        spi.read(&mut buf).await?;
        assert_eq!(0, buf[0]);
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
