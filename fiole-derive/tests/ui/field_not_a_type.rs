use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    #[fiole(codec = 1)]
    id: u8,
}

fn main() {}
