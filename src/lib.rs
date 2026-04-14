use std::{convert::Infallible, marker::PhantomData};

pub mod codec;
mod database;
pub mod error;
mod keyspace;
pub(crate) mod txn;

pub use database::Database;
pub use error::Error;
pub use keyspace::Keyspace;
pub use txn::{Rtxn, Wtxn};

/// Refer to [`fjall::Guard`] for more info.
pub struct Guard<Key, Value>(fjall::Guard, PhantomData<(Key, Value)>);

impl<Key, Value> Guard<Key, Value> {
    #[inline]
    pub(crate) fn new(guard: fjall::Guard) -> Self {
        Self(guard, PhantomData)
    }

    /// Change the codec of the key.
    #[inline]
    #[must_use]
    pub fn remap_key_type<NKey>(self) -> Guard<NKey, Value> {
        Guard(self.0, PhantomData)
    }

    /// Change the codec of the value.
    #[inline]
    #[must_use]
    pub fn remap_value_type<NValue>(self) -> Guard<Key, NValue> {
        Guard(self.0, PhantomData)
    }

    /// Change the codec of the key and value.
    #[inline]
    #[must_use]
    pub fn remap_types<NKey, NValue>(self) -> Guard<NKey, NValue> {
        Guard(self.0, PhantomData)
    }

    /// For more informations, refer to [`fjall::Guard::size`].
    #[inline]
    #[must_use]
    pub fn size(self) -> Result<u32, fjall::Error> {
        self.0.size()
    }
}

impl<Key: codec::Decode, Value> Guard<Key, Value> {
    /// For more informations, refer to [`fjall::Guard::key`].
    #[inline]
    #[must_use]
    pub fn key(self) -> Result<Key::Item, Error<Key::Error, Infallible>> {
        let key = self.0.key().map_err(Error::Fjall)?;
        Key::decode(key).map_err(Error::Key)
    }
}

impl<Key, Value: codec::Decode> Guard<Key, Value> {
    /// For more informations, refer to [`fjall::Guard::value`].
    #[inline]
    #[must_use]
    pub fn value(self) -> Result<Value::Item, Error<Infallible, Value::Error>> {
        let value = self.0.value().map_err(Error::Fjall)?;
        Value::decode(value).map_err(Error::Value)
    }
}

impl<Key: codec::Decode, Value: codec::Decode> Guard<Key, Value> {
    /// For more informations, refer to [`fjall::Guard::into_inner`].
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> Result<(Key::Item, Value::Item), Error<Key::Error, Value::Error>> {
        let (k, v) = self.0.into_inner().map_err(Error::Fjall)?;
        Ok((
            Key::decode(k).map_err(Error::Key)?,
            Value::decode(v).map_err(Error::Value)?,
        ))
    }
}

/// Refer to [`fjall::Iter`] for more info.
pub struct Iter<Key, Value>(fjall::Iter, PhantomData<(Key, Value)>);

impl<Key, Value> Iter<Key, Value> {
    #[inline]
    pub(crate) fn new(iter: fjall::Iter) -> Self {
        Self(iter, PhantomData)
    }

    /// Change the codec of the key.
    #[inline]
    pub fn remap_key_type<NKey>(self) -> Iter<NKey, Value> {
        Iter(self.0, PhantomData)
    }

    /// Change the codec of the value.
    #[inline]
    pub fn remap_value_type<NValue>(self) -> Iter<Key, NValue> {
        Iter(self.0, PhantomData)
    }

    /// Change the codec of the key and value.
    #[inline]
    pub fn remap_types<NKey, NValue>(self) -> Iter<NKey, NValue> {
        Iter(self.0, PhantomData)
    }
}

impl<Key, Value> Iterator for Iter<Key, Value> {
    type Item = Guard<Key, Value>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Guard::new)
    }
}

impl<Key, Value> DoubleEndedIterator for Iter<Key, Value> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(Guard::new)
    }
}

#[cfg(test)]
mod test {
    use fjall::KeyspaceCreateOptions;

    use crate::{codec::Str, Database};

    #[test]
    fn get_from_wtxn() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::builder(&dir.path()).unwrap();
        let ks = database
            .keyspace::<Str, Str>("hello", || KeyspaceCreateOptions::default())
            .unwrap();

        // we should be able to get both from a rtxn and a wtxn with the same method and API
        let rtxn = database.read_tx();
        ks.get(&rtxn, "hello").unwrap();
        let wtxn = database.write_tx().unwrap();
        ks.get(&wtxn, "hello").unwrap();
        drop(wtxn);
    }
}
