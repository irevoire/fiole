//! A global allocator counting the allocations of the current thread, to
//! check that some code paths don't copy anything.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    // `try_with` because the allocator can still be called while the thread is being destroyed
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Runs `f` and returns its result with the number of allocations it made.
pub fn allocations<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let before = ALLOCATIONS.with(Cell::get);
    let ret = f();
    let after = ALLOCATIONS.with(Cell::get);
    (ret, after - before)
}

/// Encodes `item` with `C` into a fjall [`fjall::Slice`], then checks that
/// decoding it from the borrowed slice gives it back without allocating.
pub fn assert_decodes_without_allocating<C>(item: &<C as crate::codec::Encode>::Item)
where
    C: crate::codec::Encode + crate::codec::Decode,
    <C as crate::codec::Encode>::Item: std::fmt::Debug,
    <C as crate::codec::Decode>::Item:
        PartialEq<<C as crate::codec::Encode>::Item> + std::fmt::Debug,
    <C as crate::codec::Decode>::Error: std::fmt::Debug,
{
    let slice = C::encode_alloc(item).ok().unwrap().into_fjall_slice();
    let mut decoder = crate::codec::DecodingVec::from(&slice);
    let (decoded, allocations) = allocations(|| C::decode(&mut decoder).unwrap());
    assert_eq!(decoded, *item);
    assert!(decoder.is_empty(), "{item:?} wasn't fully read");
    assert_eq!(allocations, 0, "decoding {item:?} allocated");
}

/// Same as [`assert_decodes_without_allocating`], for codecs that allocate
/// while decoding: decoding from the borrowed slice must allocate exactly as
/// much as decoding from bytes the decoder already owns, so the bytes aren't
/// copied.
#[cfg(any(
    feature = "facet_json",
    feature = "facet_postcard",
    feature = "facet_msgpack"
))]
pub fn assert_decodes_without_copying<C>(item: &<C as crate::codec::Encode>::Item)
where
    C: crate::codec::Encode + crate::codec::Decode,
    <C as crate::codec::Encode>::Item: std::fmt::Debug,
    <C as crate::codec::Decode>::Item:
        PartialEq<<C as crate::codec::Encode>::Item> + std::fmt::Debug,
    <C as crate::codec::Decode>::Error: std::fmt::Debug,
{
    let slice = C::encode_alloc(item).ok().unwrap().into_fjall_slice();
    // Some codecs initialize caches the first time they decode, don't count them
    C::decode(&mut crate::codec::DecodingVec::new(&slice)).unwrap();

    let mut owned = crate::codec::DecodingVec::new(&slice);
    let (decoded, owned_allocations) = allocations(|| C::decode(&mut owned).unwrap());
    assert_eq!(decoded, *item);

    let mut borrowed = crate::codec::DecodingVec::from(&slice);
    let (decoded, borrowed_allocations) = allocations(|| C::decode(&mut borrowed).unwrap());
    assert_eq!(decoded, *item);
    assert!(borrowed.is_empty(), "{item:?} wasn't fully read");
    assert_eq!(
        borrowed_allocations, owned_allocations,
        "decoding {item:?} from borrowed bytes copied them"
    );
}
