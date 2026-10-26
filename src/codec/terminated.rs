use std::{fmt, marker::PhantomData};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, Nul, Pattern, SelfDelimiting};

/// Makes any codec [`SelfDelimiting`] by writing a [`Pattern`] after it.
///
/// The format is the bytes of the inner codec `C`, followed by the pattern of
/// `T` (a single `0x00` byte by default). The inner codec is forbidden to
/// produce the pattern: encoding returns
/// [`TerminatedEncodeError::ContainsTerminator`] if it does. For example,
/// `Terminated<Str>` can't encode a string containing `\0`.
///
/// Compared to [`SizedCodec`](crate::codec::SizedCodec), it costs one byte instead
/// of four, decodes without copying, and keeps the order of the inner codec
/// when the pattern is only made of `0x00` bytes (like [`Nul`]) and the inner
/// codec is itself ordered (`Str`, `Bytes`, `OrderedU32`, but not `U32<LE>`).
/// That makes `Terminated<Str>` the codec of choice for ordered string keys,
/// even in the middle of a [`ComposeCodec`](crate::codec::ComposeCodec).
///
/// ```
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, Encode, OrderedU32, Str, Terminated};
///
/// type Key<'a> = ComposeCodec<(&'a Terminated<Str>, OrderedU32)>;
///
/// let bytes = Key::encode_alloc(&("hello", 42)).unwrap().finish();
/// assert_eq!(bytes, b"hello\0\0\0\0\x2a");
/// let (s, n) = Key::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!((&*s, n), ("hello", 42));
///
/// // The order of the strings is kept, whatever follows them
/// let a = Key::encode_alloc(&("a", u32::MAX)).unwrap().finish();
/// let aa = Key::encode_alloc(&("aa", 0)).unwrap().finish();
/// assert!(a < aa);
///
/// // The terminator can't be part of the string
/// assert!(Terminated::<Str>::encode_alloc("a\0b").is_err());
/// ```
///
/// The pattern can be configured, and can be longer than a single byte.
/// When the pattern isn't only made of `0x00` bytes, the codec is still
/// self-delimiting but doesn't keep the order of the inner codec.
///
/// ```
/// use fiole::codec::{Bytes, Decode, DecodingVec, Encode, Pattern, Terminated};
///
/// enum Separator {}
///
/// impl Pattern for Separator {
///     const PATTERN: &'static [u8] = b"\xFF\xFE";
/// }
///
/// type Codec = Terminated<Bytes, Separator>;
///
/// let bytes = Codec::encode_alloc(&[0, 1, 0xFF]).unwrap().finish();
/// assert_eq!(bytes, [0, 1, 0xFF, 0xFF, 0xFE]);
/// assert_eq!(&*Codec::decode(&mut DecodingVec::new(bytes)).unwrap(), [0, 1, 0xFF]);
///
/// // The pattern can't appear anywhere before the end, even partially overlapping the terminator
/// assert!(Codec::encode_alloc(&[1, 0xFF, 0xFE, 2]).is_err());
/// ```
///
/// The pattern can't be empty: using an empty pattern fails to compile.
///
/// ```compile_fail,E0080
/// use fiole::codec::{Bytes, Encode, Pattern, Terminated};
///
/// enum Empty {}
///
/// impl Pattern for Empty {
///     const PATTERN: &'static [u8] = &[];
/// }
///
/// Terminated::<Bytes, Empty>::encode_alloc(&[1, 2]).unwrap();
/// ```
///
/// When used as a key, keep in mind that [`crate::Keyspace::prefix`] encodes
/// the whole prefix, terminator included: with `Terminated<Str>`, the prefix
/// `"a"` only matches the key `"a"` (followed by anything in a compose), not
/// every key starting with `"a"`.
pub struct Terminated<C, T = Nul>(PhantomData<(C, T)>);

impl<C, T> SelfDelimiting for Terminated<C, T> {}

