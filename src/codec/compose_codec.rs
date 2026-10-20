use std::{
    fmt::{self},
    marker::PhantomData,
};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh, SelfDelimiting};

/// Encode a tuple by writing each element one after the other, with nothing in between.
///
/// Every element except the last must be self-delimiting.
/// Final codecs, which decode everything till the end, can only be used as the last element.
/// To use one elsewhere, wrap it in a [`SizedCodec`](crate::codec::SizedCodec).
///
/// ```
/// use fiole::codec::{ComposeCodec, Decode, Encode, SizedCodec, Str, U8};
///
/// type Codec<'a> = ComposeCodec<(&'a SizedCodec<Str>, U8, &'a Str)>;
/// let buf = Codec::encode_alloc(&("hello", 1, "world")).unwrap();
/// let decoded = Codec::decode(&mut buf.into_decoding_vec()).unwrap();
/// assert_eq!(decoded, ("hello".to_string(), 1, "world".to_string()));
///
/// // A compose made only of self-delimiting codecs can itself be followed by another codec
/// type Nested = ComposeCodec<(ComposeCodec<(U8, U8)>, U8)>;
/// let buf = Nested::encode_alloc(&((1, 2), 3)).unwrap();
/// assert_eq!(Nested::decode(&mut buf.into_decoding_vec()).unwrap(), ((1, 2), 3));
/// ```
///
/// A final codec can't be followed by anything when encoding:
///
/// ```compile_fail,E0599
/// use fiole::codec::{ComposeCodec, Encode, Str, U8};
///
/// ComposeCodec::<(&Str, U8)>::encode_alloc(&("hello", 1)).unwrap();
/// ```
///
/// nor when decoding:
///
/// ```compile_fail,E0599
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, Str, U8};
///
/// ComposeCodec::<(Str, U8)>::decode(&mut DecodingVec::new(vec![b'a', 1])).unwrap();
/// ```
///
/// even inside a nested compose:
///
/// ```compile_fail,E0599
/// use fiole::codec::{ComposeCodec, Decode, DecodingVec, Str, U8};
///
/// ComposeCodec::<(ComposeCodec<(U8, Str)>, U8)>::decode(&mut DecodingVec::new(vec![1, b'a', 2]))
///     .unwrap();
/// ```
pub struct ComposeCodec<T>(PhantomData<T>);

// We keep the following implementation made by hand to help debug the macro below

#[derive(Debug)]
pub enum ComposeCodecError1<C1> {
    C1(C1),
}

impl<C1> ComposeCodecError1<C1> {
    pub fn unwrap_error_1(self) -> C1 {
        #[allow(nonstandard_style)]
        match self {
            Self::C1(C1) => C1,
            #[allow(unused)]
            _other => panic!("Unwrapped the wrong variant"),
        }
    }
}

impl<C1: fmt::Display> fmt::Display for ComposeCodecError1<C1> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(nonstandard_style)]
        match self {
            Self::C1(C1) => C1.fmt(f),
        }
    }
}
impl<C1: std::error::Error> std::error::Error for ComposeCodecError1<C1> {}

impl<C1> Encode for ComposeCodec<(C1,)>
where
    C1: Encode,
{
    type Item = (C1::Item,);
    type Error = ComposeCodecError1<C1::Error>;

    fn encode(
        ret: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        #[allow(nonstandard_style)]
        let (C1,) = item;
        let ret = C1::encode(ret, C1).map_err(ComposeCodecError1::C1)?;
        Ok(ret)
    }
}

impl<C1: Decode> Decode for ComposeCodec<(C1,)> {
    type Item = (C1::Item,);
    type Error = ComposeCodecError1<C1::Error>;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        let c1 = C1::decode(bytes).map_err(ComposeCodecError1::C1)?;

        Ok((c1,))
    }
}

impl<C1: SelfDelimiting> SelfDelimiting for ComposeCodec<(C1,)> {}

