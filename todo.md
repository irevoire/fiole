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

## 🟢 New codecs for keys

### T14 Escaped variant of `Terminated` (optional)

- **Done:** `Terminated<C, T: Terminator = Nul>` (`src/codec/terminated.rs`)
  stores ordered string keys; the inner codec can't produce the pattern.
- **Follow-up (optional):** an escaped variant accepting any byte: encode `0x00`
  as `00 FF`, terminate with `00 00` (FoundationDB tuple layer style). Keeps
  the order and never fails to encode, at the cost of a copy when decoding.

---

## 🟡 API and design

### T22 `Database::builder` is not a builder

- **Where:** `src/database.rs`.
- **Do:** ask the owner before implementing (API decision). Proposal: add a
  `DatabaseBuilder` wrapping `fjall::…Builder`, exposing its config methods and
  `open()`; keep `Database::open(path)` as a shortcut. The existing doc comment
  already says "In the future it should return a [`fjall::DatabaseBuilder`]".

## ⚪ Minor issues and nitpicks

### T25 Small correctness/API nits in codecs

- `EncodingVec::is_empty(self)` takes `self` by value → `&self`
  (`src/codec/encoding.rs:83`; clippy `len_without_is_empty`).
- `SizedCodec::encode` does `size as u32` silently; return an error (or
  `debug_assert`/panic with a message) when `size > u32::MAX`.
- `Lazy<C>` derives `Debug`, which requires `C: Debug` (codecs never implement
  it) → manual `impl<C> Debug`. `LazyDecode<C: 'static>`: drop the `'static`
  bound; drop the useless `#[derive(Default)]` (`src/codec/lazy.rs`).
- `EncodingVec::append(Vec<u8>)` → also add/replace with `extend_from_slice(&[u8])`.

### T27 Cargo / CI hygiene

- `facet-format` is a dependency enabled by the facet features but never used
  in `src/` → remove it from `Cargo.toml` (and from the feature lists). Verify
  with `cargo build --features facet_json` etc.
- `facet` forces `jiff02` and `uuid` features on every user → move them behind
  opt-in features or drop them (ask owner).
- Add `rust-version` to `Cargo.toml` (edition 2024 ⇒ at least 1.85).
- Trailing space in the `description` field.
- CI (`.github/workflows/ci.yml`): add a `cargo clippy --all-features
  --all-targets -- -D warnings` job (after T30), a `RUSTDOCFLAGS="-D warnings"
  cargo doc --all-features --no-deps` job, and a matrix building each feature
  alone (`--no-default-features --features <f>`).

### T28 Naming consistency

- `ComposeCodecError1::unwrap_error_1` vs macro-generated `unwrap_C1`, `unwrap_C2`…
  → rename to `unwrap_C1` (breaking; or add an alias).
- `Str {}` and `Unit {}` are constructible empty structs while other codecs are
  uninhabited enums → make them `pub enum Str {}` / `pub enum Unit {}`.

### T29 Missing features

- `Keyspace::prefix` requires a full `Key::Item`, so you can't prefix-search by
  the first fields of a compose key. Idea: `prefix_with::<PrefixCodec>(&rtxn,
  &item)` that encodes with another codec, or a `prefix_raw(&[u8])`. Ask owner.

### T30 Clippy and cosmetics

- Fix the clippy warnings (`cargo clippy --all-features --all-targets`):
  redundant `#[must_use]` on functions returning `Result` (`src/lib.rs`
  `Guard::size/key/value/into_inner`), `.clone()` on `PhantomData` in
  `Keyspace::clone`, needless borrows, elidable lifetimes, redundant
  `.map_err(Into::into)` in `integer.rs`, redundant closures, complex types.
- `#[repr(transparent)]` on `Keyspace` serves no purpose → remove.
- Typos: "parkeyspacetition" (`src/keyspace.rs`, `is_empty` doc), "informations"
  (`src/lib.rs`), "availabe" and "got rids" (`README.md`).
- `len` doc test has leftover assertion messages `"hello"`/`"world"`;
  `first_key_value`/`last_key_value` examples use `&mut wtxn` where `&wtxn`
  suffices.

---

## Suggested order

1. API task T22 (needs the owner's decision).
2. Nits T25, T27–T30.

Dependencies: T27's clippy CI job needs T30 first.
