use std::{convert::Infallible, string::FromUtf8Error};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Describe a raw string without len or termination byte.
///
/// /!\ This codec is final: it decodes everything till the end, so it can only be the last element of a [`ComposeCodec`](crate::codec::ComposeCodec). To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
pub enum Str {}

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

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        String::from_utf8(bytes.consume().to_vec())
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Str, check_round_trip};

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Str, String>);
    }
}
