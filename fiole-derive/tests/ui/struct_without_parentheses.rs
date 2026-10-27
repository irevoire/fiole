use fiole::Fiole;

#[derive(Fiole)]
#[fiole]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
