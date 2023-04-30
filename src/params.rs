use arrayvec::ArrayVec;

use crate::transport::Transporter;

use super::param;

/// A collection of parameters that can be sent to the WifiNina
pub trait SerializeParams {
    /// Return the length, in bytes, of sending the parameters
    fn len(&self, long: bool) -> usize;

    /// Serialize the parameters into a `Transporter`
    async fn serialize<T: Transporter>(&self, trans: &mut T, long: bool) -> Result<(), T::Error>;
}

/// A collection of parameters that can be received from the WifiNina
pub trait ParseParams {
    /// Parse the parameters from a `Transporter`
    async fn parse<T: Transporter>(&mut self, trans: &mut T, long: bool) -> Result<(), T::Error>;
}

impl SerializeParams for () {
    fn len(&self, _long: bool) -> usize {
        1
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T, _long: bool) -> Result<(), T::Error> {
        trans.write(0).await
    }
}

impl ParseParams for () {
    async fn parse<T: Transporter>(&mut self, trans: &mut T, _long: bool) -> Result<(), T::Error> {
        assert_eq!(0, trans.read().await?);
        Ok(())
    }
}

macro_rules! count {
    () => (0u8);
    ( $x:tt $($xs:tt)* ) => (1u8 + count!($($xs)*));
}

macro_rules! tuple_impls {
    ( $head:ident, $( $tail:ident, )* ) => {
        impl<$head, $( $tail ),*> SerializeParams for ($head, $( $tail ),*)
        where
            $head: param::SerializeParam,
            $( $tail: param::SerializeParam ),*
        {
            fn len(&self, long: bool) -> usize {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                1 + $head.len_length_delimited(long) $(+ $tail.len_length_delimited(long) )*
            }

            async fn serialize<T: Transporter>(&self, trans: &mut T, long: bool) -> Result<(), T::Error> {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                let num = count!($head $( $tail )*);
                trans.write(num).await?;
                $head.serialize_length_delimited(trans, long).await?;
                $(
                    $tail.serialize_length_delimited(trans, long).await?;
                )*
                Ok(())
            }
        }

        impl<$head, $( $tail ),*> ParseParams for ($head, $( $tail ),*)
        where
            $head: param::ParseParam,
            $( $tail: param::ParseParam ),*
        {
            async fn parse<T: Transporter>(&mut self, trans: &mut T, long: bool) -> Result<(), T::Error>
            {
                #[allow(non_snake_case)]
                let ($head, $( $tail ),*) = self;
                let num = count!($head $( $tail )*);
                assert_eq!(num, trans.read().await?);
                $head.parse_length_delimited(trans, long).await?;
                $(
                    $tail.parse_length_delimited(trans, long).await?;
                )*
                Ok(())
            }
        }

        tuple_impls!($( $tail, )*);
    };

    () => {};
}

tuple_impls!(A, B, C, D, E,);

impl<U, const CAP: usize> SerializeParams for ArrayVec<U, CAP>
where
    U: param::SerializeParam,
{
    fn len(&self, long: bool) -> usize {
        1 + self
            .iter()
            .map(|p| p.len_length_delimited(long))
            .sum::<usize>()
    }

    async fn serialize<T: Transporter>(&self, trans: &mut T, long: bool) -> Result<(), T::Error> {
        use core::convert::TryFrom;

        let len = u8::try_from(self.len()).unwrap(); // TODO:: do we really want to unwrap?
        trans.write(len).await?;
        for item in self.iter() {
            item.serialize_length_delimited(trans, long).await?;
        }
        Ok(())
    }
}

impl<U, const CAP: usize> ParseParams for arrayvec::ArrayVec<U, CAP>
where
    U: param::ParseParam + Default,
{
    async fn parse<T: Transporter>(&mut self, trans: &mut T, long: bool) -> Result<(), T::Error> {
        let items = trans.read().await?;
        for _ in 0..items {
            let mut item: U = Default::default();
            item.parse_length_delimited(trans, long).await?;
            self.push(item);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn serialize_and_parse_five_tuple(params: (u8, u8, u8, u8, u8)) {
            prop_assert_eq!(params.len(false), 11);
            let mut buf: [u8; 11] = [0; 11];

            let serialized_len = params.serialize(&mut buf, false);
            prop_assert_eq!(serialized_len, 11);

            let expected = [5, 1, params.0, 1, params.1, 1, params.2, 1, params.3, 1, params.4];
            prop_assert_eq!(buf.as_slice(), &expected);

            let mut parsed = (0, 0, 0, 0, 0);
            let parsed_len = parsed.parse(&buf, false);
            prop_assert_eq!(parsed_len, serialized_len);

            prop_assert_eq!(parsed, params);
        }

        #[test]
        fn serialize_and_parse_heterogenous_tuple(first: u8, ref second in proptest::collection::vec(any::<u8>(), 0..=16)) {
            let mut buf: [u8; 20] = [0; 20];

            let mut arrayvec = ArrayVec::<u8, 16>::new();
            arrayvec.try_extend_from_slice(second.as_slice()).unwrap();
            let params = (first, arrayvec);
            let serialized_len = params.serialize(&mut buf, false);

            let mut parsed = (0, ArrayVec::<u8, 16>::new());
            let parsed_len = parsed.parse(&buf, false);
            prop_assert_eq!(parsed_len, serialized_len);

            prop_assert_eq!(parsed, params);
        }

        #[test]
        fn serialize_and_parse_arrayvec(params in proptest::collection::vec(any::<u32>(), 0..=16)) {
            use crate::param::Scalar;

            let mut buf: [u8; 81] = [0; 81];

            let mut arrayvec = ArrayVec::<Scalar<byteorder::BigEndian, u32>, 16>::new();
            arrayvec.extend(params.iter().cloned().map(Scalar::be));
            let serialized_len = arrayvec.serialize(&mut buf, false);

            let mut parsed = ArrayVec::<Scalar<byteorder::BigEndian, u32>, 16>::new();
            let parsed_len = parsed.parse(&buf, false);
            prop_assert_eq!(parsed_len, serialized_len);

            prop_assert_eq!(parsed, arrayvec);
        }
    }
}
