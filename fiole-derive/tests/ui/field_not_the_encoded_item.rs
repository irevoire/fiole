use fiole::Fiole;
use fiole::codec::U8;

#[derive(Fiole)]
struct Key {
    // A `u16` can't be written as the `u8` of `U8`
    #[fiole(codec = U8)]
    id: u16,
}

fn main() {}
