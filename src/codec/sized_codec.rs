use std::{fmt, marker::PhantomData};

use crate::codec::{
    Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting, VarU64, VarUintDecodeError,
    varint,
};

/// Makes any codec [`SelfDelimiting`] by prefixing it by its length.
///
/// The format is the length of the inner codec's bytes as a [`VarU64`],
/// followed by those bytes: a value of up to 127 bytes only costs one more
/// byte, up to 16383 bytes two more bytes, and so on. Unlike [`Terminated`](crate::codec::Terminated),
/// it works whatever bytes the inner codec writes.
///
/// ```
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, Encode, SizedCodec, Str, U8};
///
/// type Codec<'a> = ComposeCodec<(&'a SizedCodec<Str>, U8)>;
///
/// let bytes = Codec::encode_alloc(&("hi", 7)).unwrap().finish();
/// assert_eq!(bytes, [2, b'h', b'i', 7]);
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
        // Most values are short enough for their length to fit in a single
        // byte: it's written before them, and the rare longer lengths are
        // completed afterward by moving the value.
        let mut ret = into.edit();
        let token = ret.save_space_for_later(1, 0);

        let pos = ret.absolute_pos();
        let mut ret = C::encode(ret.make_fresh(), item)?;
        let size = ret.absolute_pos() - pos;

        let (header, len) = varint::encode_u64(size as u64);
        ret.retrieve_space(token)[0] = header[0];
        if len > 1 {
            ret.insert_slice(pos, &header[1..len]);
        }

        Ok(ret)
    }
}

#[derive(Debug)]
pub enum SizedCodecError<E> {
    /// The length before the value couldn't be decoded.
    Length(VarUintDecodeError),
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
            SizedCodecError::Length(e) => write!(
                f,
                "Sized codec couldn't decode the length of its value: {e}"
            ),
            SizedCodecError::Truncated {
                expected,
                available,
            } => write!(
                f,
                "Sized codec expected {expected} bytes but only {available} bytes remain"
            ),
            SizedCodecError::Other(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for SizedCodecError<E> {}

impl<C: Decode> Decode for SizedCodec<C> {
    type Item = C::Item;
    type Error = SizedCodecError<C::Error>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let size = VarU64::decode(bytes).map_err(SizedCodecError::Length)?;
        // A length that doesn't fit in memory is necessarily bigger than what's left
        let size = usize::try_from(size).unwrap_or(usize::MAX);
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
    use crate::codec::{
        Bytes, ComposeCodec, Decode, DecodingVec, Encode, SizedCodec, SizedCodecError, Str, U8,
        VarU64, VarUintDecodeError, check_self_delimiting,
    };

    #[test]
    fn the_length_takes_as_few_bytes_as_possible() {
        for (size, header_len) in [
            (0, 1),
            (127, 1),
            (128, 2),
            (16383, 2),
            (16384, 3),
            (1 << 21, 4),
        ] {
            let value = vec![0xAB; size];
            let bytes = SizedCodec::<Bytes>::encode_alloc(&value).unwrap().finish();
            let header = VarU64::encode_alloc(&(size as u64)).unwrap().finish();
            assert_eq!(header.len(), header_len);
            assert_eq!(bytes.len(), header_len + size);
            assert_eq!(bytes[..header_len], header, "{size}");
            assert!(bytes[header_len..].iter().all(|b| *b == 0xAB), "{size}");

            let mut decoder = DecodingVec::new(&bytes);
            assert_eq!(*SizedCodec::<Bytes>::decode(&mut decoder).unwrap(), value);
            assert!(decoder.is_empty());
        }
    }

    #[test]
    fn a_long_length_doesnt_shift_the_previous_codecs() {
        type Codec<'a> = ComposeCodec<(U8, &'a SizedCodec<Bytes>, U8)>;

        let value = vec![0xAB; 300];
        let bytes = Codec::encode_alloc(&(1, &value[..], 2)).unwrap().finish();
        assert_eq!(bytes[..3], [1, 0x81, 0x2C]);
        assert_eq!(bytes.len(), 1 + 2 + 300 + 1);
        assert_eq!(bytes.last(), Some(&2));
        let (a, decoded, b) = Codec::decode(&mut DecodingVec::new(bytes)).unwrap();
        assert_eq!((a, &*decoded, b), (1, &value[..], 2));
    }

    #[test]
    fn properties() {
        arbtest::arbtest(check_self_delimiting::<SizedCodec<Str>, String>);
        arbtest::arbtest(check_self_delimiting::<SizedCodec<Bytes>, Vec<u8>>);
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

        // With the Sized codec, the size is written as a varint.
        let buf = SizedCodec::<Str>::encode_alloc(letter).unwrap();
        let buf = SizedCodec::<Bytes>::encode(buf, &number).unwrap();
        let ret = buf.finish();
        assert_eq!(&ret, &[4, b'A', b'C', b'A', b'B', 2, 13, 12]);

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
        assert_eq!(&ret, &[4, b'A', b'C', b'A', b'B', 13, 12]);

        // This also works and saved us a byte for the size of the second slice
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
        let mut decoder = DecodingVec::new(vec![10, b'h', b'i']);
        assert!(matches!(
            SizedCodec::<Str>::decode(&mut decoder),
            Err(SizedCodecError::Truncated {
                expected: 10,
                available: 2
            })
        ));

        // The header announces 1 byte but nothing follows
        let mut decoder = DecodingVec::new(vec![1]);
        assert!(matches!(
            SizedCodec::<Str>::decode(&mut decoder),
            Err(SizedCodecError::Truncated {
                expected: 1,
                available: 0
            })
        ));

        // When the size matches exactly it must still work
        let mut decoder = DecodingVec::new(vec![2, b'h', b'i']);
        assert_eq!(&*SizedCodec::<Str>::decode(&mut decoder).unwrap(), "hi");
        assert!(decoder.is_empty());

        // Even if there is no data afterward
        let mut decoder = DecodingVec::new(vec![0]);
        assert_eq!(&*SizedCodec::<Str>::decode(&mut decoder).unwrap(), "");
        assert!(decoder.is_empty());

        // The length itself can be invalid
        let mut decoder = DecodingVec::new(vec![]);
        assert!(matches!(
            SizedCodec::<Str>::decode(&mut decoder),
            Err(SizedCodecError::Length(
                VarUintDecodeError::MissingBytes { .. }
            ))
        ));
        let mut decoder = DecodingVec::new(vec![0x80, 0x05, b'h', b'e', b'l', b'l', b'o']);
        assert!(matches!(
            SizedCodec::<Str>::decode(&mut decoder),
            Err(SizedCodecError::Length(VarUintDecodeError::NotMinimal))
        ));
    }

    #[test]
    fn decodes_without_copying() {
        use byteorder::BE;

        use crate::codec::{Terminated, U32, Unit};

        crate::test_alloc::assert_decodes_without_allocating::<SizedCodec<U32<BE>>>(&42);
        crate::test_alloc::assert_decodes_without_allocating::<Terminated<Unit>>(&());
    }
}
