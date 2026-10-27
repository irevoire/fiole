use fiole::Fiole;
use fiole::codec::Text;

/// The codec type it declares has the same `where` clause as the struct.
#[derive(Fiole)]
#[fiole(name = IdCodec)]
struct Id<T>
where
    T: Copy,
{
    #[fiole(codec = Text<T>)]
    id: T,
}

// `String` isn't `Copy`: `IdCodec<String>` isn't a type, like `Id<String>`
fn codec(_: Option<IdCodec<String>>) {}

fn main() {}
