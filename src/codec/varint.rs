//! Unsigned integers written on as few bytes as possible, in an order-preserving format.

use std::{convert::Infallible, fmt};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting};

/// The most bytes a `u128` takes: 7 bits of the number per byte.
const MAX_LEN: usize = 128_usize.div_ceil(7);

/// Number of bytes needed to write `n`.
const fn encoded_len(n: u128) -> usize {
    let bits = (u128::BITS - n.leading_zeros()) as usize;
    if bits == 0 { 1 } else { bits.div_ceil(7) }
}

/// Writes `n` in `buf` and returns the number of bytes used.
fn write(n: u128, buf: &mut [u8; MAX_LEN]) -> usize {
    let len = encoded_len(n);
    for (i, byte) in buf[..len].iter_mut().rev().enumerate() {
        *byte = n.checked_shr(8 * i as u32).unwrap_or(0) as u8;
    }
    // The number only uses the 7 * len low bits, the first len bits are free for the marker
    for marker in 0..len - 1 {
        buf[marker / 8] |= 0x80 >> (marker % 8);
    }
    len
}

/// Reads a number of at most `bits` bits from the start of `bytes`, and
/// returns it with the number of bytes it took.
fn read(bytes: &[u8], bits: u32) -> Result<(u128, usize), VarUintDecodeError> {
    let max_len = (bits as usize).div_ceil(7);
    // The fast path: most numbers, like the lengths of short keys, fit in a single byte
    if let Some(&first) = bytes.first()
        && first < 0x80
    {
        return Ok((first as u128, 1));
    }

    // The number of leading 1 bits gives the number of bytes after the first one
    let mut extra = 0;
    for byte in bytes {
        extra += byte.leading_ones() as usize;
        if *byte != 0xFF || extra >= max_len {
            break;
        }
    }
    let len = extra + 1;
    if len > max_len {
        return Err(VarUintDecodeError::Overflow);
    }
    if bytes.len() < len {
        return Err(VarUintDecodeError::MissingBytes {
            expected: len,
            remaining: bytes.len(),
        });
    }

    let mut n: u128 = 0;
    for (i, byte) in bytes[..len].iter().enumerate() {
        // Clears the marker bits, the first `len` bits of the encoding
        let marker_bits = len.saturating_sub(8 * i).min(8);
        let byte = byte & 0xFF_u8.checked_shr(marker_bits as u32).unwrap_or(0);
        if n.leading_zeros() < 8 {
            return Err(VarUintDecodeError::Overflow);
        }
        n = (n << 8) | byte as u128;
    }
    if u128::BITS - n.leading_zeros() > bits {
        return Err(VarUintDecodeError::Overflow);
    }
    if encoded_len(n) != len {
        return Err(VarUintDecodeError::NotMinimal);
    }
    Ok((n, len))
}

macro_rules! define_varint {
    ($name:ident, $native:ident, $max_len:literal) => {
        #[doc = concat!("Encodable version of [`", stringify!($native), "`], written on as few bytes as possible, from 1 to ", $max_len, ".")]
        ///
        /// Each byte holds 7 bits of the number: up to 127 takes one byte, up to
        /// 16383 two bytes, and so on. The encoding starts with as many `1` bits
        /// as there are bytes after the first one, then a `0` bit, then the number big-endian.
        ///
        /// The encoded bytes sort like the numbers, so it can be used for keys.
        /// Each number has a single encoding: decoding refuses the ones that
        /// use more bytes than needed.
        ///
        /// This codec is self-delimiting: it can be followed by other codecs in a [`ComposeCodec`](crate::codec::ComposeCodec).
        ///
        /// ```
        #[doc = concat!("use fiole::codec::{Decode, DecodingVec, Encode, ", stringify!($name), "};")]
        ///
        #[doc = concat!("assert_eq!(", stringify!($name), "::encode_alloc(&42).unwrap().finish(), [42]);")]
        #[doc = concat!("assert_eq!(", stringify!($name), "::encode_alloc(&300).unwrap().finish(), [0x81, 0x2C]);")]
        ///
        #[doc = concat!("let big = ", stringify!($name), "::encode_alloc(&300).unwrap().finish();")]
        #[doc = concat!("let small = ", stringify!($name), "::encode_alloc(&42).unwrap().finish();")]
        /// assert!(small < big);
        #[doc = concat!("assert_eq!(", stringify!($name), "::decode(&mut DecodingVec::new(big)).unwrap(), 300);")]
        /// ```
        pub enum $name {}

        impl SelfDelimiting for $name {}

        impl Encode for $name {
            type Item = $native;
            type Error = Infallible;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let mut buf = [0; MAX_LEN];
                let len = write(*item as u128, &mut buf);
                let mut ret = into.edit();
                ret.extend_from_slice(&buf[..len]);
                Ok(ret.make_fresh())
            }
        }

        impl Decode for $name {
            type Item = $native;
            type Error = VarUintDecodeError;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                let (n, len) = read(bytes.remaining_slice(), $native::BITS)?;
                bytes.take_next(len);
                Ok(n as $native)
            }
        }
    };
}

