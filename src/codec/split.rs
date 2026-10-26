use crate::codec::{Encode, SelfDelimiting};

/// A key codec made of a first part, the head, then the rest, the tail.
///
/// It's what [`Keyspace::prefixed`](crate::Keyspace::prefixed) uses
/// to type the keyspace of the keys starting with a given head: its keys are
/// the tails, decoded with `Tail`.
///
/// It's implemented on every tuple of two codecs or more: `(A, B)` is split
/// as `A` then `B`, and `(A, B, C)` as `A` then `(B, C)`.
///
/// Implement it on your own codec only if it writes `(head, ...)` as `Head`
/// writes `head`, directly followed by what `Tail` writes for the rest.
/// `Head` must be [`SelfDelimiting`], for no head to be the start of
/// another one. Implementing it wrongly isn't unsafe: the keys of the
/// keyspaces returned by `prefixed` won't decode, like with a wrong
/// [`Keyspace::remap_key_type`](crate::Keyspace::remap_key_type).
pub trait Split {
    /// The codec of the first part of the key.
    type Head: Encode + SelfDelimiting;
    /// The codec of the rest of the key, after its head.
    type Tail;
}
