use std::{convert::Infallible, marker::PhantomData};

use crate::codec::{Decode, DecodingVec};

/// Lazily decodes the data bytes.
///
/// It can be used to avoid CPU-intensive decoding before making sure that it
/// actually needs to be decoded (e.g. based on the key).
#[derive(Default)]
pub struct LazyDecode<C>(std::marker::PhantomData<C>);

impl<C: 'static> Decode for LazyDecode<C> {
    type Item = Lazy<C>;
    type Error = Infallible;

    fn decode(bytes: &mut DecodingVec) -> Result<Self::Item, Self::Error> {
        Ok(Lazy(bytes.consume(), PhantomData))
    }
}

/// Owns bytes that can be decoded on demand.
#[derive(Debug)]
#[repr(transparent)]
pub struct Lazy<C>(Vec<u8>, PhantomData<C>);

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
        C::decode(&mut DecodingVec::new(self.0))
    }
}
