use fiole::Fiole;
use fiole::codec::{Text, U8};

/// Ends with a final codec: it can't be followed by anything.
#[derive(Fiole)]
struct Inner {
    #[fiole(codec = U8)]
    first: u8,
    #[fiole(codec = Text<u32>)]
    second: u32,
}

#[derive(Fiole)]
struct Outer {
    #[fiole(codec = Inner)]
    inner: Inner,
    #[fiole(codec = U8)]
    last: u8,
}

fn main() {}
