use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    // `name` is the name of the codec of the struct, not of a field
    #[fiole(name = U8)]
    id: u8,
}

fn main() {}
