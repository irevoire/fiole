use fiole::Fiole;

#[derive(Fiole)]
#[fiole(error = std::convert::Infallible)]
#[fiole(error = std::convert::Infallible)]
struct Key {
    #[fiole(codec = fiole::codec::U8)]
    id: u8,
}

fn main() {}
