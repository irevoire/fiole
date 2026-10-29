#![doc = include_str!("../README.md")]
// Says which feature enables each item in the documentation of docs.rs
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/irevoire/fiole/refs/heads/main/assets/favicon.svg"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/irevoire/fiole/refs/heads/main/assets/logo.svg"
)]

use std::{convert::Infallible, marker::PhantomData};

pub mod codec;
mod database;
#[cfg(all(test, feature = "derive"))]
mod derive_tests;
mod error;
mod key_range;
mod keyspace;
#[cfg(test)]
mod test_alloc;
pub(crate) mod txn;

pub use database::Database;
pub use error::Error;
pub use fjall::{Conflict, KeyspaceCreateOptions, PersistMode};
pub use key_range::KeyRange;
pub use keyspace::{Keyspace, Prefixed, Root};
pub use txn::{Rtxn, Wtxn};

#[cfg(feature = "derive")]
pub use fiole_derive::Fiole;

#[cfg(feature = "derive")]
#[doc(hidden)]
pub mod derive_support;

// For the code `#[derive(Fiole)]` generates, which names `::fiole`, to also
// work in the tests and examples of fiole itself.
extern crate self as fiole;

pub use byteorder;
pub use byteview;
pub use fjall;

/// Guard to access key-value pairs.
pub struct Guard<Key, Value> {
    inner: fjall::Guard,
    /// Length of the prefix of the [`Keyspace`] the key was read from.
    prefix_len: usize,
    marker: PhantomData<(Key, Value)>,
}

impl<Key, Value> Guard<Key, Value> {
    #[inline]
    pub(crate) fn new(inner: fjall::Guard, prefix_len: usize) -> Self {
        Self {
            inner,
            prefix_len,
            marker: PhantomData,
        }
    }

    /// Change the codec of the key.
    #[inline]
    #[must_use]
    pub fn remap_key_type<NKey>(self) -> Guard<NKey, Value> {
        Guard::new(self.inner, self.prefix_len)
    }

    /// Change the codec of the value.
    #[inline]
    #[must_use]
    pub fn remap_value_type<NValue>(self) -> Guard<Key, NValue> {
        Guard::new(self.inner, self.prefix_len)
    }

    /// Change the codec of the key and value.
    #[inline]
    #[must_use]
    pub fn remap_types<NKey, NValue>(self) -> Guard<NKey, NValue> {
        Guard::new(self.inner, self.prefix_len)
    }

    /// For more information, refer to [`fjall::Guard::size`].
    #[inline]
    pub fn size(self) -> Result<u32, fjall::Error> {
        self.inner.size()
    }

    /// Returns the key as it's stored in fjall, including the prefix of the
    /// [`Keyspace`] it was read from.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let users = ks.prefixed_raw(b"users\0");
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// users.insert(&mut wtxn, "kefir", "hello").unwrap();
    ///
    /// assert_eq!(&*users.iter(&wtxn).next().unwrap().key().unwrap(), "kefir");
    /// assert_eq!(&*users.iter(&wtxn).next().unwrap().raw_key().unwrap(), b"users\0kefir");
    /// ```
    #[inline]
    pub fn raw_key(self) -> Result<fjall::UserKey, fjall::Error> {
        self.inner.key()
    }
}

/// Decodes a key read from fjall, without its keyspace prefix.
#[inline]
fn decode_key<Key: codec::Decode>(
    key: &fjall::UserKey,
    prefix_len: usize,
) -> Result<Key::Item, Key::Error> {
    debug_assert!(
        key.len() >= prefix_len,
        "the key is shorter than its prefix"
    );
    let key = byteview::ByteView::from(key.clone()).slice(prefix_len..);
    Key::decode(&mut key.into())
}

impl<Key: codec::Decode, Value> Guard<Key, Value> {
    /// For more information, refer to [`fjall::Guard::key`].
    #[inline]
    pub fn key(self) -> Result<Key::Item, Error<Key::Error, Infallible>> {
        let key = self.inner.key().map_err(Error::Fjall)?;
        decode_key::<Key>(&key, self.prefix_len).map_err(Error::Key)
    }
}

impl<Key, Value: codec::Decode> Guard<Key, Value> {
    /// For more information, refer to [`fjall::Guard::value`].
    #[inline]
    pub fn value(self) -> Result<Value::Item, Error<Infallible, Value::Error>> {
        let value = self.inner.value().map_err(Error::Fjall)?;
        Value::decode(&mut (&value).into()).map_err(Error::Value)
    }
}

impl<Key: codec::Decode, Value: codec::Decode> Guard<Key, Value> {
    /// For more information, refer to [`fjall::Guard::into_inner`].
    #[inline]
    #[expect(
        clippy::type_complexity,
        reason = "spelling out the precise error types is deliberate"
    )]
    pub fn into_inner(self) -> Result<(Key::Item, Value::Item), Error<Key::Error, Value::Error>> {
        let (k, v) = self.inner.into_inner().map_err(Error::Fjall)?;
        Ok((
            decode_key::<Key>(&k, self.prefix_len).map_err(Error::Key)?,
            Value::decode(&mut (&v).into()).map_err(Error::Value)?,
        ))
    }
}

