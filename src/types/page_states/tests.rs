use super::*;
use crate::core_types::{
    BlockPtr, FreeBlock, PageFlags, PageOwnerId, PageThreadId, ThreadFreeOwner, ThreadFreeWord,
    ThreadHeapPtr, ThreadId,
};
use crate::types::{mi_block_t, mi_page_t, mi_theap_t};
use core::{mem::MaybeUninit, ptr};

fn with_page<R>(xthread_id: usize, f: impl FnOnce(PagePtr) -> R) -> R {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    // SAFETY: this initializes only the
    // stack-local page fields needed by page-state typestate tests. The page is
    // never registered with allocator globals and is used only while live here.
    unsafe {
        ptr::addr_of_mut!((*page.as_mut_ptr()).xthread_id).write(xthread_id);
        f(PagePtr::new_unchecked(page.as_mut_ptr()))
    }
}

fn heap_owner() -> ThreadHeapLocal {
    // SAFETY: this is a non-null identity
    // witness for tests that only preserve or compare ownership; no test
    // dereferences the dangling thread-heap pointer.
    unsafe {
        ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(
            core::ptr::NonNull::<mi_theap_t>::dangling().as_ptr(),
        ))
    }
}

#[test]
fn uninit_initialize_yields_active_page() {
    // SAFETY: the page pointer comes from
    // `with_page` and is used only to exercise the uninit-to-active witness
    // transition while the stack-local page is live.
    with_page(0, |page| unsafe {
        let active = Page::<Uninit>::new_unchecked(page).initialize(heap_owner());
        assert_eq!(active.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn active_available_and_pop_free_block_update_page_state() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    // SAFETY: this builds the minimal
    // stack-local page/block metadata needed for one available free block and
    // consumes it before either stack value leaves scope.
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
fn local_push_free_block_updates_local_list_and_used_count() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    // SAFETY: this builds the minimal
    // stack-local page/block metadata needed to push one freed block onto the
    // page-local free list before the locals leave scope.
    unsafe {
        let raw = page.as_mut_ptr();
        ptr::addr_of_mut!((*raw).local_free).write(ptr::null_mut());
        ptr::addr_of_mut!((*raw).used).write(1);
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        ptr::addr_of_mut!((*raw).keys).write([0, 0]);

        let local = LocalPage::new_unchecked(PagePtr::new_unchecked(raw));
        let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
        let used = local.push_free_block(FreeBlock::new_unchecked(block_ptr));

        assert_eq!(used.get(), 0);
        assert_eq!((*raw).used, 0);
        assert_eq!((*raw).local_free, block_ptr.as_ptr());
        assert!(crate::internal::mi_block_next(raw, block_ptr.as_ptr()).is_null());
    }
}

#[test]
fn all_free_retire_and_arena_free_preserve_page_identity() {
    // SAFETY: the stack-local page is
    // initialized with the exact `used` counts needed to exercise all-free,
    // retired, destroyed, and arena-free witness transitions.
    with_page(0, |page| unsafe {
        ptr::addr_of_mut!((*page.as_ptr()).used).write(0);
        let active = ActivePage::new_unchecked(page);
        let all_free = active.try_mark_all_free().expect("used == 0");
        let retired = all_free.into_retired();
        assert_eq!(retired.page().as_ptr(), page.as_ptr());

        let active = ActivePage::new_unchecked(page);
        let destroyed = active
            .try_mark_all_free()
            .expect("used remains zero")
            .free_to_arena(heap_owner());
        assert_eq!(destroyed.page().as_ptr(), page.as_ptr());

        ptr::addr_of_mut!((*page.as_ptr()).used).write(1);
        let active = ActivePage::new_unchecked(page)
            .try_mark_all_free()
            .expect_err("used page remains active");
        assert_eq!(active.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn abandon_and_reclaim_transitions_preserve_page_identity() {
    // SAFETY: the stack-local page is
    // used only as an identity witness while exercising abandon, reclaim, and
    // reabandon typestate transitions.
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
        let owned = OwnedAbandonedPage::new_unchecked(page);
        let mapped = owned.into_mapped_abandoned();
        assert_eq!(mapped.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn abandoned_claim_ownership_uses_thread_free_ownership_bit() {
    // SAFETY: the fixture writes a
    // concrete unowned thread-free word into stack-local page metadata before
    // claiming ownership through the typed abandoned-page wrapper.
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

    // SAFETY: the fixture sets only the
    // owner/flag word needed to classify a stack-local page as abandoned-mapped.
    with_page(mapped.raw(), |page| unsafe {
        match page.classify_for_free(current) {
            PageFreeOwner::AbandonedMapped(mapped_page) => {
                assert_eq!(mapped_page.page().as_ptr(), page.as_ptr());
            }
            PageFreeOwner::Abandoned(_) => panic!("abandoned-mapped collapsed to abandoned"),
            PageFreeOwner::Local(_) | PageFreeOwner::Foreign(_) => {
                panic!("abandoned-mapped classified as owned")
            }
        }
    });

    let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, flags);
    // SAFETY: the fixture sets only the
    // owner/flag word needed to classify a stack-local page as abandoned.
    with_page(abandoned.raw(), |page| unsafe {
        match page.classify_for_free(current) {
            PageFreeOwner::Abandoned(abandoned_page) => {
                assert_eq!(abandoned_page.page().as_ptr(), page.as_ptr());
            }
            PageFreeOwner::AbandonedMapped(_) => {
                panic!("abandoned classified as abandoned-mapped")
            }
            PageFreeOwner::Local(_) | PageFreeOwner::Foreign(_) => {
                panic!("abandoned classified as owned")
            }
        }
    });
}
