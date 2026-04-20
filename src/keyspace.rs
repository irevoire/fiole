use fjall::OptimisticTxKeyspace;
use fjall::Readable as _;
use std::{
    convert::Infallible,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
    path::PathBuf,
};

use crate::{
    codec::{Decode, Encode},
    txn::Readable,
    Error, Guard, Iter, Wtxn,
};

/// Wrapper around a [`fjall::OptimisticTxKeyspace`].
/// You must specify the type of its key and value through the [`crate::codec`].
#[repr(transparent)]
pub struct Keyspace<Key, Value> {
    pub(crate) inner: OptimisticTxKeyspace,
    pub(crate) marker: PhantomData<(Key, Value)>,
}

impl<Key, Value> Clone for Keyspace<Key, Value> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: self.marker.clone(),
        }
    }
}

impl<Key, Value> Keyspace<Key, Value> {
    /// Change the codec of the key.
    #[inline]
    #[must_use]
    pub fn remap_key_type<NKey>(&self) -> Keyspace<NKey, Value> {
        Keyspace {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }

    /// Change the codec of the value.
    #[inline]
    #[must_use]
    pub fn remap_value_type<NValue>(&self) -> Keyspace<Key, NValue> {
        Keyspace {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }

    /// Change the codec of the key and value.
    #[inline]
    #[must_use]
    pub fn remap_types<NKey, NValue>(&self) -> Keyspace<NKey, NValue> {
        Keyspace {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }

    /// Returns the underlying LSM-tree's path.
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.inner.path()
    }

    /// Approximates the amount of items in the keyspace.
    ///
    /// For update- or delete-heavy workloads, this value will
    /// diverge from the real value, but is a O(1) operation.
    ///
    /// For insert-only workloads (e.g. logs, time series)
    /// this value is reliable.
    ///
    /// See [`fjall::OptimisticTxKeyspace::approximate_len`] for more info.
    #[inline]
    pub fn approximate_len(&self) -> usize {
        self.inner.approximate_len()
    }

    /// Returns the first key-value pair in the keyspace.
    /// The key in this pair is the minimum key in the keyspace.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// # let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "my_value").unwrap();
    /// ks.insert(&mut wtxn, "b", "my_value").unwrap();
    ///
    /// assert_eq!("a", &*ks.first_key_value(&mut wtxn).unwrap().key().unwrap());
    /// ```
    #[inline]
    pub fn first_key_value(&self, rtxn: &impl Readable) -> Option<Guard<Key, Value>> {
        rtxn.inner().first_key_value(&self.inner).map(Guard::new)
    }

    /// Returns the last key-value pair in the keyspace.
    /// The key in this pair is the maximum key in the keyspace.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// # let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "my_value").unwrap();
    /// ks.insert(&mut wtxn, "b", "my_value").unwrap();
    ///
    /// assert_eq!("b", &*ks.last_key_value(&mut wtxn).unwrap().key().unwrap());
    /// ```
    #[inline]
    pub fn last_key_value(&self, rtxn: &impl Readable) -> Option<Guard<Key, Value>> {
        rtxn.inner().last_key_value(&self.inner).map(Guard::new)
    }

    /// Returns `true` if the parkeyspacetition is empty.
    ///
    /// This operation has `O(log N)` complexity.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, Rtxn, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// assert!(ks.is_empty(&wtxn).unwrap());
    ///
    /// ks.insert(&mut wtxn, "a", "abc").unwrap();
    /// assert!(!ks.is_empty(&wtxn).unwrap());
    /// ```
    #[inline]
    pub fn is_empty(&self, rtxn: &impl Readable) -> Result<bool, fjall::Error> {
        rtxn.inner().is_empty(&self.inner)
    }

    /// Scans the entire keyspace, returning the number of items.
    ///
    /// # Caution
    ///
    /// This operation scans the entire keyspace: `O(n)` complexity!
    ///
    /// Never, under any circumstances, use .`len()` == 0 to check
    /// if the keyspace is empty, use [`Keyspace::is_empty`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, Readable, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "my_value").unwrap();
    /// ks.insert(&mut wtxn, "b", "my_value2").unwrap();
    /// wtxn.commit().unwrap();
    ///
    /// let rtxn = db.read_tx();
    ///
    /// assert_eq!(2, ks.len(&rtxn).unwrap());
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    ///
    /// ks.insert(&mut wtxn, "c", "my_value3").unwrap();
    ///
    /// // Repeatable read
    /// assert_eq!(2, ks.len(&rtxn).unwrap());
    ///
    /// // From the pov of the write txn the len is updated
    /// assert_eq!(3, ks.len(&wtxn).unwrap(), "hello");
    /// wtxn.commit().unwrap();
    ///
    /// // Once we commit we can still read the old value
    /// assert_eq!(2, ks.len(&rtxn).unwrap());
    ///
    /// // Or the new value if we take a rtxn
    /// let rtxn2 = db.read_tx();
    /// assert_eq!(3, ks.len(&rtxn2).unwrap(), "world");
    /// ```
    #[inline]
    pub fn len(&self, rtxn: &impl Readable) -> Result<usize, fjall::Error> {
        rtxn.inner().len(&self.inner)
    }

    /// Iterates over the snapshot.
    ///
    /// Avoid using this function, or limit it as otherwise it may scan a lot of items.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "abc").unwrap();
    /// ks.insert(&mut wtxn, "f", "abc").unwrap();
    /// ks.insert(&mut wtxn, "g", "abc").unwrap();
    ///
    /// assert_eq!(3, ks.iter(&wtxn).count());
    /// ```
    #[inline]
    pub fn iter(&self, rtxn: &impl Readable) -> Iter<Key, Value> {
        Iter::new(rtxn.inner().iter(&self.inner))
    }
}

impl<'a, Key: Encode<'a>, Value: Decode> Keyspace<Key, Value> {
    /// Retrieves an item from the snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "my_value").unwrap();
    /// wtxn.commit().unwrap();
    ///
    /// let rtxn = db.read_tx();
    /// let item = ks.get(&rtxn, "a").unwrap();
    /// assert_eq!(Some("my_value"), item.as_deref());
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "b", "my_updated_value").unwrap();
    /// wtxn.commit().unwrap();
    ///
    /// // Repeatable read
    /// let item = ks.get(&rtxn, "a").unwrap();
    /// assert_eq!(Some("my_value"), item.as_deref());
    /// ```
    #[inline]
    pub fn get(
        &self,
        rtxn: &impl Readable,
        key: &'a Key::Item,
    ) -> Result<Option<Value::Item>, Error<Key::Error, Value::Error>> {
        let key = Key::encode_alloc(key).map_err(Error::Key)?.finish();

        match rtxn.inner().get(&self.inner, key).map_err(Error::Fjall)? {
            Some(value) => Value::decode(&mut value.into())
                .map(Some)
                .map_err(Error::Value),
            None => Ok(None),
        }
    }

    /// Removes an item and returns its value if it existed.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, Readable, codec::Str};
    /// # use std::sync::Arc;
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "abc").unwrap();
    ///
    /// let taken = ks.take(&mut wtxn, "a").unwrap().unwrap();
    /// assert_eq!("abc", &*taken);
    ///
    /// let item = ks.get(&wtxn, "a").unwrap();
    /// assert!(item.is_none());
    /// ```
    #[inline]
    pub fn take(
        &self,
        wtxn: &mut Wtxn,
        key: &'a Key::Item,
    ) -> Result<Option<Value::Item>, Error<Key::Error, Value::Error>> {
        let key = Key::encode_alloc(key).map_err(Error::Key)?.finish();
        match wtxn.inner.take(&self.inner, key).map_err(Error::Fjall)? {
            Some(value) => Value::decode(&mut value.into())
                .map(Some)
                .map_err(Error::Value),
            None => Ok(None),
        }
    }
}

