//! `#[derive(Fiole)]`, from the point of view of its users: every struct is
//! written exactly like the tuple of the codecs of its fields.

use std::borrow::Borrow;
use std::convert::Infallible;
use std::fmt::{self, Debug};

use arbitrary::{Arbitrary, Unstructured};
use byteview::StrView;

use crate::codec::{
    Decode, DecodingVec, Encode, Fixed, Optional, OrderedU32, Preceded, SelfDelimiting, Split, Str,
    Terminated, TerminatedDecodeError, Text, TextDecodeError, TupleError2, U8, check_round_trip,
    check_round_trip_with, check_self_delimiting,
};
use crate::{Database, Fiole, Keyspace, KeyspaceCreateOptions};

/// The bytes of `item` as `C` writes them.
fn encoded<C: Encode<Error: Debug>>(item: &C::Item) -> Vec<u8> {
    C::encode_alloc(item).unwrap().finish()
}

fn decoded<C: Decode<Error: Debug>>(bytes: &[u8]) -> C::Item {
    let mut bytes = DecodingVec::new(bytes);
    let item = C::decode(&mut bytes).unwrap();
    assert!(bytes.is_empty(), "the bytes weren't all read");
    item
}

fn assert_self_delimiting<C: SelfDelimiting>() {}

fn assert_split<C: Split<Head = Head, Tail = Tail>, Head, Tail>() {}

/// What a log is filed under, like in the server fiole was made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Arbitrary, Fiole)]
struct LogKey {
    #[fiole(codec = Terminated<Text<u64>>)]
    received_at: u64,
    #[fiole(codec = Fixed<4, Text<u32>>)]
    device: u32,
    #[fiole(codec = Optional<Preceded<Text<u16>>>)]
    location: Option<u16>,
}

type LogKeyTuple = (
    Terminated<Text<u64>>,
    Fixed<4, Text<u32>>,
    Optional<Preceded<Text<u16>>>,
);

#[test]
fn a_struct_is_written_like_the_tuple_of_its_fields() {
    for location in [None, Some(7)] {
        let key = LogKey {
            received_at: 12,
            device: 1234,
            location,
        };
        let bytes = encoded::<LogKey>(&key);
        assert_eq!(bytes, encoded::<LogKeyTuple>(&(12, 1234, location)));
        assert_eq!(decoded::<LogKey>(&bytes), key);
    }
    let key = LogKey {
        received_at: 12,
        device: 1234,
        location: Some(7),
    };
    assert_eq!(encoded::<LogKey>(&key), b"12\x001234\x007");
}

#[test]
fn its_errors_are_the_ones_of_the_tuple() {
    let error = LogKey::decode(&mut DecodingVec::new(b"12")).unwrap_err();
    assert!(matches!(
        error,
        crate::codec::TupleError3::C1(TerminatedDecodeError::MissingTerminator)
    ));
}

/// Named codec, like `RoaringBitmapCodec`: the struct stays a plain struct.
#[derive(Debug, PartialEq, Fiole)]
#[fiole(name = StoredDeviceCodec)]
struct StoredDevice {
    #[fiole(codec = Fixed<3, Text<u16>>)]
    owner: u16,
    #[fiole(codec = Optional<Preceded<Text<u16>>>)]
    location: Option<u16>,
}

#[test]
fn a_named_codec_is_written_like_the_struct() {
    let device = StoredDevice {
        owner: 123,
        location: Some(4),
    };
    let bytes = encoded::<StoredDeviceCodec>(&device);
    assert_eq!(bytes, b"123\x004");
    assert_eq!(decoded::<StoredDeviceCodec>(&bytes), device);
}

/// Generic through a codec.
#[derive(Debug, PartialEq, Fiole)]
struct Owned<T> {
    #[fiole(codec = Terminated<Text<u32>>)]
    owner: u32,
    #[fiole(codec = Text<T>)]
    id: T,
}

/// Generic and named.
#[derive(Debug, PartialEq, Fiole)]
#[fiole(name = PairCodec)]
struct Pair<A, B> {
    #[fiole(codec = Terminated<Text<A>>)]
    first: A,
    #[fiole(codec = Text<B>)]
    second: B,
}

#[test]
fn generics_go_through_the_codecs_of_the_fields() {
    let owned = Owned { owner: 1, id: 'x' };
    let bytes = encoded::<Owned<char>>(&owned);
    assert_eq!(bytes, b"1\0x");
    assert_eq!(decoded::<Owned<char>>(&bytes), owned);

    let pair = Pair {
        first: 'a',
        second: 2u8,
    };
    let bytes = encoded::<PairCodec<char, u8>>(&pair);
    assert_eq!(bytes, b"a\x002");
    assert_eq!(decoded::<PairCodec<char, u8>>(&bytes), pair);
}

/// A field whose codec is a generic, given like any other codec.
#[derive(Debug, PartialEq, Fiole)]
struct Logged<T> {
    #[fiole(codec = Terminated<Text<u64>>)]
    at: u64,
    #[fiole(codec = T)]
    inner: T,
}

