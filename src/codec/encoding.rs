use std::{fmt, io::Write, marker::PhantomData, ops::Deref};

use fjall::Slice;

use crate::codec::DecodingVec;

/// Define how to encode an object to the bytes that will be stored in fjall.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a codec: it can't encode anything",
    label = "not a codec",
    note = "the codecs are types like `Str`, `OrderedU32` or `SerdeJson<T>` in `fiole::codec`, which say how an item is written",
    note = "the `codec` of a field of a struct deriving `Fiole` must be one of them"
)]
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

impl<'a, T> Encode for &'a T
where
    T: Encode + ?Sized,
{
    type Item = &'a T::Item;
    type Error = T::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        T::encode(into, item)
    }
}

/// State of an [`EncodingVec`] that's ready to be given to the next codec.
///
/// Only the methods that read the buffer are available. Call
/// [`EncodingVec::edit`] to write into it.
pub enum Fresh {}

/// State of an [`EncodingVec`] that's being written by a codec.
///
/// Call [`EncodingVec::make_fresh`] once the codec is done writing, before
/// returning it from [`Encode::encode`].
pub enum Dirty {}

/// The buffer every codec writes its bytes into.
///
/// The same buffer is passed from one codec to the next one, for example
/// between the elements of a tuple of codecs.
/// Each codec only sees its own bytes: the ones written by the previous
/// codecs can't be read or modified.
///
/// ```
/// use fiole::codec::{Encode, Str};
///
/// let vec = Str::encode_alloc("hello").unwrap();
/// assert_eq!(vec.len(), 0);
/// assert_eq!(format!("{vec:?}"), "[]");
/// assert_eq!(vec.finish(), b"hello");
/// ```
///
/// A codec writes its bytes like this:
///
/// ```
/// use std::convert::Infallible;
/// use fiole::codec::{Encode, EncodingVec, Fresh};
///
/// /// Encodes a bool on a single byte.
/// enum Bool {}
///
/// impl Encode for Bool {
///     type Item = bool;
///     type Error = Infallible;
///
///     fn encode(into: EncodingVec<Fresh>, item: &bool) -> Result<EncodingVec<Fresh>, Infallible> {
///         let mut ret = into.edit();
///         ret.push(*item as u8);
///         Ok(ret.make_fresh())
///     }
/// }
///
/// assert_eq!(Bool::encode_alloc(&true).unwrap().finish(), [1]);
/// ```
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
    /// Returns the whole buffer, including the bytes of the previous codecs.
    pub fn finish(self) -> Vec<u8> {
        self.vec
    }

    /// The bytes written by the current codec.
    pub fn as_slice(&self) -> &[u8] {
        self.deref()
    }

    /// Number of bytes written by the current codec.
    pub fn len(&self) -> usize {
        self.vec.len() - self.start_at
    }

    /// Whether the current codec wrote nothing yet.
    pub fn is_empty(&self) -> bool {
        self.vec.len() == self.start_at
    }

    /// Converts the whole buffer into a fjall [`Slice`].
    pub fn into_fjall_slice(self) -> Slice {
        Slice::from(self.vec)
    }

    /// Converts the whole buffer into a [`DecodingVec`] to decode it.
    pub fn into_decoding_vec(self) -> DecodingVec {
        DecodingVec::new(self.vec)
    }

    /// Length of the whole buffer, including the bytes of the previous codecs.
    ///
    /// Useful to measure how many bytes a sub-codec wrote, by comparing the
    /// position before and after calling it.
    pub fn absolute_pos(&self) -> usize {
        self.vec.len()
    }

    /// Gives back the space reserved by [`EncodingVec::save_space_for_later`], to fill it.
    pub fn retrieve_space(&mut self, token: SpaceToken) -> &mut [u8] {
        &mut self.vec[token.pos..token.pos + token.size]
    }
}

impl EncodingVec<Fresh> {
    /// Starts writing the bytes of the current codec.
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

    /// Inserts `count` copies of `byte` at `index` in the whole buffer, moving
    /// the bytes after it only once.
    ///
    /// Made for a codec that wraps another one and needs to write something
    /// before the bytes its inner codec wrote, like
    /// [`PadLeftKeep`](crate::codec::PadLeftKeep). The inserted bytes count as
    /// already written: the next codec starts after them.
    ///
    /// # Panics
    ///
    /// Panics if `index` is after the end of the buffer.
    pub(crate) fn insert_repeated(&mut self, index: usize, count: usize, byte: u8) {
        self.vec
            .splice(index..index, std::iter::repeat_n(byte, count));
        self.start_at = self.vec.len();
    }

    /// Same as [`EncodingVec::insert_repeated`], with the bytes of `slice`.
    pub(crate) fn insert_slice(&mut self, index: usize, slice: &[u8]) {
        self.vec.splice(index..index, slice.iter().copied());
        self.start_at = self.vec.len();
    }

    /// Starts a buffer with `prefix` already written, and room for
    /// `headroom` more bytes. The codecs only see what comes after the
    /// prefix, so they can't read or modify it.
    pub(crate) fn with_prefix(prefix: &[u8], headroom: usize) -> Self {
        let mut vec = Vec::with_capacity(prefix.len() + headroom);
        vec.extend_from_slice(prefix);
        Self {
            vec,
            start_at: prefix.len(),
            marker: PhantomData,
        }
    }
}

