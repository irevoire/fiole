use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};
use byteorder::{ByteOrder, ReadBytesExt};

use std::convert::Infallible;
use std::marker::PhantomData;

/// Encodable version of [`u8`].
pub enum U8 {}

impl Encode for U8 {
    type Item = u8;
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        ret.push(*item);
        Ok(ret.make_fresh())
    }
}

impl Decode for U8 {
    type Item = u8;
    type Error = std::io::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        bytes.read_u8()
    }
}

/// Encodable version of [`i8`].
pub enum I8 {}

impl Encode for I8 {
    type Item = i8;
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        ret.push(*item as u8);
        Ok(ret.make_fresh())
    }
}

impl Decode for I8 {
    type Item = i8;
    type Error = std::io::Error;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        bytes.read_i8()
    }
}

macro_rules! define_type {
    ($name:ident, $native:ident, $read_method:ident, $write_method:ident) => {
        #[doc = "Encodable version of [`"]
        #[doc = stringify!($native)]
        #[doc = "`]."]
        pub struct $name<O>(PhantomData<O>);

        impl<O: ByteOrder> Encode for $name<O> {
            type Item = $native;
            type Error = Infallible;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let mut ret = into.edit();
                let mut buf = [0; size_of::<Self::Item>()];
                O::$write_method(&mut buf, *item);
                ret.extend(&buf);
                Ok(ret.make_fresh())
            }
        }

        impl<O: ByteOrder> Decode for $name<O> {
            type Item = $native;
            type Error = std::io::Error;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                bytes.$read_method::<O>().map_err(Into::into)
            }
        }
    };
}

define_type!(U16, u16, read_u16, write_u16);
define_type!(U32, u32, read_u32, write_u32);
define_type!(U64, u64, read_u64, write_u64);
define_type!(U128, u128, read_u128, write_u128);
define_type!(I16, i16, read_i16, write_i16);
define_type!(I32, i32, read_i32, write_i32);
define_type!(I64, i64, read_i64, write_i64);
define_type!(I128, i128, read_i128, write_i128);
