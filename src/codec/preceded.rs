use std::{fmt, marker::PhantomData};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, Nul, Pattern, SelfDelimiting};

/// Writes a [`Pattern`] before any codec. It's the mirror of [`Terminated`](crate::codec::Terminated).
///
/// The format is the pattern of `P` (a single `0x00` byte by default), followed
/// by the bytes of the inner codec `C`. Decoding checks the bytes start with the
/// pattern and fails with [`PrecededDecodeError::MissingPattern`] otherwise.
///
/// The pattern only marks where the inner codec starts, it doesn't tell where
/// it ends: `Preceded<C>` is [`SelfDelimiting`] exactly when `C` is. To mark
/// where a codec ends, use [`Terminated`](crate::codec::Terminated) instead.
/// Every key starts with the same pattern, so it keeps the order of `C`.
///
/// It's useful when we need a separator between a codec, and an optional codec.
/// Since the separator must be part of the optional codec, using [`Terminated`](crate::codec::Terminated)
/// on the first codec ni all cases would be overkill.
///
/// ```
/// use fiole::codec::{Decode, DecodingVec, Encode, OrderedU32, Optional, Preceded, Text};
///
/// // An id, then a `\0` and a version if there's one
/// type Key = (OrderedU32, Optional<Preceded<Text<u16>>>);
///
/// let bytes = Key::encode_alloc(&(42, None)).unwrap().finish();
/// assert_eq!(bytes, b"\0\0\0\x2a");
/// let bytes = Key::encode_alloc(&(42, Some(7))).unwrap().finish();
/// assert_eq!(bytes, b"\0\0\0\x2a\x007");
/// assert_eq!(Key::decode(&mut DecodingVec::new(bytes)).unwrap(), (42, Some(7)));
///
/// // Something left that doesn't start with the separator isn't of this codec
/// assert!(Key::decode(&mut DecodingVec::new(b"\0\0\0\x2a7")).is_err());
/// ```
pub struct Preceded<C, P = Nul>(PhantomData<(C, P)>);

impl<C: Encode, P: Pattern> Encode for Preceded<C, P> {
    type Item = C::Item;
    type Error = C::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        ret.extend(P::PATTERN);
        C::encode(ret.make_fresh(), item)
    }
}

impl<C: Decode, P: Pattern> Decode for Preceded<C, P> {
    type Item = C::Item;
    type Error = PrecededDecodeError<C::Error>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        if !bytes.remaining_slice().starts_with(P::PATTERN) {
            return Err(PrecededDecodeError::MissingPattern);
        }
        bytes.take_next(P::PATTERN.len());
        C::decode(bytes).map_err(PrecededDecodeError::Inner)
    }
}

impl<C: SelfDelimiting, P> SelfDelimiting for Preceded<C, P> {}

#[derive(Debug)]
pub enum PrecededDecodeError<E> {
    /// The bytes don't start with the pattern.
    MissingPattern,
    Inner(E),
}

impl<E: fmt::Display> fmt::Display for PrecededDecodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrecededDecodeError::MissingPattern => {
                f.write_str("Preceded codec couldn't find its pattern before the inner codec")
            }
            PrecededDecodeError::Inner(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for PrecededDecodeError<E> {}

#[cfg(test)]
mod test {
    use byteorder::BE;

    use crate::codec::{
        Decode, DecodingVec, Encode, Optional, OrderedU32, Pattern, Preceded, PrecededDecodeError,
        Str, Text, U8, U32, check_order_preserving, check_round_trip, check_self_delimiting,
    };

    enum Arrow {}

    impl Pattern for Arrow {
        const PATTERN: &'static [u8] = b"->";
    }

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Preceded<U32<BE>>, u32>);
        arbtest::arbtest(check_round_trip::<Preceded<Str, Arrow>, String>);
    }

    #[test]
    fn writes_its_pattern_then_the_inner_codec() {
        assert_eq!(
            Preceded::<U8>::encode_alloc(&7).unwrap().finish(),
            b"\0\x07"
        );
        assert_eq!(
            <Preceded<&Str, Arrow>>::encode_alloc(&"a")
                .unwrap()
                .finish(),
            b"->a"
        );
    }

    #[test]
    fn fails_without_its_pattern() {
        for bytes in [&b""[..], b"\x07", b"\x01\x07"] {
            let error = Preceded::<U8>::decode(&mut DecodingVec::new(bytes)).unwrap_err();
            assert!(
                matches!(error, PrecededDecodeError::MissingPattern),
                "{bytes:?}"
            );
        }
        // The pattern is there, but not what follows it
        let error = Preceded::<U8>::decode(&mut DecodingVec::new(b"\0")).unwrap_err();
        assert!(matches!(error, PrecededDecodeError::Inner(_)));
    }

    #[test]
    fn knows_where_it_ends_when_the_inner_codec_does() {
        arbtest::arbtest(check_self_delimiting::<Preceded<U8>, u8>);
        arbtest::arbtest(check_self_delimiting::<Preceded<OrderedU32, Arrow>, u32>);

        type Codec = (Preceded<U8>, Preceded<U8, Arrow>, U8);
        let bytes = Codec::encode_alloc(&(1, 2, 3)).unwrap().finish();
        assert_eq!(bytes, b"\0\x01->\x02\x03");
        assert_eq!(
            Codec::decode(&mut DecodingVec::new(bytes)).unwrap(),
            (1, 2, 3)
        );
    }

    #[test]
    fn keeps_the_order_of_the_inner_codec() {
        arbtest::arbtest(check_order_preserving::<Preceded<OrderedU32>, u32>);
    }

    #[test]
    fn writes_a_separator_before_an_optional_part() {
        type Codec = (U8, Optional<Preceded<Text<u16>>>);
        assert_eq!(Codec::encode_alloc(&(1, None)).unwrap().finish(), b"\x01");
        assert_eq!(
            Codec::encode_alloc(&(1, Some(7))).unwrap().finish(),
            b"\x01\x007"
        );
        for item in [(1, None), (1, Some(7))] {
            let bytes = Codec::encode_alloc(&item).unwrap().finish();
            assert_eq!(Codec::decode(&mut DecodingVec::new(bytes)).unwrap(), item);
        }
        // Without its separator, what's left isn't the optional part
        assert!(Codec::decode(&mut DecodingVec::new(b"\x017")).is_err());
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<Preceded<U32<BE>>>(&42);
    }
}
