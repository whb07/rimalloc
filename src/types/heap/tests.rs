use super::*;
use crate::allocator::heap::mi_heap_new;
use crate::init::mi_heap_main;
use crate::mimalloc::mi_heap_delete;
use crate::types::{mi_heap_t, mi_theap_t, mi_tld_t};
use core::mem::MaybeUninit;

fn with_heap_ptr(f: impl FnOnce(HeapPtr)) {
    let mut heap = MaybeUninit::<mi_heap_t>::uninit();
    // SAFETY: `heap` is stack-local heap
    // metadata used only as an identity witness during the callback.
    let ptr = unsafe { HeapPtr::new_unchecked(heap.as_mut_ptr()) };
    f(ptr);
}

fn with_thread_heap_ptr(heap: Option<HeapPtr>, tld: Option<TldPtr>, f: impl FnOnce(ThreadHeapPtr)) {
    let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
    let raw = theap.as_mut_ptr();
    // SAFETY: `raw` points at stack-local,
    // zeroed thread-heap metadata that this fixture owns exclusively while it
    // writes heap/tld identity fields and constructs the witness.
    unsafe {
        (*raw).heap = heap.map(HeapPtr::as_ptr).unwrap_or(core::ptr::null_mut());
        (*raw).tld = tld.map(TldPtr::as_ptr).unwrap_or(core::ptr::null_mut());
        f(ThreadHeapPtr::new_unchecked(raw));
    }
}

#[test]
fn heap_witnesses_preserve_heap_pointer() {
    with_heap_ptr(|ptr| {
        // SAFETY: `ptr` comes from the
        // stack-local heap fixture and is used only to check witness identity.
        let main = unsafe { MainHeap::new_unchecked(ptr) };
        // SAFETY: same stack-local heap
        // identity witness as above; no allocator operation observes it.
        let owned = unsafe { OwnedHeap::new_unchecked(ptr) };
        // SAFETY: same stack-local heap
        // identity witness as above; no allocator operation observes it.
        let borrowed = unsafe { BorrowedHeap::new_unchecked(ptr) };

        assert_eq!(main.ptr().as_ptr(), ptr.as_ptr());
        assert_eq!(owned.ptr().as_ptr(), ptr.as_ptr());
        assert_eq!(borrowed.ptr().as_ptr(), ptr.as_ptr());
    });
}

