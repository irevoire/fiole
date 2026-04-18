use std::{convert::Infallible, string::FromUtf8Error};

use fjall::Slice;

use crate::codec::{Decode, Encode, EncodingVec, Fresh};

/// Describe a raw string without len or termination byte.
pub struct Str {}

impl Encode for Str {
    type Item = str;
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        ret.extend(item.as_bytes());
        Ok(ret.make_fresh())
    }
}

impl Decode for Str {
    type Item = String;
    type Error = FromUtf8Error;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        String::from_utf8(bytes.to_vec())
    }
}
