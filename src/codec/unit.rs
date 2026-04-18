use std::{convert::Infallible, fmt};

use fjall::Slice;

use crate::codec::{Decode, Encode, EncodingVec, Fresh};
use std::error::Error as StdError;

/// Describe the unit type. Fail to decode if the bytes contains anything. See [`super::DecodeIgnore`] for that purpose.
pub struct Unit {}

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

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
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
