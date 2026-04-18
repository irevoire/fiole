use std::marker::PhantomData;

use facet::Facet;
use fjall::Slice;

use crate::codec::{Decode, Encode, EncodingVec, Fresh};

/// Encode a struct as [msgpack](https://msgpack.org/) through the [`facet::Facet`] trait.
pub struct FacetMsgpack<T>(PhantomData<T>);

impl<T: Facet<'static>> Encode for FacetMsgpack<T> {
    type Item = T;
    type Error = std::io::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        facet_msgpack::to_writer(&mut ret, item)?;
        Ok(ret.make_fresh())
    }
}

impl<T: Facet<'static>> Decode for FacetMsgpack<T> {
    type Item = T;
    type Error = facet_msgpack::DeserializeError;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        facet_msgpack::from_slice(&bytes)
    }
}

#[cfg(test)]
mod test {
    use facet::Facet;

    use crate::codec::{Decode, Encode, EncodingVec, FacetMsgpack};

    #[test]
    fn encode_and_decode() {
        #[derive(Facet, Debug, PartialEq)]
        struct Example {
            name: String,
            value: i32,
        }

        let value = Example {
            name: "pi".to_string(),
            value: 31415926,
        };

        let facet_bytes = facet_msgpack::to_vec(&value).unwrap();
        let facet_deserialized = facet_msgpack::from_slice(&facet_bytes).unwrap();

        let codec_bytes = FacetMsgpack::<Example>::encode(EncodingVec::new(), &value).unwrap();
        assert_eq!(codec_bytes.as_slice(), facet_bytes);

        let codec_deserialized =
            FacetMsgpack::<Example>::decode(codec_bytes.into_fjall_slice()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
