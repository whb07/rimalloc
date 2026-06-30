use super::*;
#[cfg(all(target_os = "linux", not(debug_assertions)))]
use crate::atomic::mi_lock_t;
use crate::core_types::{OwnedAbandonedPage, PagePtr};
use crate::types::{mi_block_t, mi_page_t};
use core::{
    mem::MaybeUninit,
    ptr,
    sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize},
};

#[cfg(all(target_os = "linux", not(debug_assertions)))]
#[test]
fn linux_mi_lock_matches_pthread_mutex_layout() {
    assert_eq!(
        core::mem::size_of::<mi_lock_t>(),
        40,
        "Linux mi_lock_t should match the pthread_mutex_t-backed C layout"
    );
    assert_eq!(
        core::mem::align_of::<mi_lock_t>(),
        core::mem::align_of::<usize>(),
        "Linux mi_lock_t should keep pthread mutex alignment"
    );
}

#[test]
fn atomic_usize_field_updates_stack_atomic_word() {
    let atomic = AtomicUsize::new(4);
    let field = AtomicUsizeField((&atomic as *const AtomicUsize).cast_mut().cast::<usize>());

    assert_eq!(unsafe { field.load(Ordering::Relaxed) }, 4);
    unsafe { field.store(7, Ordering::Relaxed) };
    assert_eq!(atomic.load(Ordering::Relaxed), 7);
    assert_eq!(unsafe { field.fetch_add(5, Ordering::Relaxed) }, 7);
    assert_eq!(unsafe { field.fetch_sub(2, Ordering::Relaxed) }, 12);
    assert_eq!(unsafe { field.fetch_or(0b1000, Ordering::Relaxed) }, 10);
    assert_eq!(unsafe { field.fetch_and(0b1010, Ordering::Relaxed) }, 10);

    let mut expected = 10;
    assert!(unsafe { field.compare_exchange_acq_rel(&mut expected, 42) });
    assert_eq!(unsafe { field.load(Ordering::Relaxed) }, 42);

    let mut stale = 10;
    assert!(!unsafe { field.compare_exchange_acq_rel(&mut stale, 99) });
    assert_eq!(stale, 42);
}

#[test]
fn atomic_usize_field_weak_cas_updates_expected_on_failure() {
    let atomic = AtomicUsize::new(33);
    let field = AtomicUsizeField((&atomic as *const AtomicUsize).cast_mut().cast::<usize>());

    let mut expected = 11;
    assert!(!unsafe {
        field.compare_exchange_weak(&mut expected, 44, Ordering::AcqRel, Ordering::Acquire)
    });
    assert_eq!(expected, 33);
    assert_eq!(atomic.load(Ordering::Acquire), 33);

    while !unsafe {
        field.compare_exchange_weak(&mut expected, 44, Ordering::AcqRel, Ordering::Acquire)
    } {
        assert_eq!(expected, 33);
    }
    assert_eq!(atomic.load(Ordering::Acquire), 44);
}

#[test]
fn atomic_i64_field_updates_expected_on_cas_failure() {
    let atomic = AtomicI64::new(-4);
    let field = AtomicI64Field((&atomic as *const AtomicI64).cast_mut().cast::<i64>());

    assert_eq!(unsafe { field.load(Ordering::Relaxed) }, -4);
    unsafe { field.store(11, Ordering::Relaxed) };

    let mut expected = 9;
    assert!(!unsafe { field.compare_exchange_acq_rel(&mut expected, 12) });
    assert_eq!(expected, 11);
    assert_eq!(atomic.load(Ordering::Relaxed), 11);

    assert!(unsafe { field.compare_exchange_acq_rel(&mut expected, 12) });
    assert_eq!(atomic.load(Ordering::Relaxed), 12);
}

