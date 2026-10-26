use std::io::Read;

use byteview::ByteView;
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

/// The bytes a codec decodes, with a cursor to know what has already been read.
///
/// It shares the buffer of the [`Slice`] or [`ByteView`] it's built from, so
/// building it, [`DecodingVec::take_next`] and [`DecodingVec::consume`] never copy the bytes.
pub struct DecodingVec {
    bytes: ByteView,
    position: usize,
    /// Whether the bytes were copied when building the `DecodingVec`.
    #[cfg(test)]
    copied: bool,
}

impl From<ByteView> for DecodingVec {
    fn from(bytes: ByteView) -> Self {
        Self {
            bytes,
            position: 0,
            #[cfg(test)]
            copied: false,
        }
    }
}

impl From<Slice> for DecodingVec {
    fn from(value: Slice) -> Self {
        ByteView::from(value).into()
    }
}

impl From<&Slice> for DecodingVec {
    fn from(value: &Slice) -> Self {
        value.clone().into()
    }
}

impl DecodingVec {
    /// Copies `bytes` to decode them. Build it from a [`Slice`] or a
    /// [`ByteView`] instead to share their buffer.
    ///
    /// ```
    /// use fiole::codec::{Decode, DecodingVec, Str};
    ///
    /// let decoded = Str::decode(&mut DecodingVec::new(b"hello")).unwrap();
    /// assert_eq!(decoded, "hello");
    /// ```
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        Self {
            #[cfg(test)]
            copied: true,
            ..ByteView::new(bytes.as_ref()).into()
        }
    }

    /// Equivalent to [`std::io::Read::take`] except it return a `DecodingVec` which can be used within another codec.
    pub fn take_next(&mut self, size: usize) -> DecodingVec {
        let start = self.position.min(self.bytes.len());
        let end = self.bytes.len().min(start.saturating_add(size));
        self.position = end;
        DecodingVec {
            bytes: self.bytes.slice(start..end),
            position: 0,
            #[cfg(test)]
            copied: self.copied,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Moves the cursor to the end without reading anything.
    pub fn skip_to_end(&mut self) {
        self.position = self.bytes.len();
    }

    /// The bytes left to read, without moving the cursor.
    pub fn remaining_slice(&self) -> &[u8] {
        &self.bytes[self.position.min(self.bytes.len())..]
    }

    /// Reads everything till the end.
    ///
    /// ```
    /// use std::io::Read;
    /// use fiole::codec::DecodingVec;
    ///
    /// let mut bytes = DecodingVec::new(b"\x01hello");
    /// bytes.read_exact(&mut [0]).unwrap();
    /// assert_eq!(&*bytes.consume(), b"hello");
    /// assert!(bytes.is_empty());
    /// ```
    pub fn consume(&mut self) -> ByteView {
        self.take_next(usize::MAX).bytes
    }

    /// Number of bytes left to read.
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
impl DecodingVec {
    /// Whether the bytes were copied when building the `DecodingVec`,
    /// rather than shared with the buffer they come from.
    pub(crate) fn is_copied(&self) -> bool {
        self.copied
    }
}

impl Read for DecodingVec {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut remaining = self.remaining_slice();
        let read = remaining.read(buf)?;
        self.position += read;
        Ok(read)
    }
}

#[cfg(test)]
mod test {
    use fjall::Slice;

    use super::DecodingVec;

    #[test]
    fn consume_shares_the_bytes() {
        // Longer than the 20 bytes stored inline, so the bytes live on the heap
        let slice = Slice::from("a value long enough to not be stored inline");
        let (consumed, allocations) = crate::test_alloc::allocations(|| {
            let mut decoder = DecodingVec::from(&slice);
            decoder.take_next(2);
            let consumed = decoder.consume();
            assert!(decoder.is_empty());
            consumed
        });
        assert_eq!(&*consumed, b"value long enough to not be stored inline");
        assert_eq!(allocations, 0);
    }
}
