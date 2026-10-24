use super::*;
use crate::core_types::{
    BlockPtr, DirectBin, PageFlags, PageOwnerId, PageThreadId, ThreadFreeOwner, ThreadFreeWord,
    ThreadHeapPtr, ThreadId,
};
use crate::types::{mi_block_t, mi_page_t, mi_theap_t};
use core::{mem::MaybeUninit, ptr};

fn with_page<R>(xthread_id: usize, f: impl FnOnce(PagePtr) -> R) -> R {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    unsafe {
        ptr::addr_of_mut!((*page.as_mut_ptr()).xthread_id).write(xthread_id);
        f(PagePtr::new_unchecked(page.as_mut_ptr()))
    }
}

fn heap_owner() -> ThreadHeapLocal {
    unsafe {
        ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(
            core::ptr::NonNull::<mi_theap_t>::dangling().as_ptr(),
        ))
    }
}

#[test]
fn uninit_initialize_yields_active_page() {
    with_page(0, |page| unsafe {
        let active = Page::<Uninit>::new_unchecked(page).initialize(heap_owner());
        assert_eq!(active.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn active_available_and_pop_free_block_update_page_state() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    unsafe {
        let raw = page.as_mut_ptr();
        ptr::addr_of_mut!((*raw).free).write(ptr::addr_of_mut!(block));
        ptr::addr_of_mut!((*raw).used).write(0);
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        ptr::addr_of_mut!((*raw).keys).write([0, 0]);
        crate::internal::mi_block_set_next(raw, ptr::addr_of_mut!(block), ptr::null());

        let active = ActivePage::new_unchecked(PagePtr::new_unchecked(raw));
        let available = active
            .try_available()
            .expect("non-null free head is available");
        let (allocated, active_again) = available.pop_free_block();
        assert_eq!(allocated.as_ptr().as_ptr(), ptr::addr_of_mut!(block));
        assert_eq!(active_again.page().as_ptr(), raw);
        assert!((*raw).free.is_null());
        assert_eq!((*raw).used, 1);
        let active_again = active_again
            .try_available()
            .expect_err("empty free list returns the active capability");
        assert_eq!(active_again.page().as_ptr(), raw);
    }
}

#[test]
fn owner_push_free_block_updates_free_list_zero_flag_and_used_count() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    unsafe {
        let raw = page.as_mut_ptr();
        ptr::addr_of_mut!((*raw).free).write(ptr::null_mut());
        ptr::addr_of_mut!((*raw).free_is_zero).write(true);
        ptr::addr_of_mut!((*raw).used).write(1);
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        ptr::addr_of_mut!((*raw).keys).write([0, 0]);

        let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
        let free = crate::core_types::ValidatedFree::from_block_start_unchecked(
            PagePtr::new_unchecked(raw),
            block_ptr,
        );
        let used = free.with_local(|page, block| page.push_free_block(block));

        assert_eq!(used.get(), 0);
        assert_eq!((*raw).used, 0);
        assert_eq!((*raw).free, block_ptr.as_ptr());
        assert!(!(*raw).free_is_zero);
        assert!(crate::internal::mi_block_next(raw, block_ptr.as_ptr()).is_null());
    }
}

#[test]
fn recycled_available_publication_updates_coupled_page_fields() {
    let flags = PageFlags::new(0).expect("zero page flags are valid");
    let local_thread = ThreadId::new(crate::prim::_mi_prim_thread_id())
        .expect("platform current-thread id is nonzero and unflagged");

    let mut local_page = MaybeUninit::<mi_page_t>::uninit();
    let mut local_block = mi_block_t::default();
    // SAFETY: this initializes a distinct stack-local page as current-thread
    // owned with an unowned thread-free word and the exact list fields touched
    // by the local transfer. The block remains live throughout the transition.
    unsafe {
        let raw = local_page.as_mut_ptr();
        let page_ptr = PagePtr::new_unchecked(raw);
        let local_head = BlockPtr::new_unchecked(ptr::addr_of_mut!(local_block));
        ptr::addr_of_mut!((*raw).xthread_id)
            .write(PageThreadId::from_parts(PageOwnerId::Thread(local_thread), flags).raw());
        ptr::addr_of_mut!((*raw).xthread_free)
            .write(ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned).raw());
        ptr::addr_of_mut!((*raw).free).write(ptr::null_mut());
        ptr::addr_of_mut!((*raw).local_free).write(local_head.as_ptr());
        ptr::addr_of_mut!((*raw).free_is_zero).write(true);

        let local_owner = LocalPage::new_unchecked(page_ptr);
        local_owner.transfer_local_to_available(LocalFreeList::from_head(local_head));
        assert_eq!((*raw).free, local_head.as_ptr());
        assert!((*raw).local_free.is_null());
        assert!(!(*raw).free_is_zero);
    }

    let mut abandoned_page = MaybeUninit::<mi_page_t>::uninit();
    let mut spliced_block = mi_block_t::default();
    // SAFETY: this initializes a separate stack-local page as abandoned with
    // its thread-free ownership bit already claimed. The spliced block belongs
    // to this fixture and remains live throughout publication.
    unsafe {
        let raw = abandoned_page.as_mut_ptr();
        let page_ptr = PagePtr::new_unchecked(raw);
        let spliced_head = BlockPtr::new_unchecked(ptr::addr_of_mut!(spliced_block));
        ptr::addr_of_mut!((*raw).xthread_id)
            .write(PageThreadId::from_parts(PageOwnerId::Abandoned, flags).raw());
        ptr::addr_of_mut!((*raw).xthread_free)
            .write(ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned).raw());
        ptr::addr_of_mut!((*raw).free).write(ptr::null_mut());
        ptr::addr_of_mut!((*raw).local_free).write(ptr::null_mut());
        ptr::addr_of_mut!((*raw).free_is_zero).write(true);

        let abandoned_owner = OwnedAbandonedPage::new_unchecked(page_ptr);
        abandoned_owner.publish_recycled_available(AvailableFreeList::from_head(spliced_head));
        assert_eq!((*raw).free, spliced_head.as_ptr());
        assert!((*raw).local_free.is_null());
        assert!(!(*raw).free_is_zero);
    }
}

