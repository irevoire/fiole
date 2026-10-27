use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
