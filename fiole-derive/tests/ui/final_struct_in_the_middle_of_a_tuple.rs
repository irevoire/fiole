use fiole::Fiole;
use fiole::codec::{Encode, Text, U8};

/// Ends with a final codec: it's still a codec, but it can only be used last.
#[derive(Fiole)]
struct Inner {
    #[fiole(codec = U8)]
    first: u8,
    #[fiole(codec = Text<u32>)]
    second: u32,
}

fn main() {
    // Last in a tuple of codecs: fine
    let inner = Inner { first: 1, second: 2 };
    <(U8, Inner)>::encode_alloc(&(3, inner)).unwrap();

    // Anywhere else: refused, `Inner` doesn't know where it ends
    let inner = Inner { first: 1, second: 2 };
    <(Inner, U8)>::encode_alloc(&(inner, 3)).unwrap();
}
