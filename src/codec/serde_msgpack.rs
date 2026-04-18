use std::marker::PhantomData;

use fjall::Slice;
use serde::{de::DeserializeOwned, Serialize};

use crate::codec::{Decode, Encode, EncodingVec, Fresh};

/// Encode a struct as [`msgpack`] through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
pub struct SerdeMsgpack<T>(PhantomData<T>);

impl<T: Serialize> Encode for SerdeMsgpack<T> {
    type Item = T;
    type Error = rmp_serde::encode::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        rmp_serde::encode::write(&mut ret, item)?;
        Ok(ret.make_fresh())
    }
}

impl<T: DeserializeOwned> Decode for SerdeMsgpack<T> {
    type Item = T;
    type Error = rmp_serde::decode::Error;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        rmp_serde::from_slice(&bytes)
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::codec::{Decode, Encode, EncodingVec, SerdeMsgpack};

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

        let facet_bytes = rmp_serde::to_vec(&value).unwrap();
        let facet_deserialized = rmp_serde::from_slice(&facet_bytes).unwrap();

        let codec_bytes = SerdeMsgpack::<Example>::encode(EncodingVec::new(), &value).unwrap();
        assert_eq!(codec_bytes.as_slice(), facet_bytes);

        let codec_deserialized =
            SerdeMsgpack::<Example>::decode(codec_bytes.into_fjall_slice()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
