use fjall::Readable as _;
use fjall::{OptimisticTxKeyspace, Slice};
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    ops::{Bound, Deref},
    path::PathBuf,
};

use crate::{
    Error, Guard, Iter, KeyRange, Rtxn, Wtxn,
    codec::{Decode, Encode, EncodingVec},
};

/// Marks a [`Keyspace`] whose prefix is known to be empty: the one returned by
/// [`Database::keyspace`](crate::Database::keyspace).
///
/// It gives access to the methods that work on the whole fjall keyspace,
/// like [`Keyspace::approximate_len`] and [`Keyspace::path`].
pub enum Root {}

/// Marks a [`Keyspace`] that may have a prefix, returned by
/// [`Keyspace::prefixed`] and [`Keyspace::prefixed_raw`].
///
/// It's the default kind, so `Keyspace<Key, Value>` is a prefixed keyspace.
/// A `&Keyspace<Key, Value, Root>` derefs to a `&Keyspace<Key, Value>`, and
/// can be converted into one with [`From`].
pub enum Prefixed {}

/// Wrapper around a [`fjall::OptimisticTxKeyspace`].
/// You must specify the type of its key and value through the [`crate::codec`].
///
/// # Prefixes
///
/// fjall recommends using few keyspaces and separating the data with key
/// prefixes rather than opening many keyspaces. Every `Keyspace` has a
/// prefix, written before each of its keys and stripped when they're read
/// back. The one returned by [`Database::keyspace`](crate::Database::keyspace)
/// has an empty prefix and is marked as [`Root`]. [`Keyspace::prefixed`] and
/// [`Keyspace::prefixed_raw`] return a [`Prefixed`] keyspace whose prefix is
/// the one of the current keyspace followed by the given bytes. They can be
/// chained:
///
/// ```
/// # use fiole::{Database, KeyspaceCreateOptions, codec::{Str, U32}, byteorder::BigEndian};
/// #
/// # let folder = tempfile::tempdir().unwrap();
/// # let db = Database::builder(folder).unwrap();
/// let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
/// let users = ks.prefixed_raw(b"users\0");
/// let tamo = users.prefixed::<U32<BigEndian>>(&42).unwrap();
///
/// let mut wtxn = db.write_tx().unwrap();
/// tamo.insert(&mut wtxn, "name", "Tamo").unwrap();
///
/// assert_eq!(tamo.get(&wtxn, "name").unwrap().as_deref(), Some("Tamo"));
/// // The other keyspaces don't see the keys of `tamo` under their own prefix
/// assert_eq!(users.get(&wtxn, "name").unwrap(), None);
/// assert_eq!(ks.get(&wtxn, "name").unwrap(), None);
/// ```
///
/// Every method only sees the keys starting with the prefix: [`Keyspace::iter`],
/// [`Keyspace::len`], [`Keyspace::range`]... So the root, whose prefix is empty,
/// sees all the keys of the fjall keyspace, including the ones written
/// through its prefixed keyspaces.
///
/// The prefixes are simply concatenated, avoiding collisions is up to you: the
/// prefix `ab` followed by the key `c` gives the same bytes as the prefix `a`
/// followed by the key `bc`. Using prefixes of a fixed size, or encoded with a
/// [`SelfDelimiting`](crate::codec::SelfDelimiting) codec, avoids that.
///
/// fjall keys can't be longer than 65535 bytes, and that includes the prefix:
/// every method taking a key panics if the prefix followed by the encoded key
/// is longer than that. Keep the prefixes short.
///
/// The methods working on the whole fjall keyspace, [`Keyspace::approximate_len`]
/// and [`Keyspace::path`], are only available on the root:
///
/// ```compile_fail,E0599
/// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
/// # let folder = tempfile::tempdir().unwrap();
/// # let db = Database::builder(folder).unwrap();
/// let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
/// ks.prefixed_raw(b"users\0").approximate_len();
/// ```
// `repr(C)` so a `Keyspace<_, _, Root>` has the same layout as a
// `Keyspace<_, _, Prefixed>`, see the `Deref` impl.
#[repr(C)]
pub struct Keyspace<Key, Value, Kind = Prefixed> {
    pub(crate) inner: OptimisticTxKeyspace,
    pub(crate) prefix: Slice,
    pub(crate) marker: PhantomData<(Key, Value, Kind)>,
}

impl<Key, Value, Kind> Clone for Keyspace<Key, Value, Kind> {
    fn clone(&self) -> Self {
        self.with_prefix(self.prefix.clone())
    }
}

impl<Key, Value, Kind> fmt::Debug for Keyspace<Key, Value, Kind> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keyspace")
            .field("name", &&**self.inner.inner().name())
            .field(
                "prefix",
                &format_args!("b\"{}\"", self.prefix.escape_ascii()),
            )
            .finish_non_exhaustive()
    }
}

impl<Key, Value> Deref for Keyspace<Key, Value, Root> {
    type Target = Keyspace<Key, Value, Prefixed>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: `Keyspace` is `repr(C)` and the two types only differ by
        // the `Kind` of the `PhantomData`, a ZST, so they have the same layout.
        unsafe { &*(self as *const Self as *const Self::Target) }
    }
}

impl<Key, Value> From<Keyspace<Key, Value, Root>> for Keyspace<Key, Value, Prefixed> {
    #[inline]
    fn from(keyspace: Keyspace<Key, Value, Root>) -> Self {
        Self {
            inner: keyspace.inner,
            prefix: keyspace.prefix,
            marker: PhantomData,
        }
    }
}

impl<Key, Value> Keyspace<Key, Value, Root> {
    /// Returns the underlying LSM-tree's path.
    ///
    /// It's shared by all the keyspaces prefixed from this one.
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.inner.path()
    }

    /// Approximates the amount of items in the keyspace.
    ///
    /// It counts the items of the whole fjall keyspace, including the ones
    /// written through prefixed keyspaces.
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
}

impl<Key, Value, Kind> Keyspace<Key, Value, Kind> {
    /// Change the codec of the key. The prefix is kept.
    #[inline]
    #[must_use]
    pub fn remap_key_type<NKey>(&self) -> Keyspace<NKey, Value, Kind> {
        self.with_prefix(self.prefix.clone())
    }

    /// Change the codec of the value. The prefix is kept.
    #[inline]
    #[must_use]
    pub fn remap_value_type<NValue>(&self) -> Keyspace<Key, NValue, Kind> {
        self.with_prefix(self.prefix.clone())
    }

    /// Change the codec of the key and value. The prefix is kept.
    #[inline]
    #[must_use]
    pub fn remap_types<NKey, NValue>(&self) -> Keyspace<NKey, NValue, Kind> {
        self.with_prefix(self.prefix.clone())
    }

