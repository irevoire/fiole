//! Number codecs whose encoding sorts like the numbers themselves, nice to use for keys.

use std::{convert::Infallible, io::Read};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting};

macro_rules! define_ordered_int {
    ($name:ident, $signed:ty, $unsigned:ty) => {
        #[doc = concat!("Encodable version of [`", stringify!($signed), "`] that keeps the numeric order, to be used as a key.")]
        ///
        /// The number is stored big-endian with its sign bit flipped, so the
        /// smallest number is encoded as `0x00…` and the biggest as `0xFF…`,
        /// and the encoded bytes sort like the numbers: negative numbers come
        /// before positive ones.
        ///
        /// This codec is self-delimiting: it can be followed by other codecs in a [`ComposeCodec`](crate::codec::ComposeCodec).
        ///
        #[doc = concat!("The encoding isn't compatible with [`", stringify!($signed), "`]'s plain codec: switching an existing keyspace to it requires rewriting its keys.")]
        ///
        /// ```
        #[doc = concat!("use fiole::codec::{Decode, DecodingVec, Encode, ", stringify!($name), "};")]
        ///
        #[doc = concat!("let minus_one = ", stringify!($name), "::encode_alloc(&-1).unwrap().finish();")]
        #[doc = concat!("let zero = ", stringify!($name), "::encode_alloc(&0).unwrap().finish();")]
        /// assert!(minus_one < zero);
        #[doc = concat!("assert_eq!(", stringify!($name), "::decode(&mut DecodingVec::new(minus_one)).unwrap(), -1);")]
        /// ```
        pub enum $name {}

        impl SelfDelimiting for $name {}

        impl $name {
            const SIGN: $unsigned = 1 << (<$unsigned>::BITS - 1);
        }

        impl Encode for $name {
            type Item = $signed;
            type Error = Infallible;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let mut ret = into.edit();
                let bits = (*item as $unsigned) ^ Self::SIGN;
                ret.extend(&bits.to_be_bytes());
                Ok(ret.make_fresh())
            }
        }

        impl Decode for $name {
            type Item = $signed;
            type Error = std::io::Error;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                let mut buf = [0; size_of::<$unsigned>()];
                bytes.read_exact(&mut buf)?;
                Ok((<$unsigned>::from_be_bytes(buf) ^ Self::SIGN) as $signed)
            }
        }
    };
}

define_ordered_int!(OrderedI8, i8, u8);
define_ordered_int!(OrderedI16, i16, u16);
define_ordered_int!(OrderedI32, i32, u32);
define_ordered_int!(OrderedI64, i64, u64);
define_ordered_int!(OrderedI128, i128, u128);

macro_rules! define_ordered_float {
    ($name:ident, $float:ty, $unsigned:ty) => {
        #[doc = concat!("Encodable version of [`", stringify!($float), "`] that keeps the numeric order, to be used as a key.")]
        ///
        /// The bits of the number are stored big-endian: positive numbers get
        /// their sign bit flipped and negative numbers get all their bits
        /// flipped, so the encoded bytes sort like the numbers.
        ///
        /// This codec is self-delimiting: it can be followed by other codecs in a [`ComposeCodec`](crate::codec::ComposeCodec).
        ///
        #[doc = concat!("The order is the one of [`", stringify!($float), "::total_cmp`]:")]
        /// `-NaN < -∞ < … < -0.0 < 0.0 < … < ∞ < NaN`. Every value, including
        /// the sign of zero and the payload of NaNs, survives a round-trip.
        ///
        /// ```
        #[doc = concat!("use fiole::codec::{Decode, DecodingVec, Encode, ", stringify!($name), "};")]
        ///
        #[doc = concat!("let minus_two = ", stringify!($name), "::encode_alloc(&-2.0).unwrap().finish();")]
        #[doc = concat!("let minus_one = ", stringify!($name), "::encode_alloc(&-1.0).unwrap().finish();")]
        #[doc = concat!("let half = ", stringify!($name), "::encode_alloc(&0.5).unwrap().finish();")]
        /// assert!(minus_two < minus_one);
        /// assert!(minus_one < half);
        #[doc = concat!("assert_eq!(", stringify!($name), "::decode(&mut DecodingVec::new(half)).unwrap(), 0.5);")]
        /// ```
        pub enum $name {}

        impl SelfDelimiting for $name {}

        impl $name {
            const SIGN: $unsigned = 1 << (<$unsigned>::BITS - 1);
        }

        impl Encode for $name {
            type Item = $float;
            type Error = Infallible;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let mut ret = into.edit();
                let bits = item.to_bits();
                let bits = if bits & Self::SIGN != 0 {
                    !bits
                } else {
                    bits ^ Self::SIGN
                };
                ret.extend(&bits.to_be_bytes());
                Ok(ret.make_fresh())
            }
        }

        impl Decode for $name {
            type Item = $float;
            type Error = std::io::Error;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                let mut buf = [0; size_of::<$unsigned>()];
                bytes.read_exact(&mut buf)?;
                let bits = <$unsigned>::from_be_bytes(buf);
                let bits = if bits & Self::SIGN != 0 {
                    bits ^ Self::SIGN
                } else {
                    !bits
                };
                Ok(<$float>::from_bits(bits))
            }
        }
    };
}

