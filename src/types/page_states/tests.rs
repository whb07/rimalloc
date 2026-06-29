use super::*;
use crate::core_types::{
    PageFlags, PageOwnerId, PageThreadId, ThreadFreeOwner, ThreadFreeWord, ThreadHeapPtr, ThreadId,
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
        let available = active.available().expect("non-null free head is available");
        let (allocated, active_again) = available.pop_free_block();
        assert_eq!(allocated.block().as_ptr(), ptr::addr_of_mut!(block));
        assert_eq!(active_again.page().as_ptr(), raw);
        assert!((*raw).free.is_null());
        assert_eq!((*raw).used, 1);
        assert!(active_again.available().is_none());
    }
}

#[test]
fn all_free_retire_expire_and_arena_free_preserve_page_identity() {
    with_page(0, |page| unsafe {
        ptr::addr_of_mut!((*page.as_ptr()).used).write(0);
        let active = ActivePage::new_unchecked(page);
        let all_free = active.mark_all_free().expect("used == 0");
        let retired = all_free.retire();
        assert_eq!(retired.page().as_ptr(), page.as_ptr());
        let destroyed = retired.expire();
        assert_eq!(destroyed.page().as_ptr(), page.as_ptr());

        let active = ActivePage::new_unchecked(page);
        let destroyed = active
            .mark_all_free()
            .expect("used remains zero")
            .free_to_arena(heap_owner());
        assert_eq!(destroyed.page().as_ptr(), page.as_ptr());

        ptr::addr_of_mut!((*page.as_ptr()).used).write(1);
        assert!(ActivePage::new_unchecked(page).mark_all_free().is_none());
    });
}

#[test]
fn abandon_and_reclaim_transitions_preserve_page_identity() {
    with_page(0, |page| unsafe {
        match ActivePage::new_unchecked(page).for_abandon() {
            PageForAbandon::Active(active) => assert_eq!(active.page().as_ptr(), page.as_ptr()),
            PageForAbandon::Full(_) => panic!("active page became full abandon witness"),
        }
        match FullPage::new_unchecked(page).for_abandon() {
            PageForAbandon::Full(full) => assert_eq!(full.page().as_ptr(), page.as_ptr()),
            PageForAbandon::Active(_) => panic!("full page became active abandon witness"),
        }

        let owned = OwnedAbandonedPage::new_unchecked(page);
        let reclaimed = owned.reclaim(heap_owner());
        assert_eq!(reclaimed.page().as_ptr(), page.as_ptr());
        let mapped = owned.reabandon_to_mapped();
        assert_eq!(mapped.page().as_ptr(), page.as_ptr());
    });
}

#[test]
fn abandoned_claim_ownership_uses_thread_free_ownership_bit() {
    with_page(0, |page| unsafe {
        let thread_free = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned);
        ptr::addr_of_mut!((*page.as_ptr()).xthread_free).write(thread_free.raw());
        let abandoned = AbandonedPageType::new_unchecked(page);
        let owned = abandoned
            .claim_ownership()
            .expect("unowned abandoned page can be claimed");
        assert_eq!(owned.page().as_ptr(), page.as_ptr());
        assert!(abandoned.claim_ownership().is_none());
    });
}

#[test]
fn abandoned_mapped_classification_remains_distinct() {
    let current = ThreadId::new(0x2000).expect("valid thread id");
    let flags = PageFlags::HAS_INTERIOR_POINTERS;
    let mapped = PageThreadId::from_parts(PageOwnerId::AbandonedMapped, flags);

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
        assert!(page.try_abandoned().is_err());
    });

    let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, flags);
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
        assert!(page.try_abandoned().is_ok());
    });
}
