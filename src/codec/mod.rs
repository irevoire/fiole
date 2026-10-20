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
/// Can be useful when working with [`crate::Guard`], but keep in mind that it still does a useless allocation.
/// Ideally, you should use the [`crate::Keyspace::contains_key`] or [`crate::Keyspace::size_of`] methods instead.
pub enum DecodeIgnore {}

impl Decode for DecodeIgnore {
    type Item = ();
    type Error = Infallible;

    fn decode(_: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::codec::EncodingVec;

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
}
