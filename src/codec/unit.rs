use std::{convert::Infallible, fmt};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};
use std::error::Error as StdError;

/// Describe the unit type. Fail to decode if the bytes contains anything. See [`super::DecodeIgnore`] for that purpose.
///
/// /!\ This codec is final: it verifies that nothing remains after it, so it can only be the last element of a [`ComposeCodec`](crate::codec::ComposeCodec). To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
///
/// ```compile_fail,E0599
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, U8, Unit};
///
/// ComposeCodec::<(Unit, U8)>::decode(&mut DecodingVec::new(vec![1])).unwrap();
/// ```
pub enum Unit {}

impl Encode for Unit {
    type Item = ();
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        _item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        Ok(into)
    }
}

impl Decode for Unit {
    type Item = ();
    type Error = NonEmptyError;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        if bytes.is_empty() {
            Ok(())
        } else {
            Err(NonEmptyError)
        }
    }
}

/// The slice of bytes is non-empty and therefore is not a unit `()` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NonEmptyError;

impl fmt::Display for NonEmptyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("the slice of bytes is non-empty and therefore is not a unit `()` type")
    }
}

impl StdError for NonEmptyError {}

#[cfg(test)]
mod test {
    use crate::codec::{
        check_round_trip, ComposeCodec, Decode, DecodingVec, Encode, SizedCodec, Terminated, Unit,
        U8,
    };

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Unit, ()>);
    }

    #[test]
    fn last_element_of_a_compose() {
        arbtest::arbtest(check_round_trip::<ComposeCodec<(U8, Unit)>, (u8, ())>);

        // It still refuses any byte left after it
        let mut decoder = DecodingVec::new(vec![1, 2]);
        assert!(ComposeCodec::<(U8, Unit)>::decode(&mut decoder).is_err());
    }

    #[test]
    fn wrapped_in_the_middle_of_a_compose() {
        type WithSize = ComposeCodec<(SizedCodec<Unit>, U8)>;
        let bytes = WithSize::encode_alloc(&((), 7)).unwrap().finish();
        assert_eq!(bytes, [0, 0, 0, 0, 7]);
        assert_eq!(
            WithSize::decode(&mut DecodingVec::new(bytes)).unwrap(),
            ((), 7)
        );

        type Term = ComposeCodec<(Terminated<Unit>, U8)>;
        let bytes = Term::encode_alloc(&((), 7)).unwrap().finish();
        assert_eq!(bytes, [0, 7]);
        assert_eq!(Term::decode(&mut DecodingVec::new(bytes)).unwrap(), ((), 7));
    }
}
