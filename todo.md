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
  to the end (`Str`, `Bytes`, `LazyDecode`, `Rkyv`, `SerdePostcard`, the
  `Facet*` codecs…). It can only be the last element of a `ComposeCodec`,
  unless it is wrapped in `SizedCodec` (u32 BE length prefix). A
  *self-delimiting* codec knows where it ends (integers, `SizedCodec<_>`,
  `RoaringBitmapCodec`, `SerdeMsgpack`).
- Commands (run from repo root):
  - `cargo test --all-features`
  - `cargo clippy --all-features --all-targets`
  - `cargo doc --all-features --no-deps`
  - `cargo fmt --all` (CI checks formatting)
- Every bug fix must come with a regression test in the `#[cfg(test)] mod test`
  of the file being changed. The tests below were used to confirm each bug
  (marked ✅) and can be reused as-is.
- Line numbers are those at commit `813d092`; they may drift. Prefer the
  function names.

Legend: ✅ = reproduced with a failing test during the review.

---

## 🔴 Bugs (behavior is wrong regardless of key/value usage)

### T1 ✅ `EncodingVec<Dirty>::pop` recurses forever

- **Where:** `src/codec/encoding.rs`, `impl EncodingVec<Dirty>`, `fn pop` (~line 163).
- **Problem:** the body calls `self.pop()` (itself) instead of `self.vec.pop()`,
  so any call overflows the stack and aborts the process. The guard is also
  off by one: `self.vec.len() >= self.start_at` allows popping a byte owned
  by a previous codec when `len == start_at`.
- **Fix:**
  ```rust
  pub fn pop(&mut self) -> Option<u8> {
      if self.vec.len() > self.start_at { self.vec.pop() } else { None }
  }
  ```
- **Tests:** pushing one byte then popping returns it and leaves the vec empty;
  after a prefix written by another codec (e.g. via
  `ComposeCodec<(U8, MyPopCodec)>`), `pop` on an empty dirty section returns
  `None` and the prefix is intact.

### T2 ✅ `EncodingVec<Dirty>::resize` computes the wrong length

- **Where:** `src/codec/encoding.rs`, `fn resize` (~line 158).
- **Problem:** it does
  `let len = new_len.saturating_sub(self.vec.len()); self.vec.resize(self.start_at + len, value)`.
  It works only when `start_at == 0`. With a 4-byte prefix, `resize(2)`
  leaves 4 bytes instead of 6.
- **Fix:** `self.vec.resize(self.start_at + new_len, value)` (`new_len` is relative,
  like the other relative methods).
- **Test:** a custom codec calling `resize(2, 7)`, used as the second element of
  `ComposeCodec<(U32<BE>, ThatCodec)>`, must produce 6 bytes ending with `[7, 7]`.
  Also test shrinking: it must never truncate below `start_at`.

### T3 ✅ `SizedCodec` silently accepts truncated data

- **Where:** `src/codec/sized_codec.rs`, `impl Decode for SizedCodec` (~line 57); root
  cause in `DecodingVec::take_next` (`src/codec/decoding.rs`, ~line 57) which
  clamps to the available length.
- **Problem:** bytes `[0,0,0,10,b'h',b'i']` decode as `Ok("hi")` although the
  header announces 10 bytes. Corrupted data goes undetected.
- **Fix:** in `SizedCodec::decode`, check that at least `size` bytes remain
  before calling `take_next`; otherwise return an error. Either add a variant
  (e.g. `SizedCodecError::Truncated { expected: usize, available: usize }`) or
  reuse/rename `TooSmall` and update its `Display` message. Consider adding a
  `DecodingVec::remaining(&self) -> usize` helper. Keep `take_next`'s clamping
  behavior (or make it return `Option`/`Result` — your call, but update callers).
- **Test:** the bytes above must return `Err`. The existing `sized` test must still pass.

### T4 ✅ `SerdeJson` reads past its own value

- **Where:** `src/codec/serde_json.rs`, `impl Decode for SerdeJson` (~line 33).
- **Problem:** `serde_json::Deserializer::from_reader(...).into_iter().next()`
  must peek one byte after a top-level number to know it ended, and consumes
  that byte from the `DecodingVec`. So `ComposeCodec<(SerdeJson<u32>, U8)>`
  encoding `(12, 9)` fails to decode with `UnexpectedEof`. It is also slow
  (byte-by-byte `Read`).
- **Fix (recommended):** make it a final codec like the other serde/facet
  codecs: `serde_json::from_slice(&bytes.consume())`. Add the standard
  "/!\ This codec is final…" doc line (see T9 for the doc wording). Remove the
  now-unused `de::Error` import.
