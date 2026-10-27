use fiole::Fiole;

#[derive(Fiole)]
#[fiole(rename = KeyCodec)]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
