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
        match self {
            Self::C1(C1) => C1.fmt(f),
        }
    }
}
impl<C1: std::error::Error> std::error::Error for ComposeCodecError1<C1> {}

impl<C1: Encode> Encode for ComposeCodec<(C1,)> {
    type Item = (C1::Item,);
    type Error = ComposeCodecError1<C1::Error>;

    fn encode(
        into: EncodingVec<Fresh>,
        item: &Self::Item,
    ) -> Result<EncodingVec<Fresh>, Self::Error> {
        let ret = into.edit();
        let ret = C1::encode(ret.make_fresh(), &item.0).map_err(ComposeCodecError1::C1)?;
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


        impl<$($C),+> Encode for ComposeCodec<($($C),+)>
        where
          $(
              $C: Encode,
              $C::Item: Sized,
          )+
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
    use crate::codec::{Bytes, ComposeCodec, Decode, Encode, Str, I8, U8};

    #[test]
    fn compose() {
        let letter = "ACAB";
        let number = [13, 12];

        // type MyCodec = ComposeCodec<(Str, Bytes)>;
        type MyCodec = ComposeCodec<(U8, I8)>;
        let buf = MyCodec::encode_alloc(&(17, 12)).unwrap();
        let decode = MyCodec::decode(&mut buf.into_decoding_vec()).unwrap();
        assert_eq!(&decode, &(17, 12));
    }
}
