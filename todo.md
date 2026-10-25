# fiole — review task list

This file lists issues found during a full review of the crate. Each task is
self-contained so it can be picked up independently by an agent or a human.

## Context every task needs

- `fiole` is a typed wrapper around `fjall` (an LSM key-value store). Users pick
  a **codec** for the key and the value of each `Keyspace`
  (`src/keyspace.rs`). Codecs live in `src/codec/`.
- **Codecs are used for both keys and values.** A codec only has to round-trip
  correctly. Byte ordering only matters when it is used as a key (for `range`,
  `prefix`, `first_key_value`, …). Do **not** change the encoding of an
  existing codec to make it sort differently: that would break data already on
  disk and is not wanted. Add a new codec instead, and document the existing one.
- Encoding API (`src/codec/encoding.rs`): `Encode::encode(into:
  EncodingVec<Fresh>, item) -> Result<EncodingVec<Fresh>, _>`. A codec calls
  `into.edit()` to get an `EncodingVec<Dirty>`, writes bytes, then calls
  `.make_fresh()`. `start_at` marks where the current codec's bytes begin:
  everything before it belongs to previous codecs (e.g. earlier elements of a
  `ComposeCodec`) and must never be touched. Relative methods (`clear`,
  `insert`, `remove`, `resize`, `pop`) must respect `start_at`.
- Decoding API (`src/codec/decoding.rs`): `Decode::decode(&mut DecodingVec)`.
  `DecodingVec` is a cursor. A codec must read **only its own bytes** so the
  next codec of a `ComposeCodec` can read the rest.
- **Final vs self-delimiting codecs.** A *final* codec consumes everything up
  to the end (`Str`, `Bytes`, `Unit`, `LazyDecode`, `DecodeIgnore`,
  `SerdeJson`, `SerdePostcard`, the `Facet*` codecs…). It can only be the last
  element of a `ComposeCodec`, unless it is wrapped in `SizedCodec` (u32 BE
  length prefix). A *self-delimiting* codec knows where it ends (integers,
  `SizedCodec<_>`, `ComposeCodec` of self-delimiting codecs, the roaring
  codecs, `SerdeMsgpack`) and implements the `SelfDelimiting` marker trait
  (`src/codec/mod.rs`). `ComposeCodec` requires it on every element but the
  last, so misusing a final codec is a compile error.
- **Property checks** (`src/codec/property.rs`, built on `arbitrary`, run with
  `arbtest`): `check_round_trip::<Codec, OwnedItem>` for every codec, and
  `check_self_delimiting::<Codec, OwnedItem>` for `SelfDelimiting` ones, and
  `check_order_preserving::<Codec, OwnedItem>` for codecs meant for ordered
  keys (the `_with` variants take a generator closure). New codecs should be tested with
  them, e.g. `arbtest::arbtest(check_round_trip::<Str, String>);`.
- Commands (run from repo root):
  - `cargo test --all-features`
  - `cargo clippy --all-features --all-targets`
  - `cargo doc --all-features --no-deps`
  - `cargo fmt --all` (CI checks formatting)
- **`Keyspace` error types are deliberately precise** (owner decision): each
  method only returns the errors it can produce (`Key::Error` alone for
  `range`/`prefix`/`remove`, `Error<_, Infallible>` for `contains_key`/`size_of`,
  `fjall::Error` for `len`/`is_empty`). Don't unify them.
- Every bug fix must come with a regression test in the `#[cfg(test)] mod test`
  of the file being changed. Write it first and check it fails before fixing.
- Line numbers are those at commit `813d092`; they may drift. Prefer the
  function names.

---

## ⚪ Minor issues and nitpicks

### T29 Missing features

- `Keyspace::prefix` requires a full `Key::Item`, so you can't prefix-search by
  the first fields of a compose key. Idea: `prefix_with::<PrefixCodec>(&rtxn,
  &item)` that encodes with another codec, or a `prefix_raw(&[u8])`. Ask owner.

---

## Suggested order

1. T29 (needs the owner's decision).