define_varint!(VarU16, u16, 3);
define_varint!(VarU32, u32, 5);
define_varint!(VarU64, u64, 10);
define_varint!(VarU128, u128, 19);

/// Writes `n` as a [`VarU64`] and returns its bytes, for the codecs that need
/// to know the encoding before writing it, like [`SizedCodec`](crate::codec::SizedCodec).
pub(crate) fn encode_u64(n: u64) -> ([u8; MAX_LEN], usize) {
    let mut buf = [0; MAX_LEN];
    let len = write(n as u128, &mut buf);
    (buf, len)
}

/// Why a varint couldn't be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarUintDecodeError {
    /// The encoding announces `expected` bytes, only `remaining` are left.
    MissingBytes { expected: usize, remaining: usize },
    /// The number doesn't fit in the type it's decoded to.
    Overflow,
    /// The number is written on more bytes than needed.
    NotMinimal,
}

impl fmt::Display for VarUintDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarUintDecodeError::MissingBytes {
                expected,
                remaining,
            } => write!(
                f,
                "the varint is {expected} bytes long but only {remaining} bytes are left"
            ),
            VarUintDecodeError::Overflow => f.write_str("the varint doesn't fit in its type"),
            VarUintDecodeError::NotMinimal => {
                f.write_str("the varint is written on more bytes than needed")
            }
        }
    }
}

impl std::error::Error for VarUintDecodeError {}

#[cfg(test)]
mod test {
    use super::{VarU16, VarU32, VarU64, VarU128, VarUintDecodeError};
    use crate::codec::{
        Decode, DecodingVec, Encode, check_order_preserving, check_round_trip,
        check_self_delimiting,
    };

