use std::{convert::Infallible, fmt, marker::PhantomData};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, Pattern, SelfDelimiting};

/// Describe the bytes that are always the same in a key or value.
///
/// It encodes the unit type `()` as the pattern of `P`, a single `0x00` byte with [`Nul`](crate::codec::Nul).
/// Decoding checks the bytes start with the pattern, reads them, and fails with a [`MissingConstantError`] otherwise.
/// It knows where it ends, so it can be used anywhere in a tuple of codecs, and keeps the order of what follows it.
/// When used as a separator, you should prefer using `Terminated` which also make sure your inner codec doesn't
/// contains the separator.
///
/// ```
/// use fiole::codec::{Constant, Decode, DecodingVec, Encode, Nul, U8};
///
/// type Codec = (U8, Constant<Nul>, U8);
///
/// let bytes = Codec::encode_alloc(&(1, (), 2)).unwrap().finish();
/// assert_eq!(bytes, b"\x01\0\x02");
/// assert_eq!(Codec::decode(&mut DecodingVec::new(bytes)).unwrap(), (1, (), 2));
///
/// // Without the constant, the bytes aren't of this codec
/// assert!(Codec::decode(&mut DecodingVec::new(b"\x01\x02")).is_err());
/// ```
pub struct Constant<P>(PhantomData<P>);

impl<P: Pattern> Encode for Constant<P> {
    type Item = ();
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        _item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        ret.extend(P::PATTERN);
        Ok(ret.make_fresh())
    }
}

impl<P: Pattern> Decode for Constant<P> {
    type Item = ();
    type Error = MissingConstantError;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        if bytes.remaining_slice().starts_with(P::PATTERN) {
            bytes.take_next(P::PATTERN.len());
            Ok(())
        } else {
            Err(MissingConstantError)
        }
    }
}

impl<P> SelfDelimiting for Constant<P> {}

/// The bytes don't start with the pattern of a [`Constant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MissingConstantError;

impl fmt::Display for MissingConstantError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("the bytes don't start with the constant pattern")
    }
}

impl std::error::Error for MissingConstantError {}

#[cfg(test)]
mod test {
    use crate::codec::{
        Constant, Decode, DecodingVec, Encode, MissingConstantError, Nul, Pattern, U8,
    };

    enum Arrow {}

    impl Pattern for Arrow {
        const PATTERN: &'static [u8] = b"->";
    }

    #[test]
    fn writes_its_pattern() {
        assert_eq!(Constant::<Nul>::encode_alloc(&()).unwrap().finish(), b"\0");
        assert_eq!(
            Constant::<Arrow>::encode_alloc(&()).unwrap().finish(),
            b"->"
        );
    }

    #[test]
    fn reads_only_its_pattern() {
        let mut decoder = DecodingVec::new(b"->\x07");
        Constant::<Arrow>::decode(&mut decoder).unwrap();
        assert_eq!(decoder.remaining_slice(), b"\x07");
    }

    #[test]
    fn fails_without_its_pattern() {
        for bytes in [&b""[..], b"-", b"<-", b"\0"] {
            let error = Constant::<Arrow>::decode(&mut DecodingVec::new(bytes)).unwrap_err();
            assert_eq!(error, MissingConstantError);
        }
    }

    #[test]
    fn separates_a_tuple() {
        type Codec = (U8, Constant<Arrow>, U8);
        let bytes = Codec::encode_alloc(&(1, (), 2)).unwrap().finish();
        assert_eq!(bytes, b"\x01->\x02");
        assert_eq!(
            Codec::decode(&mut DecodingVec::new(bytes)).unwrap(),
            (1, (), 2)
        );
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<Constant<Nul>>(&());
    }
}
