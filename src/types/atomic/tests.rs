use super::*;
#[cfg(all(target_os = "linux", not(debug_assertions)))]
use crate::atomic::mi_lock_t;
use crate::core_types::{
    LocalFreeList, OwnedAbandonedPage, PageFlags, PageOwnerId, PagePtr, PageThreadId,
    ThreadFreeOwner, ThreadFreeWord,
};
use crate::types::{mi_block_t, mi_page_t};
use core::{
    ffi::c_void,
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
    let field = AtomicUsizeField::from_cell(&atomic);

    assert_eq!(field.load(Ordering::Relaxed), 4);
    field.store(7, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 7);
    assert_eq!(field.fetch_add(5, Ordering::Relaxed), 7);
    assert_eq!(field.fetch_sub(2, Ordering::Relaxed), 12);
    assert_eq!(field.fetch_or(0b1000, Ordering::Relaxed), 10);
    assert_eq!(field.fetch_and(0b1010, Ordering::Relaxed), 10);

    let mut expected = 10;
    assert!(field.compare_exchange_acq_rel(&mut expected, 42));
    assert_eq!(field.load(Ordering::Relaxed), 42);

    let mut stale = 10;
    assert!(!field.compare_exchange_acq_rel(&mut stale, 99));
    assert_eq!(stale, 42);
}

#[test]
fn atomic_usize_field_weak_cas_updates_expected_on_failure() {
    let atomic = AtomicUsize::new(33);
    let field = AtomicUsizeField::from_cell(&atomic);

    let mut expected = 11;
    assert!(!field.compare_exchange_weak(&mut expected, 44, Ordering::AcqRel, Ordering::Acquire));
    assert_eq!(expected, 33);
    assert_eq!(atomic.load(Ordering::Acquire), 33);

    while !field.compare_exchange_weak(&mut expected, 44, Ordering::AcqRel, Ordering::Acquire) {
        assert_eq!(expected, 33);
    }
    assert_eq!(atomic.load(Ordering::Acquire), 44);
}

#[test]
fn atomic_i64_field_updates_expected_on_cas_failure() {
    let atomic = AtomicI64::new(-4);
    let field = AtomicI64Field::from_cell(&atomic);

    assert_eq!(field.load(Ordering::Relaxed), -4);
    field.store(11, Ordering::Relaxed);

    let mut expected = 9;
    assert!(!field.compare_exchange_acq_rel(&mut expected, 12));
    assert_eq!(expected, 11);
    assert_eq!(atomic.load(Ordering::Relaxed), 11);

    assert!(field.compare_exchange_acq_rel(&mut expected, 12));
    assert_eq!(atomic.load(Ordering::Relaxed), 12);
}

#[test]
fn atomic_ptr_field_round_trips_pointer_values() {
    let mut first = 1u8;
    let mut second = 2u8;
    let first_ptr = (&mut first as *mut u8).cast::<c_void>();
    let second_ptr = (&mut second as *mut u8).cast::<c_void>();
    let atomic = AtomicPtr::new(first_ptr);
    let field = || AtomicPtrField::<c_void>::from_cell(&atomic);

    assert_eq!(field().load(Ordering::Acquire), first_ptr);
    assert_eq!(field().exchange_acq_rel(second_ptr), first_ptr);
    assert_eq!(atomic.load(Ordering::Acquire), second_ptr);

    let mut expected = first_ptr;
    assert!(!field().compare_exchange_acq_rel(&mut expected, first_ptr));
    assert_eq!(expected, second_ptr);
    assert!(field().compare_exchange_release(&mut expected, first_ptr));
    assert_eq!(atomic.load(Ordering::Acquire), first_ptr);
}

#[test]
fn atomic_arena_page_loads_and_stores_page_table_pointer() {
    let mut pages = MaybeUninit::<crate::types::mi_arena_pages_t>::zeroed();
    let pages_ptr = pages.as_mut_ptr();
    let atomic = AtomicPtr::new(ptr::null_mut::<c_void>());
    let field = AtomicArenaPage::from_cell(&atomic);

    assert_eq!(field.load_acquire(), ptr::null_mut());
    field.store_release(pages_ptr);
    assert_eq!(field.load_acquire(), pages_ptr);
    assert_eq!(atomic.load(Ordering::Acquire), pages_ptr.cast::<c_void>());
}

