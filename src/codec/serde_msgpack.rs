use std::marker::PhantomData;

use serde::{Serialize, de::DeserializeOwned};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

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

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        rmp_serde::from_read(bytes)
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::codec::{Decode, Encode, SerdeMsgpack};

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

        let codec_bytes = SerdeMsgpack::<Example>::encode_alloc(&value)
            .unwrap()
            .into_fjall_slice();
        assert_eq!(&codec_bytes, &facet_bytes);

        let codec_deserialized = SerdeMsgpack::<Example>::decode(&mut codec_bytes.into()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }

    #[test]
    fn self_delimiting() {
        use std::collections::BTreeMap;

        use crate::codec::assert_self_delimiting;

        #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
        struct Example {
            name: String,
            value: i32,
        }

        let value = Example {
            name: "pi".to_string(),
            value: 31415926,
        };
        assert_self_delimiting::<SerdeMsgpack<Example>>(value.clone(), value);
        assert_self_delimiting::<SerdeMsgpack<u32>>(12, 12);
        assert_self_delimiting::<SerdeMsgpack<u64>>(u64::MAX, u64::MAX);
        assert_self_delimiting::<SerdeMsgpack<i8>>(-1, -1);
        assert_self_delimiting::<SerdeMsgpack<bool>>(true, true);
        assert_self_delimiting::<SerdeMsgpack<f64>>(1.5, 1.5);
        assert_self_delimiting::<SerdeMsgpack<String>>("hi".to_string(), "hi".to_string());
        assert_self_delimiting::<SerdeMsgpack<Vec<u8>>>(vec![1, 2], vec![1, 2]);
        assert_self_delimiting::<SerdeMsgpack<Option<u8>>>(None, None);
        let map = BTreeMap::from([("a".to_string(), 1)]);
        assert_self_delimiting::<SerdeMsgpack<BTreeMap<String, u8>>>(map.clone(), map);
    }
}