#[test]
fn atomic_ptr_field_round_trips_pointer_values() {
    let mut first = 1u8;
    let mut second = 2u8;
    let first_ptr = (&mut first as *mut u8).cast::<c_void>();
    let second_ptr = (&mut second as *mut u8).cast::<c_void>();
    let atomic = AtomicPtr::new(first_ptr);
    let field =
        || AtomicPtrField::<c_void>((&atomic as *const AtomicPtr<c_void>).cast_mut().cast());

    assert_eq!(unsafe { field().load(Ordering::Acquire) }, first_ptr);
    assert_eq!(unsafe { field().exchange_acq_rel(second_ptr) }, first_ptr);
    assert_eq!(atomic.load(Ordering::Acquire), second_ptr);

    let mut expected = first_ptr;
    assert!(!unsafe { field().compare_exchange_acq_rel(&mut expected, first_ptr) });
    assert_eq!(expected, second_ptr);
    assert!(unsafe { field().compare_exchange_release(&mut expected, first_ptr) });
    assert_eq!(atomic.load(Ordering::Acquire), first_ptr);
}

#[test]
fn atomic_arena_page_loads_and_stores_page_table_pointer() {
    let mut pages = MaybeUninit::<crate::types::mi_arena_pages_t>::zeroed();
    let pages_ptr = pages.as_mut_ptr();
    let atomic = AtomicPtr::new(ptr::null_mut::<c_void>());
    let field = AtomicArenaPage(&atomic);

    assert_eq!(unsafe { field.load_acquire() }, ptr::null_mut());
    unsafe { field.store_release(pages_ptr) };
    assert_eq!(unsafe { field.load_acquire() }, pages_ptr);
    assert_eq!(atomic.load(Ordering::Acquire), pages_ptr.cast::<c_void>());
}

#[test]
fn atomic_page_thread_id_updates_owner_and_flags() {
    let initial =
        PageThreadId::from_parts(PageOwnerId::Abandoned, PageFlags::HAS_INTERIOR_POINTERS);
    let atomic = AtomicUsize::new(initial.raw());
    let field = AtomicPageThreadId(AtomicUsizeField(
        (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
    ));

    assert!(unsafe {
        field
            .load()
            .flags()
            .contains(PageFlags::HAS_INTERIOR_POINTERS)
    });

    let old = unsafe { field.set_flags(PageFlags::IN_FULL_QUEUE, true) };
    assert!(!old.flags().contains(PageFlags::IN_FULL_QUEUE));
    assert!(unsafe { field.load().flags().contains(PageFlags::IN_FULL_QUEUE) });

    unsafe { field.store_owner(PageOwnerId::AbandonedMapped) };
    let stored = unsafe { field.load() };
    assert!(stored.is_abandoned_mapped());
    assert!(stored.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));
    assert!(stored.flags().contains(PageFlags::IN_FULL_QUEUE));

    let desired = PageThreadId::from_parts(PageOwnerId::Abandoned, stored.flags());
    let mut expected = PageThreadId::new(0);
    assert!(!unsafe { field.compare_exchange_owner(&mut expected, desired) });
    assert_eq!(expected.raw(), stored.raw());

    while !unsafe { field.compare_exchange_owner(&mut expected, desired) } {
        assert_eq!(expected.raw(), stored.raw());
    }
    assert!(unsafe { field.load().is_abandoned() });
}

#[test]
fn atomic_thread_free_claims_and_releases_ownership_bit() {
    let atomic = AtomicUsize::new(0);
    let field = AtomicThreadFree(AtomicUsizeField(
        (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
    ));

    assert!(!field.load_relaxed().is_owned());
    assert!(unsafe { field.claim_acq_rel() });
    assert!(field.load_relaxed().is_owned());
    assert!(!unsafe { field.claim_acq_rel() });
    let mut expected = field.load_relaxed();
    while !unsafe { field.release_acq_rel(&mut expected) } {
        assert!(expected.is_owned());
    }
    assert!(!field.load_relaxed().is_owned());
    let mut expected = field.load_relaxed();
    assert!(!unsafe { field.release_acq_rel(&mut expected) });
}

#[test]
fn atomic_thread_free_take_head_preserves_owner_bit() {
    let mut block = crate::types::mi_block_t::default();
    let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
    let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
    let atomic = AtomicUsize::new(word.raw());
    let field = AtomicThreadFree(AtomicUsizeField(
        (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
    ));

    let list = unsafe { field.collect_acq_rel() };
    assert_eq!(list.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));

    let remaining = field.load_relaxed();
    assert!(remaining.is_owned());
    assert!(remaining.head().is_none());
}

#[test]
fn empty_remote_free_list_collects_through_detached_atomic_path() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut local = mi_block_t::default();
    unsafe {
        let raw_page = page.as_mut_ptr();
        ptr::addr_of_mut!((*raw_page).xthread_free)
            .write(ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned).raw());
        ptr::addr_of_mut!((*raw_page).local_free).write(ptr::addr_of_mut!(local));

        let page_ptr = PagePtr::new_unchecked(raw_page);
        let atomic = AtomicThreadFree(AtomicUsizeField(ptr::addr_of_mut!(
            (*raw_page).xthread_free
        )));
        let remote = atomic.collect_acq_rel();
        assert!(remote.head().is_none());

        let local_before = LocalFreeList::from_raw((*raw_page).local_free);
        let local_after =
            remote.collect_into_local(OwnedAbandonedPage::new_unchecked(page_ptr), local_before);
        assert_eq!(
            local_after.0.map(BlockPtr::as_ptr),
            Some(ptr::addr_of_mut!(local))
        );
    }
}