/// Space reserved by [`EncodingVec::save_space_for_later`], to be filled with
/// [`EncodingVec::retrieve_space`] once its content is known.
///
/// [`SizedCodec`](crate::codec::SizedCodec) uses it to write the length of the
/// inner codec before its bytes.
pub struct SpaceToken {
    pos: usize,
    size: usize,
}

impl EncodingVec<Dirty> {
    /// Appends a byte at the end.
    pub fn push(&mut self, byte: u8) {
        self.vec.push(byte);
    }

    /// Appends all the bytes of `other` at the end.
    pub fn append(&mut self, mut other: Vec<u8>) {
        self.vec.append(&mut other)
    }

    /// Appends a copy of all the bytes of `other` at the end.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.vec.extend_from_slice(other)
    }

    /// Inserts a byte at `index` in the bytes of the current codec.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, element: u8) {
        self.vec.insert(index + self.start_at, element)
    }

    /// Removes and returns the byte at `index` in the bytes of the current codec.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`.
    pub fn remove(&mut self, index: usize) -> u8 {
        self.vec.remove(index + self.start_at)
    }

    /// Appends `size` bytes set to `fill_with`, and returns a token to fill them
    /// later with [`EncodingVec::retrieve_space`].
    pub fn save_space_for_later(&mut self, size: usize, fill_with: u8) -> SpaceToken {
        let pos = self.vec.len();
        self.vec.resize(pos + size, fill_with);
        SpaceToken { pos, size }
    }

    /// Reserves capacity for at least `additional` more bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.vec.reserve(additional)
    }

    /// Removes the bytes of the current codec, the previous codecs ones stays untouched.
    pub fn clear(&mut self) {
        self.vec.truncate(self.start_at)
    }

    /// Resizes the bytes of the current codec to `new_len`, filling with `value`.
    pub fn resize(&mut self, new_len: usize, value: u8) {
        self.vec.resize(self.start_at + new_len, value)
    }

    /// Removes and returns the last byte of the current codec, or `None` if it wrote nothing.
    pub fn pop(&mut self) -> Option<u8> {
        if self.vec.len() > self.start_at {
            self.vec.pop()
        } else {
            None
        }
    }

    /// Ends the writing of the current codec. The returned vec starts a new,
    /// empty, view for the next codec.
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

#[cfg(test)]
mod test {
    use std::convert::Infallible;

    use byteorder::BE;

    use crate::codec::{Encode, EncodingVec, Fresh, U8, U32};

    #[test]
    fn is_empty_borrows() {
        let vec = EncodingVec::new();
        assert!(vec.is_empty());
        // `is_empty` must not consume the buffer
        let mut vec = vec.edit();
        vec.push(1);
        assert!(!vec.is_empty());
        assert_eq!(vec.finish(), [1]);
    }

    #[test]
    fn extend_from_slice() {
        let mut vec = EncodingVec::new().edit();
        vec.push(1);
        vec.extend_from_slice(&[2, 3]);
        vec.extend_from_slice(&[]);
        assert_eq!(&*vec, [1, 2, 3]);
        assert_eq!(vec.finish(), [1, 2, 3]);
    }

    #[test]
    fn pop() {
        let mut vec = EncodingVec::new().edit();
        vec.push(1);
        vec.push(2);
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
        assert!(vec.finish().is_empty());
    }

    #[test]
    fn pop_does_not_touch_previous_codecs() {
        /// Pushes a byte, pops it, then tries to pop one more byte that belongs to the previous codec.
        enum PopCodec {}

        impl Encode for PopCodec {
            type Item = Option<u8>;
            type Error = Infallible;

            fn encode(
                into: EncodingVec<Fresh>,
                expected: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let mut ret = into.edit();
                ret.push(3);
                assert_eq!(ret.pop(), Some(3));
                assert_eq!(ret.pop(), *expected);
                Ok(ret.make_fresh())
            }
        }

        let buf = <(U8, PopCodec)>::encode_alloc(&(42, None)).unwrap();
        assert_eq!(buf.finish(), vec![42]);
    }

    #[test]
    fn resize_after_previous_codecs() {
        /// Resizes its own section to two bytes of `7`.
        enum ResizeCodec {}

        impl Encode for ResizeCodec {
            type Item = ();
            type Error = Infallible;

            fn encode(
                into: EncodingVec<Fresh>,
                _: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let mut ret = into.edit();
                ret.resize(2, 7);
                Ok(ret.make_fresh())
            }
        }

        let buf = <(U32<BE>, ResizeCodec)>::encode_alloc(&(1, ())).unwrap();
        assert_eq!(buf.finish(), vec![0, 0, 0, 1, 7, 7]);
    }

    #[test]
    fn resize_grows_existing_bytes() {
        let mut vec = EncodingVec::new().edit();
        vec.extend([1, 2, 3]);
        vec.resize(5, 9);
        assert_eq!(vec.finish(), vec![1, 2, 3, 9, 9]);
    }

    #[test]
    fn resize_never_shrinks_below_start_at() {
        let mut prefix = EncodingVec::new().edit();
        prefix.extend([42, 43]);

        let mut vec = prefix.make_fresh().edit();
        vec.extend([1, 2, 3]);
        vec.resize(1, 0);
        assert_eq!(vec.as_slice(), &[1]);
        vec.resize(0, 0);
        assert!(vec.as_slice().is_empty());
        assert_eq!(vec.finish(), vec![42, 43]);
    }
}
