use super::*;
use crate::allocator::heap::mi_heap_new;
use crate::allocator::init::mi_heap_main;
use crate::mimalloc::mi_heap_delete;
use crate::types::{mi_heap_t, mi_theap_t, mi_tld_t};
use core::mem::MaybeUninit;

fn with_heap_ptr(f: impl FnOnce(HeapPtr)) {
    let mut heap = MaybeUninit::<mi_heap_t>::uninit();
    let ptr = unsafe { HeapPtr::new_unchecked(heap.as_mut_ptr()) };
    f(ptr);
}

fn with_thread_heap_ptr(heap: Option<HeapPtr>, tld: Option<TldPtr>, f: impl FnOnce(ThreadHeapPtr)) {
    let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
    let raw = theap.as_mut_ptr();
    unsafe {
        (*raw).heap = heap.map(HeapPtr::as_ptr).unwrap_or(core::ptr::null_mut());
        (*raw).tld = tld.map(TldPtr::as_ptr).unwrap_or(core::ptr::null_mut());
        f(ThreadHeapPtr::new_unchecked(raw));
    }
}

#[test]
fn heap_witnesses_preserve_heap_pointer() {
    with_heap_ptr(|ptr| {
        let main = unsafe { MainHeap::new_unchecked(ptr) };
        let owned = unsafe { OwnedHeap::new_unchecked(ptr) };
        let borrowed = unsafe { BorrowedHeap::new_unchecked(ptr) };

        assert_eq!(main.ptr().as_ptr(), ptr.as_ptr());
        assert_eq!(owned.ptr().as_ptr(), ptr.as_ptr());
        assert_eq!(borrowed.ptr().as_ptr(), ptr.as_ptr());
    });
}

#[test]
#[cfg_attr(
    miri,
    ignore = "requires OS allocator initialization (unsupported sysconf)"
)]
fn main_heap_checked_constructor_accepts_only_main_heap() {
    let main_raw = mi_heap_main();
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
    let main = unsafe { MainHeap::new(main_ptr) }.expect("main heap should be accepted");
    assert_eq!(main.ptr().as_ptr(), main_raw);

    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    assert!(
        unsafe { MainHeap::new(owned_ptr) }.is_none(),
        "non-main heaps must not produce MainHeap witnesses"
    );
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
#[cfg_attr(
    miri,
    ignore = "requires OS allocator initialization (unsupported sysconf)"
)]
fn owned_heap_checked_constructor_rejects_main_heap() {
    let main_raw = mi_heap_main();
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
    assert!(
        unsafe { OwnedHeap::new(main_ptr) }.is_none(),
        "main heap must not produce an OwnedHeap witness"
    );

    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    let owned = unsafe { OwnedHeap::new(owned_ptr) }.expect("non-main heap should be accepted");
    assert_eq!(owned.ptr().as_ptr(), owned_raw);
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn main_heap_cannot_create_owned_delete_witness() {
    let main_raw = mi_heap_main();
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };

    assert!(unsafe { MainHeap::new(main_ptr) }.is_some());
    assert!(
        unsafe { OwnedHeap::new(main_ptr) }.is_none(),
        "main heap must remain unable to enter OwnedHeap deletion helpers"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "requires OS allocator initialization (unsupported sysconf)"
)]
fn borrowed_heap_raw_constructors_reject_null_and_preserve_heap() {
    assert!(
        unsafe { BorrowedHeap::from_allocation_raw(core::ptr::null_mut()) }.is_none(),
        "allocation raw constructor should reject null heaps"
    );
    assert!(
        unsafe { BorrowedHeap::from_lookup_raw(core::ptr::null_mut()) }.is_none(),
        "lookup raw constructor should preserve failed heap lookups"
    );

    let main_raw = mi_heap_main();
    let main = unsafe { BorrowedHeap::from_allocation_raw(main_raw) }
        .expect("main heap can be borrowed for allocation");
    assert_eq!(main.ptr().as_ptr(), main_raw);

    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let owned = unsafe { BorrowedHeap::from_lookup_raw(owned_raw) }
        .expect("looked-up non-main heap can be borrowed");
    assert_eq!(owned.ptr().as_ptr(), owned_raw);
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
#[cfg_attr(
    miri,
    ignore = "requires OS allocator initialization (unsupported sysconf)"
)]
fn thread_heap_borrowed_accepts_only_initialized_theaps() {
    with_thread_heap_ptr(None, None, |theap| {
        assert!(
            unsafe { ThreadHeapBorrowed::new(theap) }.is_none(),
            "cleared heap field should not produce an initialized thread-heap witness"
        );
    });

    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    with_thread_heap_ptr(Some(owned_ptr), None, |theap| {
        let borrowed = unsafe { ThreadHeapBorrowed::new(theap) }
            .expect("non-null heap field should produce borrowed thread-heap witness");
        assert_eq!(borrowed.ptr().as_ptr(), theap.as_ptr());
    });
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
#[cfg_attr(
    miri,
    ignore = "requires OS allocator initialization (unsupported sysconf)"
)]
fn thread_heap_state_classifies_main_initialized_uninitialized_and_deleted() {
    let main_raw = mi_heap_main();
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
    with_thread_heap_ptr(Some(main_ptr), None, |theap| {
        match unsafe { theap.state() } {
            ThreadHeapState::Main(main) => assert_eq!(main.ptr().as_ptr(), theap.as_ptr()),
            _ => panic!("main heap thread-heap should classify as main"),
        }
    });

    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    with_thread_heap_ptr(Some(owned_ptr), None, |theap| {
        match unsafe { theap.state() } {
            ThreadHeapState::Initialized(initialized) => {
                assert_eq!(initialized.ptr().as_ptr(), theap.as_ptr())
            }
            _ => panic!("non-main initialized thread-heap should classify as initialized"),
        }
    });
    unsafe { mi_heap_delete(owned_raw) };

    let tld = unsafe { TldPtr::new_unchecked(core::ptr::NonNull::<mi_tld_t>::dangling().as_ptr()) };
    with_thread_heap_ptr(None, Some(tld), |theap| match unsafe { theap.state() } {
        ThreadHeapState::Uninitialized => {}
        _ => panic!("null heap with tld should classify as uninitialized"),
    });

    with_thread_heap_ptr(None, None, |theap| match unsafe { theap.state() } {
        ThreadHeapState::Deleted => {}
        _ => panic!("cleared heap and tld should classify as deleted"),
    });
}

