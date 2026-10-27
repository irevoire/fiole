use fiole::Fiole;

/// 20 fields, one more than the most.
#[derive(Fiole)]
struct Key(
    #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P,
    #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P,
    #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P,
    #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P,
    #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P, #[fiole(codec = P)] P,
);

/// Only there to be the type of the fields.
struct P;

fn main() {}