#[test]
fn local_page_ensure_available_covers_existing_empty_and_transfer_states() {
    let flags = PageFlags::new(0).expect("zero page flags are valid");
    let local_thread = ThreadId::new(crate::prim::_mi_prim_thread_id())
        .expect("platform current-thread id is nonzero and unflagged");
    let local_id = PageThreadId::from_parts(PageOwnerId::Thread(local_thread), flags).raw();
    let unowned = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned).raw();

    let mut existing_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut existing_block = mi_block_t::default();
    let mut existing_local = mi_block_t::default();
    // SAFETY: this fully initialized stack fixture models a live local page
    // with both list heads present for the duration of the consuming check.
    unsafe {
        let raw = existing_page.as_mut_ptr();
        let page = PagePtr::new_unchecked(raw);
        let available_head = BlockPtr::new_unchecked(ptr::addr_of_mut!(existing_block));
        let local_head = BlockPtr::new_unchecked(ptr::addr_of_mut!(existing_local));
        ptr::addr_of_mut!((*raw).xthread_id).write(local_id);
        ptr::addr_of_mut!((*raw).xthread_free).write(unowned);
        ptr::addr_of_mut!((*raw).free).write(available_head.as_ptr());
        ptr::addr_of_mut!((*raw).local_free).write(local_head.as_ptr());
        ptr::addr_of_mut!((*raw).free_is_zero).write(true);

        let available = LocalPage::new_unchecked(page)
            .try_ensure_available()
            .expect("existing available head must succeed");
        assert_eq!(available.page(), page);
        assert_eq!((*raw).free, available_head.as_ptr());
        assert_eq!((*raw).local_free, local_head.as_ptr());
        assert!((*raw).free_is_zero);
    }

    let mut empty_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY: this fully initialized stack fixture models a live local page
    // with both page-local free lists empty.
    unsafe {
        let raw = empty_page.as_mut_ptr();
        let page = PagePtr::new_unchecked(raw);
        ptr::addr_of_mut!((*raw).xthread_id).write(local_id);
        ptr::addr_of_mut!((*raw).xthread_free).write(unowned);
        ptr::addr_of_mut!((*raw).free_is_zero).write(true);

        let local = LocalPage::new_unchecked(page)
            .try_ensure_available()
            .expect_err("empty local page must preserve its owner witness");
        assert_eq!(local.page(), page);
        assert!((*raw).free.is_null());
        assert!((*raw).local_free.is_null());
        assert!((*raw).free_is_zero);
    }

    let mut transfer_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut transfer_block = mi_block_t::default();
    // SAFETY: this fully initialized stack fixture models a live local page
    // whose current non-empty local list can be transferred while it is live.
    unsafe {
        let raw = transfer_page.as_mut_ptr();
        let page = PagePtr::new_unchecked(raw);
        let local_head = BlockPtr::new_unchecked(ptr::addr_of_mut!(transfer_block));
        ptr::addr_of_mut!((*raw).xthread_id).write(local_id);
        ptr::addr_of_mut!((*raw).xthread_free).write(unowned);
        ptr::addr_of_mut!((*raw).local_free).write(local_head.as_ptr());
        ptr::addr_of_mut!((*raw).free_is_zero).write(true);

        let available = LocalPage::new_unchecked(page)
            .try_ensure_available()
            .expect("non-empty local list must become available");
        assert_eq!(available.page(), page);
        assert_eq!((*raw).free, local_head.as_ptr());
        assert!((*raw).local_free.is_null());
        assert!(!(*raw).free_is_zero);
    }
}