impl<'a, Key: Encode<'a>, Value> Keyspace<Key, Value> {
    /// A typed version of [`fjall::Readable::contains_key`], see the original documentation for more infos.
    #[inline]
    pub fn contains_key(
        &self,
        rtxn: &impl Readable,
        key: &'a Key::Item,
    ) -> Result<bool, Error<Key::Error, Infallible>> {
        let key = Key::encode_alloc(key).map_err(Error::Key)?.finish();
        rtxn.inner()
            .contains_key(&self.inner, key)
            .map_err(Error::Fjall)
    }

    /// A typed version of [`fjall::Readable::size_of`], see the original documentation for more infos.
    #[inline]
    pub fn size_of(
        &self,
        rtxn: &impl Readable,
        key: &'a Key::Item,
    ) -> Result<Option<u32>, Error<Key::Error, Infallible>> {
        let key = Key::encode_alloc(key).map_err(Error::Key)?.finish();
        rtxn.inner().size_of(&self.inner, key).map_err(Error::Fjall)
    }

    #[inline]
    pub fn range<R: RangeBounds<Key::Item> + 'a>(
        &self,
        rtxn: &impl Readable,
        range: &'a R,
    ) -> Result<Iter<Key, Value>, Key::Error> {
        let start = match range.start_bound() {
            Bound::Included(key) => Bound::Excluded(Key::encode_alloc(key)?.finish()),
            Bound::Excluded(key) => Bound::Included(Key::encode_alloc(key)?.finish()),
            Bound::Unbounded => Bound::Unbounded,
        };
        let end = match range.end_bound() {
            Bound::Included(key) => Bound::Excluded(Key::encode_alloc(key)?.finish()),
            Bound::Excluded(key) => Bound::Included(Key::encode_alloc(key)?.finish()),
            Bound::Unbounded => Bound::Unbounded,
        };

        Ok(Iter::new(rtxn.inner().range(&self.inner, (start, end))))
    }

    /// A typed version of [`fjall::Readable::prefix`], see the original documentation for more infos.
    #[inline]
    pub fn prefix(
        &self,
        rtxn: &impl Readable,
        prefix: &'a Key::Item,
    ) -> Result<Iter<Key, Value>, Key::Error> {
        let prefix = Key::encode_alloc(prefix)?.finish();

        Ok(Iter::new(rtxn.inner().prefix(&self.inner, prefix)))
    }

    /// Removes an item from the keyspace.
    ///
    /// The key may be up to 65536 bytes long.
    /// Shorter keys result in better performance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, Readable, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "abc").unwrap();
    /// assert!(!ks.is_empty(&wtxn).unwrap());
    ///
    /// ks.remove(&mut wtxn, "a").unwrap();
    /// assert!(ks.is_empty(&wtxn).unwrap());
    /// ```
    #[inline]
    pub fn remove(&self, wtxn: &mut Wtxn, key: &'a Key::Item) -> Result<(), Key::Error> {
        let key = Key::encode_alloc(key)?.finish();
        wtxn.inner.remove(&self.inner, key);
        Ok(())
    }
}

impl<'a, Key: Encode<'a>, Value: Encode<'a>> Keyspace<Key, Value> {
    /// Inserts a key-value pair into the keyspace.
    ///
    /// Keys may be up to 65536 bytes long, values up to 2^32 bytes.
    /// Shorter keys and values result in better performance.
    ///
    /// If the key already exists, the item will be overwritten.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, Readable, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "abc").unwrap();
    ///
    /// assert!(!ks.is_empty(&wtxn).unwrap());
    /// ```
    #[inline]
    pub fn insert(
        &self,
        wtxn: &mut Wtxn,
        key: &'a Key::Item,
        value: &'a Value::Item,
    ) -> Result<(), Error<Key::Error, Value::Error>> {
        let key = Key::encode_alloc(key).map_err(Error::Key)?.finish();
        let value = Value::encode_alloc(value).map_err(Error::Value)?.finish();
        wtxn.inner.insert(&self.inner, key, value);
        Ok(())
    }
}