    /// Returns a keyspace whose keys are prefixed by the prefix of this
    /// keyspace followed by `prefix`.
    ///
    /// See [the prefixes section](Keyspace#prefixes).
    ///
    /// The prefix counts toward the 65535 bytes fjall allows per key: the
    /// methods taking a key panic if the prefix followed by the key is longer.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let users = ks.prefixed_raw(b"users\0");
    /// let posts = ks.prefixed_raw(b"posts\0");
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// users.insert(&mut wtxn, "tamo", "Tamo").unwrap();
    /// posts.insert(&mut wtxn, "hello", "Hello world").unwrap();
    ///
    /// assert_eq!(users.len(&wtxn).unwrap(), 1);
    /// assert_eq!(posts.len(&wtxn).unwrap(), 1);
    /// assert_eq!(posts.get(&wtxn, "tamo").unwrap(), None);
    /// // The root sees everything
    /// assert_eq!(ks.get(&wtxn, "users\0tamo").unwrap().as_deref(), Some("Tamo"));
    ///
    /// // Prefixes can be chained
    /// let admins = users.prefixed_raw(b"admins\0");
    /// admins.insert(&mut wtxn, "kero", "Kero").unwrap();
    /// assert_eq!(users.get(&wtxn, "admins\0kero").unwrap().as_deref(), Some("Kero"));
    /// assert_eq!(ks.get(&wtxn, "users\0admins\0kero").unwrap().as_deref(), Some("Kero"));
    /// ```
    #[must_use]
    pub fn prefixed_raw(&self, prefix: impl Into<Slice>) -> Keyspace<Key, Value, Prefixed> {
        let prefix = prefix.into();
        if self.prefix.is_empty() {
            self.with_prefix(prefix)
        } else {
            // Stored inline, without allocating, when it's short enough
            let fused = byteview::ByteView::fused(&self.prefix, &prefix);
            self.with_prefix(fused.into())
        }
    }

    /// Returns a keyspace whose keys are prefixed by the prefix of this
    /// keyspace followed by `prefix` encoded with the codec `C`.
    ///
    /// See [the prefixes section](Keyspace#prefixes).
    ///
    /// The prefix counts toward the 65535 bytes fjall allows per key: the
    /// methods taking a key panic if the prefix followed by the key is longer.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::{Str, U32}, byteorder::BigEndian};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let user = ks.prefixed::<U32<BigEndian>>(&42).unwrap();
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// user.insert(&mut wtxn, "name", "Tamo").unwrap();
    ///
    /// let same = ks.prefixed_raw(42u32.to_be_bytes());
    /// assert_eq!(same.get(&wtxn, "name").unwrap().as_deref(), Some("Tamo"));
    /// ```
    pub fn prefixed<C: Encode>(
        &self,
        prefix: &C::Item,
    ) -> Result<Keyspace<Key, Value, Prefixed>, C::Error> {
        let prefix = C::encode(EncodingVec::with_prefix(&self.prefix, 0), prefix)?;
        Ok(self.with_prefix(prefix.into_fjall_slice()))
    }

    #[inline]
    fn with_prefix<NKey, NValue, NKind>(&self, prefix: Slice) -> Keyspace<NKey, NValue, NKind> {
        Keyspace {
            inner: self.inner.clone(),
            prefix,
            marker: PhantomData,
        }
    }

    /// Iterates over all the keys starting with the prefix.
    #[inline]
    fn raw_iter(&self, rtxn: &Rtxn) -> fjall::Iter {
        if self.prefix.is_empty() {
            rtxn.inner().iter(&self.inner)
        } else {
            rtxn.inner().prefix(&self.inner, &self.prefix)
        }
    }

    #[inline]
    fn guard(&self, guard: fjall::Guard) -> Guard<Key, Value> {
        Guard::new(guard, self.prefix.len())
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
    /// assert_eq!("a", &*ks.first_key_value(&wtxn).unwrap().key().unwrap());
    /// ```
    #[inline]
    pub fn first_key_value(&self, rtxn: &Rtxn) -> Option<Guard<Key, Value>> {
        let guard = if self.prefix.is_empty() {
            rtxn.inner().first_key_value(&self.inner)
        } else {
            self.raw_iter(rtxn).next()
        };
        guard.map(|guard| self.guard(guard))
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
    /// assert_eq!("b", &*ks.last_key_value(&wtxn).unwrap().key().unwrap());
    /// ```
    #[inline]
    pub fn last_key_value(&self, rtxn: &Rtxn) -> Option<Guard<Key, Value>> {
        let guard = if self.prefix.is_empty() {
            rtxn.inner().last_key_value(&self.inner)
        } else {
            self.raw_iter(rtxn).next_back()
        };
        guard.map(|guard| self.guard(guard))
    }

    /// Returns `true` if the keyspace is empty.
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
    pub fn is_empty(&self, rtxn: &Rtxn) -> Result<bool, fjall::Error> {
        if self.prefix.is_empty() {
            rtxn.inner().is_empty(&self.inner)
        } else {
            Ok(self
                .raw_iter(rtxn)
                .next()
                .map(fjall::Guard::key)
                .transpose()?
                .is_none())
        }
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
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "a", "my_value").unwrap();
    /// ks.insert(&mut wtxn, "b", "my_value2").unwrap();
    /// wtxn.commit().unwrap().unwrap();
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
    /// assert_eq!(3, ks.len(&wtxn).unwrap());
    /// wtxn.commit().unwrap().unwrap();
    ///
    /// // Once we commit we can still read the old value
    /// assert_eq!(2, ks.len(&rtxn).unwrap());
    ///
    /// // Or the new value if we take a rtxn
    /// let rtxn2 = db.read_tx();
    /// assert_eq!(3, ks.len(&rtxn2).unwrap());
    /// ```
    #[inline]
    pub fn len(&self, rtxn: &Rtxn) -> Result<usize, fjall::Error> {
        if self.prefix.is_empty() {
            rtxn.inner().len(&self.inner)
        } else {
            let mut count = 0;
            for guard in self.raw_iter(rtxn) {
                guard.key()?;
                count += 1;
            }
            Ok(count)
        }
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
    pub fn iter(&self, rtxn: &Rtxn) -> Iter<Key, Value> {
        Iter::new(self.raw_iter(rtxn), self.prefix.len())
    }

    /// Removes all the keys of the keyspace.
    ///
    /// On a prefixed keyspace, only the keys starting with its prefix are
    /// removed, including the ones of the keyspaces prefixed from it. On the
    /// root, everything is removed.
    ///
    /// The keys are removed one by one in the transaction, so it can be rolled
    /// back, and it conflicts with the transactions writing to one of them.
    ///
    /// # Caution
    ///
    /// This operation reads all the keys to remove them: `O(n)` complexity,
    /// and the keys are held in memory until they're removed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder).unwrap();
    /// let ks = db.keyspace::<Str, Str>("default", KeyspaceCreateOptions::default).unwrap();
    /// let users = ks.prefixed_raw(b"users\0");
    /// let posts = ks.prefixed_raw(b"posts\0");
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// users.insert(&mut wtxn, "tamo", "Tamo").unwrap();
    /// posts.insert(&mut wtxn, "hello", "Hello world").unwrap();
    ///
    /// users.clear(&mut wtxn).unwrap();
    /// assert!(users.is_empty(&wtxn).unwrap());
    /// assert_eq!(posts.len(&wtxn).unwrap(), 1);
    /// ```
    pub fn clear(&self, wtxn: &mut Wtxn) -> Result<(), fjall::Error> {
        let keys = self
            .raw_iter(wtxn)
            .map(fjall::Guard::key)
            .collect::<Result<Vec<_>, _>>()?;
        let wtxn = wtxn.write_inner();
        for key in keys {
            wtxn.remove(&self.inner, key);
        }
        Ok(())
    }
}

impl<Key: Encode, Value, Kind> Keyspace<Key, Value, Kind> {
    /// Encodes `key` after the prefix.
    #[inline]
    fn encode_key(&self, key: &Key::Item) -> Result<Vec<u8>, Key::Error> {
        let headroom = if self.prefix.is_empty() {
            0
        } else {
            KEY_HEADROOM
        };
        let vec = EncodingVec::with_prefix(&self.prefix, headroom);
        Ok(Key::encode(vec, key)?.finish())
    }
}

impl<Key: Encode, Value: Decode, Kind> Keyspace<Key, Value, Kind> {
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
    /// wtxn.commit().unwrap().unwrap();
    ///
    /// let rtxn = db.read_tx();
    /// let item = ks.get(&rtxn, "a").unwrap();
    /// assert_eq!(Some("my_value"), item.as_deref());
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "b", "my_updated_value").unwrap();
    /// wtxn.commit().unwrap().unwrap();
    ///
    /// // Repeatable read
    /// let item = ks.get(&rtxn, "a").unwrap();
    /// assert_eq!(Some("my_value"), item.as_deref());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by the encoded key is longer than
    /// 65535 bytes, the maximum key size of fjall.
    #[inline]
    #[expect(
        clippy::type_complexity,
        reason = "spelling out the precise error types is deliberate"
    )]
    pub fn get(
        &self,
        rtxn: &Rtxn,
        key: &Key::Item,
    ) -> Result<Option<Value::Item>, Error<Key::Error, Value::Error>> {
        let key = self.encode_key(key).map_err(Error::Key)?;

        match rtxn.inner().get(&self.inner, key).map_err(Error::Fjall)? {
            Some(value) => Value::decode(&mut (&value).into())
                .map(Some)
                .map_err(Error::Value),
            None => Ok(None),
        }
    }

