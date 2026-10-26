use std::{fmt, marker::PhantomData};

use crate::codec::{
    Decode, DecodingVec, Dirty, Encode, EncodingVec, Fresh, Pattern, SelfDelimiting,
};

/// Makes any codec [`SelfDelimiting`] when its encoding always takes `N` bytes,
/// or at most `N` bytes with padding.
///
/// The format is the bytes of the inner codec `C`, with nothing before or after them:
/// decoding gives `C` exactly the next `N` bytes. It's made for the values whose
/// encoding always has the same length, like a code, a hash, a date or an id in its text form.
/// Encoding a value that isn't written on exactly `N` bytes returns
/// [`FixedEncodeError::WrongLength`].
///
/// Compared to [`SizedCodec`](crate::codec::SizedCodec) and [`Terminated`](crate::codec::Terminated),
/// it costs no byte at all, and keeps the order of the inner codec.
///
/// ```
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, Encode, Fixed, OrderedU32, Str};
///
/// // A two-letter country code, then a number
/// type Key<'a> = ComposeCodec<(&'a Fixed<2, Str>, OrderedU32)>;
///
/// let bytes = Key::encode_alloc(&("fr", 42)).unwrap().finish();
/// assert_eq!(bytes, b"fr\0\0\0\x2a");
/// let (country, number) = Key::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!((&*country, number), ("fr", 42));
///
/// // A code of another length can't be written
/// assert!(Key::encode_alloc(&("fra", 42)).is_err());
/// ```
///
/// # Padding
///
/// The last parameter pads the values shorter than `N` bytes up to `N`, with a
/// one-byte [`Pattern`]. It says on which side the padding goes, and whether
/// decoding strips it or gives it to the inner codec:
///
/// | Padding | Padding goes | Decoding gives `C` |
/// |---------|--------------|--------------------|
/// | [`NoPadding`] (default) | nowhere, the value must be `N` bytes long | the `N` bytes |
/// | [`PadLeftStripped`] | before the value | the value, without the padding |
/// | [`PadRightStripped`] | after the value | the value, without the padding |
/// | [`PadLeftKeep`] | before the value | the `N` bytes, padding included |
/// | [`PadRightKeep`] | after the value | the `N` bytes, padding included |
///
/// A value longer than `N` bytes returns [`FixedEncodeError::TooLong`].
///
/// The padding must be exactly one byte: a longer pattern fails to compile.
///
/// ```compile_fail,E0080
/// use fiole::codec::{Encode, Fixed, PadRightStripped, Pattern, Str};
///
/// enum TwoBytes {}
///
/// impl Pattern for TwoBytes {
///     const PATTERN: &'static [u8] = b"ab";
/// }
///
/// Fixed::<8, Str, PadRightStripped<TwoBytes>>::encode_alloc("hello").unwrap();
/// ```
pub struct Fixed<const N: usize, C, P = NoPadding>(PhantomData<(C, P)>);

impl<const N: usize, C: Encode, P: Padding> Encode for Fixed<N, C, P> {
    type Item = C::Item;
    type Error = FixedEncodeError<C::Error>;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        // Reserving the whole slot makes sure padding never reallocates
        let mut into = into.edit();
        into.reserve(N);
        let into = into.make_fresh();

        let start = into.absolute_pos();
        let ret = C::encode(into, item).map_err(FixedEncodeError::Inner)?;
        P::pad(ret, start, N)
    }
}

impl<const N: usize, C: Decode, P: Padding> Decode for Fixed<N, C, P> {
    type Item = C::Item;
    type Error = FixedDecodeError<C::Error>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let remaining = bytes.remaining();
        if remaining < N {
            return Err(FixedDecodeError::MissingBytes {
                expected: N,
                remaining,
            });
        }
        let slot = bytes.take_next(N);
        C::decode(&mut P::strip(slot)).map_err(FixedDecodeError::Inner)
    }
}

impl<const N: usize, C, P> SelfDelimiting for Fixed<N, C, P> {}

mod private {
    pub trait Sealed {}
}

