//! Types that can be used to serialize and deserialize keys or values inside [`crate::Keyspace`]

use std::convert::Infallible;

mod encoding;
pub use encoding::*;
mod decoding;
pub use decoding::*;

mod bytes;
pub use bytes::*;
mod sized_codec;
pub use sized_codec::*;
mod compose_codec;
pub use compose_codec::*;
mod lazy;
pub use lazy::*;
mod integer;
pub use integer::*;
mod str;
pub use str::*;
mod unit;
pub use unit::*;
#[cfg(any(test, feature = "arbitrary"))]
mod property;
#[cfg(any(test, feature = "arbitrary"))]
pub use property::*;

#[cfg(feature = "facet_json")]
mod facet_json;
#[cfg(feature = "facet_json")]
pub use facet_json::*;
#[cfg(feature = "facet_postcard")]
mod facet_postcard;
#[cfg(feature = "facet_postcard")]
pub use facet_postcard::*;
#[cfg(feature = "facet_msgpack")]
mod facet_msgpack;
#[cfg(feature = "facet_msgpack")]
pub use facet_msgpack::*;
#[cfg(feature = "serde_json")]
mod serde_json;
#[cfg(feature = "serde_json")]
pub use serde_json::*;
#[cfg(feature = "serde_postcard")]
mod serde_postcard;
#[cfg(feature = "serde_postcard")]
pub use serde_postcard::*;
#[cfg(feature = "serde_msgpack")]
mod serde_msgpack;
#[cfg(feature = "serde_msgpack")]
pub use serde_msgpack::*;
#[cfg(feature = "roaring")]
mod roaring;
#[cfg(feature = "roaring")]
pub use roaring::*;
#[cfg(feature = "rkyv")]
mod rkyv;
#[cfg(feature = "rkyv")]
pub use rkyv::*;

/// Marker trait for the codecs that know where their own bytes end.
///
/// A self-delimiting codec decodes exactly the bytes it encoded and leaves the
/// cursor on the first byte that follows, so it can be followed by another
/// codec in a [`ComposeCodec`]. Every element of a [`ComposeCodec`] except the
/// last one must implement this trait.
///
/// Codecs that don't implement it are *final*: they decode everything till the
/// end. Wrap them in a [`SizedCodec`] to make them self-delimiting.
///
/// Only implement it on your own codec if its [`Decode::decode`] never reads
/// past the bytes written by its [`Encode::encode`], whatever bytes follow.
pub trait SelfDelimiting {}

impl<T: SelfDelimiting + ?Sized> SelfDelimiting for &T {}

/// Dummy codec if you don't know yet which codec will be used
pub enum Unspecified {}

/// Convenient struct to ignore the decoding part and return the unit type instead.
/// Can be useful when working with [`crate::Guard`], but keep in mind that the bytes are still copied out of fjall before being ignored.
/// Ideally, you should use the [`crate::Keyspace::contains_key`] or [`crate::Keyspace::size_of`] methods instead.
///
/// /!\ This codec is final: It ignores everything till the end and can't be used with other codec if it's not being wrapped in a [`SizedCodec`] codec.
/// To ignore a field in the middle of a [`ComposeCodec`], use `SizedCodec<DecodeIgnore>`.
///
/// ```compile_fail,E0599
/// use fiole::codec::{ComposeCodec, Decode, DecodeIgnore, DecodingVec, U8};
///
/// ComposeCodec::<(DecodeIgnore, U8)>::decode(&mut DecodingVec::new(vec![1, 2])).unwrap();
/// ```
pub enum DecodeIgnore {}

impl Decode for DecodeIgnore {
    type Item = ();
    type Error = Infallible;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        bytes.skip_to_end();
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{
        ComposeCodec, Decode, DecodeIgnore, DecodingVec, Encode, EncodingVec, SizedCodec, Str, U8,
    };

    #[test]
    fn encoding_vec() {
        // convert from dirty to fresh
        let vec = EncodingVec::new().edit();
        let vec = vec.make_fresh();

        let mut vec = vec.edit();
        vec.clear();
        vec.push(42);
        vec.push(36);
        let mut vec = vec.make_fresh().edit();
        vec.clear();
        let bytes = vec.into_fjall_slice();
        assert_eq!(&bytes, &[42, 36]);
    }

    #[test]
    fn decode_ignore_consumes_everything() {
        let mut decoder = DecodingVec::new(vec![1, 2, 3]);
        DecodeIgnore::decode(&mut decoder).unwrap();
        assert!(decoder.is_empty());

        let mut decoder = DecodingVec::new(vec![1, 2, 3]);
        let decoded = ComposeCodec::<(U8, DecodeIgnore)>::decode(&mut decoder).unwrap();
        assert_eq!(decoded, (1, ()));
        assert!(decoder.is_empty());
    }

    #[test]
    fn decode_ignore_in_the_middle_of_a_compose() {
        type Codec<'a> = ComposeCodec<(&'a SizedCodec<Str>, U8)>;
        let buf = Codec::encode_alloc(&("hello", 9)).unwrap();

        type Ignore = ComposeCodec<(SizedCodec<DecodeIgnore>, U8)>;
        let mut decoder = buf.into_decoding_vec();
        assert_eq!(Ignore::decode(&mut decoder).unwrap(), ((), 9));
        assert!(decoder.is_empty());
    }
}
