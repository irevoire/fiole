use std::marker::PhantomData;

use facet::Facet;
use fjall::Slice;

use crate::codec::{Decode, Dirty, Encode, EncodingVec, Fresh};

/// Encode a struct as postcard through the [`facet::Facet`] trait.
pub struct FacetPostcard<T>(PhantomData<T>);

impl<T: Facet<'static>> Encode for FacetPostcard<T> {
    type Item = T;
    type Error = facet_postcard::SerializeError;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        facet_postcard::to_writer_fallible(item, &mut ret)?;
        Ok(ret.make_fresh())
    }
}

impl<T: Facet<'static>> Decode for FacetPostcard<T> {
    type Item = T;
    type Error = facet_postcard::DeserializeError;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        facet_postcard::from_slice(&bytes)
    }
}

impl facet_postcard::Writer for EncodingVec<Dirty> {
    fn write_byte(&mut self, byte: u8) -> Result<(), facet_postcard::SerializeError> {
        self.vec.push(byte);
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), facet_postcard::SerializeError> {
        self.vec.extend_from_slice(bytes);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use facet::Facet;

    use crate::codec::{Decode, Encode, EncodingVec, FacetPostcard};

    #[test]
    fn encode_and_decode() {
        #[derive(Facet, Debug, PartialEq)]
        struct Example {
            name: String,
            value: i32,
        }

        let value = Example {
            name: "pi".to_string(),
            value: 31415926,
        };

        let facet_bytes = facet_postcard::to_vec(&value).unwrap();
        let facet_deserialized = facet_postcard::from_slice(&facet_bytes).unwrap();

        let codec_bytes = FacetPostcard::<Example>::encode(EncodingVec::new(), &value).unwrap();
        assert_eq!(codec_bytes.as_slice(), facet_bytes);

        let codec_deserialized =
            FacetPostcard::<Example>::decode(codec_bytes.into_fjall_slice()).unwrap();

        assert_eq!(codec_deserialized, facet_deserialized);
        assert_eq!(codec_deserialized, value);
    }
}
