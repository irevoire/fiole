use std::{fmt, io::Write, marker::PhantomData, ops::Deref};

use fjall::Slice;

use crate::codec::DecodingVec;

/// Define how to encode an object to the bytes that will be stored in fjall.
pub trait Encode {
    /// The type to encode.
    type Item: ?std::marker::Sized;
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

pub enum Fresh {}
pub enum Dirty {}

pub struct EncodingVec<T> {
    pub(crate) vec: Vec<u8>,
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

    pub fn into_decoding_vec(self) -> DecodingVec<'static> {
        DecodingVec::new(self.vec)
    }

    pub fn absolute_pos(&self) -> usize {
        self.vec.len()
    }

    pub fn retrieve_space(&mut self, token: SpaceToken) -> &mut [u8] {
        &mut self.vec[token.pos..token.pos + token.size]
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

    pub(super) fn new() -> Self {
        Self {
            vec: Vec::new(),
            start_at: 0,
            marker: PhantomData,
        }
    }
}

pub struct SpaceToken {
    pos: usize,
    size: usize,
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

    pub fn save_space_for_later(&mut self, size: usize, fill_with: u8) -> SpaceToken {
        let pos = self.vec.len();
        self.vec.resize(pos + size, fill_with);
        SpaceToken { pos, size }
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
