use std::{marker::PhantomData, path::Path};

use byteview::StrView;
use fjall::{KeyspaceCreateOptions, OptimisticTxDatabase, PersistMode};

use crate::{Keyspace, Rtxn, Wtxn};

#[repr(transparent)]
pub struct Database {
    inner: OptimisticTxDatabase,
}

impl Database {
    #[inline]
    pub fn builder(path: impl AsRef<Path>) -> Result<Self, fjall::Error> {
        Ok(Self {
            inner: OptimisticTxDatabase::builder(path).open()?,
        })
    }

    #[inline]
    pub fn write_tx(&self) -> Result<Wtxn, fjall::Error> {
        Ok(Wtxn {
            inner: self.inner.write_tx()?,
        })
    }

    #[inline]
    pub fn read_tx(&self) -> Rtxn {
        Rtxn {
            inner: self.inner.read_tx(),
        }
    }

    #[inline]
    pub fn persist(&self, mode: PersistMode) -> Result<(), fjall::Error> {
        self.inner.persist(mode)
    }

    #[inline]
    pub fn keyspace_count(&self) -> usize {
        self.inner.keyspace_count()
    }

    #[inline]
    pub fn list_keyspace_names(&self) -> Vec<StrView> {
        self.inner.list_keyspace_names()
    }

    #[inline]
    pub fn keyspace_exists(&self, name: &str) -> bool {
        self.inner.keyspace_exists(name)
    }

    #[inline]
    pub fn write_buffer_size(&self) -> u64 {
        self.inner.write_buffer_size()
    }

    #[inline]
    pub fn journal_count(&self) -> usize {
        self.inner.journal_count()
    }

    #[inline]
    pub fn disk_space(&self) -> Result<u64, fjall::Error> {
        self.inner.disk_space()
    }

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
}
