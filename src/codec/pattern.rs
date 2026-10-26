//! Fixed sequences of bytes used by codecs like [`Constant`](crate::codec::Constant)
//! and [`Terminated`](crate::codec::Terminated).

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

/// A single `0x00` byte. As the terminator of a [`Terminated`](crate::codec::Terminated)
/// codec, it preserves the order of the inner codec.
pub type Nul = U8Pattern<0>;