    /// Removes an item and returns its value if it existed.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
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
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by the encoded key is longer than
    /// 65535 bytes, the maximum key size of fjall.
    #[inline]
    #[expect(
        clippy::type_complexity,
        reason = "spelling out the precise error types is deliberate"
    )]
    pub fn take(
        &self,
        wtxn: &mut Wtxn,
        key: &Key::Item,
    ) -> Result<Option<Value::Item>, Error<Key::Error, Value::Error>> {
        let key = self.encode_key(key).map_err(Error::Key)?;
        match wtxn
            .write_inner()
            .take(&self.inner, key)
            .map_err(Error::Fjall)?
        {
            Some(value) => Value::decode(&mut (&value).into())
                .map(Some)
                .map_err(Error::Value),
            None => Ok(None),
        }
    }
}

impl<Key: Encode, Value, Kind> Keyspace<Key, Value, Kind> {
    /// A typed version of [`fjall::Readable::contains_key`], see the original documentation for more information.
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by the encoded key is longer than
    /// 65535 bytes, the maximum key size of fjall.
    #[inline]
    pub fn contains_key(
        &self,
        rtxn: &Rtxn,
        key: &Key::Item,
    ) -> Result<bool, Error<Key::Error, Infallible>> {
        let key = self.encode_key(key).map_err(Error::Key)?;
        rtxn.inner()
            .contains_key(&self.inner, key)
            .map_err(Error::Fjall)
    }

    /// A typed version of [`fjall::Readable::size_of`], see the original documentation for more information.
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by the encoded key is longer than
    /// 65535 bytes, the maximum key size of fjall.
    #[inline]
    pub fn size_of(
        &self,
        rtxn: &Rtxn,
        key: &Key::Item,
    ) -> Result<Option<u32>, Error<Key::Error, Infallible>> {
        let key = self.encode_key(key).map_err(Error::Key)?;
        rtxn.inner().size_of(&self.inner, key).map_err(Error::Fjall)
    }

    /// Returns an iterator over the key-value pairs whose key is in `range`.
    ///
    /// A typed version of [`fjall::Readable::range`]. The bounds are encoded
    /// with the key codec and the keys are compared by their encoded bytes.
    /// See [the ordering section](crate::codec#keys-and-ordering) of the codecs.
    ///
    /// Any [`KeyRange`] is accepted: the ranges of the standard library and
    /// the tuples of [`Bound`]s, holding keys or references to keys, so
    /// unsized keys work too, e.g. `"a".."c"` for a [`Str`](crate::codec::Str) key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::{OrderedI32, Unit}};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder.path()).unwrap();
    /// let ks = db.keyspace::<OrderedI32, Unit>("numbers", KeyspaceCreateOptions::default).unwrap();
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// for n in -5..5 {
    ///     ks.insert(&mut wtxn, &n, &()).unwrap();
    /// }
    ///
    /// let keys: Vec<i32> = ks
    ///     .range(&wtxn, &(-3..3))
    ///     .unwrap()
    ///     .map(|guard| guard.key().unwrap())
    ///     .collect();
    /// assert_eq!(keys, [-3, -2, -1, 0, 1, 2]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by one of the encoded bounds is longer
    /// than 65535 bytes, the maximum key size of fjall.
    #[inline]
    pub fn range<R: KeyRange<Key::Item>>(
        &self,
        rtxn: &Rtxn,
        range: &R,
    ) -> Result<Iter<Key, Value>, Key::Error> {
        let start = match range.start_bound() {
            Bound::Included(key) => Bound::Included(self.encode_key(key)?),
            Bound::Excluded(key) => Bound::Excluded(self.encode_key(key)?),
            Bound::Unbounded if self.prefix.is_empty() => Bound::Unbounded,
            Bound::Unbounded => Bound::Included(self.prefix.to_vec()),
        };
        let end = match range.end_bound() {
            Bound::Included(key) => Bound::Included(self.encode_key(key)?),
            Bound::Excluded(key) => Bound::Excluded(self.encode_key(key)?),
            Bound::Unbounded => match successor(&self.prefix) {
                Some(successor) => Bound::Excluded(successor),
                None => Bound::Unbounded,
            },
        };

        let iter = rtxn.inner().range(&self.inner, (start, end));
        Ok(Iter::new(iter, self.prefix.len()))
    }

    /// A typed version of [`fjall::Readable::prefix`], see the original documentation for more information.
    ///
    /// # Panics
    ///
    /// Panics if the prefix of the keyspace followed by the encoded `prefix`
    /// is longer than 65535 bytes, the maximum key size of fjall.
    #[inline]
    pub fn prefix(&self, rtxn: &Rtxn, prefix: &Key::Item) -> Result<Iter<Key, Value>, Key::Error> {
        let prefix = self.encode_key(prefix)?;

        let iter = rtxn.inner().prefix(&self.inner, prefix);
        Ok(Iter::new(iter, self.prefix.len()))
    }

    /// Removes an item from the keyspace.
    ///
    /// The key may be up to 65535 bytes long, prefix included.
    /// Shorter keys result in better performance.
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
    /// assert!(!ks.is_empty(&wtxn).unwrap());
    ///
    /// ks.remove(&mut wtxn, "a").unwrap();
    /// assert!(ks.is_empty(&wtxn).unwrap());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by the encoded key is longer than
    /// 65535 bytes, the maximum key size of fjall.
    #[inline]
    pub fn remove(&self, wtxn: &mut Wtxn, key: &Key::Item) -> Result<(), Key::Error> {
        let key = self.encode_key(key)?;
        wtxn.write_inner().remove(&self.inner, key);
        Ok(())
    }
}

impl<Key: Encode, Value: Encode, Kind> Keyspace<Key, Value, Kind> {
    /// Inserts a key-value pair into the keyspace.
    ///
    /// Keys may be up to 65535 bytes long, prefix included, values up to 2^32 bytes.
    /// Shorter keys and values result in better performance.
    ///
    /// If the key already exists, the item will be overwritten.
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
    ///
    /// assert!(!ks.is_empty(&wtxn).unwrap());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the prefix followed by the encoded key is longer than
    /// 65535 bytes, the maximum key size of fjall.
    #[inline]
    pub fn insert(
        &self,
        wtxn: &mut Wtxn,
        key: &Key::Item,
        value: &Value::Item,
    ) -> Result<(), Error<Key::Error, Value::Error>> {
        let key = self.encode_key(key).map_err(Error::Key)?;
        let value = Value::encode_alloc(value).map_err(Error::Value)?.finish();
        wtxn.write_inner().insert(&self.inner, key, value);
        Ok(())
    }
}

