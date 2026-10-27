use fiole::Fiole;

#[derive(Fiole)]
#[fiole(error = 1)]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
