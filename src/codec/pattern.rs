//! Fixed sequences of bytes used by codecs like [`Constant`](crate::codec::Constant)
//! and [`Terminated`](crate::codec::Terminated).

use std::marker::PhantomData;

use byteorder::{BigEndian, ByteOrder, LittleEndian};

/// A fixed sequence of bytes.
///
/// Implement it on your own type for any pattern, or use a helper like
/// [`U8Pattern`] for the common ones.
///
/// ```
/// use fiole::codec::{ComposeCodec, Constant, Decode, DecodingVec, Encode, Pattern, U8};
///
/// enum Arrow {}
///
/// impl Pattern for Arrow {
///     const PATTERN: &'static [u8] = b"->";
/// }
///
/// type Codec = ComposeCodec<(U8, Constant<Arrow>, U8)>;
/// let bytes = Codec::encode_alloc(&(1, (), 2)).unwrap().finish();
/// assert_eq!(bytes, b"\x01->\x02");
/// ```
pub trait Pattern {
    /// The bytes of the pattern.
    const PATTERN: &'static [u8];
}

/// A pattern made of the single byte `B`.
///
/// ```
/// use fiole::codec::{Pattern, U8Pattern};
///
/// assert_eq!(U8Pattern::<b'/'>::PATTERN, b"/");
/// ```
pub enum U8Pattern<const B: u8> {}

impl<const B: u8> Pattern for U8Pattern<B> {
    const PATTERN: &'static [u8] = &[B];
}

/// A pattern made of the single byte `B`, encoded as two's complement.
///
/// ```
/// use fiole::codec::{I8Pattern, Pattern};
///
/// assert_eq!(I8Pattern::<-2>::PATTERN, [0xFE]);
/// ```
pub enum I8Pattern<const B: i8> {}

impl<const B: i8> Pattern for I8Pattern<B> {
    const PATTERN: &'static [u8] = &[B as u8];
}