- **Test:** round-trip of a bare number (`SerdeJson<u32>`) and of a struct;
  `SizedCodec<SerdeJson<u32>>` followed by `U8` in a `ComposeCodec` round-trips.

### T5 ✅ `DecodeIgnore` doesn't advance the cursor

- **Where:** `src/codec/mod.rs`, `impl Decode for DecodeIgnore` (~line 70).
- **Problem:** it reads nothing, so the next codec in a compose re-reads the same
  bytes: decoding `[1, 2]` with `ComposeCodec<(DecodeIgnore, U8)>` gives `1`.
- **Fix:** move the cursor to the end without allocating. Add a helper on
  `DecodingVec`, e.g. `pub fn skip_to_end(&mut self)` (set cursor position to
  the inner length), and call it. Document `DecodeIgnore` as final (it ignores
  everything until the end; to ignore a middle field, wrap it:
  `SizedCodec<DecodeIgnore>`). Update its doc comment, which currently
  mentions "a useless allocation" — no longer true once fixed.
- **Test:** `ComposeCodec<(SizedCodec<DecodeIgnore>, U8)>` decodes the U8
  correctly; `DecodeIgnore` alone leaves `bytes.is_empty() == true`.

### T6 `Rkyv` decoding relies on allocator alignment (not reproduced)

- **Where:** `src/codec/rkyv.rs`, `impl Decode for Rkyv` (~line 52).
- **Problem:** `rkyv::from_bytes` requires a buffer aligned for the archived
  root, but it receives a plain `Vec<u8>` from `consume()`. It works today only
  because the system allocator happens to over-align; another global allocator
  or a sub-slice copy can make validation fail.
- **Fix:** copy into `rkyv::util::AlignedVec` (e.g.
  `let mut aligned = AlignedVec::<16>::new(); aligned.extend_from_slice(&bytes.consume());`)
  then `rkyv::from_bytes(&aligned)`. While there, replace
  `ret.extend(bytes.into_iter().copied())` in `encode` by
  `ret.extend(bytes.iter())` / a slice extend.
- **Test:** existing `encode_and_decode` test still passes; optionally decode
  from a deliberately misaligned slice (copy into a `Vec` at offset 1) and
  check it succeeds.

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
  Use [`OrderedI32`] (T13) if you need numeric ordering." The macro builds docs
  with `#[doc = ...]`; add a parameter or a separate `#[doc]` line for signed
  types. If T13 is not done yet, omit the link and just state the caveat.

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
     self-delimiting codecs (see Context above), and that every element of a
     `ComposeCodec` except the last must be self-delimiting (wrap final codecs
     in `SizedCodec`, or use the terminated codec from T14). Show that
     `ComposeCodec<(Str, Str)>` encodes `("ab","cd")` and decodes as
     `("abcd", "")`.
  2. Fix the broken intra-doc links: ``[`Sized`]`` currently resolves to
     `std::marker::Sized`. Replace with ``[`SizedCodec`]`` in `bytes.rs`,
     `str.rs`, `rkyv.rs`, `facet_json.rs`, `facet_postcard.rs`,
     `facet_msgpack.rs`.
  3. Add the "final" line to codecs missing it: `LazyDecode`, `SerdePostcard`,
     `SerdeJson` (after T4), `DecodeIgnore` (after T5), `Unit` (T10).
  4. Document `ComposeCodec` itself (currently has no doc comment).
  5. Fix the `Bytes` doc: it says "totally borrowed" but decodes into an owned `Vec<u8>`.

### T10 `Unit` is final

- **Where:** `src/codec/unit.rs`.
- **Context:** `Unit::decode` fails with `NonEmptyError` if any byte remains,
  so `ComposeCodec<(Unit, U8)>` encodes fine but cannot decode.
