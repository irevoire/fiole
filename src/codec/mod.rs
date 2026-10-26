//! Types that can be used to serialize and deserialize keys or values inside [`crate::Keyspace`]
//!
//! # Keys and ordering
//!
//! A codec used for values only has to round-trip. A codec used for keys also
//! decides how they're sorted, since fjall sorts the keys by their encoded bytes:
//! that's the order used by [`crate::Keyspace::range`], [`crate::Keyspace::iter`]
//! or [`crate::Keyspace::first_key_value`].
//!
//! - [`Str`] and [`Bytes`] and [`Text`] sort lexicographically.
//! - Numbers sort numerically with the `Ordered*` codecs, like [`OrderedU32`],
//!   [`OrderedI32`] or [`OrderedF64`]. Unsigned numbers also sort numerically
//!   with the `VarU*` codecs, like [`VarU64`], that write them on as few bytes as possible.
//! - [`Terminated`] keeps the order of the codec it wraps, while [`SizedCodec`] doesn't.
//!
//! # Composing codecs
//!
//! A [`ComposeCodec`] encodes a tuple by writing the encoding of each element
//! one after the other. To decode it back, each codec must know where its own
//! bytes end:
//!
//! - *Self-delimiting* codecs know where they end and implement [`SelfDelimiting`]:
//!   the integers, the `Ordered*` and `VarU*` codecs, [`SizedCodec`], [`Terminated`], [`Fixed`], [`Constant`], a
//!   [`ComposeCodec`] of self-delimiting codecs, `SerdeMsgpack` and the roaring codecs.
//! - *Final* codecs decode everything till the end: [`Str`], [`Bytes`], [`Text`], [`Unit`],
//!   [`Optional`], [`LazyDecode`], [`DecodeIgnore`], and the other serde and facet codecs.
//!
//! Every element of a [`ComposeCodec`] except the last one must be
//! self-delimiting: using a final codec anywhere else is a compile error. To
//! put a final codec anywhere, wrap it:
//!
//! - [`SizedCodec`] writes the length of the inner codec as a [`VarU64`] before it,
//!   a single byte for up to 127 bytes.
//!   It works with every codec, but doesn't keep the order of the keys.
//! - [`Terminated`] writes a pattern after the inner codec, a single `0x00` byte by
//!   default. It keeps the order of the keys, but can't encode a value whose
//!   encoding contains the pattern, like a string containing `\0`.
//! - [`Fixed`] writes nothing more, and reads exactly `N` bytes. It keeps the
//!   order of the keys, but only encodes the values written on exactly `N` bytes.
//!
//! ```
//! use fiole::codec::{Bytes, ComposeCodec, Decode, DecodingVec, Encode, OrderedI32, Str, Terminated};
//!
//! // A name, a signed number, then raw bytes: only the last element can be final.
//! type Key<'a> = ComposeCodec<(&'a Terminated<Str>, OrderedI32, &'a Bytes)>;
//!
//! let bytes = Key::encode_alloc(&("kefir", -3, &[1, 2][..])).unwrap().finish();
//! let (name, n, raw) = Key::decode(&mut DecodingVec::new(bytes)).unwrap();
//! assert_eq!((&*name, n, &*raw), ("kefir", -3, &[1, 2][..]));
//! ```
//!
//! A [`ComposeCodec`] of two elements or more is [`Split`] between it's head
//! (first element) and it's tail (the rest).
//! [`crate::Keyspace::prefixed`] let's you specify the head and returns
//! a keyspace now expecting the tail to be specified as its key.

use std::convert::Infallible;

mod encoding;
pub use encoding::*;
mod decoding;
pub use decoding::*;

mod bytes;
pub use bytes::*;
mod sized_codec;
pub use sized_codec::*;
mod split;
pub use split::*;
mod compose_codec;
pub use compose_codec::*;
mod constant;
pub use constant::*;
mod fixed;
pub use fixed::*;
mod lazy;
pub use lazy::*;
mod integer;
pub use integer::*;
mod optional;
pub use optional::*;
mod ordered;
pub use ordered::*;
mod pattern;
pub use pattern::*;
mod str;
pub use str::*;
mod terminated;
pub use terminated::*;
mod text;
pub use text::*;
mod unit;
pub use unit::*;
mod varint;
pub use varint::*;
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

/// Marker trait for the codecs that know where their own bytes end.
///
/// A self-delimiting codec decodes exactly the bytes it encoded and leaves the
/// cursor on the first byte that follows, so it can be followed by another
/// codec in a [`ComposeCodec`]. Every element of a [`ComposeCodec`] except the
/// last one must implement this trait.
///
/// Codecs that don't implement it are *final*: they decode everything till the
/// end. Wrap them in a [`SizedCodec`] or a [`Terminated`] to make them self-delimiting.
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
/// /!\ This codec is final: it ignores everything till the end, so it can only be the last element of a [`ComposeCodec`].
/// To ignore a field in the middle of a [`ComposeCodec`], wrap it like the field it ignores, e.g. `SizedCodec<DecodeIgnore>` or `Terminated<DecodeIgnore>`.
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
