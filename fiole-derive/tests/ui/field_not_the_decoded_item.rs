use fiole::Fiole;
use fiole::codec::{Str, Terminated, U8};

#[derive(Fiole)]
struct Key {
    // A `String` is written as a `str`, but `Str` decodes a `StrView`, not a `String`
    #[fiole(codec = Terminated<Str>)]
    name: String,
    #[fiole(codec = U8)]
    age: u8,
}

fn main() {}
