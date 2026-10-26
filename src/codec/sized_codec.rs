use std::{fmt, io::Read, marker::PhantomData};

use byteorder::ByteOrder;

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting};

/// Makes any codec [`SelfDelimiting`] by prefixing it by its length.
///
/// The format is the length of the inner codec's bytes as a big-endian `u32`
/// on 4 bytes, followed by those bytes. Unlike [`Terminated`](crate::codec::Terminated),
/// it works whatever bytes the inner codec writes. Values whose encoding is
/// bigger than 4 GiB aren't supported.
///
/// # Panics
///
/// Encoding panics if the inner codec writes more than [`u32::MAX`] bytes.
///
/// ```
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, Encode, SizedCodec, Str, U8};
///
/// type Codec<'a> = ComposeCodec<(&'a SizedCodec<Str>, U8)>;
///
/// let bytes = Codec::encode_alloc(&("hi", 7)).unwrap().finish();
/// assert_eq!(bytes, [0, 0, 0, 2, b'h', b'i', 7]);
/// let (s, n) = Codec::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!((&*s, n), ("hi", 7));
/// ```
///
/// It's ideal for values and for keys whose order doesn't matter.
///
/// /!\ Since the length comes first, the keys are sorted by length before
/// their content: `"b"` sorts before `"aa"`, and a prefix search on the
/// beginning of a string can't work. Use [`Terminated`](crate::codec::Terminated) for ordered string keys.
///
/// ```
/// use fiole::codec::{Encode, SizedCodec, Str};
///
/// let b = SizedCodec::<Str>::encode_alloc("b").unwrap().finish();
/// let aa = SizedCodec::<Str>::encode_alloc("aa").unwrap().finish();
/// assert!(b < aa);
/// ```
pub struct SizedCodec<C>(PhantomData<C>);

impl<C> SelfDelimiting for SizedCodec<C> {}

impl<C: Encode> Encode for SizedCodec<C> {
    type Item = C::Item;
    type Error = C::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        let token = ret.save_space_for_later(std::mem::size_of::<u32>(), 0);

        let pos = ret.absolute_pos();
        let mut ret = C::encode(ret.make_fresh(), item)?;
        let size = ret.absolute_pos() - pos;

        let size = u32::try_from(size).unwrap_or_else(|_| {
            panic!("SizedCodec can't encode more than 4 GiB, got {size} bytes")
        });
        let bytes = ret.retrieve_space(token);
        byteorder::BE::write_u32(bytes, size);

        Ok(ret)
    }
}

#[derive(Debug)]
pub enum SizedCodecError<E> {
    TooSmall,
    /// The size header announces more bytes than what remains.
    Truncated {
        expected: usize,
        available: usize,
    },
    Other(E),
}

