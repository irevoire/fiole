use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting};
#[cfg(doc)]
use crate::codec::{
    OrderedF32, OrderedF64, OrderedI8, OrderedI16, OrderedI32, OrderedI64, OrderedI128,
};
use byteorder::{ByteOrder, ReadBytesExt};

use std::convert::Infallible;
use std::marker::PhantomData;

/// Encodable version of [`u8`].
///
/// This codec is self-delimiting: it can be followed by other codecs in a [`ComposeCodec`](crate::codec::ComposeCodec).
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

impl SelfDelimiting for U8 {}

/// Encodable version of [`i8`].
///
/// This codec is self-delimiting: it can be followed by other codecs in a [`ComposeCodec`](crate::codec::ComposeCodec).
///
/// Encoded as two's complement.
/// /!\ Be careful, when used as a key, negative numbers sort
/// after positive ones. Use [`OrderedI8`] if you need numeric ordering.
pub enum I8 {}

impl SelfDelimiting for I8 {}

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
    (@define $name:ident, $native:ident, $read_method:ident, $write_method:ident, $(#[$doc:meta])*) => {
        #[doc = concat!("Encodable version of [`", stringify!($native), "`].")]
        ///
        /// This codec is self-delimiting: it can be followed by other codecs in a [`ComposeCodec`](crate::codec::ComposeCodec).
        $(#[$doc])*
        pub struct $name<O>(PhantomData<O>);

        impl<O> SelfDelimiting for $name<O> {}

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
    // Unsigned integers
    ($name:ident, $native:ident, $read_method:ident, $write_method:ident) => {
        define_type!(
            @define $name, $native, $read_method, $write_method,
            ///
            /// /!\ When used as a key: only [`BigEndian`](byteorder::BigEndian) sorts like you would expect.
            /// [`LittleEndian`](byteorder::LittleEndian) and [`NativeEndian`](byteorder::NativeEndian)
            /// are better suited for values.
        );
    };
    // Signed integers
    ($name:ident, $native:ident, $read_method:ident, $write_method:ident, ordered: $ordered:ident) => {
        define_type!(
            @define $name, $native, $read_method, $write_method,
            ///
            /// /!\ When used as a key: only [`BigEndian`](byteorder::BigEndian) sorts like you would expect.
            /// [`LittleEndian`](byteorder::LittleEndian) and [`NativeEndian`](byteorder::NativeEndian)
            /// are better suited for values.
            ///
            /// Encoded as two's complement.
            /// /!\ Be careful, when used as a key, negative numbers sort
            #[doc = concat!("after positive ones. Use [`", stringify!($ordered), "`] if you need numeric ordering.")]
        );
    };
    // Floats
    ($name:ident, $native:ident, $read_method:ident, $write_method:ident, float: $ordered:ident) => {
        define_type!(
            @define $name, $native, $read_method, $write_method,
            ///
            /// Encoded as its IEEE 754 bits, so every value round-trips, including NaNs.
            ///
            /// /!\ Be careful, when used as a key, floats don't sort numerically, whatever the byte
            /// order: even with [`BigEndian`](byteorder::BigEndian), negative numbers sort after positive ones,
            #[doc = concat!("and in reverse order. Use [`", stringify!($ordered), "`] if you need numeric ordering.")]
        );
    };
}

define_type!(U16, u16, read_u16, write_u16);
define_type!(U32, u32, read_u32, write_u32);
define_type!(U64, u64, read_u64, write_u64);
define_type!(U128, u128, read_u128, write_u128);
define_type!(I16, i16, read_i16, write_i16, ordered: OrderedI16);
define_type!(I32, i32, read_i32, write_i32, ordered: OrderedI32);
define_type!(I64, i64, read_i64, write_i64, ordered: OrderedI64);
define_type!(I128, i128, read_i128, write_i128, ordered: OrderedI128);
define_type!(F32, f32, read_f32, write_f32, float: OrderedF32);
define_type!(F64, f64, read_f64, write_f64, float: OrderedF64);

#[cfg(test)]
mod test {
    use arbitrary::{Arbitrary, Unstructured};
    use byteorder::{BE, LE};

    use crate::codec::{
        Decode, DecodingVec, Encode, F32, F64, check_round_trip_with, check_self_delimiting_with,
    };

    /// NaN is never equal to itself, so the generic checks skip it and
    /// `bitwise_round_trip` covers it.
    fn not_nan<F: for<'a> Arbitrary<'a> + Default + PartialEq>(
        u: &mut Unstructured,
    ) -> arbitrary::Result<F> {
        let f = F::arbitrary(u)?;
        #[allow(clippy::eq_op)]
        Ok(if f == f { f } else { F::default() })
    }

    #[test]
    fn floats() {
        arbtest::arbtest(|u| check_round_trip_with::<F32<BE>, f32>(u, not_nan));
        arbtest::arbtest(|u| check_round_trip_with::<F32<LE>, f32>(u, not_nan));
        arbtest::arbtest(|u| check_round_trip_with::<F64<BE>, f64>(u, not_nan));
        arbtest::arbtest(|u| check_round_trip_with::<F64<LE>, f64>(u, not_nan));

        arbtest::arbtest(|u| check_self_delimiting_with::<F32<BE>, f32>(u, not_nan));
        arbtest::arbtest(|u| check_self_delimiting_with::<F32<LE>, f32>(u, not_nan));
        arbtest::arbtest(|u| check_self_delimiting_with::<F64<BE>, f64>(u, not_nan));
        arbtest::arbtest(|u| check_self_delimiting_with::<F64<LE>, f64>(u, not_nan));
    }

    #[test]
    fn bitwise_round_trip() {
        arbtest::arbtest(|u| {
            let f = f32::from_bits(u.arbitrary()?);
            let bytes = F32::<BE>::encode_alloc(&f).unwrap().finish();
            let decoded = F32::<BE>::decode(&mut DecodingVec::new(bytes)).unwrap();
            assert_eq!(decoded.to_bits(), f.to_bits(), "{f} became {decoded}");
            Ok(())
        });
        arbtest::arbtest(|u| {
            let f = f64::from_bits(u.arbitrary()?);
            let bytes = F64::<LE>::encode_alloc(&f).unwrap().finish();
            let decoded = F64::<LE>::decode(&mut DecodingVec::new(bytes)).unwrap();
            assert_eq!(decoded.to_bits(), f.to_bits(), "{f} became {decoded}");
            Ok(())
        });
    }

    #[test]
    fn exact_float_encoding() {
        assert_eq!(
            F32::<BE>::encode_alloc(&1.0).unwrap().finish(),
            [0x3F, 0x80, 0x00, 0x00]
        );
        assert_eq!(
            F32::<LE>::encode_alloc(&1.0).unwrap().finish(),
            [0x00, 0x00, 0x80, 0x3F]
        );
        assert_eq!(
            F64::<BE>::encode_alloc(&-2.0).unwrap().finish(),
            [0xC0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn truncated_float() {
        let mut decoder = DecodingVec::new(vec![0x3F, 0x80]);
        assert!(F32::<BE>::decode(&mut decoder).is_err());
    }

    #[test]
    fn floats_do_not_sort_numerically() {
        // What the documentation warns about, even in big-endian
        let encode = |f: f64| F64::<BE>::encode_alloc(&f).unwrap().finish();
        assert!(encode(1.0) < encode(2.0));
        assert!(encode(-1.0) > encode(1.0)); // negative numbers sort after the positive ones
        assert!(encode(-2.0) > encode(-1.0)); // and in reverse order
    }
}
