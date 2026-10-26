use std::convert::Infallible;

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Describes a byte slice `[u8]`. The bytes are stored as they are, without length or terminator,
///
/// /!\ This codec is final: it decodes everything till the end, so it can only be the last element of a tuple of codecs. To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
pub enum Bytes {}

impl Encode for Bytes {
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
    type Item = byteview::ByteView;
    type Error = Infallible;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        Ok(bytes.consume())
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Bytes, check_round_trip};

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Bytes, Vec<u8>>);
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<Bytes>(&[42; 32]);
    }
}
