use fjall::{Conflict, OptimisticWriteTx, PersistMode, Snapshot};

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

    #[inline]
    pub fn commit(self) -> Result<Result<(), Conflict>, fjall::Error> {
        self.inner.commit()
    }

    #[inline]
    pub fn rollback(self) {
        self.inner.rollback()
    }
}

#[repr(transparent)]
#[derive(Clone)]
pub struct Rtxn {
    pub(crate) inner: Snapshot,
}

pub trait Readable {
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
