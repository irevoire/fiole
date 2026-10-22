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
  to the end (`Str`, `Bytes`, `Unit`, `LazyDecode`, `DecodeIgnore`, `Rkyv`,
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
- Every bug fix must come with a regression test in the `#[cfg(test)] mod test`
  of the file being changed. Write it first and check it fails before fixing.
- Line numbers are those at commit `813d092`; they may drift. Prefer the
  function names.

---

## 🔴 Bugs (behavior is wrong regardless of key/value usage)

### T6 `Rkyv` decoding relies on allocator alignment (not reproduced)

- **Where:** `src/codec/rkyv.rs`, `impl Decode for Rkyv` (~line 52).
- **Problem:** `rkyv::from_bytes` requires a buffer aligned for the archived
  root, but it receives a plain `Vec<u8>` from `consume()`. It works today only
  because the system allocator happens to over-align; another global allocator
  or a sub-slice copy can make validation fail. `from_bytes` validates, so it's an error and never UB. It becomes the common
  case once T16 decodes straight from fjall's `Slice` (arbitrary offsets), so
  fix it before or with T16.
- **Fix:** add `DecodingVec::remaining_slice(&self) -> &[u8]`. In `decode`, if
  the remaining slice is already 16-aligned, call `rkyv::from_bytes` on it
  directly (no copy); otherwise copy it into `rkyv::util::AlignedVec::<16>`
  first. Then `bytes.skip_to_end()`. Document that types needing more than
  16-byte alignment aren't supported. While there, replace
  `ret.extend(bytes.into_iter().copied())` in `encode` by a slice extend, and
  fix the ``[`Sized`]`` doc link (T9).
- **Test:** it can't fail before the fix (it depends on the global allocator).
  Add `impl From<&[u8]> for DecodingVec`, then decode a value whose root needs
  16-byte alignment (e.g. contains a `u128`) from a buffer at every offset
  `0..16`: at most one of them is aligned, whatever the allocator.

---

## 🟠 Expected behavior that must be documented (no behavior change)

> Do not change any encoding in these tasks. Documentation only (doc
> comments + README where relevant). Run `cargo doc --all-features --no-deps`
> and make sure there are no new warnings.

### T7 Signed integers don't sort numerically as keys

- **Where:** `src/codec/integer.rs` — `I8` and the `define_type!` macro (for
  `I16`…`I128`).
- **Context:** they are stored as raw two's complement. Correct for values,
  but as keys negatives sort *after* positives: with keys `-5..5` in
  `I32<BE>`, `range(-3..3)` returns nothing and `first_key_value` returns `0`.
- **Do:** add a doc paragraph on each signed codec: "Encoded as two's
  complement: when used as a key, negative numbers sort after positive ones.
  Use [`OrderedI32`] if you need numeric ordering." The macro builds docs
  with `#[doc = ...]`; add a parameter or a separate `#[doc]` line for signed
  types. The `Ordered*` codecs live in `src/codec/ordered.rs`.

### T8 Endianness and key ordering

- **Where:** `src/codec/integer.rs` (unsigned multi-byte codecs), `README.md`,
  and the `codec` module doc (`src/codec/mod.rs`).
- **Do:** state that only `BigEndian` (`byteorder::BE`) preserves numeric
  ordering for keys; `LittleEndian`/`NativeEndian` are fine for values. The
  README example uses `U32<NativeEndian>` as a *value* — keep it, but add a
  one-line comment saying keys should use `BE`.

### T9 Document final vs self-delimiting codecs

- **Where:** `src/codec/mod.rs` module doc, and each codec's doc.
- **Do:**
  1. Add a "Composing codecs" section to the module doc explaining final vs
     self-delimiting codecs (see Context above) and the `SelfDelimiting` trait:
     every element of a `ComposeCodec` except the last must implement it (wrap
     final codecs in `SizedCodec` or `Terminated`). The
     `ComposeCodec` and `SelfDelimiting` docs already cover it, link to them.
  2. Fix the broken intra-doc links: ``[`Sized`]`` currently resolves to
     `std::marker::Sized`. Replace with ``[`SizedCodec`]`` (as `serde_json.rs`
     and `DecodeIgnore` do) in `bytes.rs`,
     `str.rs`, `rkyv.rs`, `facet_json.rs`, `facet_postcard.rs`,
     `facet_msgpack.rs`.
  3. Add the "final" line to codecs missing it: `LazyDecode`, `SerdePostcard`,
     `Unit` (T10).
  4. Fix the `Bytes` doc: it says "totally borrowed" but decodes into an owned `Vec<u8>`.

### T10 `Unit` is final

- **Where:** `src/codec/unit.rs`.
- **Context:** `Unit::decode` fails with `NonEmptyError` if any byte remains.
  It doesn't implement `SelfDelimiting`, so `ComposeCodec<(Unit, U8)>` doesn't
  compile anymore.
- **Do:** document it as final ("must be the last element of a compose; it
  verifies nothing remains"). Behavior stays unchanged — owner decided. Add a
  test asserting `(U8, Unit)` round-trips (`check_round_trip::<Unit, ()>`
  already exists in `unit.rs`).

### T11 `SizedCodec` doesn't preserve key ordering

- **Where:** `src/codec/sized_codec.rs`.
- **Context:** because of the u32 length prefix, `"b"` sorts before `"aa"`,
  and a prefix search on a partial string cannot work. This is expected.
- **Do:** add a doc comment on `SizedCodec` (there is none) describing the
  format (u32 big-endian length, then the inner bytes), that it's ideal for
  values / non-ordered keys, and point to `Terminated` (`src/codec/terminated.rs`)
  for ordered string keys. Mention the 4 GiB limit (see T25 for the silent
  truncation on encode).

### T12 Clarify `EncodingVec`'s relative view

- **Where:** `src/codec/encoding.rs`.
- **Context:** `len`, `Deref`, `as_slice`, `AsRef` and `Debug` only cover the
  bytes after `start_at`. Since every `encode` returns a `Fresh` vec with
  `start_at == len`, `Str::encode_alloc("hello").unwrap().len() == 0` and it
  prints `[]`, while `finish()`/`into_fjall_slice()` return everything.
- **Do:** document the type (`EncodingVec`, `Fresh`, `Dirty`, `SpaceToken`
  have no docs) and each method, stating which are relative and which are
  absolute. Recommended small API improvement (ask owner before doing it):
  make `Debug` print the whole buffer, or add `full_slice()`.

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

### T15 Doc examples ignore commit conflicts

- **Where:** `README.md`, doc examples in `src/database.rs` and `src/keyspace.rs`.
- **Problem:** `Wtxn::commit` returns `Result<Result<(), Conflict>, fjall::Error>`;
  examples do `wtxn.commit().unwrap();`, silently dropping a `Conflict`.
- **Do:** use `wtxn.commit().unwrap().unwrap();` everywhere. Document `commit`
  explaining both layers. (Alternative, ask owner first: flatten into a single
  error type.)

### T16 Every read copies the data

- **Where:** `src/codec/decoding.rs` (`impl From<Slice> for DecodingVec<'static>`
  does `value.to_vec()`); callers in `src/keyspace.rs` (`get`, `take`) and
  `src/lib.rs` (`Guard::key/value/into_inner`).
- **Do:** avoid the copy. Simplest: at call sites, keep the `Slice` alive and
  decode from `(&slice).into()` (the borrowed `From<&Slice>` impl already
  exists). Check that no lifetime escapes (`Decode::Item` has no lifetime, so
  it's fine). Optionally make `DecodingVec` hold a `Slice` directly. Fix T6
  first: borrowed slices into fjall's buffers break `Rkyv`'s alignment.

### T17 Inconsistent error types across `Keyspace` methods

- **Where:** `src/keyspace.rs`.
- **Problem:** `range`, `prefix`, `remove` return a bare `Key::Error`; `get`,
  `contains_key`, `size_of`, `insert`, `take` return `Error<K, V>`.
- **Do:** make them return `Error<Key::Error, Infallible>` like `contains_key`.
  Breaking change — note it in the changelog/commit.

### T18 Make `Error` more usable

- **Where:** `src/error.rs`.
- **Do:** add `impl From<fjall::Error>`; implement `source()` returning the
  inner error; prefix `Display` with context (`"key codec error: …"`,
  `"value codec error: …"`); relax `Display` bounds to `Display` instead of
  `StdError`; make `unwrap_fjall` include the variant in its panic message
  (requires `Debug` bounds; or add `fjall_error(self) -> Option<fjall::Error>`
  as a non-panicking alternative).

### T19 `byteview::StrView` is exposed but not re-exported

- **Where:** `src/database.rs` (`list_keyspace_names`), `Cargo.toml`, `src/lib.rs`.
- **Problem:** if `fjall` moves to another `byteview` version, users get type mismatches.
- **Do:** either `pub use byteview;` from `lib.rs` (or `pub use fjall::…` if
  fjall re-exports it), or return `Vec<String>`. Owner preference unknown — prefer
  re-export (non-breaking).

### T20 Re-export `PersistMode` and `Conflict`

- **Where:** `src/lib.rs`.
- **Do:** `pub use fjall::{Conflict, PersistMode};` next to the existing
  `KeyspaceCreateOptions` re-export. Optionally `pub use fjall;`.

### T21 `Database` should be `Clone` and `Debug`

- **Where:** `src/database.rs`.
- **Do:** `#[derive(Clone)]` (fjall's `OptimisticTxDatabase` is a cheap handle — verify it
  implements `Clone`). Add `Debug` if the inner type allows it, else a manual impl.

### T22 `Database::builder` is not a builder

- **Where:** `src/database.rs`.
- **Do:** ask the owner before implementing (API decision). Proposal: add a
  `DatabaseBuilder` wrapping `fjall::…Builder`, exposing its config methods and
  `open()`; keep `Database::open(path)` as a shortcut. The existing doc comment
  already says "In the future it should return a [`fjall::DatabaseBuilder`]".

### T23 Seal the `Readable` trait

- **Where:** `src/txn.rs`.
- **Do:** use the sealed trait pattern (`mod private { pub trait Sealed {} }`,
  `pub trait Readable: private::Sealed + Send + Sync`) and make `inner()`
  `#[doc(hidden)]`. Document `Readable`'s method. Keep it usable as
  `&impl Readable` in user code.

---

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

### T26 Documentation gaps

- `Keyspace::range` has no doc comment (`src/keyspace.rs`).
- `Wtxn::commit`, `rollback`, `durability` have no docs (`src/txn.rs`).
- `serde_msgpack.rs` doc links to `[`msgpack`]`, which doesn't resolve (rustdoc
  warning); use `[msgpack](https://msgpack.org/)` like `facet_msgpack.rs`.

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

- No plain `F32`/`F64` codecs (`byteorder` supports `read_f32`/`write_f32`; the
  `define_type!` macro can be reused). `OrderedF32`/`OrderedF64` exist for keys.
- `Keyspace::prefix` requires a full `Key::Item`, so you can't prefix-search by
  the first fields of a compose key. Idea: `prefix_with::<PrefixCodec>(&rtxn,
  &item)` that encodes with another codec, or a `prefix_raw(&[u8])`. Ask owner.

### T30 Clippy and cosmetics

- Fix the clippy warnings (`cargo clippy --all-features --all-targets`):
  redundant `#[must_use]` on functions returning `Result` (`src/lib.rs`
  `Guard::size/key/value/into_inner`), `.clone()` on `PhantomData` in
  `Keyspace::clone`, needless borrows, elidable lifetimes,
  `bytes.into_iter()` → `bytes.iter()` in `rkyv.rs`, redundant
  `.map_err(Into::into)` in `integer.rs`, redundant closures, complex types.
- `#[repr(transparent)]` on `Database`, `Keyspace`, `Wtxn`, `Rtxn` serves no
  purpose → remove.
- Typos: "parkeyspacetition" (`src/keyspace.rs`, `is_empty` doc), "informations"
  (`src/lib.rs`), "availabe" and "got rids" (`README.md`).
- `len` doc test has leftover assertion messages `"hello"`/`"world"`;
  `first_key_value`/`last_key_value` examples use `&mut wtxn` where `&wtxn`
  suffices.

---

## Suggested order

1. T6 (postponed, but needed before T16).
2. Docs batch T7–T12, T15, T26.
3. API tasks T16–T23 (T17 and T22 are breaking or need the owner's decision).
4. Nits T25, T27–T30.

Dependencies: T9 references T10; T16
needs T6; T27's clippy CI job needs T30 first.
