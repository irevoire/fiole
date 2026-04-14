use std::convert::Infallible;

use fjall::Slice;

mod bytes;
pub use bytes::*;
mod lazy;
pub use lazy::*;
mod integer;
pub use integer::*;
mod str;
pub use str::*;
mod unit;
pub use unit::*;

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

/// Dummy codec if you don't know yet which codec will be used
pub enum Unspecified {}

/// Define how to encode an object to the bytes that will be stored in fjall.
pub trait Encode {
    /// The type to encode.
    type Item: ?Sized;
    /// The error returned if the type can't be encoded. Uses [`std::convert::Infallible`] if the encoding can't fail
    type Error;

    /// Encode the given item as bytes.
    fn encode(item: &Self::Item) -> Result<Slice, Self::Error>;
}

/// Define how to decode an object from the bytes stored in fjall to your type.
pub trait Decode {
    /// The type to decode.
    type Item;
    /// The error returned if the type can't be decoded. Uses [`std::convert::Infallible`] if the decoding can't fail
    type Error;

    /// Decode the given bytes as your item.
    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error>;
}

/// Convenient struct to ignore the decoding part and return the unit type instead.
/// Can be useful when working with [`crate::Guard`], but keep in mind that it still does a useless allocation.
/// Ideally, you should use the [`crate::Keyspace::contains_key`] or [`crate::Keyspace::size_of`] methods instead.
pub enum DecodeIgnore {}

impl Decode for DecodeIgnore {
    type Item = ();
    type Error = Infallible;

    fn decode(_: Slice) -> Result<Self::Item, Self::Error> {
        Ok(())
    }
}
