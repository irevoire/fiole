//! Every error `#[derive(Fiole)]` can lead to, with its message: each case of
//! `tests/ui` must fail to compile with exactly its `.stderr`.
//!
//! Their wording changes with the version of rustc, so the `.stderr` are
//! written with the one of `rust-toolchain.toml`, which every cargo command
//! of the repository uses. When it's bumped,
//! `TRYBUILD=overwrite cargo test -p fiole-derive --test errors` writes them
//! again, to review like a snapshot.

#[test]
fn errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