/// Position of the first occurrence of `pattern` in `haystack`.
fn find(haystack: &[u8], pattern: &[u8]) -> Option<usize> {
    match pattern {
        [byte] => haystack.iter().position(|b| b == byte),
        _ => haystack
            .windows(pattern.len())
            .position(|window| window == pattern),
    }
}

impl<C: Encode, T: Pattern> Encode for Terminated<C, T> {
    type Item = C::Item;
    type Error = TerminatedEncodeError<C::Error>;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        const {
            assert!(
                !T::PATTERN.is_empty(),
                "The terminator pattern can't be empty"
            )
        };

        let start = into.absolute_pos();
        let mut ret = C::encode(into, item)
            .map_err(TerminatedEncodeError::Inner)?
            .edit();
        ret.extend(T::PATTERN);

        // The decoder stops at the first occurrence of the pattern, which must be
        // the one we just wrote, even when it overlaps the end of the inner bytes.
        let written = &ret.vec[start..];
        let end = written.len() - T::PATTERN.len();
        match find(written, T::PATTERN) {
            Some(position) if position < end => {
                Err(TerminatedEncodeError::ContainsTerminator { position })
            }
            _ => Ok(ret.make_fresh()),
        }
    }
}

impl<C: Decode, T: Pattern> Decode for Terminated<C, T> {
    type Item = C::Item;
    type Error = TerminatedDecodeError<C::Error>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        const {
            assert!(
                !T::PATTERN.is_empty(),
                "The terminator pattern can't be empty"
            )
        };

        let position = find(bytes.remaining_slice(), T::PATTERN)
            .ok_or(TerminatedDecodeError::MissingTerminator)?;
        let ret = C::decode(&mut bytes.take_next(position)).map_err(TerminatedDecodeError::Inner);
        bytes.take_next(T::PATTERN.len());
        ret
    }
}

#[derive(Debug)]
pub enum TerminatedEncodeError<E> {
    /// The inner codec wrote the terminator pattern, starting at `position` in its own bytes.
    ContainsTerminator {
        position: usize,
    },
    Inner(E),
}

