use std::marker::PhantomData;

use rkyv::{
    api::high::{HighSerializer, HighValidator},
    bytecheck::CheckBytes,
    de::Pool,
    rancor,
    ser::allocator::ArenaHandle,
    util::AlignedVec,
    Archive, Deserialize, Serialize,
};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Encode a struct with the rkyv format. Caution, this is not zerocopy.
/// - `T` is the type you want to encode.
/// - `E` is the error type that'll be returned by rkyv. It must implements the [`rkyv::rancor::Source`] trait.
/// /!\ This codec is final: It decode everything till the end and can't be used with other codec if it's not being wrapped in a [`Sized`] codec.
pub struct Rkyv<T, E>(PhantomData<(T, E)>);

impl<T, E> Encode for Rkyv<T, E>
where
    T: Archive + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, E>>,
    E: rancor::Source,
{
    type Item = T;
    type Error = E;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        // TODO: Get rid of this useless alloc
        let bytes = rkyv::to_bytes(item)?;
        ret.extend(bytes.into_iter().copied());
        Ok(ret.make_fresh())
    }
}

impl<T, E> Decode for Rkyv<T, E>
where
    T: Archive,
    T::Archived:
        for<'a> CheckBytes<HighValidator<'a, E>> + Deserialize<T, rancor::Strategy<Pool, E>>,
    E: rancor::Source,
{
    type Item = T;
    type Error = E;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        rkyv::from_bytes(&bytes.consume())
    }
}

#[cfg(test)]
mod test {
    use rkyv::{Archive, Deserialize, Serialize};

    use crate::codec::{Decode, Encode, EncodingVec, Rkyv};

    #[test]
    fn encode_and_decode() {
        #[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
        struct Example {
            name: String,
            value: i32,
        }

        let value = Example {
            name: "pi".to_string(),
            value: 31415926,
        };

        let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Panic>(&value).unwrap();
        let rkyv_deserialized =
            rkyv::from_bytes::<Example, rkyv::rancor::Panic>(&rkyv_bytes).unwrap();

        let codec_bytes = Rkyv::<Example, rkyv::rancor::Panic>::encode_alloc(&value).unwrap();
        assert_eq!(rkyv_bytes.as_slice(), codec_bytes.as_slice());

        let codec_deserialized =
            Rkyv::<Example, rkyv::rancor::Panic>::decode(&mut codec_bytes.into_decoding_vec())
                .unwrap();
        assert_eq!(codec_deserialized, rkyv_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