#[test]
#[cfg_attr(
    miri,
    ignore = "requires OS allocator initialization (unsupported sysconf)"
)]
fn borrowed_thread_heap_converts_to_local_only_for_matching_thread() {
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let mut tld = MaybeUninit::<mi_tld_t>::zeroed();
    let tld_raw = tld.as_mut_ptr();
    unsafe {
        (*tld_raw).thread_id = 16;
    }

    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    let tld_ptr = unsafe { TldPtr::new_unchecked(tld_raw) };
    with_thread_heap_ptr(Some(owned_ptr), Some(tld_ptr), |theap| {
        let borrowed =
            unsafe { ThreadHeapBorrowed::new(theap) }.expect("thread heap is initialized");
        let current = crate::core_types::ThreadId::new(16).expect("valid thread id");
        let local = match unsafe { borrowed.try_local(current) } {
            Ok(local) => local,
            Err(_) => panic!("matching thread id should produce ThreadHeapLocal"),
        };
        assert_eq!(local.ptr().as_ptr(), theap.as_ptr());

        let other = crate::core_types::ThreadId::new(24).expect("valid thread id");
        assert!(
            unsafe { borrowed.try_local(other) }.is_err(),
            "different thread id should not produce ThreadHeapLocal"
        );
    });
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn deleted_thread_heap_cannot_be_promoted_to_local() {
    let current = crate::core_types::ThreadId::new(16).expect("valid thread id");
    with_thread_heap_ptr(None, None, |theap| {
        match unsafe { theap.state() } {
            ThreadHeapState::Deleted => {}
            _ => panic!("cleared heap and tld should classify as deleted"),
        }
        assert!(
            unsafe { ThreadHeapBorrowed::new(theap) }.is_none(),
            "deleted thread heaps should not produce borrowed witnesses"
        );
        assert!(
            unsafe { theap.try_local(current) }.is_err(),
            "deleted thread heaps should be rejected before ThreadHeapLocal construction"
        );
    });
}
