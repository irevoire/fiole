use std::{fmt, io::Read, marker::PhantomData};

use byteorder::ByteOrder;

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

pub struct SizedCodec<C>(PhantomData<C>);

impl<'a, C: Encode<'a>> Encode<'a> for SizedCodec<C> {
    type Item = C::Item;
    type Error = C::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &'a Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        let token = ret.save_space_for_later(std::mem::size_of::<u32>(), 0);

        let pos = ret.absolute_pos();
        let mut ret = C::encode(ret.make_fresh(), item)?;
        let size = ret.absolute_pos() - pos;

        let bytes = ret.retrieve_space(token);
        byteorder::BE::write_u32(bytes, size as u32);

        Ok(ret)
    }
}

#[derive(Debug)]
pub enum SizedCodecError<E> {
    TooSmall,
    Other(E),
}

impl<E: fmt::Display> fmt::Display for SizedCodecError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizedCodecError::TooSmall => f.write_str("Sized codec couldn't deserialize the size because it received a slice of less than 4 bytes"),
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
        let size = u32::from_be_bytes(size);
        let mut sub = bytes.take_next(size as usize);
        C::decode(&mut sub).map_err(SizedCodecError::Other)
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Bytes, Decode, DecodingVec, Encode, SizedCodec, Str};

    #[test]
    fn sized() {
        let letter = "ACAB";
        let number = [13, 12];

        // === Without the sized codec

        // With the normal codec all the bytes are put one after the other with no size in between
        let buf = Str::encode_alloc(&letter).unwrap();
        let buf = Bytes::encode(buf, &number).unwrap();
        let ret = buf.into_decoding_vec().consume();
        assert_eq!(&ret, &[b'A', b'C', b'A', b'B', 13, 12]);

        // The decoder can't make the difference between the two slices of bytes
        let decode = Bytes::decode(&mut DecodingVec::new(ret)).unwrap();
        assert_eq!(&decode, &[b'A', b'C', b'A', b'B', 13, 12]);

        // === With the sized codec used everywhere

        // With the Sized codec, the size is written as an u32.
        let buf = SizedCodec::<Str>::encode_alloc(&letter).unwrap();
        let buf = SizedCodec::<Bytes>::encode(buf, &number).unwrap();
        let ret = buf.into_decoding_vec().consume();
        assert_eq!(
            &ret,
            &[0, 0, 0, 4, b'A', b'C', b'A', b'B', 0, 0, 0, 2, 13, 12]
        );

        // The decoder can extract each slice as expected
        let mut decoder = DecodingVec::new(ret);
        let decode_letter = SizedCodec::<Str>::decode(&mut decoder).unwrap();
        let decode_bytes = SizedCodec::<Bytes>::decode(&mut decoder).unwrap();
        assert_eq!(&decode_letter, "ACAB");
        assert_eq!(&decode_bytes, &[13, 12]);
        assert!(decoder.is_empty());
        assert!(decoder.consume().is_empty());

        // === With the sized codec for all entries except the last

        let buf = SizedCodec::<Str>::encode_alloc(&letter).unwrap();
        let buf = Bytes::encode(buf, &number).unwrap();
        let ret = buf.into_decoding_vec().consume();
        assert_eq!(&ret, &[0, 0, 0, 4, b'A', b'C', b'A', b'B', 13, 12]);

        // This also works and saved us 4 bytes for the size of the second slice
        let mut decoder = DecodingVec::new(ret);
        let decode_letter = SizedCodec::<Str>::decode(&mut decoder).unwrap();
        let decode_bytes = Bytes::decode(&mut decoder).unwrap();
        assert_eq!(&decode_letter, "ACAB");
        assert_eq!(&decode_bytes, &[13, 12]);
        assert!(decoder.is_empty());
        assert!(decoder.consume().is_empty());
    }
}
