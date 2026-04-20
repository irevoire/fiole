use std::{
    borrow::Cow,
    io::{Cursor, Read},
};

use fjall::Slice;

/// Define how to decode an object from the bytes stored in fjall to your type.
pub trait Decode {
    /// The type to decode.
    type Item;
    /// The error returned if the type can't be decoded. Uses [`std::convert::Infallible`] if the decoding can't fail
    type Error;

    /// Decode the given bytes as your item. You must not read more bytes than required otherwise you might be eating the bytes of another codec.
    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error>;
}

pub struct DecodingVec<'a> {
    cursor: Cursor<Cow<'a, [u8]>>,
}

impl From<Slice> for DecodingVec<'static> {
    fn from(value: Slice) -> Self {
        Self::new(value.to_vec())
    }
}

impl<'a> DecodingVec<'a> {
    pub fn new(vec: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(vec.into()),
        }
    }

    /// Equivalent to [`std::io::Read::take`] except it return a `DecodingVec` which can be used within another codec.
    pub fn take_next<'b>(&'b mut self, size: usize) -> DecodingVec<'b> {
        let current_pos = self.cursor.position() as usize;
        let inner = self.cursor.get_ref();
        let take_until = inner.len().min(current_pos.saturating_add(size));
        self.cursor.set_position(take_until as u64);

        // Rust don't understand we're not touching the inner vec so we have to take the ref again.
        let inner = self.cursor.get_ref();
        let data = &inner[current_pos..take_until];
        DecodingVec {
            cursor: Cursor::new(Cow::Borrowed(data)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cursor.get_ref().len() == self.cursor.position() as usize
    }

    /// Useful when your codec is going to consume everything till the end.
    pub fn consume(&mut self) -> Vec<u8> {
        let cursor = std::mem::take(&mut self.cursor);
        let pos = cursor.position() as usize;
        match cursor.into_inner() {
            Cow::Borrowed(slice) => slice[pos..].to_vec(),
            Cow::Owned(mut vec) => {
                vec.drain(..pos);
                vec
            }
        }
    }
}

impl Read for DecodingVec<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}
