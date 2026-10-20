use std::marker::PhantomData;

use serde::{Serialize, de::DeserializeOwned};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting};

/// Encode a struct as [`msgpack`] through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
pub struct SerdeMsgpack<T>(PhantomData<T>);

impl<T> SelfDelimiting for SerdeMsgpack<T> {}

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

        use arbitrary::Arbitrary;

        use crate::codec::{check_self_delimiting, check_self_delimiting_with};

        #[derive(Serialize, Deserialize, Arbitrary, Debug, PartialEq)]
        struct Example {
            name: String,
            value: i32,
            tags: Vec<Option<bool>>,
        }

        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<Example>, Example>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<u32>, u32>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<u64>, u64>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<i8>, i8>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<bool>, bool>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<String>, String>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<Vec<u8>>, Vec<u8>>);
        arbtest::arbtest(check_self_delimiting::<SerdeMsgpack<Option<u8>>, Option<u8>>);
        arbtest::arbtest(|u| {
            check_self_delimiting::<SerdeMsgpack<BTreeMap<String, u8>>, BTreeMap<String, u8>>(u)
        });
        // NaN is never equal to itself, so it can't be compared after a round-trip
        arbtest::arbtest(|u| {
            check_self_delimiting_with::<SerdeMsgpack<f64>, f64>(u, |u| {
                let f: f64 = u.arbitrary()?;
                Ok(if f.is_nan() { 0.0 } else { f })
            })
        });
    }

    #[test]
    fn round_trip() {
        use crate::codec::check_round_trip;

        #[derive(Serialize, Deserialize, arbitrary::Arbitrary, Debug, PartialEq)]
        struct Example {
            name: String,
            value: i32,
            big: u64,
            tags: Vec<Option<bool>>,
        }

        arbtest::arbtest(check_round_trip::<SerdeMsgpack<Example>, Example>);
        arbtest::arbtest(check_round_trip::<SerdeMsgpack<String>, String>);
        arbtest::arbtest(check_round_trip::<SerdeMsgpack<i64>, i64>);
    }
}