macro_rules! define_int_pattern {
    ($name:ident, $native:ident, $example:literal, $be:literal, $le:literal $(, #[$doc:meta])*) => {
        #[doc = concat!("A pattern made of the bytes of the [`", stringify!($native), "`] `N`, in the byte order `O`.")]
        ///
        $(#[$doc] #[doc = ""])*
        /// ```
        /// use fiole::byteorder::{BigEndian, LittleEndian};
        #[doc = concat!("use fiole::codec::{Pattern, ", stringify!($name), "};")]
        ///
        #[doc = concat!("assert_eq!(", stringify!($name), "::<", $example, ", BigEndian>::PATTERN, ", $be, ");")]
        #[doc = concat!("assert_eq!(", stringify!($name), "::<", $example, ", LittleEndian>::PATTERN, ", $le, ");")]
        /// ```
        pub struct $name<const N: $native, O: ByteOrder>(PhantomData<O>);

        impl<const N: $native> Pattern for $name<N, BigEndian> {
            const PATTERN: &'static [u8] = &N.to_be_bytes();
        }

        impl<const N: $native> Pattern for $name<N, LittleEndian> {
            const PATTERN: &'static [u8] = &N.to_le_bytes();
        }
    };
}

define_int_pattern!(U16Pattern, u16, "0x0102", "[1, 2]", "[2, 1]");
define_int_pattern!(
    U32Pattern,
    u32,
    "0x01020304",
    "[1, 2, 3, 4]",
    "[4, 3, 2, 1]"
);
define_int_pattern!(
    U64Pattern,
    u64,
    "1",
    "[0, 0, 0, 0, 0, 0, 0, 1]",
    "[1, 0, 0, 0, 0, 0, 0, 0]"
);
define_int_pattern!(
    U128Pattern,
    u128,
    "1",
    "[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]",
    "[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]"
);

define_int_pattern!(
    I16Pattern,
    i16,
    "-2",
    "[0xFF, 0xFE]",
    "[0xFE, 0xFF]",
    #[doc = "Encoded as two's complement."]
);
define_int_pattern!(
    I32Pattern,
    i32,
    "-2",
    "[0xFF, 0xFF, 0xFF, 0xFE]",
    "[0xFE, 0xFF, 0xFF, 0xFF]",
    #[doc = "Encoded as two's complement."]
);
define_int_pattern!(
    I64Pattern,
    i64,
    "-2",
    "[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE]",
    "[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]",
    #[doc = "Encoded as two's complement."]
);
define_int_pattern!(
    I128Pattern,
    i128,
    "-2",
    "[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE]",
    "[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]",
    #[doc = "Encoded as two's complement."]
);

macro_rules! define_ordered_uint_pattern {
    ($name:ident, $codec:ident, $native:ident, $pattern:ty) => {
        #[doc = concat!("A pattern made of the bytes of the [`", stringify!($native), "`] `N`, encoded like [`", stringify!($codec), "`](crate::codec::", stringify!($codec), ").")]
        ///
        #[doc = concat!("It's only an alias to [`", stringify!($pattern), "`]: unsigned integers stored big-endian already sort like the numbers.")]
        pub type $name<const N: $native> = $pattern;
    };
}

define_ordered_uint_pattern!(OrderedU8Pattern, OrderedU8, u8, U8Pattern<N>);
define_ordered_uint_pattern!(OrderedU16Pattern, OrderedU16, u16, U16Pattern<N, BigEndian>);
define_ordered_uint_pattern!(OrderedU32Pattern, OrderedU32, u32, U32Pattern<N, BigEndian>);
define_ordered_uint_pattern!(OrderedU64Pattern, OrderedU64, u64, U64Pattern<N, BigEndian>);
define_ordered_uint_pattern!(OrderedU128Pattern, OrderedU128, u128, U128Pattern<N, BigEndian>);

macro_rules! define_ordered_int_pattern {
    ($name:ident, $codec:ident, $signed:ident, $unsigned:ident) => {
        #[doc = concat!("A pattern made of the bytes of the [`", stringify!($signed), "`] `N`, encoded like [`", stringify!($codec), "`](crate::codec::", stringify!($codec), ").")]
        ///
        /// The number is stored big-endian with its sign bit flipped, so the
        /// pattern sorts like the number among the keys encoded with
        #[doc = concat!("[`", stringify!($codec), "`](crate::codec::", stringify!($codec), ").")]
        ///
        /// ```
        #[doc = concat!("use fiole::codec::{Encode, ", stringify!($codec), ", Pattern, ", stringify!($name), "};")]
        ///
        #[doc = concat!("let encoded = ", stringify!($codec), "::encode_alloc(&-2).unwrap().finish();")]
        #[doc = concat!("assert_eq!(", stringify!($name), "::<-2>::PATTERN, encoded);")]
        /// ```
        pub enum $name<const N: $signed> {}

        impl<const N: $signed> Pattern for $name<N> {
            const PATTERN: &'static [u8] =
                &((N as $unsigned) ^ (1 << ($unsigned::BITS - 1))).to_be_bytes();
        }
    };
}

define_ordered_int_pattern!(OrderedI8Pattern, OrderedI8, i8, u8);
define_ordered_int_pattern!(OrderedI16Pattern, OrderedI16, i16, u16);
define_ordered_int_pattern!(OrderedI32Pattern, OrderedI32, i32, u32);
define_ordered_int_pattern!(OrderedI64Pattern, OrderedI64, i64, u64);
define_ordered_int_pattern!(OrderedI128Pattern, OrderedI128, i128, u128);

/// A pattern made of the UTF-8 encoding of the character `C`, from one to four bytes.
///
/// ```
/// use fiole::codec::{CharPattern, Pattern};
///
/// assert_eq!(CharPattern::<'/'>::PATTERN, b"/");
/// assert_eq!(CharPattern::<'é'>::PATTERN, "é".as_bytes());
/// ```
pub enum CharPattern<const C: char> {}

impl<const C: char> CharPattern<C> {
    /// The UTF-8 encoding of `C`, followed by zeroes up to four bytes.
    const UTF8: [u8; 4] = {
        let mut buf = [0; 4];
        C.encode_utf8(&mut buf);
        buf
    };
}

impl<const C: char> Pattern for CharPattern<C> {
    const PATTERN: &'static [u8] = (&Self::UTF8 as &[u8]).split_at(C.len_utf8()).0;
}

/// A single `0x00` byte. As the terminator of a [`Terminated`](crate::codec::Terminated)
/// codec, it preserves the order of the inner codec.
pub type Nul = U8Pattern<0>;

#[cfg(test)]
mod test {
    use byteorder::{BE, LE, NativeEndian, NetworkEndian};

    use super::*;
    use crate::codec::{
        ComposeCodec, Constant, Decode, DecodingVec, Encode, OrderedI8, OrderedI16, OrderedI32,
        OrderedI64, OrderedI128, OrderedU8, OrderedU16, OrderedU32, OrderedU64, OrderedU128, Str,
        Terminated, U8, check_self_delimiting,
    };

    /// Checks the pattern of each value in both byte orders against the standard library.
    macro_rules! check_int_pattern {
        ($pattern:ident, $native:ident, [$($value:expr),* $(,)?]) => {
            $(
                assert_eq!(
                    $pattern::<{ $value }, BE>::PATTERN,
                    ($value as $native).to_be_bytes(),
                    "{} {} in big-endian", stringify!($pattern), $value as $native,
                );
                assert_eq!(
                    $pattern::<{ $value }, LE>::PATTERN,
                    ($value as $native).to_le_bytes(),
                    "{} {} in little-endian", stringify!($pattern), $value as $native,
                );
            )*
        };
    }

    /// Checks the pattern of each value is what its codec writes, and that the
    /// patterns sort like the values, which must be given in increasing order.
    macro_rules! check_ordered_pattern {
        ($pattern:ident, $codec:ident, $native:ident, [$($value:expr),* $(,)?]) => {
            let patterns = [$(
                {
                    let pattern = $pattern::<{ $value }>::PATTERN;
                    let encoded = $codec::encode_alloc(&($value as $native)).unwrap().finish();
                    assert_eq!(pattern, encoded, "{} {}", stringify!($pattern), $value as $native);
                    pattern
                }
            ),*];
            assert!(patterns.is_sorted(), "{} doesn't sort like the numbers: {patterns:?}", stringify!($pattern));
        };
    }

    #[test]
    fn ordered_unsigned_patterns() {
        check_ordered_pattern!(OrderedU8Pattern, OrderedU8, u8, [0, 1, 0x7F, 0x80, u8::MAX]);
        check_ordered_pattern!(
            OrderedU16Pattern,
            OrderedU16,
            u16,
            [0, 1, 0xFF, 0x100, u16::MAX]
        );
        check_ordered_pattern!(
            OrderedU32Pattern,
            OrderedU32,
            u32,
            [0, 1, 0xFF, 0x100, u32::MAX]
        );
        check_ordered_pattern!(
            OrderedU64Pattern,
            OrderedU64,
            u64,
            [0, 1, 0xFF, 0x100, u64::MAX]
        );
        check_ordered_pattern!(
            OrderedU128Pattern,
            OrderedU128,
            u128,
            [0, 1, 0xFF, 0x100, u128::MAX]
        );
    }

    #[test]
    fn ordered_signed_patterns() {
        check_ordered_pattern!(
            OrderedI8Pattern,
            OrderedI8,
            i8,
            [i8::MIN, -2, -1, 0, 1, i8::MAX]
        );
        check_ordered_pattern!(
            OrderedI16Pattern,
            OrderedI16,
            i16,
            [i16::MIN, -256, -1, 0, 1, 256, i16::MAX]
        );
        check_ordered_pattern!(
            OrderedI32Pattern,
            OrderedI32,
            i32,
            [i32::MIN, -256, -1, 0, 1, 256, i32::MAX]
        );
        check_ordered_pattern!(
            OrderedI64Pattern,
            OrderedI64,
            i64,
            [i64::MIN, -256, -1, 0, 1, 256, i64::MAX]
        );
        check_ordered_pattern!(
            OrderedI128Pattern,
            OrderedI128,
            i128,
            [i128::MIN, -256, -1, 0, 1, 256, i128::MAX]
        );
    }

    #[test]
    fn single_byte_patterns() {
        assert_eq!(Nul::PATTERN, [0]);
        assert_eq!(U8Pattern::<0>::PATTERN, [0]);
        assert_eq!(U8Pattern::<0x7F>::PATTERN, [0x7F]);
        assert_eq!(U8Pattern::<{ u8::MAX }>::PATTERN, [0xFF]);
        assert_eq!(I8Pattern::<0>::PATTERN, [0]);
        assert_eq!(I8Pattern::<1>::PATTERN, [1]);
        assert_eq!(I8Pattern::<-1>::PATTERN, [0xFF]);
        assert_eq!(I8Pattern::<{ i8::MIN }>::PATTERN, [0x80]);
        assert_eq!(I8Pattern::<{ i8::MAX }>::PATTERN, [0x7F]);
    }

    #[test]
    fn unsigned_patterns() {
        check_int_pattern!(U16Pattern, u16, [0, 1, 0x0102, u16::MAX]);
        check_int_pattern!(U32Pattern, u32, [0, 1, 0x0102_0304, u32::MAX]);
        check_int_pattern!(U64Pattern, u64, [0, 1, 0x0102_0304_0506_0708, u64::MAX]);
        check_int_pattern!(
            U128Pattern,
            u128,
            [0, 1, 0x0102_0304_0506_0708_090A_0B0C_0D0E_0F10, u128::MAX]
        );
    }

    #[test]
    fn signed_patterns() {
        check_int_pattern!(I16Pattern, i16, [0, 1, -1, -2, 0x0102, i16::MIN, i16::MAX]);
        check_int_pattern!(
            I32Pattern,
            i32,
            [0, 1, -1, -2, 0x0102_0304, i32::MIN, i32::MAX]
        );
        check_int_pattern!(
            I64Pattern,
            i64,
            [0, 1, -1, -2, 0x0102_0304_0506_0708, i64::MIN, i64::MAX]
        );
        check_int_pattern!(
            I128Pattern,
            i128,
            [
                0,
                1,
                -1,
                -2,
                0x0102_0304_0506_0708_090A_0B0C_0D0E_0F10,
                i128::MIN,
                i128::MAX
            ]
        );
    }

    #[test]
    fn int_patterns_accept_every_byteorder_alias() {
        assert_eq!(U16Pattern::<0x0102, BE>::PATTERN, [1, 2]);
        assert_eq!(U16Pattern::<0x0102, NetworkEndian>::PATTERN, [1, 2]);
        assert_eq!(U16Pattern::<0x0102, LE>::PATTERN, [2, 1]);
        assert_eq!(
            U16Pattern::<0x0102, NativeEndian>::PATTERN,
            0x0102u16.to_ne_bytes()
        );
        assert_eq!(
            I32Pattern::<-2, NativeEndian>::PATTERN,
            (-2i32).to_ne_bytes()
        );
    }

    #[test]
    fn char_pattern_is_the_utf8_encoding() {
        assert_eq!(CharPattern::<'\0'>::PATTERN, b"\0");
        assert_eq!(CharPattern::<'a'>::PATTERN, b"a");
        assert_eq!(CharPattern::<'é'>::PATTERN, "é".as_bytes());
        assert_eq!(CharPattern::<'€'>::PATTERN, "€".as_bytes());
        assert_eq!(CharPattern::<'🦀'>::PATTERN, "🦀".as_bytes());
        assert_eq!(CharPattern::<{ char::MAX }>::PATTERN, b"\xF4\x8F\xBF\xBF");
    }

    #[test]
    fn int_pattern_as_a_constant() {
        type Codec = ComposeCodec<(U8, Constant<U32Pattern<0xCAFE_BABE, BE>>, U8)>;

        let bytes = Codec::encode_alloc(&(1, (), 2)).unwrap().finish();
        assert_eq!(bytes, [1, 0xCA, 0xFE, 0xBA, 0xBE, 2]);
        assert_eq!(
            Codec::decode(&mut DecodingVec::new(bytes)).unwrap(),
            (1, (), 2)
        );
    }

    #[test]
    fn signed_pattern_as_a_constant() {
        type Codec = ComposeCodec<(Constant<I16Pattern<-2, LE>>, U8)>;

        let bytes = Codec::encode_alloc(&((), 7)).unwrap().finish();
        assert_eq!(bytes, [0xFE, 0xFF, 7]);
        assert_eq!(
            Codec::decode(&mut DecodingVec::new(bytes)).unwrap(),
            ((), 7)
        );
        // The same value in the other byte order isn't the constant
        assert!(Codec::decode(&mut DecodingVec::new([0xFF, 0xFE, 7])).is_err());
    }

    #[test]
    fn char_pattern_as_a_terminator() {
        type Codec<'a> = ComposeCodec<(&'a Terminated<Str, CharPattern<'é'>>, U8)>;

        let bytes = Codec::encode_alloc(&("caf", 1)).unwrap().finish();
        assert_eq!(bytes, b"caf\xC3\xA9\x01");
        let (s, n) = Codec::decode(&mut DecodingVec::new(bytes)).unwrap();
        assert_eq!((&*s, n), ("caf", 1));
        // The string can't contain the terminator
        assert!(Codec::encode_alloc(&("café", 1)).is_err());

        arbtest::arbtest(check_self_delimiting::<Terminated<Str, CharPattern<'é'>>, String>);
    }
}
