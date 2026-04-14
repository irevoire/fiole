use std::{marker::PhantomData, path::Path};

use byteview::StrView;
use fjall::{KeyspaceCreateOptions, OptimisticTxDatabase, PersistMode};

use crate::{Keyspace, Rtxn, Wtxn};

/// Wrapper around a [`fjall::OptimisticTxDatabase`].
#[repr(transparent)]
pub struct Database {
    inner: OptimisticTxDatabase,
}

impl Database {
    /// Create a new database. In the future it should return a [`fjall::DatabaseBuilder`].
    #[inline]
    pub fn builder(path: impl AsRef<Path>) -> Result<Self, fjall::Error> {
        Ok(Self {
            inner: OptimisticTxDatabase::builder(path).open()?,
        })
    }

    /// Starts a new writeable transaction.
    ///
    /// ```
    /// use fiole::{Database, KeyspaceCreateOptions};
    /// use fiole::codec::Str;
    ///
    /// let folder = tempfile::tempdir().unwrap();
    /// let db = Database::builder(folder.path()).unwrap();
    /// let ks = db.keyspace::<Str, Str>("my_items", KeyspaceCreateOptions::default).unwrap();
    ///
    /// let mut wtxn = db.write_tx().unwrap();
    /// ks.insert(&mut wtxn, "Hello", "World").unwrap();
    /// wtxn.commit().unwrap();
    /// ```
    #[inline]
    pub fn write_tx(&self) -> Result<Wtxn, fjall::Error> {
        Ok(Wtxn {
            inner: self.inner.write_tx()?,
        })
    }

    /// Starts a new read-only transaction (a.k.a. [`fjall::Snapshot`]).
    ///
    /// ```
    /// use fiole::{Database, KeyspaceCreateOptions};
    /// use fiole::codec::Str;
    ///
    /// let folder = tempfile::tempdir().unwrap();
    /// let db = Database::builder(folder.path()).unwrap();
    /// let ks = db.keyspace::<Str, Str>("my_items", KeyspaceCreateOptions::default).unwrap();
    ///
    /// let mut rtxn = db.read_tx();
    /// let ret = ks.get(&rtxn, "Hello").unwrap();
    /// assert_eq!(ret, None);
    /// ```
    #[inline]
    pub fn read_tx(&self) -> Rtxn {
        Rtxn {
            inner: self.inner.read_tx(),
        }
    }

    /// Flushes the active journal. The durability depends on the PersistMode used.
    ///
    /// Persisting only affects durability, NOT consistency! Even without flushing data is crash-safe.
    #[inline]
    pub fn persist(&self, mode: PersistMode) -> Result<(), fjall::Error> {
        self.inner.persist(mode)
    }

    /// Creates or opens a keyspace.
    ///
    /// If the keyspace does not yet exist, it will be created configured with `create_options`.
    /// Otherwise simply a handle to the existing keyspace will be returned.
    ///
    /// Keyspace names can be up to 255 characters long and can not be empty.
    ///
    /// The keyspace must be typed with a [`crate::codec`].
    ///
    /// ```
    /// use fiole::{Database, KeyspaceCreateOptions};
    /// use fiole::codec::Str;
    ///
    /// let folder = tempfile::tempdir().unwrap();
    /// let db = Database::builder(folder.path()).unwrap();
    /// let ks = db.keyspace::<Str, Str>("my_items", KeyspaceCreateOptions::default).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error, if an IO error occurred.
    ///
    /// # Panics
    ///
    /// Panics if the keyspace name is invalid.
    #[inline]
    pub fn keyspace<Key, Value>(
        &self,
        name: &str,
        create_options: impl FnOnce() -> KeyspaceCreateOptions,
    ) -> Result<Keyspace<Key, Value>, fjall::Error> {
        Ok(Keyspace {
            inner: self.inner.keyspace(name, create_options)?,
            marker: PhantomData,
        })
    }

    /// Returns the number of keyspaces.
    #[inline]
    pub fn keyspace_count(&self) -> usize {
        self.inner.keyspace_count()
    }

    /// Gets a list of all keyspace names in the database.
    #[inline]
    pub fn list_keyspace_names(&self) -> Vec<StrView> {
        self.inner.list_keyspace_names()
    }

    /// Returns `true` if the keyspace with the given name exists.
    #[inline]
    pub fn keyspace_exists(&self, name: &str) -> bool {
        self.inner.keyspace_exists(name)
    }

    /// Returns the current write buffer size (active + sealed memtables).
    #[inline]
    pub fn write_buffer_size(&self) -> u64 {
        self.inner.write_buffer_size()
    }

    /// Returns the number of journal fragments on disk.
    #[inline]
    pub fn journal_count(&self) -> usize {
        self.inner.journal_count()
    }

    /// Returns the disk space usage of the entire database.
    #[inline]
    pub fn disk_space(&self) -> Result<u64, fjall::Error> {
        self.inner.disk_space()
    }
}
