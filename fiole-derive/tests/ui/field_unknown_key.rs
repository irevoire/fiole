use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    #[fiole(with = U8)]
    id: u8,
}

fn main() {}