#[test]
fn direct_available_page_decodes_sentinel_and_free_head_states() {
    let mut raw_theap = MaybeUninit::<mi_theap_t>::zeroed();
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut block = mi_block_t::default();
    let flags = PageFlags::new(0).expect("zero page flags are valid");
    let local_thread = ThreadId::new(crate::prim::_mi_prim_thread_id())
        .expect("platform current-thread id is nonzero and unflagged");
    let local_id = PageThreadId::from_parts(PageOwnerId::Thread(local_thread), flags).raw();
    let unowned = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned).raw();

    // SAFETY: the stack fixtures initialize the direct slot and exact page
    // ownership/free fields read by this owner-coupled boundary.
    unsafe {
        let theap_raw = raw_theap.as_mut_ptr();
        let page_raw = raw_page.as_mut_ptr();
        let bin = DirectBin::new_unchecked(1);
        ptr::addr_of_mut!((*theap_raw).pages_free_direct)
            .write(crate::allocator::init::MI_SMALL_PAGES_EMPTY());
        let owner = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap_raw));

        assert!(owner.direct_available_page(bin).is_none());

        ptr::addr_of_mut!((*page_raw).theap).write(theap_raw);
        ptr::addr_of_mut!((*page_raw).xthread_id).write(local_id);
        ptr::addr_of_mut!((*page_raw).xthread_free).write(unowned);
        (*theap_raw).pages_free_direct[bin.as_usize()] = page_raw;
        assert!(owner.direct_available_page(bin).is_none());

        let free = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
        ptr::addr_of_mut!((*page_raw).free).write(free.as_ptr());
        let available = owner
            .direct_available_page(bin)
            .expect("non-empty direct page must be immediately available");
        assert_eq!(available.page().as_ptr(), page_raw);
        assert_eq!(available.page().free_raw(), free.as_ptr());
    }
}

