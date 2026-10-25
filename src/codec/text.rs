use std::{convert::Infallible, fmt, io::Write, marker::PhantomData, str::FromStr, str::Utf8Error};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

/// Describe a type as its text: encoded through its [`fmt::Display`] implementation, and decoded back by parsing it with its [`FromStr`] implementation.
///
/// The keys are sorted by their text, not by their value: `10` sorts before `9`.
///
/// /!\ This codec is final: it decodes everything till the end, so it can only be the last element of a [`ComposeCodec`](crate::codec::ComposeCodec). To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
///
/// ```
/// use std::net::Ipv4Addr;
///
/// use fiole::codec::{Decode, DecodingVec, Encode, Text, TextDecodeError};
///
/// let home = Ipv4Addr::new(127, 0, 0, 1);
/// let bytes = Text::<Ipv4Addr>::encode_alloc(&home).unwrap().finish();
/// assert_eq!(bytes, b"127.0.0.1");
///
/// let decoded = Text::<Ipv4Addr>::decode(&mut DecodingVec::new(bytes)).unwrap();
/// assert_eq!(decoded, home);
///
/// // What doesn't parse fails with the error of `FromStr`
/// let error = Text::<Ipv4Addr>::decode(&mut DecodingVec::borrowed(b"localhost")).unwrap_err();
/// assert!(matches!(error, TextDecodeError::Parse(_)));
/// ```
///
/// # Panics
///
/// Encoding panics if the [`fmt::Display`] implementation returns an error, like [`ToString::to_string`] does.
pub struct Text<T>(PhantomData<T>);

impl<T: fmt::Display> Encode for Text<T> {
    type Item = T;
    type Error = Infallible;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let mut ret = into.edit();
        // Writing in memory only fails when `Display` does
        write!(ret, "{item}").expect("a Display implementation returned an error unexpectedly");
        Ok(ret.make_fresh())
    }
}

impl<T: FromStr> Decode for Text<T> {
    type Item = T;
    type Error = TextDecodeError<T::Err>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let text = std::str::from_utf8(bytes.remaining_slice()).map_err(TextDecodeError::Utf8)?;
        let item = text.parse().map_err(TextDecodeError::Parse)?;
        bytes.skip_to_end();
        Ok(item)
    }
}

/// Why a [`Text`] couldn't be decoded.
#[derive(Debug)]
pub enum TextDecodeError<E> {
    /// The bytes aren't valid UTF-8.
    Utf8(Utf8Error),
    /// The text doesn't parse, with the error of [`FromStr`].
    Parse(E),
}

impl<E: fmt::Display> fmt::Display for TextDecodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextDecodeError::Utf8(e) => e.fmt(f),
            TextDecodeError::Parse(e) => e.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TextDecodeError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TextDecodeError::Utf8(e) => Some(e),
            TextDecodeError::Parse(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{
        check_round_trip, ComposeCodec, Decode, DecodingVec, Encode, Terminated, Text,
        TextDecodeError, U8,
    };

    #[test]
    fn round_trip() {
        arbtest::arbtest(check_round_trip::<Text<u64>, u64>);
        arbtest::arbtest(check_round_trip::<Text<i32>, i32>);
        arbtest::arbtest(check_round_trip::<Text<char>, char>);
    }

    #[test]
    fn is_written_as_displayed() {
        let bytes = Text::<i32>::encode_alloc(&-42).unwrap().finish();
        assert_eq!(bytes, b"-42");
    }

    #[test]
    fn decodes_without_copying() {
        let mut decoder = DecodingVec::borrowed(b"42");
        assert!(decoder.is_borrowed());
        assert_eq!(Text::<u8>::decode(&mut decoder).unwrap(), 42);
        assert!(decoder.is_empty());
    }

    #[test]
    fn reports_why_it_did_not_decode() {
        let error = Text::<u8>::decode(&mut DecodingVec::borrowed(b"\xFF")).unwrap_err();
        assert!(matches!(error, TextDecodeError::Utf8(_)));

        let error = Text::<u8>::decode(&mut DecodingVec::borrowed(b"256")).unwrap_err();
        assert!(matches!(error, TextDecodeError::Parse(_)));
        assert_eq!(error.to_string(), "number too large to fit in target type");
    }

    #[test]
    fn can_be_terminated_in_a_compose() {
        type Codec = ComposeCodec<(Terminated<Text<u32>>, U8)>;
        let bytes = Codec::encode_alloc(&(1234, 5)).unwrap().finish();
        assert_eq!(bytes, b"1234\0\x05");
        let decoded = Codec::decode(&mut DecodingVec::new(bytes)).unwrap();
        assert_eq!(decoded, (1234, 5));
    }
}
