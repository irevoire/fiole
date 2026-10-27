//! The derive macros of [fiole](https://docs.rs/fiole), re-exported by it
//! with its `derive` feature.

mod attrs;
mod expand;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// Makes codec from structures annotation, written using the tuple codec with the
/// codecs defined for all fields.
///
/// Every fields have to define their codec with `#[fiole(codec = ...)]`, and the fields are written
/// one after the other in the order they're declared.
///
/// ```
/// use fiole::Fiole;
/// use fiole::codec::{Decode, DecodingVec, Encode, Fixed, Optional, Preceded, Terminated, Text};
///
/// /// A log, by when it was received, its device, then its location if it has one.
/// #[derive(Debug, PartialEq, Fiole)]
/// struct LogKey {
///     #[fiole(codec = Terminated<Text<u64>>)]
///     received_at: u64,
///     #[fiole(codec = Fixed<4, Text<u32>>)]
///     device: u32,
///     #[fiole(codec = Optional<Preceded<Text<u16>>>)]
///     location: Option<u16>,
/// }
///
/// let key = LogKey { received_at: 12, device: 1234, location: Some(7) };
/// let bytes = LogKey::encode_alloc(&key).unwrap().finish();
/// assert_eq!(bytes, b"12\x001234\x007");
/// assert_eq!(LogKey::decode(&mut DecodingVec::new(bytes)).unwrap(), key);
/// ```
///
/// # The codec
///
/// By default the struct is its own codec: `Keyspace<LogKey, V>` is a
/// keyspace of `LogKey`s. `#[fiole(name = ...)]` on the struct generates a
/// separate codec type instead, with the visibility and the generics of the
/// struct, like the `RoaringBitmapCodec` of a `RoaringBitmap`:
///
/// ```
/// use fiole::Fiole;
/// use fiole::codec::{Encode, Text, U8};
///
/// #[derive(Fiole)]
/// #[fiole(name = DeviceCodec)]
/// struct Device {
///     #[fiole(codec = U8)]
///     kind: u8,
///     #[fiole(codec = Text<u32>)]
///     serial: u32,
/// }
///
/// let bytes = DeviceCodec::encode_alloc(&Device { kind: 1, serial: 42 }).unwrap().finish();
/// assert_eq!(bytes, b"\x0142");
/// ```
///
/// What's generated for it:
///
/// - [`Encode`] and [`Decode`], whose item is the struct. Each field is
///   written through its own codec, without being copied.
/// - [`SelfDelimiting`] when the codec of its last field is: the codecs of
///   the other fields must be anyway. A struct that knows where it ends can
///   be a field of another one, or an element of a tuple of codecs.
/// - [`Split`] when it has two fields or more, like a tuple: the codec of the
///   first field, then the tuple of the codecs of the others.
///   [`Keyspace::prefixed`] gives the keyspace of the structs with a given
///   first field, keyed by the tuple of their other fields.
///
/// # The fields
///
/// `#[fiole(codec = ...)]` describes the codec a field should be encoded with.
/// The type of the field must match type of the items the codec writes, or [`Borrow`].
/// It must also matches the type of the items the codec reads, or can be made [`From`].
/// For example, a [`StrView`] can be written with `Terminated<Str>`, which writes a
/// `str` and reads a `StrView`.
/// A `String` can't, but it can be written with `Text<String>`.
///
/// The derive doesn't give you any guarantee on the generated codecs. It is heavily
/// recommended to tests your codecs yourself with the provided helpers, see the "Testing your
/// codecs" part of the [crate documentation](https://docs.rs/fiole/latest/fiole/#testing-your-codecs).
///
/// Every field must specify its codec. Fiole never assume anything implicitely.
/// Every field but the last must know where it ends, as in a tuple of codecs:
///
/// ```compile_fail
/// use fiole::Fiole;
/// use fiole::codec::{Text, U8};
///
/// #[derive(Fiole)]
/// struct Key {
///     // `Text<u32>` decodes everything till the end.
///     // Swap the order of the field, wrap it in a `SizedCodec`, `Terminated` or `Fixed` to make it compile
///     #[fiole(codec = Text<u32>)]
///     first: u32,
///     #[fiole(codec = U8)]
///     second: u8,
/// }
/// ```
///
/// Generics are handled like any type:
///
/// ```
/// use fiole::Fiole;
/// use fiole::codec::{Decode, DecodingVec, Encode, Terminated, Text, U8};
///
/// #[derive(Debug, PartialEq, Fiole)]
/// struct Owned<T> {
///     #[fiole(codec = Terminated<Text<u32>>)]
///     owner: u32,
///     #[fiole(codec = Text<T>)]
///     id: T,
/// }
///
/// /// Something with the time it was made at.
/// #[derive(Debug, PartialEq, Fiole)]
/// struct Dated<T> {
///     #[fiole(codec = Terminated<Text<u64>>)]
///     at: u64,
///     #[fiole(codec = T)]
///     inner: T,
/// }
///
/// let dated = Dated { at: 3, inner: Owned { owner: 1, id: 'x' } };
/// let bytes = Dated::<Owned<char>>::encode_alloc(&dated).unwrap().finish();
/// assert_eq!(bytes, b"3\x001\0x");
/// assert_eq!(Dated::<Owned<char>>::decode(&mut DecodingVec::new(bytes)).unwrap(), dated);
/// ```
///
/// Named and tuple structs work the same way, from 1 to 19 fields.
///
/// # The errors
///
/// By default, they're the ones of the tuple codec:
/// [`TupleError2`] for a struct of two fields, whose `C1` variant holds the
/// error of the first field. `#[fiole(error = MyError)]` makes them yours,
/// for both [`Encode`] and [`Decode`]: `MyError` must implement `From` the
/// encoding and the decoding errors of every field. Many codecs can't fail to
/// encode, so it often needs a `From<Infallible>` too.
///
/// [`Encode`]: https://docs.rs/fiole/latest/fiole/codec/trait.Encode.html
/// [`Decode`]: https://docs.rs/fiole/latest/fiole/codec/trait.Decode.html
/// [`SelfDelimiting`]: https://docs.rs/fiole/latest/fiole/codec/trait.SelfDelimiting.html
/// [`Split`]: https://docs.rs/fiole/latest/fiole/codec/trait.Split.html
/// [`Keyspace::prefixed`]: https://docs.rs/fiole/latest/fiole/struct.Keyspace.html#method.prefixed
/// [`TupleError2`]: https://docs.rs/fiole/latest/fiole/codec/enum.TupleError2.html
/// [`check_round_trip`]: https://docs.rs/fiole/latest/fiole/codec/fn.check_round_trip.html
/// [`StrView`]: https://docs.rs/byteview/latest/byteview/struct.StrView.html
/// [`Borrow`]: std::borrow::Borrow
#[proc_macro_derive(Fiole, attributes(fiole))]
pub fn derive_fiole(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::codec(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
