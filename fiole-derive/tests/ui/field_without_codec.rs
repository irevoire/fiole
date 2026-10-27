use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    // Every field must say its codec, even when its type is a codec itself
    id: u32,
}

fn main() {}
