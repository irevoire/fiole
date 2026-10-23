use fjall::{Conflict, OptimisticWriteTx, PersistMode, Snapshot};

/// A read and write transaction.
#[repr(transparent)]
pub struct Wtxn {
    pub(crate) inner: OptimisticWriteTx,
}

impl Wtxn {
    #[inline]
    pub fn durability(self, mode: Option<PersistMode>) -> Wtxn {
        Wtxn {
            inner: self.inner.durability(mode),
        }
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
    /// assert!(first.commit().unwrap().is_err());
    /// ```
    #[inline]
    pub fn commit(self) -> Result<Result<(), Conflict>, fjall::Error> {
        self.inner.commit()
    }

    #[inline]
    pub fn rollback(self) {
        self.inner.rollback()
    }
}

/// A read-only transaction.
///
/// When defining function parameters, prefer using `&impl` [`Readable`] to make your function work both with `Rtxn` and [`Wtxn`].
#[repr(transparent)]
#[derive(Clone)]
pub struct Rtxn {
    pub(crate) inner: Snapshot,
}

/// To be used when your function needs to read data from the database.
pub trait Readable: Send + Sync {
    fn inner(&self) -> &impl fjall::Readable;
}

impl Readable for Rtxn {
    #[inline]
    fn inner(&self) -> &impl fjall::Readable {
        &self.inner
    }
}

impl Readable for Wtxn {
    #[inline]
    fn inner(&self) -> &impl fjall::Readable {
        &self.inner
    }
}
