use fiole::Fiole;

#[derive(Fiole)]
struct Key {
    // `u32` is the item, not the codec it's written with
    #[fiole(codec = u32)]
    id: u32,
}

fn main() {}