impl<E: fmt::Display> fmt::Display for SizedCodecError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizedCodecError::TooSmall => f.write_str("Sized codec couldn't deserialize the size because it received a slice of less than 4 bytes"),
            SizedCodecError::Truncated { expected, available } => write!(f, "Sized codec expected {expected} bytes but only {available} bytes remain"),
            SizedCodecError::Other(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for SizedCodecError<E> {}

impl<C: Decode> Decode for SizedCodec<C> {
    type Item = C::Item;
    type Error = SizedCodecError<C::Error>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let mut size = [0; std::mem::size_of::<u32>()];
        bytes
            .read_exact(&mut size)
            .map_err(|_| SizedCodecError::TooSmall)?;
        let size = u32::from_be_bytes(size) as usize;
        let available = bytes.remaining();
        if available < size {
            return Err(SizedCodecError::Truncated {
                expected: size,
                available,
            });
        }
        let mut sub = bytes.take_next(size);
        C::decode(&mut sub).map_err(SizedCodecError::Other)
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Bytes, Decode, DecodingVec, Encode, SizedCodec, SizedCodecError, Str};

    /// Writes as many zeros as its item says.
    ///
    /// The zeros come from `vec![0; _]`, which the OS gives lazily: the pages
    /// that are never touched don't use any memory, so it can write gigabytes.
    #[cfg(target_pointer_width = "64")]
    enum Zeros {}

    #[cfg(target_pointer_width = "64")]
    impl Encode for Zeros {
        type Item = usize;
        type Error = std::convert::Infallible;

        fn encode(
            into: crate::codec::EncodingVec<crate::codec::Fresh>,
            len: &usize,
        ) -> Result<crate::codec::EncodingVec<crate::codec::Fresh>, Self::Error> {
            let mut ret = into.edit();
            let mut zeros = vec![0; ret.vec.len() + len];
            zeros[..ret.vec.len()].copy_from_slice(&ret.vec);
            ret.vec = zeros;
            Ok(ret.make_fresh())
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn encode_up_to_4_gib() {
        let size = u32::MAX as usize;
        let bytes = SizedCodec::<Zeros>::encode_alloc(&size).unwrap().finish();
        assert_eq!(bytes.len(), 4 + size);
        assert_eq!(bytes[..4], u32::MAX.to_be_bytes());
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    #[should_panic(expected = "SizedCodec can't encode more than 4 GiB, got 4294967296 bytes")]
    fn encode_more_than_4_gib_panics() {
        let _ = SizedCodec::<Zeros>::encode_alloc(&(u32::MAX as usize + 1));
    }

    #[test]
    fn sized() {
        let letter = "ACAB";
        let number = [13, 12];

        // === Without the sized codec

        // With the normal codec all the bytes are put one after the other with no size in between
        let buf = Str::encode_alloc(letter).unwrap();
        let buf = Bytes::encode(buf, &number).unwrap();
        let ret = buf.finish();
        assert_eq!(&ret, &[b'A', b'C', b'A', b'B', 13, 12]);

        // The decoder can't make the difference between the two slices of bytes
        let decode = Bytes::decode(&mut DecodingVec::new(ret)).unwrap();
        assert_eq!(&*decode, &[b'A', b'C', b'A', b'B', 13, 12]);

        // === With the sized codec used everywhere

        // With the Sized codec, the size is written as an u32.
        let buf = SizedCodec::<Str>::encode_alloc(letter).unwrap();
        let buf = SizedCodec::<Bytes>::encode(buf, &number).unwrap();
        let ret = buf.finish();
        assert_eq!(
            &ret,
            &[0, 0, 0, 4, b'A', b'C', b'A', b'B', 0, 0, 0, 2, 13, 12]
        );

        // The decoder can extract each slice as expected
        let mut decoder = DecodingVec::new(ret);
        let decode_letter = SizedCodec::<Str>::decode(&mut decoder).unwrap();
        let decode_bytes = SizedCodec::<Bytes>::decode(&mut decoder).unwrap();
        assert_eq!(&*decode_letter, "ACAB");
        assert_eq!(&*decode_bytes, &[13, 12]);
        assert!(decoder.is_empty());
        assert!(decoder.consume().is_empty());

        // === With the sized codec for all entries except the last

        let buf = SizedCodec::<Str>::encode_alloc(letter).unwrap();
        let buf = Bytes::encode(buf, &number).unwrap();
        let ret = buf.finish();
        assert_eq!(&ret, &[0, 0, 0, 4, b'A', b'C', b'A', b'B', 13, 12]);

        // This also works and saved us 4 bytes for the size of the second slice
        let mut decoder = DecodingVec::new(ret);
        let decode_letter = SizedCodec::<Str>::decode(&mut decoder).unwrap();
        let decode_bytes = Bytes::decode(&mut decoder).unwrap();
        assert_eq!(&*decode_letter, "ACAB");
        assert_eq!(&*decode_bytes, &[13, 12]);
        assert!(decoder.is_empty());
        assert!(decoder.consume().is_empty());
    }

    #[test]
    fn truncated_data_is_an_error() {
        // The header announces 10 bytes but only 2 follow
        let mut decoder = DecodingVec::new(vec![0, 0, 0, 10, b'h', b'i']);
        assert!(matches!(
            SizedCodec::<Str>::decode(&mut decoder),
            Err(SizedCodecError::Truncated {
                expected: 10,
                available: 2
            })
        ));

        // The header announces 1 byte but nothing follows
        let mut decoder = DecodingVec::new(vec![0, 0, 0, 1]);
        assert!(matches!(
            SizedCodec::<Str>::decode(&mut decoder),
            Err(SizedCodecError::Truncated {
                expected: 1,
                available: 0
            })
        ));

        // When the size matches exactly it must still work
        let mut decoder = DecodingVec::new(vec![0, 0, 0, 2, b'h', b'i']);
        assert_eq!(&*SizedCodec::<Str>::decode(&mut decoder).unwrap(), "hi");
        assert!(decoder.is_empty());

        // Even if there is no data afterward
        let mut decoder = DecodingVec::new(vec![0, 0, 0, 0]);
        assert_eq!(&*SizedCodec::<Str>::decode(&mut decoder).unwrap(), "");
        assert!(decoder.is_empty());
    }

    #[test]
    fn decodes_without_copying() {
        use byteorder::BE;

        use crate::codec::{Terminated, U32, Unit};

        crate::test_alloc::assert_decodes_without_allocating::<SizedCodec<U32<BE>>>(&42);
        crate::test_alloc::assert_decodes_without_allocating::<Terminated<Unit>>(&());
    }
}