macro_rules! compose_impl {
    // `$C` lists every element, `$Init` every element except the last one.
    ($error_name:ident => [$($C:ident)+] [$($Init:ident)+]) => {
        #[derive(Debug)]
        pub enum $error_name<$($C),+> {
            $($C($C)),+
        }

        paste! {
        impl<$($C),+> $error_name<$($C),+> {
            $(
                #[allow(nonstandard_style)]
                pub fn [<unwrap_ $C>](self) -> $C {
                    match self {
                        Self::$C($C) => $C,
                        _other => panic!("Unwrapped the wrong variant"),
                    }
                }
            )+
        }
        }

        impl<$($C: fmt::Display),+> fmt::Display for $error_name<$($C),+> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                #[allow(nonstandard_style)]
                match self {
                	$(Self::$C($C) => $C.fmt(f)),+
                }
            }
        }
        impl<$($C: std::error::Error),+> std::error::Error for $error_name<$($C),+> {}


        impl<$($C),+> Encode for ComposeCodec<($($C),+)>
        where
          $(
              $C: Encode,
              $C::Item: Sized,
          )+
          $($Init: SelfDelimiting,)+
          {
            type Item = ($($C::Item),+);
            type Error = $error_name<$($C::Error),+>;

            fn encode(
                ret: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
            	#[allow(nonstandard_style)]
            	let ($($C,)+) = item;
                $(let ret = $C::encode(ret, $C).map_err($error_name::$C)?;)+
                Ok(ret)
            }
        }

        impl<$($C),+> Decode for ComposeCodec<($($C),+)>
        where
          $($C: Decode,)+
          $($Init: SelfDelimiting,)+
          {
            type Item = ($($C::Item),+);
            type Error = $error_name<$($C::Error),+>;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                Ok(
                    ($($C::decode(bytes).map_err($error_name::$C)?),+)
                )
            }
        }

        impl<$($C: SelfDelimiting),+> SelfDelimiting for ComposeCodec<($($C),+)> {}
    };
    ($N:literal) => {
        paste! {
        seq!{M in 1..$N {
            compose_impl! {
                [<ComposeCodecError $N>]
                    => [#(C~M)* [<C $N>]] [#(C~M)*]
                }
            }}
        }
    }
}

use paste::paste;
use seq_macro::seq;

seq!(N in 2..20 {
        compose_impl!(N);
});

#[cfg(test)]
mod test {
    use std::{borrow::Cow, error::Error, marker::PhantomData};

    use crate::codec::{
        Bytes, ComposeCodec, ComposeCodecError2, Decode, DecodingVec, Encode, EncodingVec, Fresh,
        SizedCodec, Str, U8,
    };

    #[test]
    fn compose_simple_test() {
        let letter = "ACAB";
        let number = [13, 12];

        type MyCodec<'a> = ComposeCodec<(&'a SizedCodec<Str>, &'a Bytes)>;

        let buf = MyCodec::encode_alloc(&(letter, &number)).unwrap();
        let decode = MyCodec::decode(&mut buf.into_decoding_vec()).unwrap();
        assert_eq!((letter.to_string(), number.to_vec()), (decode.0, decode.1));

        type MyCodec2<'a> = ComposeCodec<(&'a SizedCodec<Bytes>, &'a Str)>;

        let buf = MyCodec2::encode_alloc(&(&number, &letter)).unwrap();
        let decode = MyCodec2::decode(&mut buf.into_decoding_vec()).unwrap();
        assert_eq!((number.to_vec(), letter.to_string()), (decode.0, decode.1));
    }

    #[test]
    fn compose_used_inside_another_codec_simple_case() {
        #[derive(Debug, PartialEq, Eq)]
        struct MyStruct {
            s: String,
            n: Vec<u8>,
        }

        impl Encode for MyStruct {
            type Item = Self;
            type Error = Box<dyn Error>;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                ComposeCodec::<(&SizedCodec<Str>, &Bytes)>::encode(into, &(&item.s, &item.n))
                    .map_err(|err| Box::new(err) as Box<dyn Error>)
            }
        }