/// A nested struct that knows where it ends, in the middle of another one.
#[derive(Debug, Clone, Copy, PartialEq, Arbitrary, Fiole)]
struct Point {
    #[fiole(codec = U8)]
    x: u8,
    #[fiole(codec = U8)]
    y: u8,
}

#[derive(Debug, PartialEq, Arbitrary, Fiole)]
struct Segment {
    #[fiole(codec = Point)]
    from: Point,
    #[fiole(codec = Point)]
    to: Point,
    #[fiole(codec = Text<String>)]
    name: String,
}

#[test]
fn a_struct_can_hold_another_one() {
    let logged = Logged {
        at: 3,
        inner: LogKey {
            received_at: 12,
            device: 1234,
            location: None,
        },
    };
    let bytes = encoded::<Logged<LogKey>>(&logged);
    assert_eq!(bytes, b"3\x0012\x001234");
    assert_eq!(decoded::<Logged<LogKey>>(&bytes), logged);

    // `Point` knows where it ends, so it can be followed by anything
    assert_self_delimiting::<Point>();
    let segment = Segment {
        from: Point { x: 1, y: 2 },
        to: Point { x: 3, y: 4 },
        name: "diagonal".to_string(),
    };
    let bytes = encoded::<Segment>(&segment);
    assert_eq!(bytes, b"\x01\x02\x03\x04diagonal");
    assert_eq!(decoded::<Segment>(&bytes), segment);
}

#[test]
fn a_struct_ending_with_a_final_codec_can_only_be_last() {
    // `Segment` ends with `Text<String>`, which reads till the end: it isn't
    // self-delimiting, but it's still a codec, last in a tuple of codecs.
    // `tests/ui/final_struct_in_the_middle_of_a_tuple.rs` checks it can't be anywhere else.
    let segment = Segment {
        from: Point { x: 1, y: 2 },
        to: Point { x: 3, y: 4 },
        name: "diagonal".to_string(),
    };
    let item = (7, segment);
    let bytes = encoded::<(U8, Segment)>(&item);
    assert_eq!(bytes, b"\x07\x01\x02\x03\x04diagonal");
    assert_eq!(decoded::<(U8, Segment)>(&bytes), item);
}

/// Generic with its own `where` clause: the impls must keep it, or `Bounded<T>`
/// wouldn't be well-formed in them.
#[derive(Debug, PartialEq, Fiole)]
struct Bounded<T>
where
    T: Copy,
{
    #[fiole(codec = Terminated<Text<T>>)]
    first: T,
    #[fiole(codec = Terminated<Text<T>>)]
    second: T,
}

/// Same for the codec type it declares.
#[derive(Debug, PartialEq, Fiole)]
#[fiole(name = BoundedPairCodec)]
struct BoundedPair<A, B>
where
    A: Copy,
    B: Copy + Default,
{
    #[fiole(codec = Terminated<Text<A>>)]
    first: A,
    #[fiole(codec = Text<B>)]
    second: B,
}

#[test]
fn the_where_clause_of_the_struct_is_kept() {
    let bounded = Bounded {
        first: 'a',
        second: 'b',
    };
    let bytes = encoded::<Bounded<char>>(&bounded);
    assert_eq!(bytes, b"a\0b\0");
    assert_eq!(decoded::<Bounded<char>>(&bytes), bounded);
    assert_self_delimiting::<Bounded<char>>();
    assert_split::<Bounded<char>, Terminated<Text<char>>, Terminated<Text<char>>>();

    let pair = BoundedPair {
        first: 'a',
        second: 2u8,
    };
    let bytes = encoded::<BoundedPairCodec<char, u8>>(&pair);
    assert_eq!(bytes, b"a\x002");
    assert_eq!(decoded::<BoundedPairCodec<char, u8>>(&bytes), pair);
    assert_split::<BoundedPairCodec<char, u8>, Terminated<Text<char>>, Text<u8>>();
}

#[derive(Debug, PartialEq, Arbitrary, Fiole)]
struct Tuple(
    #[fiole(codec = Terminated<Text<u8>>)] u8,
    #[fiole(codec = OrderedU32)] u32,
);

#[test]
fn tuple_structs_too() {
    let bytes = encoded::<Tuple>(&Tuple(7, 1));
    assert_eq!(bytes, b"7\0\0\0\0\x01");
    assert_eq!(decoded::<Tuple>(&bytes), Tuple(7, 1));
}

/// A field converts from and to the item of its codec.
#[derive(Debug, PartialEq, Fiole)]
struct Named {
    #[fiole(codec = Terminated<Str>)]
    name: StrView,
    #[fiole(codec = U8)]
    age: u8,
}

#[test]
fn fields_borrow_and_convert_the_items_of_their_codecs() {
    let named = Named {
        name: StrView::from("kefir"),
        age: 7,
    };
    let bytes = encoded::<Named>(&named);
    assert_eq!(bytes, b"kefir\0\x07");
    assert_eq!(decoded::<Named>(&bytes), named);
}

/// The user's own error, built from the error of every field.
#[derive(Debug, PartialEq)]
enum KeyError {
    MissingSeparator,
    NotANumber,
}

