use std::marker::PhantomData;

use fjall::Slice;
use rkyv::{
    api::high::{HighSerializer, HighValidator},
    bytecheck::CheckBytes,
    de::Pool,
    rancor,
    ser::allocator::ArenaHandle,
    util::AlignedVec,
    Archive, Deserialize, Serialize,
};

use crate::codec::{Decode, Encode};

/// Encode a struct with the rkyv format. Caution, this is not zerocopy.
/// - `T` is the type you want to encode.
/// - `E` is the error type that'll be returned by rkyv. It must implements the [`rkyv::rancor::Source`] trait.
pub struct Rkyv<T, E>(PhantomData<(T, E)>);

impl<T, E> Encode for Rkyv<T, E>
where
    T: Archive + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, E>>,
    E: rancor::Source,
{
    type Item = T;
    type Error = E;

    fn encode(item: &Self::Item) -> Result<Slice, Self::Error> {
        Ok(rkyv::to_bytes(item)?.into_vec().into())
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

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        rkyv::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod test {
    use rkyv::{Archive, Deserialize, Serialize};

    use crate::codec::{Decode, Encode, Rkyv};

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

        let codec_bytes = Rkyv::<Example, rkyv::rancor::Panic>::encode(&value).unwrap();
        assert_eq!(rkyv_bytes.as_slice(), codec_bytes);

        let codec_deserialized = Rkyv::<Example, rkyv::rancor::Panic>::decode(codec_bytes).unwrap();
        assert_eq!(codec_deserialized, rkyv_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