        impl Decode for MyStruct {
            type Item = Self;
            type Error = Box<dyn Error>;

            fn decode(bytes: &mut crate::codec::DecodingVec) -> Result<Self::Item, Self::Error> {
                let (s, n) = ComposeCodec::<(SizedCodec<Str>, Bytes)>::decode(bytes)
                    .map_err(|err| Box::new(err) as Box<dyn Error>)?;
                Ok(Self { s, n })
            }
        }

        let s = MyStruct {
            s: String::from("ACAB"),
            n: vec![13, 12],
        };
        let buf = MyStruct::encode_alloc(&s).unwrap();
        let decode = MyStruct::decode(&mut buf.into_decoding_vec()).unwrap();
        assert_eq!(s, decode);
    }

    #[test]
    fn compose_used_inside_another_codec_with_lifetime() {
        #[derive(Debug, PartialEq, Eq)]
        struct MyStruct<'s, 'n> {
            s: Cow<'s, str>,
            n: Cow<'n, [u8]>,
        }

        impl<'a> Encode for MyStruct<'a, 'a> {
            type Item = Self;
            type Error = Box<dyn Error>;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &'_ Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                ComposeCodec::<(&SizedCodec<Str>, &Bytes)>::encode(into, &(&item.s, &item.n))
                    .map_err(|err| Box::new(err) as Box<dyn Error>)
            }
        }

        impl Decode for MyStruct<'static, 'static> {
            type Item = Self;
            type Error = Box<dyn Error>;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                let (s, n) = ComposeCodec::<(SizedCodec<Str>, Bytes)>::decode(bytes)
                    .map_err(|err| Box::new(err) as Box<dyn Error>)?;
                Ok(Self {
                    s: Cow::Owned(s),
                    n: Cow::Owned(n),
                })
            }
        }

        let s = MyStruct {
            s: Cow::Borrowed("ACAB"),
            n: Cow::Owned(vec![13, 12]),
        };
        let buf = MyStruct::encode_alloc(&s).unwrap();
        let decode = MyStruct::decode(&mut buf.into_decoding_vec()).unwrap();
        assert_eq!(s, decode);
    }

    #[test]
    fn compose_used_inside_another_codec_that_contains_yet_another_codec_with_lifetime() {
        #[derive(PartialEq, Eq)]
        struct MyStruct<Value> {
            level: u8,
            value: Value,
        }

        struct MyCodec<'a, Value>(PhantomData<(&'a (), Value)>);

        impl<'a, Value> Encode for MyCodec<'a, Value>
        where
            Value: Encode,
            <Value as Encode>::Item: Sized + ToOwned,
        {
            type Item = MyStruct<<Value as Encode>::Item>;
            type Error = <Value as Encode>::Error;

            fn encode(
                encoding: EncodingVec<Fresh>,
                item: &Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                let key = (&item.level, &item.value);

                match ComposeCodec::<(&U8, &Value)>::encode(encoding, &key) {
                    Err(ComposeCodecError2::C1(_)) => unreachable!(),
                    Err(ComposeCodecError2::C2(err)) => Err(err),
                    Ok(ret) => Ok(ret),
                }
            }
        }

        impl<Value> Decode for MyCodec<'static, Value>
        where
            Value: Decode,
            Value::Item: 'static + Sized + Clone,
        {
            type Item = MyStruct<<Value as Decode>::Item>;
            type Error = ComposeCodecError2<<U8 as Decode>::Error, <Value as Decode>::Error>;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                let (level, value) = ComposeCodec::<(U8, Value)>::decode(bytes)?;
                Ok(MyStruct { level, value })
            }
        }

        let s = MyStruct::<u8> {
            level: 0,
            value: 0u8,
        };
        let buf = MyCodec::<U8>::encode_alloc(&s).unwrap();
        let decode = MyCodec::<U8>::decode(&mut buf.into_decoding_vec()).unwrap();
        assert!(s == decode);
    }

    #[test]
    fn make_sure_all_tuple_size_support_encoding_and_decoding() {
        struct Tester<T: Encode + Decode>(PhantomData<T>);

        // let _ = Tester::<(crate::codec::DecodeIgnore,)>(PhantomData);
        // Every element but the last must be self-delimiting
        let _ = Tester::<ComposeCodec<(&Str,)>>(PhantomData);
        let _ = Tester::<ComposeCodec<(&SizedCodec<Str>, &Str)>>(PhantomData);
        let _ = Tester::<ComposeCodec<(&SizedCodec<Str>, &SizedCodec<Str>, &Str)>>(PhantomData);
        let _ = Tester::<ComposeCodec<(&SizedCodec<Str>, &SizedCodec<Str>, &SizedCodec<Str>, &Str)>>(
            PhantomData,
        );
        let _ = Tester::<
            ComposeCodec<(
                &SizedCodec<Str>, // 1
                &SizedCodec<Str>, // 2
                &SizedCodec<Str>, // 3
                &SizedCodec<Str>, // 4
                &SizedCodec<Str>, // 5
                &SizedCodec<Str>, // 6
                &SizedCodec<Str>, // 7
                &SizedCodec<Str>, // 8
                &SizedCodec<Str>, // 9
                &SizedCodec<Str>, // 10
                &SizedCodec<Str>, // 11
                &SizedCodec<Str>, // 12
                &SizedCodec<Str>, // 13
                &SizedCodec<Str>, // 14
                &SizedCodec<Str>, // 15
                &SizedCodec<Str>, // 16
                &SizedCodec<Str>, // 17
                &SizedCodec<Str>, // 18
                &Str,             // 19
            )>,
        >(PhantomData);
    }

    #[test]
    fn integers_are_self_delimiting() {
        use byteorder::{BE, LE, NativeEndian};

        use crate::codec::{I8, I16, I32, I64, I128, U16, U32, U64, U128, check_self_delimiting};

        arbtest::arbtest(|u| check_self_delimiting::<U8, u8>(u));
        arbtest::arbtest(|u| check_self_delimiting::<I8, i8>(u));
        arbtest::arbtest(|u| check_self_delimiting::<U16<BE>, u16>(u));
        arbtest::arbtest(|u| check_self_delimiting::<I16<LE>, i16>(u));
        arbtest::arbtest(|u| check_self_delimiting::<U32<LE>, u32>(u));
        arbtest::arbtest(|u| check_self_delimiting::<I32<BE>, i32>(u));
        arbtest::arbtest(|u| check_self_delimiting::<U64<BE>, u64>(u));
        arbtest::arbtest(|u| check_self_delimiting::<I64<NativeEndian>, i64>(u));
        arbtest::arbtest(|u| check_self_delimiting::<U128<NativeEndian>, u128>(u));
        arbtest::arbtest(|u| check_self_delimiting::<I128<BE>, i128>(u));
    }

    #[test]
    fn sized_codec_is_self_delimiting() {
        use crate::codec::check_self_delimiting;

        arbtest::arbtest(|u| check_self_delimiting::<SizedCodec<Str>, String>(u));
        arbtest::arbtest(|u| check_self_delimiting::<SizedCodec<Bytes>, Vec<u8>>(u));
        arbtest::arbtest(|u| check_self_delimiting::<SizedCodec<U8>, u8>(u));
    }

    #[test]
    fn compose_codec_is_self_delimiting() {
        use byteorder::{BE, LE};

        use crate::codec::{U16, U32, check_self_delimiting};

        arbtest::arbtest(|u| check_self_delimiting::<ComposeCodec<(U8,)>, (u8,)>(u));
        arbtest::arbtest(|u| check_self_delimiting::<ComposeCodec<(U8, U16<BE>)>, (u8, u16)>(u));
        arbtest::arbtest(|u| {
            check_self_delimiting::<
                ComposeCodec<(SizedCodec<U16<BE>>, ComposeCodec<(U8, U32<LE>)>)>,
                (u16, (u8, u32)),
            >(u)
        });
    }
}