define_ordered_float!(OrderedF32, f32, u32);
define_ordered_float!(OrderedF64, f64, u64);

#[cfg(test)]
mod test {
    use std::{borrow::Borrow, cmp::Ordering};

    use arbitrary::{Arbitrary, Unstructured};
    use fjall::KeyspaceCreateOptions;

    use crate::{
        Database,
        codec::{
            Decode, DecodingVec, Encode, OrderedF32, OrderedF64, OrderedI8, OrderedI16, OrderedI32,
            OrderedI64, OrderedI128, U8, check_order_preserving, check_round_trip,
            check_self_delimiting, check_self_delimiting_with,
        },
    };

    #[test]
    fn exact_integer_encoding() {
        let encode = |n: i32| OrderedI32::encode_alloc(&n).unwrap().finish();
        assert_eq!(encode(i32::MIN), [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(encode(-1), [0x7F, 0xFF, 0xFF, 0xFF]);
        assert_eq!(encode(0), [0x80, 0x00, 0x00, 0x00]);
        assert_eq!(encode(1), [0x80, 0x00, 0x00, 0x01]);
        assert_eq!(encode(i32::MAX), [0xFF, 0xFF, 0xFF, 0xFF]);

        assert_eq!(OrderedI8::encode_alloc(&-128).unwrap().finish(), [0x00]);
        assert_eq!(OrderedI8::encode_alloc(&127).unwrap().finish(), [0xFF]);
    }

    #[test]
    fn integer_edge_cases() {
        for n in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
            let bytes = OrderedI64::encode_alloc(&n).unwrap().finish();
            assert_eq!(OrderedI64::decode(&mut DecodingVec::new(bytes)).unwrap(), n);
        }
    }

    #[test]
    fn truncated_integer() {
        let mut decoder = DecodingVec::new(vec![0x80, 0x00]);
        assert!(OrderedI32::decode(&mut decoder).is_err());
    }

    #[test]
    fn integers() {
        arbtest::arbtest(check_round_trip::<OrderedI8, i8>);
        arbtest::arbtest(check_round_trip::<OrderedI16, i16>);
        arbtest::arbtest(check_round_trip::<OrderedI32, i32>);
        arbtest::arbtest(check_round_trip::<OrderedI64, i64>);
        arbtest::arbtest(check_round_trip::<OrderedI128, i128>);

        arbtest::arbtest(check_self_delimiting::<OrderedI8, i8>);
        arbtest::arbtest(check_self_delimiting::<OrderedI16, i16>);
        arbtest::arbtest(check_self_delimiting::<OrderedI32, i32>);
        arbtest::arbtest(check_self_delimiting::<OrderedI64, i64>);
        arbtest::arbtest(check_self_delimiting::<OrderedI128, i128>);

        arbtest::arbtest(check_order_preserving::<OrderedI8, i8>);
        arbtest::arbtest(check_order_preserving::<OrderedI16, i16>);
        arbtest::arbtest(check_order_preserving::<OrderedI32, i32>);
        arbtest::arbtest(check_order_preserving::<OrderedI64, i64>);
        arbtest::arbtest(check_order_preserving::<OrderedI128, i128>);
    }

    /// Floats ordered by `total_cmp`, since they don't implement [`Ord`].
    macro_rules! total {
        ($name:ident, $float:ty) => {
            #[derive(Debug, Clone, Copy, Arbitrary)]
            struct $name($float);

            impl Borrow<$float> for $name {
                fn borrow(&self) -> &$float {
                    &self.0
                }
            }

            impl PartialEq for $name {
                fn eq(&self, other: &Self) -> bool {
                    self.cmp(other) == Ordering::Equal
                }
            }

            impl Eq for $name {}

            impl PartialOrd for $name {
                fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl Ord for $name {
                fn cmp(&self, other: &Self) -> Ordering {
                    self.0.total_cmp(&other.0)
                }
            }
        };
    }

    total!(TotalF32, f32);
    total!(TotalF64, f64);

    /// NaN is never equal to itself, so the generic checks compare floats
    /// without it and `bitwise_round_trip` covers NaN.
    fn not_nan<F: for<'a> Arbitrary<'a> + Default + PartialEq>(
        u: &mut Unstructured,
    ) -> arbitrary::Result<F> {
        let f = F::arbitrary(u)?;
        #[allow(clippy::eq_op)]
        Ok(if f == f { f } else { F::default() })
    }

    #[test]
    fn floats() {
        arbtest::arbtest(|u| check_self_delimiting_with::<OrderedF32, f32>(u, not_nan));
        arbtest::arbtest(|u| check_self_delimiting_with::<OrderedF64, f64>(u, not_nan));

        arbtest::arbtest(check_order_preserving::<OrderedF32, TotalF32>);
        arbtest::arbtest(check_order_preserving::<OrderedF64, TotalF64>);
    }

    #[test]
    fn bitwise_round_trip() {
        arbtest::arbtest(|u| {
            let f = f32::from_bits(u.arbitrary()?);
            let bytes = OrderedF32::encode_alloc(&f).unwrap().finish();
            let decoded = OrderedF32::decode(&mut DecodingVec::new(bytes)).unwrap();
            assert_eq!(decoded.to_bits(), f.to_bits(), "{f} became {decoded}");
            Ok(())
        });
        arbtest::arbtest(|u| {
            let f = f64::from_bits(u.arbitrary()?);
            let bytes = OrderedF64::encode_alloc(&f).unwrap().finish();
            let decoded = OrderedF64::decode(&mut DecodingVec::new(bytes)).unwrap();
            assert_eq!(decoded.to_bits(), f.to_bits(), "{f} became {decoded}");
            Ok(())
        });
    }

    #[test]
    fn float_edge_cases_are_sorted() {
        let sorted = [
            -f64::NAN,
            f64::NEG_INFINITY,
            f64::MIN,
            -1.0,
            -f64::MIN_POSITIVE,
            -f64::from_bits(1), // smallest negative subnormal
            -0.0,
            0.0,
            f64::from_bits(1), // smallest positive subnormal
            f64::MIN_POSITIVE,
            1.0,
            f64::MAX,
            f64::INFINITY,
            f64::NAN,
        ];
        let encoded: Vec<_> = sorted
            .iter()
            .map(|f| OrderedF64::encode_alloc(f).unwrap().finish())
            .collect();
        for (i, window) in encoded.windows(2).enumerate() {
            assert!(
                window[0] < window[1],
                "{} should sort before {}",
                sorted[i],
                sorted[i + 1]
            );
        }
        for (f, bytes) in sorted.iter().zip(encoded) {
            let decoded = OrderedF64::decode(&mut DecodingVec::new(bytes)).unwrap();
            assert_eq!(decoded.to_bits(), f.to_bits());
        }
    }

    #[test]
    fn ordered_integer_keys_in_a_keyspace() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<OrderedI32, U8>("hello", KeyspaceCreateOptions::default)
            .unwrap();

        let mut wtxn = database.write_tx().unwrap();
        for n in -5..5 {
            ks.insert(&mut wtxn, &n, &0).unwrap();
        }
        wtxn.commit().unwrap().unwrap();

        let rtxn = database.read_tx();
        let keys: Vec<_> = ks
            .range(&rtxn, &(-3..3))
            .unwrap()
            .map(|guard| guard.key().unwrap())
            .collect();
        assert_eq!(keys, [-3, -2, -1, 0, 1, 2]);

        let first = ks.first_key_value(&rtxn).unwrap().key().unwrap();
        assert_eq!(first, -5);
        let last = ks.last_key_value(&rtxn).unwrap().key().unwrap();
        assert_eq!(last, 4);
    }

    #[test]
    fn ordered_float_keys_in_a_keyspace() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<OrderedF64, U8>("hello", KeyspaceCreateOptions::default)
            .unwrap();

        let mut wtxn = database.write_tx().unwrap();
        for f in [2.5, -0.5, f64::NEG_INFINITY, 0.0, -3.0, 1.0] {
            ks.insert(&mut wtxn, &f, &0).unwrap();
        }
        wtxn.commit().unwrap().unwrap();

        let rtxn = database.read_tx();
        let keys: Vec<_> = ks
            .range(&rtxn, &(-1.0..2.0))
            .unwrap()
            .map(|guard| guard.key().unwrap())
            .collect();
        assert_eq!(keys, [-0.5, 0.0, 1.0]);

        let first = ks.first_key_value(&rtxn).unwrap().key().unwrap();
        assert_eq!(first, f64::NEG_INFINITY);
    }
}
