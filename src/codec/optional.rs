use std::marker::PhantomData;

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Describe an [`Option`]al codec. `None` is not written at all, and `Some` is written as the inner codec `C`.
///
/// Decoding gives `None` when no bytes are left, and decodes the rest with `C` otherwise.
/// That makes `None` sort before every `Some`. And it make `Some` keep the order of `C`.
///
/// /!\ This codec is final: it needs to know whether anything is left, so it can only be the last element of a tuple of codecs. To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec).
///
/// /!\ A `Some` whose inner codec writes nothing, like `Some(())` or `Some("")`, is read back as `None`.
///
/// ```
/// use fiole::codec::{Constant, Decode, DecodingVec, Encode, Nul, Optional, Str, Terminated};
///
/// // A name, then a `\0` and a nickname if there's one
/// type Name<'a> = (&'a Terminated<Str>, Optional<(Constant<Nul>, &'a Str)>);
///
/// let bytes = Name::encode_alloc(&("kefir", None)).unwrap().finish();
/// assert_eq!(bytes, b"kefir\0");
/// let (name, nickname) = Name::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!((&*name, nickname), ("kefir", None));
///
/// let bytes = Name::encode_alloc(&("kefir", Some(((), "")))).unwrap().finish();
/// assert_eq!(bytes, b"kefir\0\0");
/// let (_, nickname) = Name::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!(nickname.map(|((), nickname)| nickname.to_string()), Some(String::new()));
/// ```
pub struct Optional<C>(PhantomData<C>);

impl<C: Encode> Encode for Optional<C>
where
    C::Item: Sized,
{
    type Item = Option<C::Item>;
    type Error = C::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        match item {
            Some(item) => C::encode(into, item),
            None => Ok(into),
        }
    }
}

impl<C: Decode> Decode for Optional<C> {
    type Item = Option<C::Item>;
    type Error = C::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        if bytes.is_empty() {
            Ok(None)
        } else {
            C::decode(bytes).map(Some)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{
        Constant, Decode, DecodingVec, Encode, Nul, Optional, OrderedU32, U8, Unit,
        check_round_trip,
    };

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Optional<U8>, Option<u8>>);
        arbtest::arbtest(check_round_trip::<Optional<OrderedU32>, Option<u32>>);
    }

    #[test]
    fn none_is_nothing() {
        assert_eq!(Optional::<U8>::encode_alloc(&None).unwrap().finish(), b"");
        assert_eq!(
            Optional::<U8>::encode_alloc(&Some(7)).unwrap().finish(),
            b"\x07"
        );
    }

    #[test]
    fn none_sorts_first() {
        let encode = |item: Option<u32>| {
            Optional::<OrderedU32>::encode_alloc(&item)
                .unwrap()
                .finish()
        };
        assert!(encode(None) < encode(Some(0)));
        assert!(encode(Some(1)) < encode(Some(2)));
    }

    #[test]
    fn an_empty_some_is_read_as_none() {
        // What the documentation warns about
        let bytes = Optional::<Unit>::encode_alloc(&Some(())).unwrap().finish();
        assert_eq!(
            Optional::<Unit>::decode(&mut DecodingVec::new(bytes)).unwrap(),
            None
        );
    }

    #[test]
    fn ends_a_tuple() {
        type Codec = (U8, Optional<(Constant<Nul>, U8)>);
        for item in [(1, None), (1, Some(((), 2)))] {
            let bytes = Codec::encode_alloc(&item).unwrap().finish();
            assert_eq!(Codec::decode(&mut DecodingVec::new(bytes)).unwrap(), item);
        }
        assert_eq!(
            Codec::encode_alloc(&(1, Some(((), 2)))).unwrap().finish(),
            b"\x01\0\x02"
        );
        // Something left that isn't the optional part
        assert!(Codec::decode(&mut DecodingVec::new(b"\x01\x02")).is_err());
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<Optional<U8>>(&None);
        crate::test_alloc::assert_decodes_without_allocating::<Optional<U8>>(&Some(7));
    }
}
