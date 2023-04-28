use arrayvec::ArrayVec;

use super::param;

/// A collection of parameters that can be sent to the WifiNina
pub trait SendParams {
    /// Return the length, in bytes, of sending the parameters
    fn len(&self, long: bool) -> usize;

    /// Serialize the parameters into the provided buffer, returning the length written
    fn serialize(&self, buf: &mut [u8], long: bool) -> usize;
}

/// A collection of parameters that can be received from the WifiNina
pub trait RecvParams {
    /// Parse the parameters from the contents of a buffer
    fn parse(&mut self, buf: &[u8], long: bool) -> usize;
}

impl SendParams for () {
    fn len(&self, _long: bool) -> usize {
        1
    }

    fn serialize(&self, buf: &mut [u8], _long: bool) -> usize {
        buf[0] = 0;
        1
    }
}

impl RecvParams for () {
    fn parse(&mut self, buf: &[u8], _long: bool) -> usize {
        assert_eq!(0, buf[0]);
        1
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
            fn len(&self, long: bool) -> usize {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                1 + $head.len_length_delimited(long) $(+ $tail.len_length_delimited(long) )*
            }

            fn serialize(&self, buf: &mut [u8], long: bool) -> usize {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                let num = count!($head $( $tail )*);
                buf[0] = num;
                let mut len = 1;
                len = len + $head.serialize_length_delimited(&mut buf[len..], long);
                $(
                    len = len + $tail.serialize_length_delimited(&mut buf[len..], long);
                )*
                len
            }
        }

        impl<$head, $( $tail ),*> RecvParams for ($head, $( $tail ),*)
        where
            $head: param::RecvParam,
            $( $tail: param::RecvParam ),*
        {
            fn parse(&mut self, buf: &[u8], long: bool) -> usize
            {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                let num = count!($head $( $tail )*);

                assert_eq!(num, buf[0]);
                let mut len = 1;
                len = len + $head.parse_length_delimited(&buf[len..], long);
                $(
                    len = len + $tail.parse_length_delimited(&buf[len..], long);
                )*
                len
            }
        }

        tuple_impls!($( $tail, )*);
    };

    () => {};
}

tuple_impls!(A, B, C, D, E,);

impl<T, const CAP: usize> SendParams for ArrayVec<T, CAP>
where
    T: param::SendParam,
{
    fn len(&self, long: bool) -> usize {
        1 + self
            .iter()
            .map(|p| p.len_length_delimited(long))
            .sum::<usize>()
    }

    fn serialize(&self, buf: &mut [u8], long: bool) -> usize {
        use core::convert::TryFrom;

        let len = u8::try_from(self.len()).unwrap(); // TODO:: do we really want to unwrap?
        buf[0] = len;
        let mut cursor = 1;
        for item in self.iter() {
            cursor = cursor + item.serialize_length_delimited(&mut buf[cursor..], long);
        }
        cursor
    }
}

impl<T, const CAP: usize> RecvParams for arrayvec::ArrayVec<T, CAP>
where
    T: param::RecvParam + Default,
{
    fn parse(&mut self, buf: &[u8], long: bool) -> usize {
        let items = buf[0];
        let mut cursor = 1;
        for _ in 0..items {
            let mut item: T = Default::default();
            cursor = cursor + item.parse_length_delimited(&buf[cursor..], long);
            self.push(item);
        }
        cursor
    }
}
