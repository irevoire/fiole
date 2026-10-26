`fiole` is a high level opinionated wrapper around [`fjall`].

# Why "opinionated"

It's because I made some big changes to the `fjall` API that I didn't like. Here's a non exhaustive list:

- I only re-exposed the [`fjall::OptimisticTxDatabase`] because I don't think I ever needed a database without transaction.
- I got rid of all the methods that run wrapped in a transaction like [`fjall::OptimisticTxKeyspace::take`], forcing you to always use a transaction. I find that less error prone.
- I "inverted" the way we access to data. Instead of doing `txn.get(keyspace)` we're now doing `keyspace.get(txn)`. I find that easier to understand as a end user.

# Why "high level"

Because we have _types_, you can now specify the type of the key values in a keyspace through a codec system.
See all the codecs available at [`codec`].

# Example

```rust
use std::fs;
use std::path::Path;
use fiole::{KeyspaceCreateOptions, Database};
use fiole::codec::{Str, U32};

let dir = tempfile::tempdir().unwrap();
let db = Database::builder(&dir.path()).unwrap();

// Any byte order is fine for a value, but use `OrderedU32` for keys if you want them to be sorted numerically
let ks = db.keyspace::<Str, U32<byteorder::NativeEndian>>("Hey you!", KeyspaceCreateOptions::default).unwrap();

// opening a write transaction
let mut wtxn = db.write_tx().unwrap();

ks.insert(&mut wtxn, "seven", &7).unwrap();
ks.insert(&mut wtxn, "zero", &0).unwrap();
ks.insert(&mut wtxn, "five", &5).unwrap();
ks.insert(&mut wtxn, "three", &3).unwrap();
wtxn.commit().unwrap().unwrap();

// opening a read transaction
// to check if those values are now available
let mut rtxn = db.read_tx();

let ret = ks.get(&rtxn, "zero").unwrap();
assert_eq!(ret, Some(0));

let ret = ks.get(&rtxn, "five").unwrap();
assert_eq!(ret, Some(5));
```

# Prefixed keyspaces

Fiole gives you the ablity to create "fake" keyspaces living inside of a "root"
keyspaces. All their keys are prefixed by used defined bytes, and they gives
you a seamless window into the root keyspace.

Every fjall keyspace comes with its own memtable, buffers and other internal
state, so each new keyspace uses memory that could have served a single one.
A good rule of thumb is to create one keyspace per storage pattern you need:
the data that needs different options (e.g. key-value separation or not), or
that you want to be able to drop in a single call. Inside a keyspace, split
your data with key prefixes.

In `fiole` every [`Keyspace`] has a prefix. The one returned by
[`Database::keyspace`] has an empty prefix, and [`Keyspace::prefixed_raw`] or
[`Keyspace::prefixed`] (which specifies the first part of a key made of parts)
give you a new keyspace whose keys all start with a prefix. The prefix is written before
every key and stripped when reading them back, and you can chain them as much
as you want.

```rust
use fiole::{KeyspaceCreateOptions, Database};
use fiole::codec::{ComposeCodec, OrderedU32, Str};

let dir = tempfile::tempdir().unwrap();
let db = Database::builder(dir.path()).unwrap();
let root = db.keyspace::<Str, Str>("data", KeyspaceCreateOptions::default).unwrap();

// Ending the prefixes with `\0` is a good default to keep sibling prefixes apart
let users = root.prefixed_raw(b"users\0");
let posts = root.prefixed_raw(b"posts\0");
// Prefixes can be chained, and specify the first part of a key made of parts:
// here the fields of each user, by the id of the user then the name of the field
let kefir = users
    .remap_key_type::<ComposeCodec<(OrderedU32, Str)>>()
    .prefixed(&42)
    .unwrap();

let mut wtxn = db.write_tx().unwrap();
kefir.insert(&mut wtxn, "name", "Kefir").unwrap();
posts.insert(&mut wtxn, "hello", "Hello world").unwrap();

// Every method only sees the keys of its own prefix
assert_eq!(kefir.get(&wtxn, "name").unwrap().as_deref(), Some("Kefir"));
assert_eq!(posts.iter(&wtxn).count(), 1);
assert_eq!(users.get(&wtxn, "name").unwrap(), None);

// The keys are returned without their prefix, `raw_key` gives the full key
let guard = kefir.iter(&wtxn).next().unwrap();
assert_eq!(&*guard.raw_key().unwrap(), b"users\0\0\0\0\x2aname");

// Clearing a keyspace removes its keys and the ones of the keyspaces prefixed from it
users.clear(&mut wtxn).unwrap();
assert!(kefir.is_empty(&wtxn).unwrap());
assert_eq!(posts.len(&wtxn).unwrap(), 1);
wtxn.commit().unwrap().unwrap();
```

A few things to keep in mind:

- A keyspace doesn't know if it has prefixes and will iterate on all of it's
  keys, including the one of its inner keyspaces.
- The prefixes are simply concatenated, avoiding collisions is up to you: the
  prefix `ab` followed by the key `c` is the same key as the prefix `a`
  followed by the key `bc`. Ending your prefixes with `\0`, or giving them a
  fixed size, keeps sibling prefixes apart: `users\0` and `users2\0` can't
  overlap. A keyspace can still write keys that fall under the prefix of one
  of its children, e.g. `users` writing the key `\0\0\0\x2aname`.
- The prefix counts toward the 65535 bytes fjall allows per key, the methods
  taking a key panic if the prefix followed by the key is longer.
- The methods working on the whole fjall keyspace, like
  [`Keyspace::approximate_len`], are only available on the root.
