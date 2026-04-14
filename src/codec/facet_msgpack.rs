use std::marker::PhantomData;

use facet::Facet;
use fjall::Slice;

use crate::codec::{Decode, Encode};

/// Encode a struct as [msgpack](https://msgpack.org/) through the [`facet::Facet`] trait.
pub struct FacetMsgpack<T>(PhantomData<T>);

impl<T: Facet<'static>> Encode for FacetMsgpack<T> {
    type Item = T;
    type Error = facet_format::SerializeError<facet_msgpack::MsgPackSerializeError>;

    fn encode(item: &Self::Item) -> Result<Slice, Self::Error> {
        let buf = facet_msgpack::to_vec(item)?;
        Ok(buf.into())
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

    use crate::codec::{Decode, Encode, FacetMsgpack};

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

        let codec_bytes = FacetMsgpack::<Example>::encode(&value).unwrap();
        assert_eq!(codec_bytes, facet_bytes);

        let codec_deserialized = FacetMsgpack::<Example>::decode(codec_bytes).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