/// The room reserved after the prefix when encoding a key, so the keys up
/// to that size don't need to grow the buffer: they cost as many allocations
/// with a prefix as without.
const KEY_HEADROOM: usize = 64;

/// Returns the smallest key greater than every key starting with `prefix`,
/// or `None` if there isn't any (`prefix` is empty or only made of `0xFF`).
fn successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let last = prefix.iter().rposition(|&byte| byte != u8::MAX)?;
    let mut successor = prefix[..=last].to_vec();
    successor[last] += 1;
    Some(successor)
}

#[cfg(test)]
mod test {
    use byteorder::BigEndian;

    use super::successor;
    use crate::{
        Database, Keyspace, KeyspaceCreateOptions, Root,
        codec::{Bytes, Str, U8, U32},
    };

    fn setup() -> (tempfile::TempDir, Database, Keyspace<U8, Str, Root>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::builder(dir.path()).unwrap();
        let ks = db
            .keyspace::<U8, Str>("hello", KeyspaceCreateOptions::default)
            .unwrap();
        (dir, db, ks)
    }

    /// All the raw keys stored in the fjall keyspace.
    fn raw_keys<K, V>(root: &Keyspace<K, V, Root>, rtxn: &crate::Rtxn) -> Vec<Vec<u8>> {
        root.remap_types::<Bytes, Bytes>()
            .iter(rtxn)
            .map(|guard| guard.raw_key().unwrap().to_vec())
            .collect()
    }

    fn keys<V>(iter: impl Iterator<Item = crate::Guard<U8, V>>) -> Vec<u8> {
        iter.map(|guard| guard.key().unwrap()).collect()
    }

    #[test]
    fn successor_of_a_prefix() {
        assert_eq!(successor(b""), None);
        assert_eq!(successor(&[0xFF]), None);
        assert_eq!(successor(&[0xFF, 0xFF]), None);
        assert_eq!(successor(b"a"), Some(b"b".to_vec()));
        assert_eq!(successor(&[1, 0xFF]), Some(vec![2]));
        assert_eq!(successor(&[1, 0xFE, 0xFF, 0xFF]), Some(vec![1, 0xFF]));
    }

    #[test]
    fn keys_are_written_after_the_prefix() {
        let (_dir, db, root) = setup();
        let a = root.prefixed_raw(b"a");
        let mut wtxn = db.write_tx().unwrap();
        root.insert(&mut wtxn, &0, "root").unwrap();
        a.insert(&mut wtxn, &1, "a").unwrap();
        a.prefixed_raw(b"b").insert(&mut wtxn, &2, "ab").unwrap();
        assert_eq!(raw_keys(&root, &wtxn), [&[0][..], b"a\x01", b"ab\x02"]);
    }

    #[test]
    fn chained_prefixes_are_concatenated() {
        let (_dir, _db, root) = setup();
        let chained = root.prefixed_raw(b"a").prefixed_raw(b"bc");
        assert_eq!(&*chained.prefix, b"abc");
        let chained = root
            .prefixed::<U32<BigEndian>>(&1)
            .unwrap()
            .prefixed::<Str>("z")
            .unwrap();
        assert_eq!(&*chained.prefix, b"\0\0\0\x01z");
        assert_eq!(&*root.prefix, b"");
    }

    #[test]
    fn typed_and_raw_prefixes_are_the_same() {
        let (_dir, db, root) = setup();
        let typed = root.prefixed::<U32<BigEndian>>(&7).unwrap();
        let raw = root.prefixed_raw(7u32.to_be_bytes());
        let mut wtxn = db.write_tx().unwrap();
        typed.insert(&mut wtxn, &1, "hello").unwrap();
        assert_eq!(raw.get(&wtxn, &1).unwrap().as_deref(), Some("hello"));
    }

    #[test]
    fn range_is_clamped_to_the_prefix() {
        let (_dir, db, root) = setup();
        let prefixes: [&[u8]; 4] = [b"a", &[1, 0xFF], &[0xFF], &[0xFF, 0xFF]];

        let mut wtxn = db.write_tx().unwrap();
        // neighbours around every prefix
        for raw in [
            &[0][..],
            &[1],
            &[1, 0xFE],
            &[2],
            &[0xFE],
            &[0xFF],
            b"`",
            b"b",
        ] {
            root.prefixed_raw(raw).insert(&mut wtxn, &0, "x").unwrap();
        }
        for prefix in prefixes {
            let ks = root.prefixed_raw(prefix);
            for i in 1..=5 {
                ks.insert(&mut wtxn, &i, "x").unwrap();
            }
        }

        for prefix in prefixes {
            let ks = root.prefixed_raw(prefix);
            let all = keys(ks.iter(&wtxn));
            // `[0xFF] + 0xFF` is both a key of `[0xFF]` and the prefix of `[0xFF, 0xFF]`
            if prefix == [0xFF] {
                assert_eq!(all, [0, 1, 2, 3, 4, 5, 255, 255, 255, 255, 255]);
            } else {
                assert_eq!(all, [1, 2, 3, 4, 5], "{prefix:?}");
            }
            assert_eq!(keys(ks.range(&wtxn, &(..)).unwrap()), all);
            if prefix == [0xFF] {
                continue;
            }
            assert_eq!(keys(ks.range(&wtxn, &(2..)).unwrap()), [2, 3, 4, 5]);
            assert_eq!(keys(ks.range(&wtxn, &(..3)).unwrap()), [1, 2]);
            assert_eq!(keys(ks.range(&wtxn, &(..=3)).unwrap()), [1, 2, 3]);
            assert_eq!(keys(ks.range(&wtxn, &(2..4)).unwrap()), [2, 3]);
            assert_eq!(keys(ks.range(&wtxn, &(2..=4)).unwrap()), [2, 3, 4]);
            let range = (std::ops::Bound::Excluded(2), std::ops::Bound::Unbounded);
            assert_eq!(keys(ks.range(&wtxn, &range).unwrap()), [3, 4, 5]);
            let rev: Vec<u8> = keys(ks.range(&wtxn, &(..)).unwrap().rev());
            assert_eq!(rev, [5, 4, 3, 2, 1]);
        }
    }

    #[test]
    fn remap_keeps_the_prefix() {
        let (_dir, db, root) = setup();
        let a = root.prefixed_raw(b"a");
        let mut wtxn = db.write_tx().unwrap();
        a.insert(&mut wtxn, &1, "a1").unwrap();

        let bytes = a.remap_value_type::<Bytes>();
        assert_eq!(bytes.get(&wtxn, &1).unwrap().as_deref(), Some(&b"a1"[..]));
        let bytes = a.remap_key_type::<Bytes>();
        assert_eq!(bytes.get(&wtxn, &[1]).unwrap().as_deref(), Some("a1"));
        let bytes = a.remap_types::<Bytes, Bytes>();
        assert_eq!(bytes.get(&wtxn, &[1]).unwrap().as_deref(), Some(&b"a1"[..]));
        let guard = a.iter(&wtxn).next().unwrap();
        assert_eq!(guard.remap_key_type::<Bytes>().key().unwrap(), [1]);
        let iter = a.iter(&wtxn).remap_key_type::<Bytes>();
        assert_eq!(iter.map(|g| g.key().unwrap()).collect::<Vec<_>>(), [[1]]);
    }