    #[test]
    fn exact_encoding() {
        let encode = |n: u64| VarU64::encode_alloc(&n).unwrap().finish();
        assert_eq!(encode(0), [0x00]);
        assert_eq!(encode(127), [0x7F]);
        assert_eq!(encode(128), [0x80, 0x80]);
        assert_eq!(encode(300), [0x81, 0x2C]);
        assert_eq!(encode(16383), [0xBF, 0xFF]);
        assert_eq!(encode(16384), [0xC0, 0x40, 0x00]);
        assert_eq!(encode((1 << 21) - 1), [0xDF, 0xFF, 0xFF]);
        assert_eq!(
            encode((1 << 56) - 1),
            [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
        // 8 bytes after the first one: the marker fills the whole first byte
        assert_eq!(encode(1 << 56), [0xFF, 0x01, 0, 0, 0, 0, 0, 0, 0]);
        // 9 bytes after the first one: the marker spills on the second byte
        assert_eq!(
            encode(u64::MAX),
            [0xFF, 0x80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn max_lengths() {
        let len = |bytes: Vec<u8>| bytes.len();
        assert_eq!(len(VarU16::encode_alloc(&u16::MAX).unwrap().finish()), 3);
        assert_eq!(len(VarU32::encode_alloc(&u32::MAX).unwrap().finish()), 5);
        assert_eq!(len(VarU64::encode_alloc(&u64::MAX).unwrap().finish()), 10);
        assert_eq!(len(VarU128::encode_alloc(&u128::MAX).unwrap().finish()), 19);
    }

    #[test]
    fn properties() {
        arbtest::arbtest(check_round_trip::<VarU16, u16>);
        arbtest::arbtest(check_round_trip::<VarU32, u32>);
        arbtest::arbtest(check_round_trip::<VarU64, u64>);
        arbtest::arbtest(check_round_trip::<VarU128, u128>);

        arbtest::arbtest(check_self_delimiting::<VarU16, u16>);
        arbtest::arbtest(check_self_delimiting::<VarU32, u32>);
        arbtest::arbtest(check_self_delimiting::<VarU64, u64>);
        arbtest::arbtest(check_self_delimiting::<VarU128, u128>);

        arbtest::arbtest(check_order_preserving::<VarU16, u16>);
        arbtest::arbtest(check_order_preserving::<VarU32, u32>);
        arbtest::arbtest(check_order_preserving::<VarU64, u64>);
        arbtest::arbtest(check_order_preserving::<VarU128, u128>);
    }

    #[test]
    fn small_numbers_are_property_tested_too() {
        // Arbitrary integers are mostly big, make sure every length is covered
        arbtest::arbtest(|u| {
            let shift = u.int_in_range(0..=127)?;
            let n = u.arbitrary::<u128>()? >> shift;
            let bytes = VarU128::encode_alloc(&n).unwrap().finish();
            assert_eq!(VarU128::decode(&mut DecodingVec::new(&bytes)).unwrap(), n);

            let m = u.arbitrary::<u128>()? >> u.int_in_range(0..=127)?;
            let other = VarU128::encode_alloc(&m).unwrap().finish();
            assert_eq!(
                n.cmp(&m),
                bytes.cmp(&other),
                "{n} vs {m}: {bytes:?} vs {other:?}"
            );
            Ok(())
        });
    }

    #[test]
    fn every_boundary_round_trips() {
        for bits in 0..=128 {
            for n in [
                1u128.checked_shl(bits).map_or(u128::MAX, |n| n - 1),
                1u128.checked_shl(bits).unwrap_or(0),
            ] {
                let bytes = VarU128::encode_alloc(&n).unwrap().finish();
                assert_eq!(
                    VarU128::decode(&mut DecodingVec::new(&bytes)).unwrap(),
                    n,
                    "{n}: {bytes:?}"
                );
            }
        }
    }

    #[test]
    fn refuses_numbers_that_dont_fit() {
        // 65536 needs 3 bytes, like u16::MAX, but doesn't fit in a u16
        let bytes = VarU32::encode_alloc(&65536).unwrap().finish();
        assert_eq!(bytes.len(), 3);
        let error = VarU16::decode(&mut DecodingVec::new(bytes)).unwrap_err();
        assert_eq!(error, VarUintDecodeError::Overflow);

        // More bytes than a u16 ever takes
        let bytes = VarU32::encode_alloc(&u32::MAX).unwrap().finish();
        let error = VarU16::decode(&mut DecodingVec::new(bytes)).unwrap_err();
        assert_eq!(error, VarUintDecodeError::Overflow);

        // A marker announcing more bytes than a u128 ever takes
        let error = VarU128::decode(&mut DecodingVec::new([0xFF; 30])).unwrap_err();
        assert_eq!(error, VarUintDecodeError::Overflow);
    }

    #[test]
    fn refuses_non_minimal_encodings() {
        // 5 written on two bytes
        let error = VarU64::decode(&mut DecodingVec::new([0x80, 0x05])).unwrap_err();
        assert_eq!(error, VarUintDecodeError::NotMinimal);
        let error = VarU64::decode(&mut DecodingVec::new([0xC0, 0x00, 0x05])).unwrap_err();
        assert_eq!(error, VarUintDecodeError::NotMinimal);
    }

    #[test]
    fn truncated() {
        let error = VarU64::decode(&mut DecodingVec::new([0xC0, 0x40])).unwrap_err();
        assert_eq!(
            error,
            VarUintDecodeError::MissingBytes {
                expected: 3,
                remaining: 2
            }
        );
        let error = VarU64::decode(&mut DecodingVec::new([])).unwrap_err();
        assert_eq!(
            error,
            VarUintDecodeError::MissingBytes {
                expected: 1,
                remaining: 0
            }
        );
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<VarU64>(&42);
        crate::test_alloc::assert_decodes_without_allocating::<VarU64>(&u64::MAX);
        crate::test_alloc::assert_decodes_without_allocating::<VarU128>(&u128::MAX);
    }
}
