use fjall::Slice;
use roaring::{RoaringBitmap, RoaringTreemap};

use crate::codec::{Decode, Encode, EncodingVec, Fresh};

/// Encode a roaring bitmap with [the standard on-disk format](https://github.com/RoaringBitmap/RoaringFormatSpec).
pub enum RoaringBitmapCodec {}

impl Encode for RoaringBitmapCodec {
    type Item = RoaringBitmap;
    type Error = std::io::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        item.serialize_into(&mut ret)?;
        Ok(ret.make_fresh())
    }
}

impl Decode for RoaringBitmapCodec {
    type Item = RoaringBitmap;
    type Error = std::io::Error;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        RoaringBitmap::deserialize_from(&mut bytes.as_ref())
    }
}

/// Encode a roaring treemap with [the standard on-disk format](https://github.com/RoaringBitmap/RoaringFormatSpec).
pub enum RoaringTreemapCodec {}

impl Encode for RoaringTreemapCodec {
    type Item = RoaringTreemap;
    type Error = std::io::Error;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        item.serialize_into(&mut ret)?;
        Ok(ret.make_fresh())
    }
}

impl Decode for RoaringTreemapCodec {
    type Item = RoaringTreemap;
    type Error = std::io::Error;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        RoaringTreemap::deserialize_from(&mut bytes.as_ref())
    }
}

#[cfg(test)]
mod test {
    use roaring::{RoaringBitmap, RoaringTreemap};

    use crate::codec::{Decode, Encode, EncodingVec, RoaringBitmapCodec, RoaringTreemapCodec};

    #[test]
    fn encode_and_decode_bitmap() {
        let bitmap = RoaringBitmap::from_iter(&[1, 2, 3, 5, 2058, 2064, 2080, 2090]);
        let mut roaring_bytes = Vec::new();
        bitmap.serialize_into(&mut roaring_bytes).unwrap();
        let roaring_deserialized =
            RoaringBitmap::deserialize_from(&mut roaring_bytes.as_slice()).unwrap();

        let codec_bytes = RoaringBitmapCodec::encode(EncodingVec::new(), &bitmap).unwrap();
        assert_eq!(codec_bytes.as_slice(), roaring_bytes);

        let codec_deserialized =
            RoaringBitmapCodec::decode(codec_bytes.into_fjall_slice()).unwrap();

        assert_eq!(codec_deserialized, roaring_deserialized);
        assert_eq!(codec_deserialized, bitmap);
    }

    #[test]
    fn encode_and_decode_treemap() {
        let bitmap = RoaringTreemap::from_iter(&[1, 2, 3, 5, 2058, 2064, 2080, 2090]);
        let mut roaring_bytes = Vec::new();
        bitmap.serialize_into(&mut roaring_bytes).unwrap();
        let roaring_deserialized =
            RoaringTreemap::deserialize_from(&mut roaring_bytes.as_slice()).unwrap();

        let codec_bytes = RoaringTreemapCodec::encode(EncodingVec::new(), &bitmap).unwrap();
        assert_eq!(codec_bytes.as_slice(), roaring_bytes);

        let codec_deserialized =
            RoaringTreemapCodec::decode(codec_bytes.into_fjall_slice()).unwrap();

        assert_eq!(codec_deserialized, roaring_deserialized);
        assert_eq!(codec_deserialized, bitmap);
    }
}