    #[test]
    fn root_converts_to_prefixed() {
        fn len(ks: &Keyspace<U8, Str>, rtxn: &crate::Rtxn) -> usize {
            ks.len(rtxn).unwrap()
        }

        let (_dir, db, root) = setup();
        let mut wtxn = db.write_tx().unwrap();
        root.insert(&mut wtxn, &1, "one").unwrap();

        // through `Deref`
        assert_eq!(len(&root, &wtxn), 1);
        // through `From`
        let prefixed: Keyspace<U8, Str> = root.clone().into();
        assert_eq!(len(&prefixed, &wtxn), 1);
        assert_eq!(prefixed.get(&wtxn, &1).unwrap().as_deref(), Some("one"));
        // the root-only methods are still there
        let _ = root.approximate_len();
        let _ = root.path();
    }

    /// The prefix used by [`for_each_kind`] for its prefixed keyspace.
    const PREFIX: &[u8] = b"p\0";
    /// The prefix used by [`for_each_kind`] for its chained keyspace.
    const CHAINED: &[u8] = b"p\0q\0";

    /// Runs `check` on the root, a prefixed and a chained prefixed keyspace,
    /// each in its own database, with the full prefix of the keyspace.
    ///
    /// The prefixed keyspaces are surrounded by noise they must not see: a
    /// sibling sorted before them, one sorted after them, and for the chained
    /// one, its parent.
    fn for_each_kind(check: impl Fn(&Database, &Keyspace<U8, Str>, &[u8])) {
        let (_dir, db, root) = setup();
        check(&db, &root, b"");

        for (ks_prefix, noise) in [
            (PREFIX, &[&b"o\0"[..], b"q\0"][..]),
            (CHAINED, &[b"o\0", b"q\0", PREFIX]),
        ] {
            let (_dir, db, root) = setup();
            let mut wtxn = db.write_tx().unwrap();
            for noise in noise {
                let noise = root.prefixed_raw(*noise);
                for key in 0..=u8::MAX {
                    noise.insert(&mut wtxn, &key, "noise").unwrap();
                }
            }
            wtxn.commit().unwrap().unwrap();

            let ks = match ks_prefix {
                PREFIX => root.prefixed_raw(PREFIX),
                _ => root.prefixed_raw(PREFIX).prefixed_raw(&b"q\0"[..]),
            };
            assert_eq!(&*ks.prefix, ks_prefix);
            check(&db, &ks, ks_prefix);
        }
    }

    /// Inserts the keys 3, 1 and 2, with the values "three", "one" and "two".
    fn insert_123(db: &Database, ks: &Keyspace<U8, Str>) {
        let mut wtxn = db.write_tx().unwrap();
        ks.insert(&mut wtxn, &3, "three").unwrap();
        ks.insert(&mut wtxn, &1, "one").unwrap();
        ks.insert(&mut wtxn, &2, "two").unwrap();
        wtxn.commit().unwrap().unwrap();
    }

    #[test]
    fn insert_and_get() {
        for_each_kind(|db, ks, prefix| {
            let mut wtxn = db.write_tx().unwrap();
            assert_eq!(ks.get(&wtxn, &1).unwrap(), None, "{prefix:?}");
            ks.insert(&mut wtxn, &1, "one").unwrap();
            assert_eq!(ks.get(&wtxn, &1).unwrap().as_deref(), Some("one"));
            // overwrite
            ks.insert(&mut wtxn, &1, "uno").unwrap();
            assert_eq!(ks.get(&wtxn, &1).unwrap().as_deref(), Some("uno"));
            assert_eq!(ks.get(&wtxn, &2).unwrap(), None);
            wtxn.commit().unwrap().unwrap();

            let rtxn = db.read_tx();
            assert_eq!(ks.get(&rtxn, &1).unwrap().as_deref(), Some("uno"));
        });
    }

    #[test]
    fn contains_key() {
        for_each_kind(|db, ks, prefix| {
            assert!(!ks.contains_key(&db.read_tx(), &1).unwrap(), "{prefix:?}");
            insert_123(db, ks);
            let rtxn = db.read_tx();
            assert!(ks.contains_key(&rtxn, &1).unwrap());
            assert!(!ks.contains_key(&rtxn, &4).unwrap());
        });
    }

    #[test]
    fn size_of() {
        for_each_kind(|db, ks, prefix| {
            assert_eq!(ks.size_of(&db.read_tx(), &1).unwrap(), None, "{prefix:?}");
            insert_123(db, ks);
            let rtxn = db.read_tx();
            assert_eq!(ks.size_of(&rtxn, &3).unwrap(), Some(5));
            assert_eq!(ks.size_of(&rtxn, &4).unwrap(), None);
        });
    }

    #[test]
    fn take() {
        for_each_kind(|db, ks, prefix| {
            insert_123(db, ks);
            let mut wtxn = db.write_tx().unwrap();
            assert_eq!(ks.take(&mut wtxn, &4).unwrap(), None, "{prefix:?}");
            assert_eq!(ks.take(&mut wtxn, &2).unwrap().as_deref(), Some("two"));
            assert_eq!(ks.take(&mut wtxn, &2).unwrap(), None);
            wtxn.commit().unwrap().unwrap();
            assert_eq!(keys(ks.iter(&db.read_tx())), [1, 3]);
        });
    }

    #[test]
    fn remove() {
        for_each_kind(|db, ks, prefix| {
            insert_123(db, ks);
            let mut wtxn = db.write_tx().unwrap();
            ks.remove(&mut wtxn, &4).unwrap();
            ks.remove(&mut wtxn, &1).unwrap();
            assert_eq!(ks.get(&wtxn, &1).unwrap(), None, "{prefix:?}");
            wtxn.commit().unwrap().unwrap();
            assert_eq!(keys(ks.iter(&db.read_tx())), [2, 3]);
        });
    }

    #[test]
    fn iter() {
        for_each_kind(|db, ks, prefix| {
            assert_eq!(ks.iter(&db.read_tx()).count(), 0, "{prefix:?}");
            insert_123(db, ks);
            let rtxn = db.read_tx();
            assert_eq!(keys(ks.iter(&rtxn)), [1, 2, 3]);
            assert_eq!(keys(ks.iter(&rtxn).rev()), [3, 2, 1]);
            let values: Vec<String> = ks.iter(&rtxn).map(|g| g.value().unwrap()).collect();
            assert_eq!(values, ["one", "two", "three"]);
        });
    }

    #[test]
    fn len_and_is_empty() {
        for_each_kind(|db, ks, prefix| {
            let rtxn = db.read_tx();
            assert_eq!(ks.len(&rtxn).unwrap(), 0, "{prefix:?}");
            assert!(ks.is_empty(&rtxn).unwrap());
            insert_123(db, ks);
            let rtxn = db.read_tx();
            assert_eq!(ks.len(&rtxn).unwrap(), 3);
            assert!(!ks.is_empty(&rtxn).unwrap());
        });
    }

    #[test]
    fn first_and_last_key_value() {
        for_each_kind(|db, ks, prefix| {
            let rtxn = db.read_tx();
            assert!(ks.first_key_value(&rtxn).is_none(), "{prefix:?}");
            assert!(ks.last_key_value(&rtxn).is_none());
            insert_123(db, ks);
            let rtxn = db.read_tx();
            let first = ks.first_key_value(&rtxn).unwrap().into_inner().unwrap();
            assert_eq!(first, (1, "one".to_string()));
            let last = ks.last_key_value(&rtxn).unwrap().into_inner().unwrap();
            assert_eq!(last, (3, "three".to_string()));
        });
    }