#[test]
fn all_free_and_retire_preserve_page_identity() {
    let flags = PageFlags::new(0).expect("zero page flags are valid");
    let current = ThreadId::new(crate::prim::_mi_prim_thread_id())
        .expect("platform current-thread id is nonzero and unflagged");
    let owner = PageThreadId::from_parts(PageOwnerId::Thread(current), flags).raw();
    // SAFETY: the stack-local page is current-thread owned and initialized with the exact used and
    // countdown states needed to exercise all-free, retired, and countdown-cleared transitions.
    with_page(owner, |page| unsafe {
        ptr::addr_of_mut!((*page.as_ptr()).used).write(0);
        let active = ActivePage::new_unchecked(page);
        let all_free = active.try_mark_all_free().expect("used == 0");
        let retired = all_free.into_retired();
        assert_eq!(retired.page().as_ptr(), page.as_ptr());
        ptr::addr_of_mut!((*page.as_ptr()).retire_expire).write(1);
        ptr::addr_of_mut!((*page.as_ptr()).retire_expire).write(0);
        let active = retired.into_active_after_countdown_cleared();
        assert_eq!(active.page().as_ptr(), page.as_ptr());

        ptr::addr_of_mut!((*page.as_ptr()).used).write(1);
        let active = active
            .try_mark_all_free()
            .expect_err("used page remains active");
        assert_eq!(active.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn abandon_and_reclaim_transitions_preserve_page_identity() {
    with_page(0, |page| unsafe {
        match ActivePage::new_unchecked(page).into_abandon_candidate() {
            PageForAbandon::Active(active) => assert_eq!(active.page().as_ptr(), page.as_ptr()),
            PageForAbandon::Full(_) => panic!("active page became full abandon witness"),
        }
        match FullPage::new_unchecked(page).into_abandon_candidate() {
            PageForAbandon::Full(full) => assert_eq!(full.page().as_ptr(), page.as_ptr()),
            PageForAbandon::Active(_) => panic!("full page became active abandon witness"),
        }

        let owned = OwnedAbandonedPage::new_unchecked(page);
        ptr::addr_of_mut!((*page.as_ptr()).used).write(0);
        assert_eq!(
            owned
                .try_mark_all_free()
                .expect("owned abandoned page with used == 0 is all-free")
                .page()
                .as_ptr(),
            page.as_ptr()
        );
        ptr::addr_of_mut!((*page.as_ptr()).used).write(1);
        let owned = OwnedAbandonedPage::new_unchecked(page);
        let owned = owned
            .try_mark_all_free()
            .expect_err("used abandoned page remains owned");
        let reclaimed = owned.into_reclaimed(heap_owner());
        assert_eq!(reclaimed.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn detach_and_owner_clear_transitions_preserve_one_page_capability() {
    let current = ThreadId::new(crate::prim::_mi_prim_thread_id())
        .expect("platform current-thread id is nonzero and unflagged");
    let local = PageThreadId::from_parts(PageOwnerId::Thread(current), PageFlags::new(0).unwrap());
    let full = PageThreadId::from_parts(PageOwnerId::Thread(current), PageFlags::IN_FULL_QUEUE);
    let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, PageFlags::new(0).unwrap());
    let mut page = MaybeUninit::<mi_page_t>::zeroed();
    let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
    let page_raw = page.as_mut_ptr();
    let theap_raw = theap.as_mut_ptr();
    // SAFETY: these stack fixtures remain live for the complete identity-only transition. The page
    // starts current-thread/full with an owned thread-free word; the test models raw queue removal
    // by clearing links/full state, then models owner clearing while retaining the former heap.
    unsafe {
        ptr::addr_of_mut!((*page_raw).xthread_id).write(full.raw());
        ptr::addr_of_mut!((*page_raw).xthread_free)
            .write(ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned).raw());
        ptr::addr_of_mut!((*page_raw).theap).write(theap_raw);
        let page_ptr = PagePtr::new_unchecked(page_raw);
        page_ptr.set_queue_next(None);
        page_ptr.set_queue_prev(None);

        let full = LocalPage::new_unchecked(page_ptr).into_full_after_classification();
        ptr::addr_of_mut!((*page_raw).xthread_id).write(local.raw());
        let active = full.into_active_after_detach();
        assert_eq!(active.page().as_ptr(), page_raw);

        ptr::addr_of_mut!((*page_raw).xthread_id).write(abandoned.raw());
        let owned = active.into_owned_abandoned_after_owner_clear();
        assert_eq!(owned.page().as_ptr(), page_raw);

        let mut active_page = MaybeUninit::<mi_page_t>::zeroed();
        let active_raw = active_page.as_mut_ptr();
        ptr::addr_of_mut!((*active_raw).xthread_id).write(local.raw());
        let active_ptr = PagePtr::new_unchecked(active_raw);
        let active = LocalPage::new_unchecked(active_ptr).into_active_after_classification();
        assert_eq!(active.page().as_ptr(), active_raw);
    }
}

#[test]
fn local_active_classification_refines_all_free_without_duplicating_identity() {
    let current = ThreadId::new(crate::prim::_mi_prim_thread_id())
        .expect("platform current-thread id is nonzero and unflagged");
    let local = PageThreadId::from_parts(PageOwnerId::Thread(current), PageFlags::new(0).unwrap());
    let full = PageThreadId::from_parts(PageOwnerId::Thread(current), PageFlags::IN_FULL_QUEUE);
    let mut empty_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut used_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut full_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY: all three stack-local fixtures remain live through their consuming identity
    // transitions. The first two owner/state words classify current-thread active pages, with
    // `used` initialized to the exact zero/nonzero value tested. The third starts full and
    // all-free with zeroed queue links; writing its current-thread active owner word models raw
    // detach clearing the full flag, while the zeroed links satisfy the detach precondition.
    unsafe {
        let empty_raw = empty_page.as_mut_ptr();
        ptr::addr_of_mut!((*empty_raw).xthread_id).write(local.raw());
        ptr::addr_of_mut!((*empty_raw).used).write(0);
        let empty = LocalPage::new_unchecked(PagePtr::new_unchecked(empty_raw))
            .into_active_after_classification()
            .try_mark_all_free()
            .expect("zero-used local page refines to all-free");
        assert_eq!(empty.page().as_ptr(), empty_raw);

        let used_raw = used_page.as_mut_ptr();
        ptr::addr_of_mut!((*used_raw).xthread_id).write(local.raw());
        ptr::addr_of_mut!((*used_raw).used).write(1);
        let active = LocalPage::new_unchecked(PagePtr::new_unchecked(used_raw))
            .into_active_after_classification()
            .try_mark_all_free()
            .expect_err("nonzero-used local page retains its active capability");
        assert_eq!(active.page().as_ptr(), used_raw);

        let full_raw = full_page.as_mut_ptr();
        ptr::addr_of_mut!((*full_raw).xthread_id).write(full.raw());
        ptr::addr_of_mut!((*full_raw).used).write(0);
        let full_all_free = LocalPage::new_unchecked(PagePtr::new_unchecked(full_raw))
            .into_full_after_classification()
            .try_mark_all_free()
            .expect("zero-used full page refines to all-free");
        ptr::addr_of_mut!((*full_raw).xthread_id).write(local.raw());
        let active_all_free = full_all_free.into_active_after_detach();
        assert_eq!(active_all_free.page().as_ptr(), full_raw);
    }
}

#[test]
fn abandoned_claim_ownership_uses_thread_free_ownership_bit() {
    with_page(0, |page| unsafe {
        let thread_free = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned);
        ptr::addr_of_mut!((*page.as_ptr()).xthread_free).write(thread_free.raw());
        let abandoned = AbandonedPageType::new_unchecked(page);
        let owned = abandoned
            .try_claim_ownership()
            .expect("unowned abandoned page can be claimed");
        assert_eq!(owned.page().as_ptr(), page.as_ptr());
        let abandoned = AbandonedPageType::new_unchecked(page);
        let abandoned = abandoned
            .try_claim_ownership()
            .expect_err("owned thread-free word returns the abandoned capability");
        assert_eq!(abandoned.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn abandoned_mapped_classification_remains_distinct() {
    let current = ThreadId::new(0x2000).expect("valid thread id");
    let flags = PageFlags::HAS_INTERIOR_POINTERS;
    let mapped = PageThreadId::from_parts(PageOwnerId::AbandonedMapped, flags);

    with_page(mapped.raw(), |page| unsafe {
        match page.snapshot_free_route(current) {
            FreeRouteSnapshot::AbandonedMapped(mapped_page) => {
                assert_eq!(mapped_page.page().as_ptr(), page.as_ptr());
            }
            FreeRouteSnapshot::Abandoned(_) => panic!("abandoned-mapped collapsed to abandoned"),
            FreeRouteSnapshot::LocalFast(_)
            | FreeRouteSnapshot::LocalSlow(_)
            | FreeRouteSnapshot::ForeignPlain(_)
            | FreeRouteSnapshot::ForeignSlow(_) => {
                panic!("abandoned-mapped classified as owned")
            }
        }
    });

    let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, flags);
    with_page(abandoned.raw(), |page| unsafe {
        match page.snapshot_free_route(current) {
            FreeRouteSnapshot::Abandoned(abandoned_page) => {
                assert_eq!(abandoned_page.page().as_ptr(), page.as_ptr());
            }
            FreeRouteSnapshot::AbandonedMapped(_) => {
                panic!("abandoned classified as abandoned-mapped")
            }
            FreeRouteSnapshot::LocalFast(_)
            | FreeRouteSnapshot::LocalSlow(_)
            | FreeRouteSnapshot::ForeignPlain(_)
            | FreeRouteSnapshot::ForeignSlow(_) => {
                panic!("abandoned classified as owned")
            }
        }
    });
}

#[test]
fn free_route_snapshot_couples_owner_and_path() {
    let current = ThreadId::new(0x2000).expect("valid thread id");
    let foreign = ThreadId::new(0x4000).expect("valid foreign thread id");

    let cases = [
        (
            PageThreadId::from_parts(PageOwnerId::Thread(current), PageFlags::new(0).unwrap()),
            0,
        ),
        (
            PageThreadId::from_parts(
                PageOwnerId::Thread(current),
                PageFlags::HAS_INTERIOR_POINTERS,
            ),
            1,
        ),
        (
            PageThreadId::from_parts(PageOwnerId::Thread(foreign), PageFlags::new(0).unwrap()),
            2,
        ),
        (
            PageThreadId::from_parts(
                PageOwnerId::Thread(foreign),
                PageFlags::HAS_INTERIOR_POINTERS,
            ),
            3,
        ),
    ];

    for (thread_id, expected) in cases {
        // SAFETY: each stack-local page contains the complete owner/flag word
        // consumed by the snapshot classifier and remains live for the match.
        with_page(thread_id.raw(), |page| unsafe {
            let actual = match page.snapshot_free_route(current) {
                FreeRouteSnapshot::LocalFast(_) => 0,
                FreeRouteSnapshot::LocalSlow(_) => 1,
                FreeRouteSnapshot::ForeignPlain(_) => 2,
                FreeRouteSnapshot::ForeignSlow(_) => 3,
                FreeRouteSnapshot::Abandoned(_) | FreeRouteSnapshot::AbandonedMapped(_) => 4,
            };
            assert_eq!(actual, expected);
        });
    }
}
