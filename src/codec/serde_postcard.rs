use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Encode a struct as [`postcard`] through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
pub struct SerdePostcard<T>(PhantomData<T>);

impl<T: Serialize> Encode for SerdePostcard<T> {
    type Item = T;
    type Error = postcard::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        postcard::to_io(item, &mut ret)?;
        Ok(ret.make_fresh())
    }
}

impl<T: DeserializeOwned> Decode for SerdePostcard<T> {
    type Item = T;
    type Error = postcard::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        postcard::from_bytes(&bytes.consume())
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::codec::{Decode, Encode, EncodingVec, SerdePostcard};

    #[test]
    fn encode_and_decode() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Example {
            name: String,
            value: i32,
        }

        let value = Example {
            name: "pi".to_string(),
            value: 31415926,
        };

        let facet_bytes = postcard::to_allocvec(&value).unwrap();
        let facet_deserialized = postcard::from_bytes(&facet_bytes).unwrap();

        let codec_bytes = SerdePostcard::<Example>::encode_alloc(&value).unwrap();
        assert_eq!(codec_bytes.as_slice(), facet_bytes);

        let codec_deserialized =
            SerdePostcard::<Example>::decode(&mut codec_bytes.into_decoding_vec()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