/// How a [`Fixed`] codec pads the values shorter than its size.
///
/// It's implemented by [`NoPadding`], [`PadLeftStripped`], [`PadRightStripped`],
/// [`PadLeftKeep`] and [`PadRightKeep`], and can't be implemented outside of fiole.
pub trait Padding: private::Sealed {
    /// Pads the value written since the absolute position `start` up to `size` bytes.
    #[doc(hidden)]
    fn pad<E>(
        vec: EncodingVec<Fresh>,
        start: usize,
        size: usize,
    ) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>>;

    /// Returns the bytes of the value, out of the `size` bytes of the slot.
    #[doc(hidden)]
    fn strip(slot: DecodingVec) -> DecodingVec;
}

/// The value must fill the whole [`Fixed`] slot. It's the default.
pub enum NoPadding {}

impl private::Sealed for NoPadding {}

impl Padding for NoPadding {
    fn pad<E>(
        vec: EncodingVec<Fresh>,
        start: usize,
        size: usize,
    ) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
        match vec.absolute_pos() - start {
            length if length == size => Ok(vec),
            length => Err(FixedEncodeError::WrongLength {
                expected: size,
                length,
            }),
        }
    }

    fn strip(slot: DecodingVec) -> DecodingVec {
        slot
    }
}

/// The single byte of the padding pattern `P`, checked at compile time.
const fn padding_byte<P: Pattern>() -> u8 {
    const {
        assert!(
            P::PATTERN.len() == 1,
            "The padding pattern must be exactly one byte"
        )
    };
    P::PATTERN[0]
}

/// Checks the value written since `start` fits in `size` bytes, and returns its length.
fn check_length<E>(
    vec: &EncodingVec<Fresh>,
    start: usize,
    size: usize,
) -> Result<usize, FixedEncodeError<E>> {
    match vec.absolute_pos() - start {
        length if length <= size => Ok(length),
        length => Err(FixedEncodeError::TooLong { max: size, length }),
    }
}

/// Inserts the padding before the value written since `start`.
fn pad_left<E>(
    mut vec: EncodingVec<Fresh>,
    start: usize,
    size: usize,
    byte: u8,
) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
    let length = check_length(&vec, start, size)?;
    vec.insert_repeated(start, size - length, byte);
    Ok(vec)
}

/// Appends the padding after the value written since `start`.
fn pad_right<E>(
    vec: EncodingVec<Fresh>,
    start: usize,
    size: usize,
    byte: u8,
) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
    let length = check_length(&vec, start, size)?;
    let mut vec: EncodingVec<Dirty> = vec.edit();
    vec.resize(size - length, byte);
    Ok(vec.make_fresh())
}

/// Pads the values before them, and strips the padding when decoding.
///
/// Decoding strips every padding byte at the start of the slot.
/// /!\ If the value itself starts with the padding byte, those bytes are stripped too.
///
/// ```
/// use fiole::codec::{Bytes, Decode, DecodingVec, Encode, Fixed, PadLeftStripped, U8Pattern};
///
/// type Id = Fixed<4, Bytes, PadLeftStripped<U8Pattern<0xFF>>>;
///
/// let bytes = Id::encode_alloc(&[1, 2]).unwrap().finish();
/// assert_eq!(bytes, [0xFF, 0xFF, 1, 2]);
/// assert_eq!(&*Id::decode(&mut DecodingVec::new(bytes)).unwrap(), [1, 2]);
///
/// // The value can't be told apart from the padding
/// let bytes = Id::encode_alloc(&[0xFF, 1]).unwrap().finish();
/// assert_eq!(&*Id::decode(&mut DecodingVec::new(bytes)).unwrap(), [1]);
/// ```
pub struct PadLeftStripped<P>(PhantomData<P>);

impl<P> private::Sealed for PadLeftStripped<P> {}

impl<P: Pattern> Padding for PadLeftStripped<P> {
    fn pad<E>(
        vec: EncodingVec<Fresh>,
        start: usize,
        size: usize,
    ) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
        pad_left(vec, start, size, padding_byte::<P>())
    }

    fn strip(mut slot: DecodingVec) -> DecodingVec {
        let byte = padding_byte::<P>();
        let padding = slot
            .remaining_slice()
            .iter()
            .take_while(|b| **b == byte)
            .count();
        slot.take_next(padding);
        slot
    }
}

