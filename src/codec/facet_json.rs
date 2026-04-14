use std::marker::PhantomData;

use facet::Facet;
use facet_format::SerializeError;
use facet_json::{DeserializeError, JsonSerializeError};
use fjall::Slice;

use crate::codec::{Decode, Encode};

/// Encode a struct as json through the [`facet::Facet`] trait.
pub struct FacetJson<T>(PhantomData<T>);

impl<T: Facet<'static>> Encode for FacetJson<T> {
    type Item = T;
    type Error = SerializeError<JsonSerializeError>;

    fn encode(item: &Self::Item) -> Result<Slice, Self::Error> {
        let buf = facet_json::to_vec(item)?;
        Ok(buf.into())
    }
}

impl<T: Facet<'static>> Decode for FacetJson<T> {
    type Item = T;
    type Error = DeserializeError;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        facet_json::from_slice(&bytes)
    }
}

#[cfg(test)]
mod test {
    use facet::Facet;

    use crate::codec::{Decode, Encode, FacetJson};

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

        let facet_bytes = facet_json::to_vec(&value).unwrap();
        let facet_deserialized = facet_json::from_slice(&facet_bytes).unwrap();

        let codec_bytes = FacetJson::<Example>::encode(&value).unwrap();
        assert_eq!(codec_bytes, facet_bytes);

        let codec_deserialized = FacetJson::<Example>::decode(codec_bytes).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
