use std::ops::{Deref, RangeBounds};

use fjall::{Conflict, Guard, Iter, Keyspace, OptimisticWriteTx, PersistMode, Snapshot, UserValue};

/// A read and write transaction.
///
/// It can be used everywhere an [`Rtxn`] is expected: `&wtxn` is
/// automatically converted to an `&Rtxn` thanks to [`Deref`]. Reading
/// through it sees the uncommitted writes of the transaction, and the keys
/// read are checked for conflicts when committing.
pub struct Wtxn {
    /// Always a [`ReadTx::Write`].
    rtxn: Rtxn,
}

impl Wtxn {
    #[inline]
    pub(crate) fn new(inner: OptimisticWriteTx) -> Self {
        Self {
            rtxn: Rtxn {
                inner: ReadTx::Write(inner),
            },
        }
    }

    #[inline]
    pub(crate) fn write_inner(&mut self) -> &mut OptimisticWriteTx {
        match &mut self.rtxn.inner {
            ReadTx::Write(inner) => inner,
            ReadTx::Snapshot(_) => unreachable!("a `Wtxn` always holds a write transaction"),
        }
    }

    #[inline]
    fn into_write_inner(self) -> OptimisticWriteTx {
        match self.rtxn.inner {
            ReadTx::Write(inner) => inner,
            ReadTx::Snapshot(_) => unreachable!("a `Wtxn` always holds a write transaction"),
        }
    }

    /// Sets how the transaction is persisted to disk when it's committed.
    ///
    /// With `None`, the default, committing doesn't wait for the data to be
    /// flushed: it's persisted later, and the last commits can be lost in a
    /// crash. With `Some(mode)`, committing flushes the journal with the given
    /// [`PersistMode`], which is slower but durable.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// use fiole::PersistMode;
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder.path()).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default).unwrap();
    ///
    /// let mut wtxn = db.write_tx().unwrap().durability(Some(PersistMode::SyncAll));
    /// ks.insert(&mut wtxn, "important", "data").unwrap();
    /// wtxn.commit().unwrap().unwrap();
    /// ```
    #[inline]
    pub fn durability(self, mode: Option<PersistMode>) -> Wtxn {
        Wtxn::new(self.into_write_inner().durability(mode))
    }

    /// Commits the transaction, making its writes visible to the next transactions.
    ///
    /// There are two layers of errors:
    /// - The outer `Err(fjall::Error)` means fjall couldn't write the
    ///   transaction, e.g. because of an I/O error.
    /// - The inner `Ok(Err(Conflict))` means another transaction committed a
    ///   write to a key read by this one since it started. Nothing has been
    ///   written: run the transaction again to retry.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder.path()).unwrap();
    /// let ks = db.keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default).unwrap();
    ///
    /// let mut first = db.write_tx().unwrap();
    /// let mut second = db.write_tx().unwrap();
    ///
    /// // `first` reads a key that `second` writes and commits before it
    /// assert_eq!(ks.get(&first, "counter").unwrap(), None);
    /// ks.insert(&mut second, "counter", "1").unwrap();
    /// second.commit().unwrap().unwrap();
    ///
    /// ks.insert(&mut first, "counter", "1").unwrap();
    /// assert!(matches!(first.commit().unwrap(), Err(fiole::Conflict)));
    /// ```
    #[inline]
    pub fn commit(self) -> Result<Result<(), Conflict>, fjall::Error> {
        self.into_write_inner().commit()
    }

    /// Discards all the writes of the transaction.
    ///
    /// Dropping the transaction without committing it does the same, this is a
    /// more explicit alternative.
    ///
    /// ```
    /// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
    /// #
    /// # let folder = tempfile::tempdir().unwrap();
    /// # let db = Database::builder(folder.path()).unwrap();
    /// # let ks = db.keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default).unwrap();
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "hello", "world").unwrap();
    /// wtxn.rollback();
    ///
    /// let rtxn = db.read_tx();
    /// assert_eq!(ks.get(&rtxn, "hello").unwrap(), None);
    /// ```
    #[inline]
    pub fn rollback(self) {
        self.into_write_inner().rollback()
    }
}

