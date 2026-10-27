use fiole::Fiole;

#[derive(Fiole)]
#[fiole(codec = fiole::codec::U8)]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
