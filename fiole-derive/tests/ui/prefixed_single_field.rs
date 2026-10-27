use fiole::codec::{Str, U8};
use fiole::{Fiole, Database, Keyspace, KeyspaceCreateOptions, Root};

/// A single field: nothing is left after its first part, it can't be `Split`.
#[derive(Fiole)]
struct Key {
    #[fiole(codec = U8)]
    id: u8,
}

fn main() {
    let db = Database::builder("db").unwrap();
    let keys: Keyspace<Key, Str, Root> = db.keyspace("keys", KeyspaceCreateOptions::default).unwrap();
    let _ = keys.prefixed(&1);
}
