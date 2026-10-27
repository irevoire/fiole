use std::convert::Infallible;

use fiole::Fiole;
use fiole::codec::{Text, U8};

#[derive(Debug)]
enum KeyError {}

impl From<Infallible> for KeyError {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

// `KeyError` can't be built from the error of `Text<u32>`
#[derive(Fiole)]
#[fiole(error = KeyError)]
struct Key {
    #[fiole(codec = U8)]
    first: u8,
    #[fiole(codec = Text<u32>)]
    second: u32,
}

fn main() {}
