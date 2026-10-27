use fiole::Fiole;

#[derive(Fiole)]
#[fiole(name = KeyCodec, name = OtherCodec)]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