impl<E: fmt::Display> fmt::Display for TerminatedEncodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminatedEncodeError::ContainsTerminator { position } => write!(
                f,
                "Terminated codec can't encode a value containing its terminator pattern, found at byte {position}"
            ),
            TerminatedEncodeError::Inner(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for TerminatedEncodeError<E> {}

#[derive(Debug)]
pub enum TerminatedDecodeError<E> {
    /// The terminator pattern couldn't be found in the remaining bytes.
    MissingTerminator,
    Inner(E),
}

impl<E: fmt::Display> fmt::Display for TerminatedDecodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminatedDecodeError::MissingTerminator => {
                f.write_str("Terminated codec couldn't find its terminator pattern")
            }
            TerminatedDecodeError::Inner(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error> std::error::Error for TerminatedDecodeError<E> {}

#[cfg(test)]
mod test {
    use fjall::KeyspaceCreateOptions;

    use crate::{
        Database,
        codec::{
            Bytes, ComposeCodec, Decode, DecodingVec, Encode, Pattern, Str, Terminated,
            TerminatedDecodeError, TerminatedEncodeError, U8, check_order_preserving,
            check_self_delimiting, check_self_delimiting_with,
        },
    };

    /// A pattern that can overlap with itself.
    enum DoubleA {}

    impl Pattern for DoubleA {
        const PATTERN: &'static [u8] = b"aa";
    }

    /// Generates bytes only made of `a` and `b`, to hit the `aa` pattern often.
    fn a_and_b(u: &mut arbitrary::Unstructured) -> arbitrary::Result<Vec<u8>> {
        let bits: Vec<bool> = u.arbitrary()?;
        Ok(bits
            .into_iter()
            .map(|a| if a { b'a' } else { b'b' })
            .collect())
    }

    #[test]
    fn exact_encoding() {
        let bytes = Terminated::<Str>::encode_alloc("").unwrap().finish();
        assert_eq!(bytes, [0]);
        let bytes = Terminated::<Str>::encode_alloc("ab").unwrap().finish();
        assert_eq!(bytes, [b'a', b'b', 0]);
        let bytes = Terminated::<Bytes, DoubleA>::encode_alloc(b"ab")
            .unwrap()
            .finish();
        assert_eq!(bytes, *b"abaa");
    }

    #[test]
    fn refuses_the_terminator() {
        let err = Terminated::<Str>::encode_alloc("a\0b").unwrap_err();
        assert!(matches!(
            err,
            TerminatedEncodeError::ContainsTerminator { position: 1 }
        ));

        let err = Terminated::<Bytes, DoubleA>::encode_alloc(b"baab").unwrap_err();
        assert!(matches!(
            err,
            TerminatedEncodeError::ContainsTerminator { position: 1 }
        ));

        // `ba` + `aa` would be decoded as `b` because the pattern starts one byte earlier
        let err = Terminated::<Bytes, DoubleA>::encode_alloc(b"ba").unwrap_err();
        assert!(matches!(
            err,
            TerminatedEncodeError::ContainsTerminator { position: 1 }
        ));
    }

    #[test]
    fn missing_terminator() {
        let mut decoder = DecodingVec::new(b"hello");
        assert!(matches!(
            Terminated::<Str>::decode(&mut decoder),
            Err(TerminatedDecodeError::MissingTerminator)
        ));
    }

    #[test]
    fn self_delimiting() {
        arbtest::arbtest(check_self_delimiting::<Terminated<Str>, String>);
        arbtest::arbtest(check_self_delimiting::<Terminated<Bytes>, Vec<u8>>);
        arbtest::arbtest(check_self_delimiting::<Terminated<Bytes, DoubleA>, Vec<u8>>);
        arbtest::arbtest(|u| {
            check_self_delimiting_with::<Terminated<Bytes, DoubleA>, _>(u, a_and_b)
        });
    }

    #[test]
    fn order_preserving() {
        arbtest::arbtest(check_order_preserving::<Terminated<Str>, String>);
        arbtest::arbtest(check_order_preserving::<Terminated<Bytes>, Vec<u8>>);
    }

    #[test]
    fn order_preserving_when_followed_by_other_bytes() {
        arbtest::arbtest(|u| {
            let left: (Vec<u8>, Vec<u8>) = u.arbitrary()?;
            let right: (Vec<u8>, Vec<u8>) = u.arbitrary()?;
            if left.0 == right.0 {
                return Ok(());
            }
            let encode = |(bytes, suffix): &(Vec<u8>, Vec<u8>)| {
                Terminated::<Bytes>::encode_alloc(bytes).map(|vec| {
                    let mut vec = vec.finish();
                    vec.extend(suffix);
                    vec
                })
            };
            if let (Ok(left_bytes), Ok(right_bytes)) = (encode(&left), encode(&right)) {
                assert_eq!(
                    left_bytes.cmp(&right_bytes),
                    left.0.cmp(&right.0),
                    "{left:?} {right:?}"
                );
            }
            Ok(())
        });
    }

    #[test]
    fn ordered_keys_in_a_keyspace() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<ComposeCodec<(&'static Terminated<Str>, U8)>, U8>("hello", || {
                KeyspaceCreateOptions::default()
            })
            .unwrap();

        let mut wtxn = database.write_tx().unwrap();
        for key in [("b", 0), ("aa", 0), ("a", 5), ("ab", 1)] {
            ks.insert(&mut wtxn, &key, &0).unwrap();
        }
        wtxn.commit().unwrap().unwrap();

        let rtxn = database.read_tx();
        let keys: Vec<_> = ks.iter(&rtxn).map(|guard| guard.key().unwrap()).collect();
        let keys: Vec<_> = keys.iter().map(|(s, n)| (&**s, *n)).collect();
        assert_eq!(keys, [("a", 5), ("aa", 0), ("ab", 1), ("b", 0)]);

        let first = ks.first_key_value(&rtxn).unwrap().key().unwrap();
        assert_eq!((&*first.0, first.1), ("a", 5));
    }
}