/// Pads the values after them, and strips the padding when decoding.
///
/// /!\ If the value itself ends with the padding byte, those bytes are stripped too.
///
/// With [`Nul`](crate::codec::Nul), it keeps the order of strings and bytes.
///
/// ```
/// use fiole::codec::{Decode, DecodingVec, Encode, Fixed, Nul, PadRightStripped, Str};
///
/// type Name = Fixed<8, Str, PadRightStripped<Nul>>;
///
/// let bytes = Name::encode_alloc("tamo").unwrap().finish();
/// assert_eq!(bytes, b"tamo\0\0\0\0");
/// assert_eq!(&*Name::decode(&mut DecodingVec::new(bytes)).unwrap(), "tamo");
///
/// // Shorter strings sort first, like without padding
/// assert!(Name::encode_alloc("tam").unwrap().finish() < Name::encode_alloc("tamo").unwrap().finish());
/// ```
pub struct PadRightStripped<P>(PhantomData<P>);

impl<P> private::Sealed for PadRightStripped<P> {}

impl<P: Pattern> Padding for PadRightStripped<P> {
    fn pad<E>(
        vec: EncodingVec<Fresh>,
        start: usize,
        size: usize,
    ) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
        pad_right(vec, start, size, padding_byte::<P>())
    }

    fn strip(mut slot: DecodingVec) -> DecodingVec {
        let byte = padding_byte::<P>();
        let bytes = slot.remaining_slice();
        let padding = bytes.iter().rev().take_while(|b| **b == byte).count();
        let length = bytes.len() - padding;
        slot.take_next(length)
    }
}

/// Pads the values before them, and gives the padding to the inner codec when decoding.
///
/// The inner codec must decode its value with the padding in front of it.
/// That's the case of the numbers in [`Text`](crate::codec::Text) padded with
/// `'0'`, and it makes them sort like the numbers, as long as they aren't negative.
///
/// ```
/// use fiole::codec::{CharPattern, Decode, DecodingVec, Encode, Fixed, PadLeftKeep, Text};
///
/// type Number = Fixed<4, Text<u16>, PadLeftKeep<CharPattern<'0'>>>;
///
/// let bytes = Number::encode_alloc(&7).unwrap().finish();
/// assert_eq!(bytes, b"0007");
/// assert_eq!(Number::decode(&mut DecodingVec::new(bytes)).unwrap(), 7);
///
/// assert!(Number::encode_alloc(&9).unwrap().finish() < Number::encode_alloc(&10).unwrap().finish());
/// ```
pub struct PadLeftKeep<P>(PhantomData<P>);

impl<P> private::Sealed for PadLeftKeep<P> {}

impl<P: Pattern> Padding for PadLeftKeep<P> {
    fn pad<E>(
        vec: EncodingVec<Fresh>,
        start: usize,
        size: usize,
    ) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
        pad_left(vec, start, size, padding_byte::<P>())
    }

    fn strip(slot: DecodingVec) -> DecodingVec {
        slot
    }
}

/// Pads the values after them, and gives the padding to the inner codec when decoding.
///
/// The inner codec should be [`SelfDelimiting`], like `SerdeMsgpack` or
/// [`Terminated`](crate::codec::Terminated): it stops at the end of its value,
/// and the padding after it is ignored. A final codec would decode the padding as part of its value.
///
/// ```
/// use fiole::codec::{Decode, DecodingVec, Encode, Fixed, PadRightKeep, Str, Terminated, U8Pattern};
///
/// type Name = Fixed<8, Terminated<Str>, PadRightKeep<U8Pattern<b' '>>>;
///
/// let bytes = Name::encode_alloc("tamo").unwrap().finish();
/// assert_eq!(bytes, b"tamo\0   ");
/// assert_eq!(&*Name::decode(&mut DecodingVec::new(bytes)).unwrap(), "tamo");
/// ```
pub struct PadRightKeep<P>(PhantomData<P>);

