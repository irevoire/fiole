use std::convert::Infallible;

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Describes a byte slice `[u8]` that is totally borrowed and doesn't depend on any memory alignment.
/// /!\ This codec is final: It decode everything till the end and can't be used with other codec if it's not being wrapped in a [`Sized`] codec.
pub enum Bytes {}

impl Encode<'_> for Bytes {
    type Item = [u8];
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        ret.extend(item);
        Ok(ret.make_fresh())
    }
}

impl Decode for Bytes {
    type Item = Vec<u8>;
    type Error = Infallible;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        Ok(bytes.consume())
    }
}
