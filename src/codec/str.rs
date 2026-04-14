use std::{convert::Infallible, string::FromUtf8Error};

use fjall::Slice;

use crate::codec::{Decode, Encode};

/// Describe a raw string without len or termination byte.
pub struct Str {}

impl Encode for Str {
    type Item = str;
    type Error = Infallible;

    fn encode(item: &Self::Item) -> Result<Slice, Self::Error> {
        Ok(Slice::new(&item.as_bytes()))
    }
}

impl Decode for Str {
    type Item = String;
    type Error = FromUtf8Error;

    fn decode(bytes: Slice) -> Result<Self::Item, Self::Error> {
        String::from_utf8(bytes.to_vec())
    }
}