    #[test]
    fn range() {
        use crate::KeyRange;
        use std::ops::Bound::{Excluded, Unbounded};

        fn range(ks: &Keyspace<U8, Str>, rtxn: &crate::Rtxn, r: impl KeyRange<u8>) -> Vec<u8> {
            keys(ks.range(rtxn, &r).unwrap())
        }

        for_each_kind(|db, ks, prefix| {
            insert_123(db, ks);
            let rtxn = &db.read_tx();
            assert_eq!(range(ks, rtxn, ..), [1, 2, 3], "{prefix:?}");
            assert_eq!(range(ks, rtxn, 2..), [2, 3]);
            assert_eq!(range(ks, rtxn, ..2), [1]);
            assert_eq!(range(ks, rtxn, ..=2), [1, 2]);
            assert_eq!(range(ks, rtxn, 1..3), [1, 2]);
            assert_eq!(range(ks, rtxn, 1..=3), [1, 2, 3]);
            assert_eq!(range(ks, rtxn, (Excluded(1), Unbounded)), [2, 3]);
            assert_eq!(range(ks, rtxn, (Excluded(1), Excluded(3))), [2]);
            assert!(range(ks, rtxn, 4..).is_empty());
            assert_eq!(keys(ks.range(rtxn, &(..)).unwrap().rev()), [3, 2, 1]);
        });
    }

    #[test]
    fn prefix_scan() {
        for_each_kind(|db, ks, prefix| {
            let ks = ks.remap_key_type::<Bytes>();
            let mut wtxn = db.write_tx().unwrap();
            for key in [&[1][..], &[2], &[2, 1], &[2, 2], &[3]] {
                ks.insert(&mut wtxn, key, "x").unwrap();
            }
            wtxn.commit().unwrap().unwrap();

            let rtxn = db.read_tx();
            let scan = |p: &[u8]| -> Vec<Vec<u8>> {
                let iter = ks.prefix(&rtxn, p).unwrap();
                iter.map(|g| g.key().unwrap()).collect()
            };
            assert_eq!(scan(&[2]), [&[2][..], &[2, 1], &[2, 2]], "{prefix:?}");
            assert_eq!(scan(&[2, 1]), [[2, 1]]);
            assert!(scan(&[4]).is_empty());
            assert_eq!(scan(&[]).len(), 5);
        });
    }

    #[test]
    fn full_keys_are_accessible() {
        for_each_kind(|db, ks, prefix| {
            insert_123(db, ks);
            let rtxn = db.read_tx();
            let full = |key: u8| [prefix, &[key]].concat();
            let raw = |guard: crate::Guard<U8, Str>| guard.raw_key().unwrap().to_vec();

            let all: Vec<_> = ks.iter(&rtxn).map(raw).collect();
            assert_eq!(all, [full(1), full(2), full(3)], "{prefix:?}");
            let all: Vec<_> = ks.iter(&rtxn).rev().map(raw).collect();
            assert_eq!(all, [full(3), full(2), full(1)]);
            let all: Vec<_> = ks.range(&rtxn, &(2..)).unwrap().map(raw).collect();
            assert_eq!(all, [full(2), full(3)]);
            let all: Vec<_> = ks.prefix(&rtxn, &1).unwrap().map(raw).collect();
            assert_eq!(all, [full(1)]);
            assert_eq!(raw(ks.first_key_value(&rtxn).unwrap()), full(1));
            assert_eq!(raw(ks.last_key_value(&rtxn).unwrap()), full(3));
            // remapping the guard keeps the full key
            let guard = ks.iter(&rtxn).next().unwrap().remap_types::<Bytes, Bytes>();
            assert_eq!(guard.raw_key().unwrap().to_vec(), full(1));
            // while the decoded key never contains the prefix
            assert_eq!(ks.iter(&rtxn).next().unwrap().key().unwrap(), 1);
        });
    }

    #[test]
    fn the_root_sees_the_keys_of_all_its_prefixed_keyspaces() {
        let (_dir, db, root) = setup();
        let users = root.prefixed_raw(&b"users\0"[..]);
        let posts = root.prefixed_raw(&b"posts\0"[..]);
        let admins = users.prefixed_raw(&b"admins\0"[..]);
        let typed = root.prefixed::<U32<BigEndian>>(&7).unwrap();

        let mut wtxn = db.write_tx().unwrap();
        root.insert(&mut wtxn, &0, "root").unwrap();
        users.insert(&mut wtxn, &1, "user").unwrap();
        posts.insert(&mut wtxn, &2, "post").unwrap();
        admins.insert(&mut wtxn, &3, "admin").unwrap();
        typed.insert(&mut wtxn, &4, "typed").unwrap();
        wtxn.commit().unwrap().unwrap();

        let rtxn = db.read_tx();
        let expected: [(&[u8], &str); 5] = [
            (b"\0", "root"),
            (b"\0\0\0\x07\x04", "typed"),
            (b"posts\0\x02", "post"),
            (b"users\0\x01", "user"),
            (b"users\0admins\0\x03", "admin"),
        ];

        // the root iterates over the full keys of every keyspace, in order
        let all: Vec<(Vec<u8>, String)> = root
            .remap_key_type::<Bytes>()
            .iter(&rtxn)
            .map(|guard| guard.into_inner().unwrap())
            .collect();
        let expected_owned: Vec<(Vec<u8>, String)> = expected
            .iter()
            .map(|(key, value)| (key.to_vec(), value.to_string()))
            .collect();
        assert_eq!(all, expected_owned);
        assert_eq!(raw_keys(&root, &rtxn), expected.map(|(key, _)| key));
        assert_eq!(root.len(&rtxn).unwrap(), 5);

        // and can read each of them with its full key
        let bytes = root.remap_key_type::<Bytes>();
        for (key, value) in expected {
            assert_eq!(bytes.get(&rtxn, key).unwrap().as_deref(), Some(value));
        }
        // a prefix scan on the root finds a keyspace and its children
        let users_and_admins = bytes.prefix(&rtxn, b"users\0").unwrap().count();
        assert_eq!(users_and_admins, 2);

        // while each prefixed keyspace only sees its own keys and its children's
        assert_eq!(users.len(&rtxn).unwrap(), 2);
        assert_eq!(posts.len(&rtxn).unwrap(), 1);
        assert_eq!(admins.len(&rtxn).unwrap(), 1);
        assert_eq!(typed.len(&rtxn).unwrap(), 1);
    }

