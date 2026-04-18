use std::marker::PhantomData;

use fjall::Slice;
use serde::{de::DeserializeOwned, Serialize};

use crate::codec::{Decode, Encode, EncodingVec, Fresh};

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

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        postcard::from_bytes(&bytes)
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

        let codec_bytes = SerdePostcard::<Example>::encode(EncodingVec::new(), &value).unwrap();
        assert_eq!(codec_bytes.as_slice(), facet_bytes);

        let codec_deserialized =
            SerdePostcard::<Example>::decode(codec_bytes.into_fjall_slice()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
