use std::marker::PhantomData;

use serde::{
    de::{DeserializeOwned, Error},
    Serialize,
};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Encode a struct as json through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
/// /!\ Take care of the flattened struct and untyped enum. In some cases, they serialize correctly but fail to deserialize.
pub struct SerdeJson<T>(PhantomData<T>);

impl<T: Serialize> Encode for SerdeJson<T> {
    type Item = T;
    type Error = serde_json::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        serde_json::to_writer(&mut ret, item)?;
        Ok(ret.make_fresh())
    }
}

impl<T: DeserializeOwned> Decode for SerdeJson<T> {
    type Item = T;
    type Error = serde_json::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        serde_json::de::Deserializer::from_reader(bytes)
            .into_iter()
            .next()
            .ok_or_else(|| serde_json::Error::custom("Empty slice"))?
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::codec::{Decode, Encode, EncodingVec, SerdeJson};

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

        let facet_bytes = serde_json::to_vec(&value).unwrap();
        let facet_deserialized = serde_json::from_slice(&facet_bytes).unwrap();

        let codec_bytes = SerdeJson::<Example>::encode(EncodingVec::new(), &value).unwrap();
        assert_eq!(codec_bytes.as_slice(), facet_bytes);

        let codec_deserialized =
            SerdeJson::<Example>::decode(&mut codec_bytes.into_decoding_vec()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
