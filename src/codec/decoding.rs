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

impl<T> Decode for &T
where
    T: Decode,
{
    type Item = T::Item;
    type Error = T::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        T::decode(bytes)
    }
}

pub struct DecodingVec<'a> {
    cursor: Cursor<Cow<'a, [u8]>>,
}

impl From<Slice> for DecodingVec<'static> {
    fn from(value: Slice) -> Self {
        Self::new(value.to_vec())
    }
}

impl<'a> From<&'a Slice> for DecodingVec<'a> {
    fn from(value: &'a Slice) -> Self {
        Self {
            cursor: Cursor::new(Cow::Borrowed(value)),
        }
    }
}

impl<'a> DecodingVec<'a> {
    pub fn new(vec: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(vec.into()),
        }
    }

    /// Reads `bytes` without copying them.
    pub(crate) fn borrowed(bytes: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(Cow::Borrowed(bytes)),
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
        self.remaining() == 0
    }

    /// Moves the cursor to the end without reading anything.
    pub fn skip_to_end(&mut self) {
        let len = self.cursor.get_ref().len();
        self.cursor.set_position(len as u64);
    }

    /// The bytes left to read, without moving the cursor.
    pub fn remaining_slice(&self) -> &[u8] {
        let inner = self.cursor.get_ref();
        let pos = (self.cursor.position() as usize).min(inner.len());
        &inner[pos..]
    }

    /// Number of bytes left to read.
    pub fn remaining(&self) -> usize {
        self.cursor
            .get_ref()
            .len()
            .saturating_sub(self.cursor.position() as usize)
    }

    /// Useful when your codec is going to consume everything till the end
    /// and needs to own the bytes.
    ///
    /// When the bytes are borrowed (e.g. straight from fjall), this copies
    /// them. If your codec only needs to read them, use [`DecodingVec::remaining_slice`]
    /// followed by [`DecodingVec::skip_to_end`] instead.
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

#[cfg(test)]
impl DecodingVec<'_> {
    /// Whether the bytes are borrowed rather than copied.
    pub(crate) fn is_borrowed(&self) -> bool {
        matches!(self.cursor.get_ref(), Cow::Borrowed(_))
    }
}

impl Read for DecodingVec<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}
