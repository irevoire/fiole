use std::marker::PhantomData;

use serde::{Serialize, de::DeserializeOwned};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Encode a struct as json through the [`serde::Serialize`] and [`serde::Deserialize`] traits.
/// /!\ Take care of the flattened struct and untyped enum. In some cases, they serialize correctly but fail to deserialize.
///
/// /!\ This codec is final: it decodes everything till the end, so it can only be the last element of a [`ComposeCodec`](crate::codec::ComposeCodec). To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
///
/// ```
/// use fiole::codec::{Decode, DecodingVec, Encode, SerdeJson};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, PartialEq, Serialize, Deserialize)]
/// struct Person {
///     name: String,
///     age: u8,
/// }
///
/// let alice = Person { name: "Alice".to_string(), age: 30 };
///
/// // Encode the struct as json
/// let bytes = SerdeJson::<Person>::encode_alloc(&alice).unwrap().finish();
/// assert_eq!(bytes, br#"{"name":"Alice","age":30}"#);
///
/// // And decode it back
/// let decoded = SerdeJson::<Person>::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!(decoded, alice);
/// ```
///
/// It can't be followed by another codec in a [`ComposeCodec`](crate::codec::ComposeCodec):
///
/// ```compile_fail,E0599
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, SerdeJson, U8};
///
/// ComposeCodec::<(SerdeJson<u32>, U8)>::decode(&mut DecodingVec::new(b"12".to_vec())).unwrap();
/// ```
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
        serde_json::from_slice(&bytes.consume())
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::codec::{ComposeCodec, Decode, DecodingVec, Encode, SerdeJson, SizedCodec, U8};

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

        let codec_bytes = SerdeJson::<Example>::encode_alloc(&value)
            .unwrap()
            .into_fjall_slice();
        assert_eq!(&codec_bytes, &facet_bytes);

        let codec_deserialized = SerdeJson::<Example>::decode(&mut codec_bytes.into()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }

    #[test]
    fn bare_values() {
        let buf = SerdeJson::<u32>::encode_alloc(&12).unwrap();
        let mut decoder = buf.into_decoding_vec();
        assert_eq!(SerdeJson::<u32>::decode(&mut decoder).unwrap(), 12);
        assert!(decoder.is_empty());

        let buf = SerdeJson::<bool>::encode_alloc(&true).unwrap();
        let mut decoder = buf.into_decoding_vec();
        assert!(SerdeJson::<bool>::decode(&mut decoder).unwrap());
        assert!(decoder.is_empty());
    }

    #[test]
    fn is_final() {
        // It consumes everything till the end
        let mut decoder = DecodingVec::new(b"[1,2] ".to_vec());
        assert_eq!(
            SerdeJson::<Vec<u8>>::decode(&mut decoder).unwrap(),
            vec![1, 2]
        );
        assert!(decoder.is_empty());

        // And refuses anything that is not part of the json value
        let mut decoder = DecodingVec::new(b"[1,2]x".to_vec());
        assert!(SerdeJson::<Vec<u8>>::decode(&mut decoder).is_err());
        let mut decoder = DecodingVec::new(b"12x".to_vec());
        assert!(SerdeJson::<u32>::decode(&mut decoder).is_err());
    }

    #[test]
    fn wrapped_in_sized_codec() {
        type Codec = ComposeCodec<(SizedCodec<SerdeJson<u32>>, U8)>;
        let buf = Codec::encode_alloc(&(12, 9)).unwrap();
        let mut decoder = buf.into_decoding_vec();
        assert_eq!(Codec::decode(&mut decoder).unwrap(), (12, 9));
        assert!(decoder.is_empty());
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

        arbtest::arbtest(check_round_trip::<SerdeJson<Example>, Example>);
        arbtest::arbtest(check_round_trip::<SerdeJson<String>, String>);
        arbtest::arbtest(check_round_trip::<SerdeJson<i64>, i64>);
    }
}