#[test]
fn atomic_abandoned_count_uses_relaxed_cell_operations() {
    let atomic = AtomicUsize::new(2);
    let count = AtomicAbandonedCount(&atomic);

    assert_eq!(unsafe { count.load_relaxed() }, 2);
    assert_eq!(unsafe { count.fetch_add_relaxed(1) }, 2);
    assert_eq!(unsafe { count.load_relaxed() }, 3);
    assert_eq!(unsafe { count.fetch_sub_relaxed(2) }, 3);
    assert_eq!(unsafe { count.load_relaxed() }, 1);
}

#[test]
fn atomic_thread_heap_refcount_uses_acq_rel_operations() {
    let atomic = AtomicUsize::new(0);
    let refcount = AtomicThreadHeapRefcount(AtomicUsizeField(
        (&atomic as *const AtomicUsize).cast_mut().cast::<usize>(),
    ));

    unsafe { refcount.store_seq_cst(1) };
    assert_eq!(atomic.load(Ordering::SeqCst), 1);
    assert_eq!(unsafe { refcount.incref_acq_rel() }, 1);
    assert_eq!(unsafe { refcount.decref_acq_rel() }, 2);
    assert_eq!(atomic.load(Ordering::Acquire), 1);
}

#[test]
fn atomic_guard_acquires_once_and_releases_on_drop() {
    let atomic = AtomicUsize::new(0);
    let field = AtomicUsizeField((&atomic as *const AtomicUsize).cast_mut().cast::<usize>());

    let guard = unsafe { AtomicGuard::try_acquire(field) }.expect("first acquire succeeds");
    assert_eq!(atomic.load(Ordering::Acquire), 1);
    assert!(unsafe { AtomicGuard::try_acquire(field) }.is_none());

    drop(guard);
    assert_eq!(atomic.load(Ordering::Acquire), 0);
    assert!(unsafe { AtomicGuard::try_acquire(field) }.is_some());
}

#[test]
fn once_guard_enters_once_and_releases_on_drop() {
    let mut once = crate::atomic::mi_atomic_once_t::new();
    let state = OnceStatePtr(ptr::addr_of_mut!(once).cast::<usize>());

    let guard = unsafe { OnceGuard::try_enter(state) }.expect("first enter succeeds");
    assert!(once.tid.load(Ordering::Acquire) > 1);
    assert!(unsafe { OnceGuard::try_enter(state) }.is_none());

    drop(guard);
    assert_eq!(once.tid.load(Ordering::Acquire), 1);
    assert!(unsafe { OnceGuard::try_enter(state) }.is_none());
}

#[test]
fn mi_lock_guard_acquires_and_releases_on_drop() {
    let mut lock = crate::atomic::mi_lock_t::new();
    let lock_ptr = MiLockPtr(unsafe { NonNull::new_unchecked(ptr::addr_of_mut!(lock)) });

    let guard = unsafe { MiLockGuard::acquire(lock_ptr) };
    assert!(!unsafe { crate::atomic::mi_lock_try_acquire(lock_ptr.0.as_ptr()) });

    drop(guard);
    assert!(unsafe { crate::atomic::mi_lock_try_acquire(lock_ptr.0.as_ptr()) });
    unsafe { crate::atomic::mi_lock_release(lock_ptr.0.as_ptr()) };
}
