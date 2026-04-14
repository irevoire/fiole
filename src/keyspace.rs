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

#[repr(transparent)]
#[derive(Clone)]
pub struct Keyspace<Key, Value> {
    pub(crate) inner: OptimisticTxKeyspace,
    pub(crate) marker: PhantomData<(Key, Value)>,
}

impl<Key, Value> Keyspace<Key, Value> {
    /// Change the codec of the key.
    #[inline]
    #[must_use]
    pub fn remap_key_type<NKey>(self) -> Keyspace<NKey, Value> {
        Keyspace {
            inner: self.inner,
            marker: PhantomData,
        }
    }

    /// Change the codec of the value.
    #[inline]
    #[must_use]
    pub fn remap_value_type<NValue>(self) -> Keyspace<Key, NValue> {
        Keyspace {
            inner: self.inner,
            marker: PhantomData,
        }
    }

    /// Change the codec of the key and value.
    #[inline]
    #[must_use]
    pub fn remap_types<NKey, NValue>(self) -> Keyspace<NKey, NValue> {
        Keyspace {
            inner: self.inner,
            marker: PhantomData,
        }
    }

    #[inline]
    pub fn path(&self) -> PathBuf {
        self.inner.path()
    }

    #[inline]
    pub fn approximate_len(&self) -> usize {
        self.inner.approximate_len()
    }

    #[inline]
    pub fn first_key_value(&self, rtxn: &impl Readable) -> Option<Guard<Key, Value>> {
        rtxn.inner().first_key_value(&self.inner).map(Guard::new)
    }

    #[inline]
    pub fn last_key_value(&self, rtxn: &impl Readable) -> Option<Guard<Key, Value>> {
        rtxn.inner().last_key_value(&self.inner).map(Guard::new)
    }

    #[inline]
    pub fn is_empty(&self, rtxn: &impl Readable) -> Result<bool, fjall::Error> {
        rtxn.inner().is_empty(&self.inner)
    }

    #[inline]
    pub fn len(&self, rtxn: &impl Readable) -> Result<usize, fjall::Error> {
        rtxn.inner().len(&self.inner)
    }

    #[inline]
    pub fn iter(&self, rtxn: &impl Readable) -> Iter<Key, Value> {
        Iter::new(rtxn.inner().iter(&self.inner))
    }
}

impl<Key: Encode, Value: Decode> Keyspace<Key, Value> {
    #[inline]
    pub fn get(
        &self,
        rtxn: &impl Readable,
        key: &Key::Item,
    ) -> Result<Option<Value::Item>, Error<Key::Error, Value::Error>> {
        let key = Key::encode(key).map_err(Error::Key)?;
        match rtxn.inner().get(&self.inner, key).map_err(Error::Fjall)? {
            Some(value) => Value::decode(value).map(Some).map_err(Error::Value),
            None => Ok(None),
        }
    }

    #[inline]
    pub fn take(
        &self,
        wtxn: &mut Wtxn,
        key: &Key::Item,
    ) -> Result<Option<Value::Item>, Error<Key::Error, Value::Error>> {
        let key = Key::encode(key).map_err(Error::Key)?;
        match wtxn.inner.take(&self.inner, key).map_err(Error::Fjall)? {
            Some(value) => Value::decode(value).map(Some).map_err(Error::Value),
            None => Ok(None),
        }
    }
}

impl<Key: Encode, Value> Keyspace<Key, Value> {
    /// A typed version of [`fjall::Readable::contains_key`], see the original documentation for more infos.
    #[inline]
    pub fn contains_key(
        &self,
        rtxn: &impl Readable,
        key: &Key::Item,
    ) -> Result<bool, Error<Key::Error, Infallible>> {
        let key = Key::encode(key).map_err(Error::Key)?;
        rtxn.inner()
            .contains_key(&self.inner, key)
            .map_err(Error::Fjall)
    }

    /// A typed version of [`fjall::Readable::size_of`], see the original documentation for more infos.
    #[inline]
    pub fn size_of(
        &self,
        rtxn: &impl Readable,
        key: &Key::Item,
    ) -> Result<Option<u32>, Error<Key::Error, Infallible>> {
        let key = Key::encode(key).map_err(Error::Key)?;
        rtxn.inner().size_of(&self.inner, key).map_err(Error::Fjall)
    }

    #[inline]
    pub fn range<R: RangeBounds<Key::Item>>(
        &self,
        rtxn: &impl Readable,
        range: R,
    ) -> Result<Iter<Key, Value>, Key::Error> {
        let start = match range.start_bound() {
            Bound::Included(key) => Bound::Excluded(Key::encode(key)?),
            Bound::Excluded(key) => Bound::Included(Key::encode(key)?),
            Bound::Unbounded => Bound::Unbounded,
        };
        let end = match range.end_bound() {
            Bound::Included(key) => Bound::Excluded(Key::encode(key)?),
            Bound::Excluded(key) => Bound::Included(Key::encode(key)?),
            Bound::Unbounded => Bound::Unbounded,
        };

        Ok(Iter::new(rtxn.inner().range(&self.inner, (start, end))))
    }

    /// A typed version of [`fjall::Readable::prefix`], see the original documentation for more infos.
    #[inline]
    pub fn prefix(
        &self,
        rtxn: &impl Readable,
        prefix: &Key::Item,
    ) -> Result<Iter<Key, Value>, Key::Error> {
        let prefix = Key::encode(prefix)?;

        Ok(Iter::new(rtxn.inner().prefix(&self.inner, prefix)))
    }

    #[inline]
    pub fn remove(&self, wtxn: &mut Wtxn, key: &Key::Item) -> Result<(), Key::Error> {
        let key = Key::encode(key)?;
        wtxn.inner.remove(&self.inner, key);
        Ok(())
    }
}

impl<Key: Encode, Value: Encode> Keyspace<Key, Value> {
    #[inline]
    pub fn insert(
        &self,
        wtxn: &mut Wtxn,
        key: &Key::Item,
        value: &Value::Item,
    ) -> Result<(), Error<Key::Error, Value::Error>> {
        let key = Key::encode(key).map_err(Error::Key)?;
        let value = Value::encode(value).map_err(Error::Value)?;
        wtxn.inner.insert(&self.inner, key, value);
        Ok(())
    }
}