impl Deref for Wtxn {
    type Target = Rtxn;

    #[inline]
    fn deref(&self) -> &Rtxn {
        &self.rtxn
    }
}

impl AsRef<Rtxn> for Wtxn {
    #[inline]
    fn as_ref(&self) -> &Rtxn {
        &self.rtxn
    }
}

/// A read transaction.
///
/// ```
/// use fiole::{Database, Keyspace, KeyspaceCreateOptions, Rtxn, codec::Str};
///
/// fn greeting(ks: &Keyspace<Str, Str>, rtxn: &Rtxn) -> Option<String> {
///     ks.get(rtxn, "hello").unwrap()
/// }
///
/// # let folder = tempfile::tempdir().unwrap();
/// # let db = Database::builder(folder.path()).unwrap();
/// let ks = db.keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default).unwrap();
///
/// let mut wtxn = db.write_tx().unwrap();
/// ks.insert(&mut wtxn, "hello", "world").unwrap();
/// // The write transaction sees its own writes
/// assert_eq!(greeting(&ks, &wtxn).as_deref(), Some("world"));
/// // But they're not visible outside until it's committed
/// assert_eq!(greeting(&ks, &db.read_tx()), None);
///
/// wtxn.commit().unwrap().unwrap();
/// assert_eq!(greeting(&ks, &db.read_tx()).as_deref(), Some("world"));
/// ```
///
/// A read transaction can't write:
///
/// ```compile_fail,E0308
/// # use fiole::{Database, KeyspaceCreateOptions, codec::Str};
/// # let folder = tempfile::tempdir().unwrap();
/// # let db = Database::builder(folder.path()).unwrap();
/// # let ks = db.keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default).unwrap();
/// let mut rtxn = db.read_tx();
/// ks.insert(&mut rtxn, "hello", "world").unwrap();
/// ```
//
// Internally it's stored as either a read-only snapshot of the database, returned by
// [`crate::Database::read_tx`], or the read side of a [`Wtxn`].
pub struct Rtxn {
    inner: ReadTx,
}

impl Rtxn {
    #[inline]
    pub(crate) fn new(inner: Snapshot) -> Self {
        Self {
            inner: ReadTx::Snapshot(inner),
        }
    }

    #[inline]
    pub(crate) fn inner(&self) -> &ReadTx {
        &self.inner
    }
}

impl AsRef<Rtxn> for Rtxn {
    #[inline]
    fn as_ref(&self) -> &Rtxn {
        self
    }
}

/// What an [`Rtxn`] reads from.
pub(crate) enum ReadTx {
    Snapshot(Snapshot),
    Write(OptimisticWriteTx),
}

/// Calls the same method on whichever transaction is inside.
macro_rules! dispatch {
    ($self:ident.$method:ident($($arg:expr),*)) => {
        match $self {
            ReadTx::Snapshot(inner) => inner.$method($($arg),*),
            ReadTx::Write(inner) => inner.$method($($arg),*),
        }
    };
}

impl fjall::Readable for ReadTx {
    fn get<K: AsRef<[u8]>>(
        &self,
        keyspace: impl AsRef<Keyspace>,
        key: K,
    ) -> fjall::Result<Option<UserValue>> {
        dispatch!(self.get(keyspace, key))
    }

    fn contains_key<K: AsRef<[u8]>>(
        &self,
        keyspace: impl AsRef<Keyspace>,
        key: K,
    ) -> fjall::Result<bool> {
        dispatch!(self.contains_key(keyspace, key))
    }

    fn first_key_value(&self, keyspace: impl AsRef<Keyspace>) -> Option<Guard> {
        dispatch!(self.first_key_value(keyspace))
    }

    fn last_key_value(&self, keyspace: impl AsRef<Keyspace>) -> Option<Guard> {
        dispatch!(self.last_key_value(keyspace))
    }

