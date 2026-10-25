use std::ops::{Bound, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};

/// A range of keys, accepted by [`Keyspace::range`](crate::Keyspace::range).
///
/// It's like [`std::ops::RangeBounds`], except that it's implemented for the
/// ranges of references to unsized keys too: `"a".."c"` is a range of `str`
/// and `&b"a"[..]..` a range of `[u8]`, which `RangeBounds` doesn't support.
///
/// It's implemented for all the ranges of the standard library, holding
/// either keys or references to keys, and for the tuples of [`Bound`]s.
///
/// ```
/// # use fiole::{Database, KeyspaceCreateOptions, codec::{Str, U8}};
/// #
/// # let folder = tempfile::tempdir().unwrap();
/// # let db = Database::builder(folder).unwrap();
/// let names = db.keyspace::<Str, Str>("names", KeyspaceCreateOptions::default).unwrap();
/// let numbers = db.keyspace::<U8, Str>("numbers", KeyspaceCreateOptions::default).unwrap();
/// let rtxn = db.read_tx();
///
/// names.range(&rtxn, &("a".."c")).unwrap();
/// numbers.range(&rtxn, &(1..3)).unwrap();
/// numbers.range(&rtxn, &(&1..)).unwrap();
/// ```
pub trait KeyRange<K: ?Sized> {
    /// The start bound of the range.
    fn start_bound(&self) -> Bound<&K>;
    /// The end bound of the range.
    fn end_bound(&self) -> Bound<&K>;
}

impl<K: ?Sized> KeyRange<K> for RangeFull {
    fn start_bound(&self) -> Bound<&K> {
        Bound::Unbounded
    }

    fn end_bound(&self) -> Bound<&K> {
        Bound::Unbounded
    }
}

/// Implements [`KeyRange`] for a range holding keys, and for the same range
/// holding references to keys.
macro_rules! impl_key_range {
    ($range:ident, start: |$s:ident| $start:expr, end: |$e:ident| $end:expr) => {
        impl<K> KeyRange<K> for $range<K> {
            fn start_bound(&self) -> Bound<&K> {
                let $s = self;
                $start
            }

            fn end_bound(&self) -> Bound<&K> {
                let $e = self;
                $end
            }
        }

        impl<K: ?Sized> KeyRange<K> for $range<&K> {
            fn start_bound(&self) -> Bound<&K> {
                let $s = self;
                let bound: Bound<&&K> = $start;
                bound.map(|key| *key)
            }

            fn end_bound(&self) -> Bound<&K> {
                let $e = self;
                let bound: Bound<&&K> = $end;
                bound.map(|key| *key)
            }
        }
    };
}

#[rustfmt::skip]
mod impls {
    use super::*;

    impl_key_range!(Range, start: |r| Bound::Included(&r.start), end: |r| Bound::Excluded(&r.end));
    impl_key_range!(RangeFrom, start: |r| Bound::Included(&r.start), end: |_r| Bound::Unbounded);
    impl_key_range!(RangeTo, start: |_r| Bound::Unbounded, end: |r| Bound::Excluded(&r.end));
    impl_key_range!(RangeInclusive, start: |r| Bound::Included(r.start()), end: |r| Bound::Included(r.end()));
    impl_key_range!(RangeToInclusive, start: |_r| Bound::Unbounded, end: |r| Bound::Included(&r.end));
}

impl<K> KeyRange<K> for (Bound<K>, Bound<K>) {
    fn start_bound(&self) -> Bound<&K> {
        self.0.as_ref()
    }

    fn end_bound(&self) -> Bound<&K> {
        self.1.as_ref()
    }
}

impl<K: ?Sized> KeyRange<K> for (Bound<&K>, Bound<&K>) {
    fn start_bound(&self) -> Bound<&K> {
        self.0
    }

    fn end_bound(&self) -> Bound<&K> {
        self.1
    }
}