- **Do:** document it as final ("must be the last element of a compose; it
  verifies nothing remains"). Behavior stays unchanged — owner decided. Add a
  test asserting `(U8, Unit)` round-trips.

### T11 `SizedCodec` doesn't preserve key ordering

- **Where:** `src/codec/sized_codec.rs`.
- **Context:** because of the u32 length prefix, `"b"` sorts before `"aa"`,
  and a prefix search on a partial string cannot work. This is expected.
- **Do:** add a doc comment on `SizedCodec` (there is none) describing the
  format (u32 big-endian length, then the inner bytes), that it's ideal for
  values / non-ordered keys, and point to the terminated codec (T14) for
  ordered string keys. Mention the 4 GiB limit (see T25 for the silent
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

### T13 Order-preserving signed integer codecs

- **Where:** new code in `src/codec/integer.rs` (or a new `ordered.rs`
  re-exported from `src/codec/mod.rs`).
- **Do:** add `OrderedI8`, `OrderedI16`, `OrderedI32`, `OrderedI64`,
  `OrderedI128` (naming open; `SortableI32` also fine). Always big-endian (no
  byte order parameter). Encoding: flip the sign bit, i.e.
  `((x as uN) ^ (1 << (BITS-1))).to_be_bytes()`; decoding does the inverse.
  Self-delimiting (fixed size). Decode error: `std::io::Error` for consistency
  with the other integer codecs.
- **Tests:** round-trip `MIN`, `-1`, `0`, `1`, `MAX`; byte order of encodings
  matches numeric order for a sorted list of values; a keyspace test inserting
  `-5..5` and checking `range(-3..3)` yields 6 keys in order and
  `first_key_value` is `-5`.
- **Follow-up (optional):** `OrderedF32`/`OrderedF64` with the IEEE trick (if
  sign bit set, flip all bits; else flip only the sign bit). Plain `F32`/`F64`
  codecs don't exist yet either (see T29).

### T14 Terminated (NUL-delimited) string codec

- **Where:** new file `src/codec/terminated.rs`, re-exported in `src/codec/mod.rs`.
- **Goal:** a self-delimiting string codec that preserves lexicographic order,
  usable anywhere in a `ComposeCodec` key.
- **Spec:**
  - Name suggestion: `NulStr` (or `TerminatedStr`). `Item = str`, decodes to `String`.
  - Encode: write the UTF-8 bytes then `0x00`. If the string contains `\0`,
    return an error (new error type, e.g. `ContainsNulError { position: usize }`,
    implementing `Display` + `std::error::Error`). Rust `str` can contain interior
    NULs, so this check is mandatory.
  - Decode: read up to the first `0x00`, consume it, return the string.
    Errors: missing terminator, invalid UTF-8 (one error enum).
  - Ordering: `"a\0" < "aa\0" < "b\0"` — natural lexicographic order.
- **Doc caveat to write:** a `prefix` search for every key starting with `"a"`
  must use the raw prefix *without* the terminator; `ks.prefix(&rtxn, "a")`
  with this codec only matches the exact key `"a"` (plus whatever follows it in a
  compose). Mention that the partial-key prefix API (T29) would solve this.
- **Tests:** round-trip (including empty string and unicode); `\0` input errors;
  missing terminator errors; `ComposeCodec<(NulStr, NulStr)>` round-trips
  `("ab","cd")`; keyspace test where `("b",0)` and `("aa",0)` keys iterate in
  order `"aa"`, `"b"`.
- **Optional follow-up:** escaped bytes variant for `[u8]` (any byte allowed):
  encode `0x00` as `0x00 0xFF`, terminate with `0x00 0x00` (FoundationDB tuple
  layer style). Preserves ordering.

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
  it's fine). Optionally make `DecodingVec` hold a `Slice` directly.

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

### T24 (Optional) Compile-time check for final codecs in `ComposeCodec`

- **Where:** `src/codec/compose_codec.rs`, all codecs.
- **Do:** ask the owner first. Idea: a marker trait `SelfDelimiting` implemented
  by self-delimiting codecs, required on every element but the last in the
  `ComposeCodec` impls. Note that the existing test
  `make_sure_all_tuple_size_support_encoding_and_decoding` uses `(&Str, &Str, …)`,
  which is the broken pattern; switch it to `SizedCodec<Str>`.

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
- Redundant `$C::Item: Sized` bound in the `Decode` impl of the `compose_impl!`
  macro (`Decode::Item` is always `Sized`).
- `Str {}` and `Unit {}` are constructible empty structs while other codecs are
  uninhabited enums → make them `pub enum Str {}` / `pub enum Unit {}`.

### T29 Missing features

- No `F32`/`F64` codecs (`byteorder` supports `read_f32`/`write_f32`; the
  `define_type!` macro can be reused).
- `Keyspace::prefix` requires a full `Key::Item`, so you can't prefix-search by
  the first fields of a compose key. Idea: `prefix_with::<PrefixCodec>(&rtxn,
  &item)` that encodes with another codec, or a `prefix_raw(&[u8])`. Ask owner.

### T30 Clippy and cosmetics

- Fix the 21 clippy warnings (`cargo clippy --all-features --all-targets`):
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

1. T1, T2 (one-liners) → T3, T4, T5, T6.
2. T13, T14 (new codecs) — so the docs can link to them.
3. Docs batch T7–T12, T15, T26.
4. API tasks T16–T23 (T17, T22, T24 are breaking or need the owner's decision).
5. Nits T25, T27–T30.

Dependencies: T7 and T11 link to T13/T14; T9 references T4, T5, T10 and T14;
T27's clippy CI job needs T30 first.