#[test]
fn atomic_page_thread_id_updates_owner_and_flags() {
    let initial =
        PageThreadId::from_parts(PageOwnerId::Abandoned, PageFlags::HAS_INTERIOR_POINTERS);
    let atomic = AtomicUsize::new(initial.raw());
    let field = AtomicPageThreadId::from_cell(&atomic);

    unsafe {
        assert!(
            field
                .load()
                .flags()
                .contains(PageFlags::HAS_INTERIOR_POINTERS)
        );

        let old = field.set_flags(PageFlags::IN_FULL_QUEUE, true);
        assert!(!old.flags().contains(PageFlags::IN_FULL_QUEUE));
        assert!(field.load().flags().contains(PageFlags::IN_FULL_QUEUE));

        field.store_owner(PageOwnerId::AbandonedMapped);
        let stored = field.load();
        assert!(stored.is_abandoned_mapped());
        assert!(stored.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));
        assert!(stored.flags().contains(PageFlags::IN_FULL_QUEUE));

        let desired = PageThreadId::from_parts(PageOwnerId::Abandoned, stored.flags());
        let mut expected = PageThreadId::new(0);
        assert!(!field.compare_exchange_owner(&mut expected, desired));
        assert_eq!(expected.raw(), stored.raw());

        while !field.compare_exchange_owner(&mut expected, desired) {
            assert_eq!(expected.raw(), stored.raw());
        }
        assert!(field.load().is_abandoned());
    }
}

#[test]
fn atomic_thread_free_claims_and_releases_ownership_bit() {
    let atomic = AtomicUsize::new(0);
    let field = AtomicThreadFree::from_cell(&atomic);

    assert!(!field.load_relaxed().is_owned());
    unsafe {
        assert!(field.claim_acq_rel());
        assert!(field.load_relaxed().is_owned());
        assert!(!field.claim_acq_rel());
        let mut expected = field.load_relaxed();
        while !field.release_acq_rel(&mut expected) {
            assert!(expected.is_owned());
        }
        assert!(!field.load_relaxed().is_owned());
        let mut expected = field.load_relaxed();
        assert!(!field.release_acq_rel(&mut expected));
    }
}

#[test]
fn atomic_thread_free_take_head_preserves_owner_bit() {
    let mut block = crate::types::mi_block_t::default();
    let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
    let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
    let atomic = AtomicUsize::new(word.raw());
    let field = AtomicThreadFree::from_cell(&atomic);

    let list = unsafe { field.detach_head() };
    assert_eq!(list.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));

    let remaining = field.load_relaxed();
    assert!(remaining.is_owned());
    assert!(remaining.head().is_none());
}

#[test]
fn remote_push_publishes_head_and_atomic_detach_owns_it() {
    let mut block = mi_block_t::default();
    let block = unsafe { BlockPtr::new_unchecked(ptr::addr_of_mut!(block)) };
    let initial = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned);
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let raw_page = page.as_mut_ptr();
    unsafe { ptr::addr_of_mut!((*raw_page).xthread_free).write(initial.raw()) };
    let page = unsafe { PagePtr::new_unchecked(raw_page) };
    let field =
        unsafe { crate::types::atomic::AtomicThreadFree::from_cell(&*page.thread_free_field()) };

    let published = loop {
        match unsafe { field.try_push(initial, block) } {
            RemotePushResult::Published { previous, head } => {
                assert_eq!(previous.raw(), initial.raw());
                assert_eq!(head.head().as_ptr(), block.as_ptr());
                break head;
            }
            RemotePushResult::Retry { observed } => {
                assert_eq!(
                    observed.raw(),
                    initial.raw(),
                    "only a weak-CAS retry is valid"
                )
            }
        }
    };
    published.leave_published();

    let detached = unsafe { field.detach_head() };
    assert_eq!(detached.head().map(BlockPtr::as_ptr), Some(block.as_ptr()));
    assert!(field.load_relaxed().head().is_none());
}

