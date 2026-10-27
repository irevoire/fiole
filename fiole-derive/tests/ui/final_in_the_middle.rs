use fiole::Fiole;
use fiole::codec::{Text, U8};

#[derive(Fiole)]
struct Key {
    #[fiole(codec = Text<u32>)]
    first: u32,
    #[fiole(codec = U8)]
    second: u8,
}

fn main() {}