/// Refer to [`fjall::Iter`] for more info.
pub struct Iter<Key, Value> {
    inner: fjall::Iter,
    /// Length of the prefix of the [`Keyspace`] iterated over.
    prefix_len: usize,
    marker: PhantomData<(Key, Value)>,
}

impl<Key, Value> Iter<Key, Value> {
    #[inline]
    pub(crate) fn new(inner: fjall::Iter, prefix_len: usize) -> Self {
        Self {
            inner,
            prefix_len,
            marker: PhantomData,
        }
    }

    /// Change the codec of the key.
    #[inline]
    pub fn remap_key_type<NKey>(self) -> Iter<NKey, Value> {
        Iter::new(self.inner, self.prefix_len)
    }

    /// Change the codec of the value.
    #[inline]
    pub fn remap_value_type<NValue>(self) -> Iter<Key, NValue> {
        Iter::new(self.inner, self.prefix_len)
    }

    /// Change the codec of the key and value.
    #[inline]
    pub fn remap_types<NKey, NValue>(self) -> Iter<NKey, NValue> {
        Iter::new(self.inner, self.prefix_len)
    }
}

impl<Key, Value> Iterator for Iter<Key, Value> {
    type Item = Guard<Key, Value>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let prefix_len = self.prefix_len;
        self.inner.next().map(|guard| Guard::new(guard, prefix_len))
    }
}

impl<Key, Value> DoubleEndedIterator for Iter<Key, Value> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let prefix_len = self.prefix_len;
        self.inner
            .next_back()
            .map(|guard| Guard::new(guard, prefix_len))
    }
}

#[cfg(test)]
mod test {
    use fjall::KeyspaceCreateOptions;

    use crate::{
        Database,
        codec::{Str, U8},
    };

    #[test]
    fn get_from_wtxn() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default)
            .unwrap();

        // we should be able to get both from a rtxn and a wtxn with the same method and API
        let rtxn = database.read_tx();
        ks.get(&rtxn, "hello").unwrap();
        let wtxn = database.write_tx().unwrap();
        ks.get(&wtxn, "hello").unwrap();
        drop(wtxn);
    }

    #[test]
    fn range_query() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<U8, U8>("hello", KeyspaceCreateOptions::default)
            .unwrap();

        let mut wtxn = database.write_tx().unwrap();
        for i in 0..10 {
            ks.insert(&mut wtxn, &i, &i).unwrap();
        }
        wtxn.commit().unwrap().unwrap();

        let rtxn = database.read_tx();
        let ret: Vec<_> = ks.range(&rtxn, &(4..=5)).unwrap().collect();
        assert_eq!(ret.len(), 2);
    }

    /// Panics if it's given bytes that have been copied out of fjall.
    enum AssertBorrowed {}

    impl crate::codec::Decode for AssertBorrowed {
        type Item = ();
        type Error = std::convert::Infallible;

        fn decode(bytes: &mut crate::codec::DecodingVec) -> Result<(), Self::Error> {
            assert!(!bytes.is_copied(), "the bytes were copied out of fjall");
            bytes.skip_to_end();
            Ok(())
        }
    }

    #[test]
    fn reads_borrow_the_bytes_of_fjall() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<U8, Str>("hello", KeyspaceCreateOptions::default)
            .unwrap();

        let long = "a value longer than the 20 bytes fjall stores inline";
        let mut wtxn = database.write_tx().unwrap();
        ks.insert(&mut wtxn, &1, "short").unwrap();
        ks.insert(&mut wtxn, &2, long).unwrap();
        wtxn.commit().unwrap().unwrap();

        let values = ks.remap_value_type::<AssertBorrowed>();
        let both = ks.remap_types::<AssertBorrowed, AssertBorrowed>();

        let rtxn = database.read_tx();
        values.get(&rtxn, &1).unwrap().unwrap();
        values.get(&rtxn, &2).unwrap().unwrap();
        for guard in both.iter(&rtxn) {
            guard.key().unwrap();
        }
        for guard in both.iter(&rtxn) {
            guard.value().unwrap();
        }
        for guard in both.iter(&rtxn) {
            guard.into_inner().unwrap();
        }

        let mut wtxn = database.write_tx().unwrap();
        values.take(&mut wtxn, &2).unwrap().unwrap();
    }

    #[test]
    fn reads_through_a_prefix_borrow_the_bytes_of_fjall() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(dir.path()).unwrap();
        let ks = database
            .keyspace::<U8, Str>("hello", KeyspaceCreateOptions::default)
            .unwrap()
            .prefixed_raw(b"a prefix");

        let mut wtxn = database.write_tx().unwrap();
        ks.insert(&mut wtxn, &1, "short").unwrap();
        wtxn.commit().unwrap().unwrap();

        let both = ks.remap_types::<AssertBorrowed, AssertBorrowed>();
        let rtxn = database.read_tx();
        for guard in both.iter(&rtxn) {
            guard.key().unwrap();
        }
        for guard in both.iter(&rtxn) {
            guard.into_inner().unwrap();
        }
    }
}
