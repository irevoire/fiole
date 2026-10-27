use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    #[fiole(codec)]
    id: u8,
}

fn main() {}
