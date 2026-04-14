use std::marker::PhantomData;

use fjall::Slice;
use serde::{de::DeserializeOwned, Serialize};

use crate::codec::{Decode, Encode};

/// Encode a struct as json through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
/// /!\ Take care of the flattened struct and untyped enum. In some cases, they serialize correctly but fail to deserialize.
pub struct SerdeJson<T>(PhantomData<T>);

impl<T: Serialize> Encode for SerdeJson<T> {
    type Item = T;
    type Error = serde_json::Error;

    fn encode(item: &Self::Item) -> Result<Slice, Self::Error> {
        let buf = serde_json::to_vec(item)?;
        Ok(buf.into())
    }
}

impl<T: DeserializeOwned> Decode for SerdeJson<T> {
    type Item = T;
    type Error = serde_json::Error;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        serde_json::from_slice(&bytes)
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::codec::{Decode, Encode, SerdeJson};

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

        let codec_bytes = SerdeJson::<Example>::encode(&value).unwrap();
        assert_eq!(codec_bytes, facet_bytes);

        let codec_deserialized = SerdeJson::<Example>::decode(codec_bytes).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
