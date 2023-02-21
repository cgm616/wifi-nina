use embedded_hal_async::spi::{SpiBusRead, SpiBusWrite};

use super::param;

pub trait SendParams {
    fn len(&self, long: bool) -> usize {
        self.param_len(long) + 1
    }

    fn param_len(&self, long: bool) -> usize;

    async fn send<S>(&self, spi: &mut S, long: bool) -> Result<(), S::Error>
    where
        S: SpiBusWrite;
}

pub trait RecvParams {
    async fn recv<S>(&mut self, spi: &mut S, long: bool) -> Result<(), S::Error>
    where
        S: SpiBusRead;
}

impl SendParams for () {
    fn param_len(&self, _long: bool) -> usize {
        0
    }

    async fn send<S>(&self, spi: &mut S, _long: bool) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        let buf = [0; 1];
        spi.write(&buf).await
    }
}

impl RecvParams for () {
    async fn recv<S>(&mut self, spi: &mut S, _long: bool) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        let mut buf = [1; 1];
        spi.read(&mut buf).await?;
        assert_eq!(0, buf[0]);
        Ok(())
    }
}

macro_rules! count {
    () => (0u8);
    ( $x:tt $($xs:tt)* ) => (1u8 + count!($($xs)*));
}

macro_rules! tuple_impls {
    ( $head:ident, $( $tail:ident, )* ) => {
        impl<$head, $( $tail ),*> SendParams for ($head, $( $tail ),*)
        where
            $head: param::SendParam,
            $( $tail: param::SendParam ),*
        {
            fn param_len(&self, long: bool) -> usize {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                $head.len_length_delimited(long) $(+ $tail.len_length_delimited(long) )*
            }

            async fn send<S>(&self, spi: &mut S, long: bool) -> Result<(), S::Error>
            where
                S: SpiBusWrite,
            {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                let num = count!($head $( $tail )*);
                let buf = [num];
                spi.write(&buf).await?;
                $head.send_length_delimited(spi, long).await?;
                $(
                    $tail.send_length_delimited(spi, long).await?;
                )*
                Ok(())
            }
        }

        impl<$head, $( $tail ),*> RecvParams for ($head, $( $tail ),*)
        where
            $head: param::RecvParam,
            $( $tail: param::RecvParam ),*
        {
            async fn recv<S>(&mut self, spi: &mut S, long: bool) -> Result<(), S::Error>
            where
                S: SpiBusRead,
            {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                let num = count!($head $( $tail )*);
                let mut buf = [0; 1];
                spi.read(&mut buf).await?;
                assert_eq!(num, buf[0]);
                $head.recv_length_delimited(spi, long).await?;
                $(
                    $tail.recv_length_delimited(spi, long).await?;
                )*
                Ok(())
            }
        }

        tuple_impls!($( $tail, )*);
    };

    () => {};
}

tuple_impls!(A, B, C, D, E,);

impl<T, const CAP: usize> SendParams for arrayvec::ArrayVec<T, CAP>
where
    T: param::SendParam,
{
    fn param_len(&self, long: bool) -> usize {
        self.iter().map(|p| p.len_length_delimited(long)).sum()
    }

    async fn send<S>(&self, spi: &mut S, long: bool) -> Result<(), S::Error>
    where
        S: SpiBusWrite,
    {
        use core::convert::TryFrom;

        let len = u8::try_from(self.len()).unwrap(); // TODO:: do we really want to unwrap?
        let buf = [len];
        spi.write(&buf).await?;
        for item in self.iter() {
            item.send_length_delimited(spi, long).await?;
        }
        Ok(())
    }
}

impl<T, const CAP: usize> RecvParams for arrayvec::ArrayVec<T, CAP>
where
    T: param::RecvParam + Default,
{
    async fn recv<S>(&mut self, spi: &mut S, long: bool) -> Result<(), S::Error>
    where
        S: SpiBusRead,
    {
        let mut buf = [0];
        spi.read(&mut buf).await?;

        for _ in 0..buf[0] {
            let mut item: T = Default::default();
            item.recv_length_delimited(spi, long).await?;
            self.push(item);
        }
        Ok(())
    }
}
