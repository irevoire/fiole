use std::{
    fmt::{self},
    marker::PhantomData,
};

use crate::codec::{Decode, DecodingVec, Encode, EncodingVec, Fresh};

pub struct ComposeCodec<T>(PhantomData<T>);

// We keep the following implementation made by hand to help debug the macro below

#[derive(Debug)]
pub enum ComposeCodecError1<C1> {
    C1(C1),
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

impl<'a, C1: Encode<'a>> Encode<'a> for ComposeCodec<(C1,)> {
    type Item = (&'a C1::Item,);
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

macro_rules! compose_impl {
    ($error_name:ident => $($C:ident)+) => {
        #[derive(Debug)]
        pub enum $error_name<$($C),+> {
            $($C($C)),+
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


        impl<'a, $($C),+> Encode<'a> for ComposeCodec<($($C),+)>
        where
          $(
              $C: Encode<'a>,
          )+
          {
            type Item = ($(&'a $C::Item),+);
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
          $(
              $C: Decode,
              $C::Item: Sized,
          )+
          {
            type Item = ($($C::Item),+);
            type Error = $error_name<$($C::Error),+>;

            fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
                Ok(
                    ($($C::decode(bytes).map_err($error_name::$C)?),+)
                )
            }
        }
    };
    ($N:literal) => {
        paste! {
        seq!{M in 0..$N {
            compose_impl! {
                [<ComposeCodecError $N>]
                    => #(C~M)*
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
        Bytes, ComposeCodec, Decode, DecodingVec, Encode, EncodingVec, Fresh, SizedCodec, Str,
    };

    #[test]
    fn compose_simple_test() {
        let letter = "ACAB";
        let number = [13, 12];

        type MyCodec = ComposeCodec<(SizedCodec<Str>, Bytes)>;

        let buf = MyCodec::encode_alloc(&(letter, &number)).unwrap();
        let decode = MyCodec::decode(&mut buf.into_decoding_vec()).unwrap();
        assert_eq!((letter.to_string(), number.to_vec()), (decode.0, decode.1));

        type MyCodec2 = ComposeCodec<(SizedCodec<Bytes>, Str)>;

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

        impl Encode<'_> for MyStruct {
            type Item = Self;
            type Error = Box<dyn Error>;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &'_ Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                ComposeCodec::<(SizedCodec<Str>, Bytes)>::encode(into, &(&item.s, &item.n))
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

        impl<'a> Encode<'a> for MyStruct<'a, 'a> {
            type Item = Self;
            type Error = Box<dyn Error>;

            fn encode(
                into: EncodingVec<Fresh>,
                item: &'_ Self::Item,
            ) -> Result<EncodingVec<Fresh>, Self::Error> {
                ComposeCodec::<(SizedCodec<Str>, Bytes)>::encode(into, &(&item.s, &item.n))
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
    fn make_sure_all_tuple_size_support_encoding_and_decoding() {
        struct Tester<'a, T: Encode<'a> + Decode>(PhantomData<&'a T>);

        // let _ = Tester::<(crate::codec::DecodeIgnore,)>(PhantomData);
        let _ = Tester::<ComposeCodec<(Str,)>>(PhantomData);
        let _ = Tester::<ComposeCodec<(Str, Str)>>(PhantomData);
        let _ = Tester::<ComposeCodec<(Str, Str, Str)>>(PhantomData);
        let _ = Tester::<ComposeCodec<(Str, Str, Str, Str)>>(PhantomData);
        let _ = Tester::<
            ComposeCodec<(
                Str, // 1
                Str, // 2
                Str, // 3
                Str, // 4
                Str, // 5
                Str, // 6
                Str, // 7
                Str, // 8
                Str, // 9
                Str, // 10
                Str, // 11
                Str, // 12
                Str, // 13
                Str, // 14
                Str, // 15
                Str, // 16
                Str, // 17
                Str, // 18
                Str, // 19
            )>,
        >(PhantomData);
    }
}
