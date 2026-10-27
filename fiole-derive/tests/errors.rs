//! Every error `#[derive(Codec)]` can lead to, with its message: each case of
//! `tests/ui` must fail to compile with exactly its `.stderr`. When a new
//! version of rustc words them differently, `TRYBUILD=overwrite cargo test`
//! writes them again, to review like a snapshot.

#[test]
fn errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