#[test]
fn main_heap_checked_constructor_accepts_only_main_heap() {
    // SAFETY: `mi_heap_main` returns the
    // allocator-owned main heap pointer for checked witness construction.
    let main_raw = unsafe { mi_heap_main() };
    // SAFETY: the main heap pointer is non-null
    // allocator metadata for this process.
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
    // SAFETY: the checked constructor verifies
    // that `main_ptr` is the process main heap before producing the witness.
    let main = unsafe { MainHeap::new(main_ptr) }.expect("main heap should be accepted");
    assert_eq!(main.ptr().as_ptr(), main_raw);

    // SAFETY: allocates a real heap for the
    // rejection path and releases it before the test returns.
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    // SAFETY: `owned_raw` is non-null heap
    // metadata returned by `mi_heap_new`.
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    assert!(
        // SAFETY: checked constructor must
        // reject this real non-main heap.
        unsafe { MainHeap::new(owned_ptr) }.is_none(),
        "non-main heaps must not produce MainHeap witnesses"
    );
    // SAFETY: releases the heap allocated above
    // exactly once after all witnesses are consumed.
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn owned_heap_checked_constructor_rejects_main_heap() {
    // SAFETY: `mi_heap_main` returns the
    // allocator-owned main heap pointer for checked witness construction.
    let main_raw = unsafe { mi_heap_main() };
    // SAFETY: the main heap pointer is non-null
    // allocator metadata for this process.
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
    assert!(
        // SAFETY: checked constructor must
        // reject the process main heap.
        unsafe { OwnedHeap::new(main_ptr) }.is_none(),
        "main heap must not produce an OwnedHeap witness"
    );

    // SAFETY: allocates a real non-main heap
    // for the accepted owned-heap path and releases it before return.
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    // SAFETY: `owned_raw` is non-null heap
    // metadata returned by `mi_heap_new`.
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    // SAFETY: checked constructor accepts the
    // non-main heap allocated by this test.
    let owned = unsafe { OwnedHeap::new(owned_ptr) }.expect("non-main heap should be accepted");
    assert_eq!(owned.ptr().as_ptr(), owned_raw);
    // SAFETY: releases the heap allocated above
    // exactly once after all witnesses are consumed.
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn main_heap_cannot_create_owned_delete_witness() {
    // SAFETY: `mi_heap_main` returns the
    // allocator-owned main heap pointer for checked witness construction.
    let main_raw = unsafe { mi_heap_main() };
    // SAFETY: the main heap pointer is non-null
    // allocator metadata for this process.
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };

    // SAFETY: checked constructor verifies the
    // process main heap before producing the witness.
    assert!(unsafe { MainHeap::new(main_ptr) }.is_some());
    assert!(
        // SAFETY: checked constructor must
        // reject main heap ownership/deletion witnesses.
        unsafe { OwnedHeap::new(main_ptr) }.is_none(),
        "main heap must remain unable to enter OwnedHeap deletion helpers"
    );
}

#[test]
fn borrowed_heap_raw_constructors_reject_null_and_preserve_heap() {
    assert!(
        // SAFETY: null input is the explicit
        // rejection case for the raw allocation-heap constructor.
        unsafe { BorrowedHeap::from_allocation_raw(core::ptr::null_mut()) }.is_none(),
        "allocation raw constructor should reject null heaps"
    );
    assert!(
        // SAFETY: null input is the explicit
        // rejection case for the raw lookup-heap constructor.
        unsafe { BorrowedHeap::from_lookup_raw(core::ptr::null_mut()) }.is_none(),
        "lookup raw constructor should preserve failed heap lookups"
    );

    // SAFETY: `mi_heap_main` returns the
    // allocator-owned main heap pointer for borrowed witness construction.
    let main_raw = unsafe { mi_heap_main() };
    // SAFETY: raw constructor accepts non-null
    // heap metadata returned by the allocator.
    let main = unsafe { BorrowedHeap::from_allocation_raw(main_raw) }
        .expect("main heap can be borrowed for allocation");
    assert_eq!(main.ptr().as_ptr(), main_raw);

    // SAFETY: allocates a real non-main heap
    // for borrowed lookup construction and releases it before return.
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    // SAFETY: raw constructor accepts non-null
    // heap metadata returned by lookup/allocation paths.
    let owned = unsafe { BorrowedHeap::from_lookup_raw(owned_raw) }
        .expect("looked-up non-main heap can be borrowed");
    assert_eq!(owned.ptr().as_ptr(), owned_raw);
    // SAFETY: releases the heap allocated above
    // exactly once after all witnesses are consumed.
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn thread_heap_borrowed_accepts_only_initialized_theaps() {
    with_thread_heap_ptr(None, None, |theap| {
        assert!(
            // SAFETY: `theap` is stack-local
            // thread-heap metadata with a cleared heap field.
            unsafe { ThreadHeapBorrowed::new(theap) }.is_none(),
            "cleared heap field should not produce an initialized thread-heap witness"
        );
    });

    // SAFETY: allocates a real non-main heap
    // whose pointer is stored in stack-local thread-heap metadata.
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    // SAFETY: `owned_raw` is non-null heap
    // metadata returned by `mi_heap_new`.
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    with_thread_heap_ptr(Some(owned_ptr), None, |theap| {
        // SAFETY: `theap` is stack-local
        // metadata with a non-null heap field.
        let borrowed = unsafe { ThreadHeapBorrowed::new(theap) }
            .expect("non-null heap field should produce borrowed thread-heap witness");
        assert_eq!(borrowed.ptr().as_ptr(), theap.as_ptr());
    });
    // SAFETY: releases the heap allocated above
    // exactly once after the stack-local thread-heap fixture is consumed.
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn thread_heap_state_classifies_main_initialized_uninitialized_and_deleted() {
    // SAFETY: `mi_heap_main` returns the
    // allocator-owned main heap pointer for stack-local thread-heap metadata.
    let main_raw = unsafe { mi_heap_main() };
    // SAFETY: the main heap pointer is non-null
    // allocator metadata for this process.
    let main_ptr = unsafe { HeapPtr::new_unchecked(main_raw) };
    with_thread_heap_ptr(Some(main_ptr), None, |theap| {
        // SAFETY: `theap` is stack-local
        // thread-heap metadata populated with the process main heap pointer.
        match unsafe { theap.state() } {
            ThreadHeapState::Main(main) => assert_eq!(main.ptr().as_ptr(), theap.as_ptr()),
            _ => panic!("main heap thread-heap should classify as main"),
        }
    });

    // SAFETY: allocates a real non-main heap
    // whose pointer is stored in stack-local thread-heap metadata.
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    // SAFETY: `owned_raw` is non-null heap
    // metadata returned by `mi_heap_new`.
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    with_thread_heap_ptr(Some(owned_ptr), None, |theap| {
        // SAFETY: `theap` is stack-local
        // thread-heap metadata populated with a non-main heap pointer.
        match unsafe { theap.state() } {
            ThreadHeapState::Initialized(initialized) => {
                assert_eq!(initialized.ptr().as_ptr(), theap.as_ptr())
            }
            _ => panic!("non-main initialized thread-heap should classify as initialized"),
        }
    });
    // SAFETY: releases the heap allocated above
    // exactly once after the stack-local thread-heap fixture is consumed.
    unsafe { mi_heap_delete(owned_raw) };

    // SAFETY: dangling non-null TLD pointer is
    // used only as an identity sentinel; the test never dereferences it.
    let tld = unsafe { TldPtr::new_unchecked(core::ptr::NonNull::<mi_tld_t>::dangling().as_ptr()) };
    with_thread_heap_ptr(None, Some(tld), |theap| match unsafe { theap.state() } {
        ThreadHeapState::Uninitialized(raw) => assert_eq!(raw.as_ptr(), theap.as_ptr()),
        _ => panic!("null heap with tld should classify as uninitialized"),
    });

    with_thread_heap_ptr(None, None, |theap| {
        // SAFETY: `theap` is stack-local
        // thread-heap metadata with cleared heap and TLD fields.
        match unsafe { theap.state() } {
            ThreadHeapState::Deleted(raw) => assert_eq!(raw.as_ptr(), theap.as_ptr()),
            _ => panic!("cleared heap and tld should classify as deleted"),
        }
    });
}

#[test]
fn borrowed_thread_heap_converts_to_local_only_for_matching_thread() {
    // SAFETY: allocates a real non-main heap
    // whose pointer is stored in stack-local thread-heap metadata.
    let owned_raw = unsafe { mi_heap_new() };
    assert!(!owned_raw.is_null());
    let mut tld = MaybeUninit::<mi_tld_t>::zeroed();
    let tld_raw = tld.as_mut_ptr();
    // SAFETY: `tld_raw` points at stack-local
    // zeroed TLD metadata owned exclusively by this test.
    unsafe {
        (*tld_raw).thread_id = 16;
    }

    // SAFETY: `owned_raw` is non-null heap
    // metadata returned by `mi_heap_new`.
    let owned_ptr = unsafe { HeapPtr::new_unchecked(owned_raw) };
    // SAFETY: `tld_raw` is stack-local TLD
    // metadata that remains live for the thread-heap fixture callback.
    let tld_ptr = unsafe { TldPtr::new_unchecked(tld_raw) };
    with_thread_heap_ptr(Some(owned_ptr), Some(tld_ptr), |theap| {
        // SAFETY: `theap` is stack-local
        // metadata populated with matching heap and TLD identity fields.
        let borrowed =
            unsafe { ThreadHeapBorrowed::new(theap) }.expect("thread heap is initialized");
        let current = crate::core_types::ThreadId::new(16).expect("valid thread id");
        // SAFETY: the stored thread id matches
        // `current`, so the borrowed thread heap may become local for this test.
        let local = match unsafe { borrowed.try_local(current) } {
            Ok(local) => local,
            Err(_) => panic!("matching thread id should produce ThreadHeapLocal"),
        };
        assert_eq!(local.ptr().as_ptr(), theap.as_ptr());
        assert_eq!(local.borrowed().ptr().as_ptr(), theap.as_ptr());

        let other = crate::core_types::ThreadId::new(24).expect("valid thread id");
        assert!(
            // SAFETY: the mismatched thread id
            // is the explicit rejection path for local-thread promotion.
            unsafe { borrowed.try_local(other) }.is_err(),
            "different thread id should not produce ThreadHeapLocal"
        );
    });
    // SAFETY: releases the heap allocated above
    // exactly once after the stack-local thread-heap fixture is consumed.
    unsafe { mi_heap_delete(owned_raw) };
}

#[test]
fn deleted_thread_heap_cannot_be_promoted_to_local() {
    let current = crate::core_types::ThreadId::new(16).expect("valid thread id");
    with_thread_heap_ptr(None, None, |theap| {
        // SAFETY: `theap` is stack-local
        // thread-heap metadata with cleared heap and TLD fields.
        match unsafe { theap.state() } {
            ThreadHeapState::Deleted(raw) => assert_eq!(raw.as_ptr(), theap.as_ptr()),
            _ => panic!("cleared heap and tld should classify as deleted"),
        }
        assert!(
            // SAFETY: deleted stack-local
            // thread-heap metadata must not produce a borrowed witness.
            unsafe { ThreadHeapBorrowed::new(theap) }.is_none(),
            "deleted thread heaps should not produce borrowed witnesses"
        );
        assert!(
            // SAFETY: deleted stack-local
            // thread-heap metadata is the explicit local-promotion rejection.
            unsafe { theap.try_local(current) }.is_err(),
            "deleted thread heaps should be rejected before ThreadHeapLocal construction"
        );
    });
}