    fn size_of<K: AsRef<[u8]>>(
        &self,
        keyspace: impl AsRef<Keyspace>,
        key: K,
    ) -> fjall::Result<Option<u32>> {
        dispatch!(self.size_of(keyspace, key))
    }

    fn is_empty(&self, keyspace: impl AsRef<Keyspace>) -> fjall::Result<bool> {
        dispatch!(self.is_empty(keyspace))
    }

    fn iter(&self, keyspace: impl AsRef<Keyspace>) -> Iter {
        dispatch!(self.iter(keyspace))
    }

    fn len(&self, keyspace: impl AsRef<Keyspace>) -> fjall::Result<usize> {
        dispatch!(self.len(keyspace))
    }

    fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(
        &self,
        keyspace: impl AsRef<Keyspace>,
        range: R,
    ) -> Iter {
        dispatch!(self.range(keyspace, range))
    }

    fn prefix<K: AsRef<[u8]>>(&self, keyspace: impl AsRef<Keyspace>, prefix: K) -> Iter {
        dispatch!(self.prefix(keyspace, prefix))
    }
}

#[cfg(test)]
mod test {
    use crate::{Database, Keyspace, KeyspaceCreateOptions, Rtxn, codec::Str};

    fn setup() -> (tempfile::TempDir, Database, Keyspace<Str, Str>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::builder(dir.path()).unwrap();
        let ks = db
            .keyspace::<Str, Str>("hello", KeyspaceCreateOptions::default)
            .unwrap();
        (dir, db, ks)
    }

    /// A helper that only needs to read, written without any generic.
    fn len(ks: &Keyspace<Str, Str>, rtxn: &Rtxn) -> usize {
        ks.len(rtxn).unwrap()
    }

    #[test]
    #[allow(clippy::unnecessary_mut_passed)] // `&mut Wtxn` must coerce too
    fn a_wtxn_can_be_used_as_an_rtxn() {
        let (_dir, db, ks) = setup();

        let mut wtxn = db.write_tx().unwrap();
        ks.insert(&mut wtxn, "hello", "world").unwrap();
        assert_eq!(len(&ks, &wtxn), 1);
        assert_eq!(len(&ks, &mut wtxn), 1);
        // A snapshot opened before the commit doesn't see the writes
        assert_eq!(len(&ks, &db.read_tx()), 0);
        wtxn.commit().unwrap().unwrap();

        assert_eq!(len(&ks, &db.read_tx()), 1);
    }

    #[test]
    fn reads_through_the_rtxn_of_a_wtxn_see_its_writes() {
        let (_dir, db, ks) = setup();

        let mut wtxn = db.write_tx().unwrap();
        ks.insert(&mut wtxn, "hello", "world").unwrap();
        let rtxn: &Rtxn = &wtxn;
        assert_eq!(ks.get(rtxn, "hello").unwrap().as_deref(), Some("world"));
    }

    #[test]
    fn reads_through_the_rtxn_of_a_wtxn_are_checked_for_conflicts() {
        let (_dir, db, ks) = setup();

        let mut first = db.write_tx().unwrap();
        let mut second = db.write_tx().unwrap();

        assert_eq!(len(&ks, &first), 0);
        ks.insert(&mut second, "counter", "1").unwrap();
        second.commit().unwrap().unwrap();

        ks.insert(&mut first, "counter", "1").unwrap();
        assert!(matches!(first.commit().unwrap(), Err(crate::Conflict)));
    }

    #[test]
    fn as_ref() {
        fn len(ks: &Keyspace<Str, Str>, rtxn: impl AsRef<Rtxn>) -> usize {
            ks.len(rtxn.as_ref()).unwrap()
        }

        let (_dir, db, ks) = setup();
        let mut wtxn = db.write_tx().unwrap();
        ks.insert(&mut wtxn, "hello", "world").unwrap();
        assert_eq!(len(&ks, &wtxn), 1);
        assert_eq!(len(&ks, db.read_tx()), 0);
    }
}
