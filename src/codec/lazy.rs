use std::{convert::Infallible, fmt, marker::PhantomData};

use byteview::ByteView;

use crate::codec::{Decode, DecodingVec};

/// Lazily decodes the data bytes.
///
/// It can be used to avoid CPU-intensive decoding before making sure that it
/// actually needs to be decoded (e.g. based on the key).
///
/// /!\ This codec is final: it decodes everything till the end, so it can only be the last element of a [`ComposeCodec`](crate::codec::ComposeCodec). To put it anywhere else, wrap it in a [`SizedCodec`](crate::codec::SizedCodec) or a [`Terminated`](crate::codec::Terminated).
pub struct LazyDecode<C>(std::marker::PhantomData<C>);

impl<C> Decode for LazyDecode<C> {
    type Item = Lazy<C>;
    type Error = Infallible;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        Ok(Lazy(bytes.consume(), PhantomData))
    }
}

/// Owns bytes that can be decoded on demand.
#[repr(transparent)]
pub struct Lazy<C>(ByteView, PhantomData<C>);

impl<C> fmt::Debug for Lazy<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Lazy").field(&self.0).finish()
    }
}

impl<C> Clone for Lazy<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<C> Lazy<C> {
    /// Change the codec type of the given bytes, specifying the new codec.
    pub fn remap<NC>(self) -> Lazy<NC> {
        Lazy(self.0, PhantomData)
    }
}

impl<C: Decode> Lazy<C> {
    /// Decode the given bytes according to the codec.
    pub fn decode(self) -> Result<C::Item, C::Error> {
        C::decode(&mut DecodingVec::from(self.0))
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Decode, DecodingVec, Encode, Str};

    use super::{Lazy, LazyDecode};

    /// A codec that doesn't implement `Debug`, like every codec.
    enum NotDebug {}

    impl Decode for NotDebug {
        type Item = ();
        type Error = std::convert::Infallible;

        fn decode(bytes: &mut DecodingVec) -> Result<(), Self::Error> {
            bytes.skip_to_end();
            Ok(())
        }
    }

    #[test]
    fn decodes_without_copying() {
        let slice = Str::encode_alloc("a string longer than 20 bytes")
            .unwrap()
            .into_fjall_slice();
        let (decoded, allocations) = crate::test_alloc::allocations(|| {
            let lazy = LazyDecode::<Str>::decode(&mut DecodingVec::from(&slice)).unwrap();
            lazy.clone().decode().unwrap()
        });
        assert_eq!(&*decoded, "a string longer than 20 bytes");
        assert_eq!(allocations, 0);
    }

    #[test]
    fn debug_doesnt_require_a_debug_codec() {
        let lazy = LazyDecode::<NotDebug>::decode(&mut DecodingVec::new(vec![1, 2])).unwrap();
        assert_eq!(format!("{lazy:?}"), "Lazy([1, 2])");
        let _: Lazy<NotDebug> = lazy;
    }

    #[test]
    fn decode_doesnt_require_a_static_codec() {
        // `&'a Str` isn't `'static`
        fn decode<'a>(bytes: &'a [u8]) -> String {
            let mut bytes = DecodingVec::new(bytes);
            let lazy = LazyDecode::<&'a Str>::decode(&mut bytes).unwrap();
            lazy.decode().unwrap().to_string()
        }

        let bytes = Str::encode_alloc("hello").unwrap().finish();
        assert_eq!(decode(&bytes), "hello");
    }
}