#[test]
fn remote_push_retry_returns_observation_without_consuming_block() {
    let mut raw_block = mi_block_t::default();
    let block = unsafe { BlockPtr::new_unchecked(ptr::addr_of_mut!(raw_block)) };
    let live = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned);
    let stale = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned);
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let raw_page = page.as_mut_ptr();
    unsafe { ptr::addr_of_mut!((*raw_page).xthread_free).write(live.raw()) };
    let page = unsafe { PagePtr::new_unchecked(raw_page) };
    let field =
        unsafe { crate::types::atomic::AtomicThreadFree::from_cell(&*page.thread_free_field()) };

    match unsafe { field.try_push(stale, block) } {
        RemotePushResult::Retry { observed } => assert_eq!(observed.raw(), live.raw()),
        RemotePushResult::Published { .. } => panic!("stale CAS must retry"),
    }
    assert_eq!(block.as_ptr(), ptr::addr_of_mut!(raw_block));
    assert_eq!(field.load_relaxed().raw(), live.raw());
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
        let atomic = AtomicThreadFree::from_cell(
            &*(ptr::addr_of_mut!((*raw_page).xthread_free))
                .cast::<core::sync::atomic::AtomicUsize>(),
        );
        let remote = atomic.detach_head();
        assert!(remote.head().is_none());

        let local_before = LocalFreeList::from_raw((*raw_page).local_free);
        let owned = OwnedAbandonedPage::new_unchecked(page_ptr);
        let local_after = crate::allocator::page::collect_remote_for_owned_abandoned_page(
            &owned,
            remote,
            local_before,
        )
        .expect("empty detached list is valid");
        assert_eq!(local_after.as_raw(), ptr::addr_of_mut!(local));
    }
}

#[test]
fn atomic_abandoned_count_uses_relaxed_cell_operations() {
    let atomic = AtomicUsize::new(2);
    let count = AtomicAbandonedCount::from_cell(&atomic);

    assert_eq!(count.load_relaxed(), 2);
    assert_eq!(count.fetch_add_relaxed(1), 2);
    assert_eq!(count.load_relaxed(), 3);
    assert_eq!(count.fetch_sub_relaxed(2), 3);
    assert_eq!(count.load_relaxed(), 1);
}

#[test]
fn atomic_thread_heap_refcount_uses_acq_rel_operations() {
    let atomic = AtomicUsize::new(0);
    let refcount = AtomicThreadHeapRefcount::from_cell(&atomic);

    unsafe {
        refcount.store_seq_cst(1);
        assert_eq!(atomic.load(Ordering::SeqCst), 1);
        assert_eq!(refcount.incref_acq_rel(), 1);
        assert_eq!(refcount.decref_acq_rel(), 2);
        assert_eq!(atomic.load(Ordering::Acquire), 1);
    }
}

#[test]
fn atomic_guard_acquires_once_and_releases_on_drop() {
    let atomic = AtomicUsize::new(0);
    let field = &atomic;

    let guard = AtomicGuard::try_acquire(field).expect("first acquire succeeds");
    assert_eq!(atomic.load(Ordering::Acquire), 1);
    assert!(AtomicGuard::try_acquire(field).is_none());

    drop(guard);
    assert_eq!(atomic.load(Ordering::Acquire), 0);
    assert!(AtomicGuard::try_acquire(field).is_some());
}

#[test]
fn once_guard_enters_once_and_releases_on_drop() {
    let once = crate::atomic::mi_atomic_once_t::new();
    let state = &once;

    let guard = OnceGuard::try_enter(state).expect("first enter succeeds");
    assert!(once.tid.load(Ordering::Acquire) > 1);
    assert!(OnceGuard::try_enter(state).is_none());

    drop(guard);
    assert_eq!(once.tid.load(Ordering::Acquire), 1);
    assert!(OnceGuard::try_enter(state).is_none());
}

#[test]
fn mi_lock_guard_acquires_and_releases_on_drop() {
    let lock = crate::atomic::mi_lock_t::new();
    let lock_ptr = ptr::from_ref(&lock).cast_mut();

    unsafe {
        let guard = MiLockGuard::acquire(&lock);
        assert!(!crate::atomic::mi_lock_try_acquire(lock_ptr));

        drop(guard);
        assert!(crate::atomic::mi_lock_try_acquire(lock_ptr));
        crate::atomic::mi_lock_release(lock_ptr);
    }
}
