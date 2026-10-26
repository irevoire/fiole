use std::marker::PhantomData;

use serde::{Serialize, de::DeserializeOwned};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Encode a struct as [`postcard`] through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
///
/// /!\ This codec is final: it decodes everything till the end, so it can only be the last element of a tuple of codecs. To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
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

    use crate::codec::{Decode, Encode, SerdePostcard};

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

        let codec_bytes = SerdePostcard::<Example>::encode_alloc(&value)
            .unwrap()
            .into_fjall_slice();
        assert_eq!(&codec_bytes, &facet_bytes);

        let codec_deserialized = SerdePostcard::<Example>::decode(&mut codec_bytes.into()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
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

        arbtest::arbtest(check_round_trip::<SerdePostcard<Example>, Example>);
        arbtest::arbtest(check_round_trip::<SerdePostcard<String>, String>);
        arbtest::arbtest(check_round_trip::<SerdePostcard<i64>, i64>);
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<SerdePostcard<u32>>(&42);
        crate::test_alloc::assert_decodes_without_allocating::<SerdePostcard<bool>>(&true);
    }
}