    /// The prefixes are simply concatenated, so the root can write the keys
    /// of a prefixed keyspace and overwrite its values.
    #[test]
    fn the_root_can_overwrite_the_keys_of_a_prefixed_keyspace() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::builder(dir.path()).unwrap();
        let root = db
            .keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default)
            .unwrap();
        let kefir = root.prefixed_raw(&b"kefir\0"[..]);

        let mut wtxn = db.write_tx().unwrap();
        kefir.insert(&mut wtxn, "", "echo").unwrap();
        kefir.insert(&mut wtxn, "chien", "intel").unwrap();
        wtxn.commit().unwrap().unwrap();

        let rtxn = db.read_tx();
        assert_eq!(kefir.get(&rtxn, "").unwrap().as_deref(), Some("echo"));
        assert_eq!(kefir.get(&rtxn, "chien").unwrap().as_deref(), Some("intel"));

        // the root writes the same raw keys
        let mut wtxn = db.write_tx().unwrap();
        root.insert(&mut wtxn, "kefir\0", "erased by the root")
            .unwrap();
        root.insert(&mut wtxn, "kefir\0chien", "also erased by the root")
            .unwrap();
        wtxn.commit().unwrap().unwrap();

        // and overwrote the values of the prefixed keyspace
        let rtxn = db.read_tx();
        let echo = kefir.get(&rtxn, "").unwrap();
        assert_eq!(echo.as_deref(), Some("erased by the root"));
        let intel = kefir.get(&rtxn, "chien").unwrap();
        assert_eq!(intel.as_deref(), Some("also erased by the root"));
        // there's still only two keys
        assert_eq!(kefir.len(&rtxn).unwrap(), 2);
        assert_eq!(root.len(&rtxn).unwrap(), 2);
    }

    /// fjall's maximum key size.
    const MAX_KEY_LEN: usize = u16::MAX as usize;

    #[test]
    fn keys_of_the_maximum_size_including_the_prefix_work() {
        let (_dir, db, root) = setup();
        let ks = root.prefixed_raw(vec![b'p'; MAX_KEY_LEN - 1]);
        insert_123(&db, &ks);
        let rtxn = db.read_tx();
        assert_eq!(ks.get(&rtxn, &2).unwrap().as_deref(), Some("two"));
        assert_eq!(keys(ks.iter(&rtxn)), [1, 2, 3]);
        assert_eq!(keys(ks.range(&rtxn, &(2..)).unwrap()), [2, 3]);
        let raw = ks.first_key_value(&rtxn).unwrap().raw_key().unwrap();
        assert_eq!(raw.len(), MAX_KEY_LEN);
    }

    /// A keyspace whose prefix leaves no room for a key.
    fn too_long() -> (tempfile::TempDir, Database, Keyspace<U8, Str>) {
        let (dir, db, root) = setup();
        // creating the keyspace doesn't panic, using a key does
        let ks = root.prefixed_raw(vec![b'p'; MAX_KEY_LEN]);
        (dir, db, ks)
    }

    #[test]
    fn methods_without_keys_work_with_a_prefix_of_the_maximum_size() {
        let (_dir, db, ks) = too_long();
        let rtxn = db.read_tx();
        assert_eq!(ks.iter(&rtxn).count(), 0);
        assert_eq!(ks.len(&rtxn).unwrap(), 0);
        assert!(ks.is_empty(&rtxn).unwrap());
        assert!(ks.first_key_value(&rtxn).is_none());
        assert!(ks.last_key_value(&rtxn).is_none());
        assert_eq!(keys(ks.range(&rtxn, &(..)).unwrap()), Vec::<u8>::new());
    }

    /// Generates a test checking that `$call` panics because the prefix
    /// followed by the key is longer than [`MAX_KEY_LEN`].
    macro_rules! panics_when_the_key_is_too_long {
        ($($name:ident: |$db:ident, $ks:ident| $call:expr;)*) => {$(
            #[test]
            #[should_panic(expected = "keys can be 65535 bytes in length")]
            fn $name() {
                let (_dir, $db, $ks) = too_long();
                let _ = $call;
            }
        )*};
    }

    panics_when_the_key_is_too_long! {
        too_long_insert: |db, ks| ks.insert(&mut db.write_tx().unwrap(), &1, "one");
        too_long_get: |db, ks| ks.get(&db.read_tx(), &1);
        too_long_contains_key: |db, ks| ks.contains_key(&db.read_tx(), &1);
        too_long_size_of: |db, ks| ks.size_of(&db.read_tx(), &1);
        too_long_take: |db, ks| ks.take(&mut db.write_tx().unwrap(), &1);
        too_long_remove: |db, ks| ks.remove(&mut db.write_tx().unwrap(), &1);
        too_long_range: |db, ks| ks.range(&db.read_tx(), &(1..)).map(Iterator::count);
        too_long_prefix: |db, ks| ks.prefix(&db.read_tx(), &1).map(Iterator::count);
    }

    /// The fewest allocations made by `f` over a few runs: fjall sometimes
    /// allocates a bit more, for its own bookkeeping.
    fn min_allocations<R>(mut f: impl FnMut() -> R) -> usize {
        (0..5)
            .map(|_| crate::test_alloc::allocations(&mut f).1)
            .min()
            .unwrap()
    }

    /// Runs every method taking a key on `ks` with `key`, and on `root` with
    /// the same raw key, and checks that they allocate as much.
    fn assert_allocates_like_the_root(
        db: &Database,
        root: &Keyspace<Bytes, U8, Root>,
        ks: &Keyspace<Bytes, U8>,
        key: &[u8],
    ) {
        use std::ops::Bound::{Excluded, Included, Unbounded};

        let full = [&*ks.prefix, key].concat();
        let ctx = format!(
            "prefix of {} bytes, key of {} bytes",
            ks.prefix.len(),
            key.len()
        );
        let mut wtxn = db.write_tx().unwrap();

        macro_rules! compare {
            ($name:literal, |$ks:ident, $key:ident| $call:expr) => {{
                let prefixed = min_allocations(|| {
                    let ($ks, $key) = (ks, key);
                    $call
                });
                let root = min_allocations(|| {
                    let ($ks, $key): (&Keyspace<Bytes, U8>, _) = (root, &full[..]);
                    $call
                });
                assert_eq!(
                    prefixed, root,
                    "{} allocated more than the root, {ctx}",
                    $name
                );
            }};
        }

        compare!("insert", |ks, key| ks.insert(&mut wtxn, key, &1).unwrap());
        compare!("get", |ks, key| ks.get(&wtxn, key).unwrap());
        compare!("contains_key", |ks, key| ks
            .contains_key(&wtxn, key)
            .unwrap());
        compare!("size_of", |ks, key| ks.size_of(&wtxn, key).unwrap());
        compare!("prefix", |ks, key| ks.prefix(&wtxn, key).unwrap());
        compare!("take", |ks, key| ks.take(&mut wtxn, key).unwrap());
        compare!("remove", |ks, key| ks.remove(&mut wtxn, key).unwrap());

        // On a prefixed keyspace an unbounded end becomes the successor of the
        // prefix, compare with the same query on the root.
        let end = successor(&ks.prefix).unwrap();
        let prefixed = min_allocations(|| ks.range(&wtxn, &(Included(key), Unbounded)).unwrap());
        let rooted = min_allocations(|| {
            root.range(&wtxn, &(Included(&full[..]), Excluded(&end[..])))
                .unwrap()
        });
        assert_eq!(
            prefixed, rooted,
            "range allocated more than the root, {ctx}"
        );
    }

    #[test]
    fn prefixed_keyspaces_allocate_like_the_root() {
        let (_dir, db, root) = setup();
        let root = root.remap_types::<Bytes, U8>();
        let short = root.prefixed_raw(&b"p\0"[..]);
        let long = root.prefixed_raw(vec![b'p'; 40]);
        let chained = short.prefixed_raw(vec![b'q'; 40]);

        for ks in [&short, &long, &chained] {
            for key_len in [0, 1, 20, super::KEY_HEADROOM] {
                assert_allocates_like_the_root(&db, &root, ks, &vec![b'k'; key_len]);
            }
        }
    }

    #[test]
    fn reading_keys_through_a_prefix_does_not_allocate() {
        use crate::test_alloc::allocations;

        let (_dir, db, root) = setup();
        let ks = root.prefixed_raw(vec![b'p'; 40]);
        let mut wtxn = db.write_tx().unwrap();
        ks.insert(&mut wtxn, &1, "one").unwrap();

        let guard = ks.iter(&wtxn).next().unwrap();
        let (key, allocated) = allocations(|| guard.key().unwrap());
        assert_eq!((key, allocated), (1, 0));
        let guard = ks.iter(&wtxn).next().unwrap();
        let (_, allocated) = allocations(|| guard.raw_key().unwrap());
        assert_eq!(allocated, 0);
    }

    #[test]
    fn creating_prefixes_allocates_only_for_long_ones() {
        use crate::test_alloc::allocations;

        let (_dir, _db, root) = setup();
        // fjall stores up to 20 bytes inline
        let (short, allocated) = allocations(|| root.prefixed_raw(&b"p\0"[..]));
        assert_eq!(allocated, 0, "short prefix");
        let (_, allocated) = allocations(|| short.prefixed_raw(&b"q\0"[..]));
        assert_eq!(allocated, 0, "short chained prefix");
        let long = fjall::Slice::from(&[b'q'; 30][..]);
        let (_, allocated) = allocations(|| short.prefixed_raw(long.clone()));
        assert_eq!(allocated, 1, "long chained prefix");
        let (_, allocated) = allocations(|| short.clone());
        assert_eq!(allocated, 0, "clone");
    }

    #[test]
    fn debug() {
        let (_dir, _db, root) = setup();
        assert_eq!(
            format!("{root:?}"),
            r#"Keyspace { name: "hello", prefix: b"", .. }"#
        );
        let users = root.prefixed_raw(&b"users\0"[..]);
        assert_eq!(
            format!("{users:?}"),
            r#"Keyspace { name: "hello", prefix: b"users\x00", .. }"#
        );
        let typed = users.prefixed::<U32<BigEndian>>(&0xff).unwrap();
        assert_eq!(
            format!("{typed:?}"),
            r#"Keyspace { name: "hello", prefix: b"users\x00\x00\x00\x00\xff", .. }"#
        );
    }

    #[test]
    fn clear() {
        for_each_kind(|db, ks, prefix| {
            insert_123(db, ks);
            let rtxn = db.read_tx();
            let mut wtxn = db.write_tx().unwrap();
            ks.clear(&mut wtxn).unwrap();
            // the transaction sees its own clear, the others don't yet
            assert!(ks.is_empty(&wtxn).unwrap(), "{prefix:?}");
            assert_eq!(ks.len(&rtxn).unwrap(), 3);
            wtxn.commit().unwrap().unwrap();

            assert!(ks.is_empty(&db.read_tx()).unwrap());
            // the keyspace is still usable
            insert_123(db, ks);
            assert_eq!(ks.len(&db.read_tx()).unwrap(), 3);
        });
    }

    #[test]
    fn clear_only_removes_the_keys_under_the_prefix() {
        let (_dir, db, root) = setup();
        let a = root.prefixed_raw(&b"a\0"[..]);
        let ab = a.prefixed_raw(&b"b\0"[..]);
        let b = root.prefixed_raw(&b"b\0"[..]);
        for ks in [&a, &ab, &b] {
            insert_123(&db, ks);
        }

        let mut wtxn = db.write_tx().unwrap();
        a.clear(&mut wtxn).unwrap();
        wtxn.commit().unwrap().unwrap();
        let rtxn = db.read_tx();
        // `a` and its child are cleared, its sibling isn't
        assert!(a.is_empty(&rtxn).unwrap());
        assert!(ab.is_empty(&rtxn).unwrap());
        assert_eq!(b.len(&rtxn).unwrap(), 3);

        // clearing the root clears everything
        let mut wtxn = db.write_tx().unwrap();
        root.clear(&mut wtxn).unwrap();
        wtxn.commit().unwrap().unwrap();
        assert!(root.is_empty(&db.read_tx()).unwrap());
    }

    #[test]
    fn clear_can_be_rolled_back() {
        let (_dir, db, root) = setup();
        let ks = root.prefixed_raw(&b"p\0"[..]);
        insert_123(&db, &ks);
        let mut wtxn = db.write_tx().unwrap();
        ks.clear(&mut wtxn).unwrap();
        wtxn.rollback();
        assert_eq!(keys(ks.iter(&db.read_tx())), [1, 2, 3]);
    }

    #[test]
    fn clear_conflicts_with_concurrent_writes_under_the_prefix() {
        let (_dir, db, root) = setup();
        let ks = root.prefixed_raw(&b"p\0"[..]);
        let other = root.prefixed_raw(&b"q\0"[..]);
        insert_123(&db, &ks);

        // a write to another prefix doesn't conflict
        let mut clearing = db.write_tx().unwrap();
        let mut writer = db.write_tx().unwrap();
        ks.clear(&mut clearing).unwrap();
        other.insert(&mut writer, &1, "other").unwrap();
        writer.commit().unwrap().unwrap();
        clearing.commit().unwrap().unwrap();

        // a write to a cleared key does
        insert_123(&db, &ks);
        let mut clearing = db.write_tx().unwrap();
        let mut writer = db.write_tx().unwrap();
        ks.clear(&mut clearing).unwrap();
        ks.insert(&mut writer, &2, "concurrent").unwrap();
        writer.commit().unwrap().unwrap();
        assert!(matches!(clearing.commit().unwrap(), Err(crate::Conflict)));
        assert_eq!(
            ks.get(&db.read_tx(), &2).unwrap().as_deref(),
            Some("concurrent")
        );
    }

    #[test]
    fn range_accepts_borrowed_bounds_of_unsized_keys() {
        use std::ops::Bound::{Excluded, Included, Unbounded};

        let (_dir, db, root) = setup();
        let ks = root.remap_types::<Str, U8>().prefixed_raw(&b"p\0"[..]);
        let mut wtxn = db.write_tx().unwrap();
        for (i, key) in ["a", "b", "c", "d"].iter().enumerate() {
            ks.insert(&mut wtxn, key, &(i as u8)).unwrap();
        }

        let range = |iter: crate::Iter<Str, U8>| -> Vec<String> {
            iter.map(|guard| guard.key().unwrap()).collect()
        };
        assert_eq!(range(ks.range(&wtxn, &("b"..)).unwrap()), ["b", "c", "d"]);
        assert_eq!(range(ks.range(&wtxn, &(.."c")).unwrap()), ["a", "b"]);
        assert_eq!(range(ks.range(&wtxn, &(..="c")).unwrap()), ["a", "b", "c"]);
        assert_eq!(range(ks.range(&wtxn, &("b".."d")).unwrap()), ["b", "c"]);
        assert_eq!(
            range(ks.range(&wtxn, &("b"..="d")).unwrap()),
            ["b", "c", "d"]
        );
        assert_eq!(range(ks.range(&wtxn, &(..)).unwrap()), ["a", "b", "c", "d"]);
        let bounds = (Excluded("a"), Included("c"));
        assert_eq!(range(ks.range(&wtxn, &bounds).unwrap()), ["b", "c"]);
        let bounds = (Included("c"), Unbounded);
        assert_eq!(range(ks.range(&wtxn, &bounds).unwrap()), ["c", "d"]);

        let bytes = ks.remap_key_type::<Bytes>();
        let keys: Vec<Vec<u8>> = bytes
            .range(&wtxn, &(&b"b"[..]..&b"d"[..]))
            .unwrap()
            .map(|guard| guard.key().unwrap())
            .collect();
        assert_eq!(keys, [b"b", b"c"]);
    }

    #[test]
    fn range_accepts_owned_and_borrowed_bounds_of_sized_keys() {
        use std::ops::Bound::{Excluded, Unbounded};

        let (_dir, db, root) = setup();
        insert_123(&db, &root);
        let rtxn = db.read_tx();
        assert_eq!(keys(root.range(&rtxn, &(2..)).unwrap()), [2, 3]);
        assert_eq!(keys(root.range(&rtxn, &(&2..)).unwrap()), [2, 3]);
        assert_eq!(keys(root.range(&rtxn, &(&1..&3)).unwrap()), [1, 2]);
        assert_eq!(keys(root.range(&rtxn, &(..=&2)).unwrap()), [1, 2]);
        let bounds = (Excluded(1), Unbounded);
        assert_eq!(keys(root.range(&rtxn, &bounds).unwrap()), [2, 3]);
        let bounds = (Excluded(&1), Unbounded);
        assert_eq!(keys(root.range(&rtxn, &bounds).unwrap()), [2, 3]);
    }
}