impl From<Infallible> for KeyError {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

impl<E> From<TerminatedDecodeError<E>> for KeyError {
    fn from(error: TerminatedDecodeError<E>) -> Self {
        match error {
            TerminatedDecodeError::MissingTerminator => KeyError::MissingSeparator,
            TerminatedDecodeError::Inner(_) => KeyError::NotANumber,
        }
    }
}

impl From<crate::codec::TerminatedEncodeError<Infallible>> for KeyError {
    fn from(_: crate::codec::TerminatedEncodeError<Infallible>) -> Self {
        KeyError::MissingSeparator
    }
}

impl<E> From<TextDecodeError<E>> for KeyError {
    fn from(_: TextDecodeError<E>) -> Self {
        KeyError::NotANumber
    }
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, PartialEq, Fiole)]
#[fiole(error = KeyError)]
struct Checked {
    #[fiole(codec = Terminated<Text<u8>>)]
    first: u8,
    #[fiole(codec = Text<u8>)]
    second: u8,
}

#[test]
fn the_error_can_be_the_users() {
    let bytes = encoded::<Checked>(&Checked {
        first: 1,
        second: 2,
    });
    assert_eq!(
        decoded::<Checked>(&bytes),
        Checked {
            first: 1,
            second: 2
        }
    );
    let error = Checked::decode(&mut DecodingVec::new(b"1")).unwrap_err();
    assert_eq!(error, KeyError::MissingSeparator);
    let error = Checked::decode(&mut DecodingVec::new(b"1\0x")).unwrap_err();
    assert_eq!(error, KeyError::NotANumber);
}

#[test]
fn a_struct_is_split_like_its_tuple() {
    let folder = tempfile::tempdir().unwrap();
    let db = Database::builder(folder.path()).unwrap();
    let logs: Keyspace<LogKey, Str> = db
        .keyspace::<LogKey, Str>("logs", KeyspaceCreateOptions::default)
        .unwrap()
        .into();
    let mut wtxn = db.write_tx().unwrap();
    logs.insert(
        &mut wtxn,
        &LogKey {
            received_at: 1,
            device: 1000,
            location: None,
        },
        "a",
    )
    .unwrap();
    logs.insert(
        &mut wtxn,
        &LogKey {
            received_at: 2,
            device: 2000,
            location: Some(7),
        },
        "b",
    )
    .unwrap();
    logs.insert(
        &mut wtxn,
        &LogKey {
            received_at: 2,
            device: 3000,
            location: None,
        },
        "c",
    )
    .unwrap();

    // The logs received at 2, keyed by the rest of the struct
    let at_2 = logs.prefixed(&2).unwrap();
    let rest: Vec<(u32, Option<u16>)> =
        at_2.iter(&wtxn).map(|guard| guard.key().unwrap()).collect();
    assert_eq!(rest, [(2000, Some(7)), (3000, None)]);
}

#[test]
fn decodes_without_copying() {
    crate::test_alloc::assert_decodes_without_allocating::<Point>(&Point { x: 1, y: 2 });
}

#[test]
fn errors_of_two_fields_are_a_tuple_error_of_two() {
    let error = Owned::<char>::decode(&mut DecodingVec::new(b"1")).unwrap_err();
    assert!(matches!(
        error,
        TupleError2::C1(TerminatedDecodeError::MissingTerminator)
    ));
}

#[test]
fn structs_survive_a_round_trip() {
    arbtest::arbtest(check_self_delimiting::<Point, Point>);
    arbtest::arbtest(check_self_delimiting::<Tuple, Tuple>);
    arbtest::arbtest(check_round_trip::<Segment, Segment>);
    // `Fixed<4, _>` refuses the devices that aren't 4 digits long
    arbtest::arbtest(|u| {
        check_round_trip_with::<LogKey, LogKey>(u, |u| {
            Ok(LogKey {
                device: u.int_in_range(1000..=9999)?,
                ..u.arbitrary()?
            })
        })
    });
}

/// An email, compared without its case.
#[derive(Debug, PartialEq)]
struct Email(String);

impl Borrow<String> for Email {
    fn borrow(&self) -> &String {
        &self.0
    }
}

impl From<String> for Email {
    fn from(email: String) -> Self {
        Email(email.to_lowercase())
    }
}

#[derive(Debug, PartialEq, Fiole)]
struct Account {
    #[fiole(codec = Text<String>)]
    email: Email,
}

/// A field is written through `Borrow` and read back through `From`, and
/// nothing ties the two: a `From` changing the value breaks the round-trip,
/// which the derive can't see but the property check does.
#[test]
#[should_panic(
    expected = "Decoded Account { email: Email(\"kefir@fiole.rs\") } instead of Account { email: Email(\"Kefir@fiole.rs\") }"
)]
fn a_from_changing_the_field_breaks_the_round_trip() {
    let account = Account {
        email: Email("Kefir@fiole.rs".to_string()),
    };
    // Written as is, read back in lowercase
    assert_eq!(encoded::<Account>(&account), b"Kefir@fiole.rs");
    let mut u = Unstructured::new(&[]);
    check_round_trip_with::<Account, Account>(&mut u, |_| Ok(account)).unwrap();
}
