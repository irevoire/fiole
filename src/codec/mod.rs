//! Types that can be used to serialize and deserialize keys or values inside [`crate::Keyspace`]

use std::{convert::Infallible, fmt, io::Write, marker::PhantomData, ops::Deref};

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

pub enum Fresh {}
pub enum Dirty {}

pub struct EncodingVec<T> {
    vec: Vec<u8>,
    start_at: usize,
    marker: PhantomData<T>,
}

impl<T> fmt::Debug for EncodingVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_slice())
    }
}

impl<T> Deref for EncodingVec<T> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.vec[self.start_at..]
    }
}

impl<T> AsRef<[u8]> for EncodingVec<T> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<T> EncodingVec<T> {
    pub fn finish(self) -> Vec<u8> {
        self.vec
    }

    pub fn as_slice(&self) -> &[u8] {
        self.deref()
    }

    pub fn len(&self) -> usize {
        self.vec.len() - self.start_at
    }

    pub fn is_empty(self) -> bool {
        self.vec.len() == self.start_at
    }

    pub fn into_fjall_slice(self) -> Slice {
        Slice::from(self.vec)
    }
}

impl EncodingVec<Fresh> {
    pub fn edit(self) -> EncodingVec<Dirty> {
        EncodingVec {
            vec: self.vec,
            start_at: self.start_at,
            marker: PhantomData,
        }
    }

    pub(self) fn new() -> Self {
        Self {
            vec: Vec::new(),
            start_at: 0,
            marker: PhantomData,
        }
    }
}

impl EncodingVec<Dirty> {
    pub fn push(&mut self, byte: u8) {
        self.vec.push(byte);
    }

    pub fn append(&mut self, mut other: Vec<u8>) {
        self.vec.append(&mut other)
    }

    pub fn insert(&mut self, index: usize, element: u8) {
        self.vec.insert(index + self.start_at, element)
    }

    pub fn remove(&mut self, index: usize) -> u8 {
        self.vec.remove(index + self.start_at)
    }

    pub fn reserve(&mut self, additional: usize) {
        self.vec.reserve(additional)
    }

    pub fn clear(&mut self) {
        self.vec.truncate(self.start_at)
    }

    pub fn resize(&mut self, new_len: usize, value: u8) {
        let len = new_len.saturating_sub(self.vec.len());
        self.vec.resize(self.start_at + len, value)
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.vec.len() >= self.start_at {
            self.pop()
        } else {
            None
        }
    }

    pub fn make_fresh(self) -> EncodingVec<Fresh> {
        EncodingVec {
            start_at: self.vec.len(),
            vec: self.vec,
            marker: PhantomData,
        }
    }
}

impl Write for EncodingVec<Dirty> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.vec.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.vec.flush()
    }
}

impl Extend<u8> for EncodingVec<Dirty> {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        self.vec.extend(iter)
    }
}

impl<'a> Extend<&'a u8> for EncodingVec<Dirty> {
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        self.vec.extend(iter)
    }
}

/// Define how to encode an object to the bytes that will be stored in fjall.
pub trait Encode {
    /// The type to encode.
    type Item: ?Sized;
    /// The error returned if the type can't be encoded. Uses [`std::convert::Infallible`] if the encoding can't fail
    type Error;

    /// Encode the given item into the specified `EncodingVec`.
    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error>;

    /// Encode the given item as bytes in an allocated `EncodingVec`
    fn encode_alloc(item: &Self::Item) -> Result<EncodingVec<Fresh>, Self::Error> {
        Self::encode(EncodingVec::new(), item)
    }
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
