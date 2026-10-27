//! What the code `#[derive(Fiole)]` generates relies on. Not part of the
//! API: it only gives the errors about a field a message about that field.

use std::borrow::Borrow;

use crate::codec::{Decode, Encode};

/// A field written with the codec `C`: it's the item `C` writes, or borrows it.
#[diagnostic::on_unimplemented(
    message = "a field of type `{Self}` can't be written with the codec `{C}`",
    label = "`{C}` doesn't write a `{Self}`",
    note = "a field must be the item its codec writes, or borrow it"
)]
pub trait EncodedBy<C: Encode + ?Sized> {
    fn borrow_item(&self) -> &C::Item;
}

#[diagnostic::do_not_recommend]
impl<C: Encode + ?Sized, T: Borrow<C::Item> + ?Sized> EncodedBy<C> for T {
    fn borrow_item(&self) -> &C::Item {
        self.borrow()
    }
}

/// A field read with the codec `C`: it's the item `C` reads, or is made from it.
#[diagnostic::on_unimplemented(
    message = "a field of type `{Self}` can't be read with the codec `{C}`",
    label = "`{C}` doesn't read a `{Self}`",
    note = "a field must be the item its codec reads, or be made from it with `From`"
)]
pub trait DecodedBy<C: Decode + ?Sized>: Sized {
    fn from_item(item: C::Item) -> Self;
}

#[diagnostic::do_not_recommend]
impl<C: Decode + ?Sized, T> DecodedBy<C> for T
where
    C::Item: Into<T>,
{
    fn from_item(item: C::Item) -> Self {
        item.into()
    }
}