impl<P> private::Sealed for PadRightKeep<P> {}

impl<P: Pattern> Padding for PadRightKeep<P> {
    fn pad<E>(
        vec: EncodingVec<Fresh>,
        start: usize,
        size: usize,
    ) -> Result<EncodingVec<Fresh>, FixedEncodeError<E>> {
        pad_right(vec, start, size, padding_byte::<P>())
    }

    fn strip(slot: DecodingVec) -> DecodingVec {
        slot
    }
}

#[derive(Debug)]
pub enum FixedEncodeError<E> {
    /// The inner codec wrote `length` bytes instead of `expected`, without padding.
    WrongLength {
        expected: usize,
        length: usize,
    },
    /// The inner codec wrote `length` bytes, more than the `max` bytes of the slot.
    TooLong {
        max: usize,
        length: usize,
    },
    Inner(E),
}

impl<E: fmt::Display> fmt::Display for FixedEncodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixedEncodeError::WrongLength { expected, length } => write!(
                f,
                "Fixed codec can only encode a value of {expected} bytes, got {length} bytes"
            ),
            FixedEncodeError::TooLong { max, length } => write!(
                f,
                "Fixed codec can only encode a value of at most {max} bytes, got {length} bytes"
            ),
            FixedEncodeError::Inner(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for FixedEncodeError<E> {}

#[derive(Debug)]
pub enum FixedDecodeError<E> {
    /// Only `remaining` bytes were left instead of `expected`.
    MissingBytes {
        expected: usize,
        remaining: usize,
    },
    Inner(E),
}

impl<E: fmt::Display> fmt::Display for FixedDecodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FixedDecodeError::MissingBytes {
                expected,
                remaining,
            } => write!(
                f,
                "Fixed codec expected {expected} bytes, only {remaining} bytes are left"
            ),
            FixedDecodeError::Inner(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for FixedDecodeError<E> {}
#[cfg(test)]
mod test {
    use byteorder::BE;

    use arbitrary::Unstructured;

    use crate::codec::{
        Bytes, CharPattern, ComposeCodec, Decode, DecodingVec, Encode, Fixed, FixedDecodeError,
        FixedEncodeError, Nul, OrderedU32, PadLeftKeep, PadLeftStripped, PadRightKeep,
        PadRightStripped, Str, Terminated, Text, U8, U8Pattern, U16, U32, check_order_preserving,
        check_order_preserving_with, check_round_trip, check_self_delimiting,
        check_self_delimiting_with,
    };

    type Number = Fixed<5, Text<u16>, PadLeftKeep<CharPattern<'0'>>>;
    type Name = Fixed<8, Str, PadRightStripped<Nul>>;
    type Id = Fixed<8, Bytes, PadLeftStripped<U8Pattern<0xFF>>>;

    /// A string of at most 8 bytes, to fit in a [`Name`].
    fn short_string(u: &mut Unstructured) -> arbitrary::Result<String> {
        let s: String = u.arbitrary()?;
        let mut short = String::new();
        for c in s.chars() {
            if short.len() + c.len_utf8() > 8 {
                break;
            }
            short.push(c);
        }
        Ok(short)
    }

    /// Same as [`short_string`], without `\0` at the end, which would be stripped with the padding.
    fn short_string_without_trailing_nul(u: &mut Unstructured) -> arbitrary::Result<String> {
        Ok(short_string(u)?.trim_end_matches('\0').to_string())
    }

    /// Bytes without `0xFF` at the start, which would be stripped with the padding of an [`Id`].
    fn bytes_without_leading_ff(u: &mut Unstructured) -> arbitrary::Result<Vec<u8>> {
        let bytes: Vec<u8> = u.arbitrary()?;
        Ok(bytes.into_iter().skip_while(|b| *b == 0xFF).collect())
    }

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Fixed<4, U32<BE>>, u32>);
        arbtest::arbtest(check_round_trip::<Fixed<4, OrderedU32>, u32>);
    }

    #[test]
    fn writes_nothing_more_than_the_inner_codec() {
        let bytes = Fixed::<3, Text<u16>>::encode_alloc(&123).unwrap().finish();
        assert_eq!(bytes, b"123");
    }

    #[test]
    fn refuses_another_length() {
        for (item, length) in [(12, 2), (1234, 4)] {
            let error = Fixed::<3, Text<u16>>::encode_alloc(&item).unwrap_err();
            assert!(matches!(
                error,
                FixedEncodeError::WrongLength { expected: 3, length: found } if found == length
            ));
        }
    }

    #[test]
    fn reads_exactly_its_bytes() {
        let mut decoder = DecodingVec::new(b"1234");
        assert_eq!(Fixed::<3, Text<u16>>::decode(&mut decoder).unwrap(), 123);
        assert_eq!(decoder.remaining_slice(), b"4");

        let error = Fixed::<3, Text<u16>>::decode(&mut DecodingVec::new(b"12")).unwrap_err();
        assert!(matches!(
            error,
            FixedDecodeError::MissingBytes {
                expected: 3,
                remaining: 2
            }
        ));
    }

    #[test]
    fn can_be_followed_in_a_compose() {
        type Codec = ComposeCodec<(Fixed<3, Text<u16>>, U8)>;
        let bytes = Codec::encode_alloc(&(123, 4)).unwrap().finish();
        assert_eq!(bytes, b"123\x04");
        assert_eq!(
            Codec::decode(&mut DecodingVec::new(bytes)).unwrap(),
            (123, 4)
        );
    }

    #[test]
    fn keeps_the_order_of_the_inner_codec() {
        let encode = |n: u32| Fixed::<4, OrderedU32>::encode_alloc(&n).unwrap().finish();
        assert!(encode(1) < encode(2));
        assert!(encode(255) < encode(256));
    }

    #[test]
    fn decodes_without_copying() {
        crate::test_alloc::assert_decodes_without_allocating::<Fixed<4, U32<BE>>>(&42);
        // Longer than the 20 bytes stored inline, so stripping must share the bytes
        crate::test_alloc::assert_decodes_without_allocating::<Fixed<32, Str, PadRightStripped<Nul>>>(
            "a string longer than 20 bytes",
        );
    }

    #[test]
    fn pad_left_keep_writes_the_padding_before() {
        assert_eq!(Number::encode_alloc(&0).unwrap().finish(), b"00000");
        assert_eq!(Number::encode_alloc(&42).unwrap().finish(), b"00042");
        assert_eq!(Number::encode_alloc(&u16::MAX).unwrap().finish(), b"65535");
        // The inner codec gets the padding
        assert_eq!(Number::decode(&mut DecodingVec::new(b"00042")).unwrap(), 42);
        assert_eq!(Number::decode(&mut DecodingVec::new(b"00000")).unwrap(), 0);
    }

    #[test]
    fn pad_right_stripped_writes_the_padding_after() {
        assert_eq!(
            Name::encode_alloc("tamo").unwrap().finish(),
            b"tamo\0\0\0\0"
        );
        assert_eq!(Name::encode_alloc("").unwrap().finish(), [0; 8]);
        assert_eq!(
            Name::encode_alloc("12345678").unwrap().finish(),
            b"12345678"
        );
        // The inner codec doesn't get the padding
        let decoded = Name::decode(&mut DecodingVec::new(b"tamo\0\0\0\0")).unwrap();
        assert_eq!(&*decoded, "tamo");
        let decoded = Name::decode(&mut DecodingVec::new([0; 8])).unwrap();
        assert_eq!(&*decoded, "");
    }

    #[test]
    fn pad_left_stripped_writes_the_padding_before() {
        assert_eq!(
            Id::encode_alloc(&[1, 2]).unwrap().finish(),
            [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 1, 2]
        );
        assert_eq!(Id::encode_alloc(&[]).unwrap().finish(), [0xFF; 8]);
        // The padding byte can appear anywhere but at the start of the value
        let bytes = Id::encode_alloc(&[1, 0xFF]).unwrap().finish();
        assert_eq!(bytes, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 1, 0xFF]);
        assert_eq!(
            &*Id::decode(&mut DecodingVec::new(bytes)).unwrap(),
            [1, 0xFF]
        );
        assert!(
            Id::decode(&mut DecodingVec::new([0xFF; 8]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn pad_right_keep_gives_the_padding_to_the_inner_codec() {
        type Codec = Fixed<8, Terminated<Str>, PadRightKeep<U8Pattern<b' '>>>;

        let bytes = Codec::encode_alloc("tamo").unwrap().finish();
        assert_eq!(bytes, b"tamo\0   ");
        assert_eq!(
            &*Codec::decode(&mut DecodingVec::new(bytes)).unwrap(),
            "tamo"
        );

        // A final inner codec decodes the padding as part of its value
        type Final = Fixed<8, Str, PadRightKeep<U8Pattern<b' '>>>;
        let bytes = Final::encode_alloc("tamo").unwrap().finish();
        assert_eq!(
            &*Final::decode(&mut DecodingVec::new(bytes)).unwrap(),
            "tamo    "
        );
    }

    #[test]
    fn padding_can_be_followed_in_a_compose() {
        type Codec<'a> = ComposeCodec<(Number, &'a Name, &'a Id, U8)>;

        let item = (42, "tamo", &[1, 2][..], 7);
        let bytes = Codec::encode_alloc(&item).unwrap().finish();
        assert_eq!(bytes.len(), 5 + 8 + 8 + 1);
        let (number, name, id, n) = Codec::decode(&mut DecodingVec::new(bytes)).unwrap();
        assert_eq!((number, &*name, &*id, n), (42, "tamo", &[1, 2][..], 7));
    }

    #[test]
    fn padding_refuses_a_value_longer_than_the_slot() {
        let error =
            Fixed::<2, Text<u16>, PadLeftKeep<CharPattern<'0'>>>::encode_alloc(&123).unwrap_err();
        assert!(matches!(
            error,
            FixedEncodeError::TooLong { max: 2, length: 3 }
        ));
        let error = Name::encode_alloc("123456789").unwrap_err();
        assert!(matches!(
            error,
            FixedEncodeError::TooLong { max: 8, length: 9 }
        ));
    }

    #[test]
    fn stripped_padding_takes_the_padding_bytes_of_the_value_with_it() {
        // The value's own `\0` at the end is stripped with the padding
        let bytes = Name::encode_alloc("tamo\0").unwrap().finish();
        assert_eq!(bytes, b"tamo\0\0\0\0");
        assert_eq!(
            &*Name::decode(&mut DecodingVec::new(bytes)).unwrap(),
            "tamo"
        );
        // But the padding byte is kept anywhere else
        let bytes = Name::encode_alloc("\0tamo").unwrap().finish();
        assert_eq!(
            &*Name::decode(&mut DecodingVec::new(bytes)).unwrap(),
            "\0tamo"
        );

        let bytes = Id::encode_alloc(&[0xFF, 1]).unwrap().finish();
        assert_eq!(&*Id::decode(&mut DecodingVec::new(bytes)).unwrap(), [1]);
    }

    #[test]
    fn padding_needs_the_whole_slot_to_decode() {
        let error = Name::decode(&mut DecodingVec::new(b"tamo\0")).unwrap_err();
        assert!(matches!(
            error,
            FixedDecodeError::MissingBytes {
                expected: 8,
                remaining: 5
            }
        ));
    }

    #[test]
    fn padding_properties() {
        arbtest::arbtest(check_self_delimiting::<Number, u16>);
        arbtest::arbtest(|u| {
            check_self_delimiting_with::<Name, String>(u, short_string_without_trailing_nul)
        });
        arbtest::arbtest(|u| {
            check_self_delimiting_with::<Id, Vec<u8>>(u, bytes_without_leading_ff)
        });
        arbtest::arbtest(check_self_delimiting::<Fixed<3, U16<BE>, PadRightKeep<Nul>>, u16>);

        arbtest::arbtest(check_order_preserving::<Number, u16>);
        arbtest::arbtest(|u| {
            check_order_preserving_with::<Name, String>(u, short_string_without_trailing_nul)
        });
    }
}
