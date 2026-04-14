`fiole` is a high level opinionated wrapper around [`fjall`].

# Why "opinionated"

It's because I made some big changes to the `fjall` API that I didn't like. Here's a non exhaustive list:

- I only re-exposed the [`fjall::OptimisticTxDatabase`] because I don't think I ever needed a database without transaction.
- I got rids of all the methods that runs wrapped in a transaction like [`fjall::OptimisticTxKeyspace::take`], forcing you to always use a transaction. I find that less error prone.
- I "inverted" the way we access to data. Instead of doing `txn.get(keyspace)` we're now doing `keyspace.get(txn)`. I find that easier to understand as a end user.

# Why "high level"

Because we have _types_, you can now specify the type of the key values in a keyspace through a codec system.
See all the codec availabe at [`codec`].

# Example

```rust
use std::fs;
use std::path::Path;
use fiole::{Keyspace, KeyspaceCreateOptions, Database};
use fiole::codec::{Str, U32};

let dir = tempfile::tempdir().unwrap();
let db = Database::builder(&dir.path()).unwrap();

let ks: Keyspace<Str, U32<byteorder::NativeEndian>> = db.keyspace("Hey you!", || KeyspaceCreateOptions::default()).unwrap();

// opening a write transaction
let mut wtxn = db.write_tx().unwrap();

ks.insert(&mut wtxn, "seven", &7).unwrap();
ks.insert(&mut wtxn, "zero", &0).unwrap();
ks.insert(&mut wtxn, "five", &5).unwrap();
ks.insert(&mut wtxn, "three", &3).unwrap();
wtxn.commit().unwrap();

// opening a read transaction
// to check if those values are now available
let mut rtxn = db.read_tx();

let ret = ks.get(&rtxn, "zero").unwrap();
assert_eq!(ret, Some(0));

let ret = ks.get(&rtxn, "five").unwrap();
assert_eq!(ret, Some(5));
```
