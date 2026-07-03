//! Page flag and owner accessors.

use core::ffi::c_void;
use core::ptr;

use crate::types::{mi_page_flags_t, mi_page_t, mi_theap_t};

use super::{MI_LARGE_MAX_OBJ_SIZE, counts::mi_page_capacity_count};

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_huge(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:page-flag-owner-accessors]: callers pass allocator-owned
    // page metadata; this reads shape and memid fields through typed views.
    unsafe {
        if !crate::internal::mi_page_is_singleton(page) {
            return false;
        }
        if crate::internal::mi_page_block_size(page) > MI_LARGE_MAX_OBJ_SIZE {
            return true;
        }
        match crate::core_types::MemId::view((*page).memid) {
            crate::core_types::MemIdView::Os(os) => os
                .base
                .is_some_and(|base| base.as_ptr() < page.cast_mut().cast::<c_void>()),
            _ => false,
        }
    }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_flags_set(page: *mut mi_page_t, set: bool, newflag: mi_page_flags_t) -> bool {
    // SAFETY[AUDIT:page-flag-owner-accessors]: callers pass mutable
    // allocator-owned page metadata; raw flags are validated before update.
    unsafe {
        let Some(flag) = crate::core_types::PageFlags::new(newflag) else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };
        let page = crate::core_types::PagePtr::new_unchecked(page);
        mi_page_flags_set_typed(page, set, flag)
    }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
unsafe fn mi_page_flags_set_typed(
    page: crate::core_types::PagePtr,
    set: bool,
    flag: crate::core_types::PageFlags,
) -> bool {
    // SAFETY[AUDIT:page-flag-owner-accessors]: `page` is a typed non-null page
    // witness and flag mutation goes through AtomicPageThreadId.
    unsafe {
        let atomic_tid = page.thread_id_cell();
        let old = atomic_tid.set_flags(flag, set);
        old.flags().contains(flag)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_in_full(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:page-flag-owner-accessors]: callers pass allocator-owned
    // page metadata, and raw flag bits are interpreted through PageFlags.
    unsafe {
        crate::core_types::PageFlags::from_low_bits(crate::internal::mi_page_flags(page))
            .contains(crate::core_types::PageFlags::IN_FULL_QUEUE)
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_full(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:page-flag-owner-accessors]: callers pass allocator-owned
    // page metadata and this derives fullness from existing page helpers.
    unsafe {
        !crate::internal::mi_page_immediate_available(page)
            && !crate::internal::mi_page_is_expandable(page)
    }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_set_in_full(page: *mut mi_page_t, in_full: bool) {
    // SAFETY[AUDIT:page-flag-owner-accessors]: callers pass mutable
    // allocator-owned page metadata; the full flag and owner stats are updated
    // consistently.
    unsafe {
        let page_ptr = crate::core_types::PagePtr::new_unchecked(page);
        let was_in_full = mi_page_flags_set_typed(
            page_ptr,
            in_full,
            crate::core_types::PageFlags::IN_FULL_QUEUE,
        );
        if was_in_full != in_full {
            let theap = (*page).theap;
            crate::internal::mi_assert_internal!(!theap.is_null());
            if !theap.is_null() {
                let size = mi_page_capacity_count(page).as_usize()
                    * crate::internal::mi_page_block_size(page);
                if in_full {
                    (*theap).pages_full_size += size;
                } else {
                    crate::internal::mi_assert_internal!(size <= (*theap).pages_full_size);
                    (*theap).pages_full_size -= size;
                }
            }
        }
    }
}

/// # Safety
/// `page` must point to page metadata owned by this allocator.
#[inline]
pub unsafe fn mi_page_is_abandoned(page: *const mi_page_t) -> bool {
    // SAFETY[AUDIT:page-flag-owner-accessors]: callers pass allocator-owned
    // page metadata; the owner word is read through AtomicPageThreadId.
    unsafe {
        let atomic_tid =
            crate::core_types::AtomicPageThreadId(crate::core_types::AtomicUsizeField::from_raw(
                ptr::addr_of_mut!((*(page as *mut mi_page_t)).xthread_id),
            ));
        let tid = atomic_tid.load();
        tid.is_abandoned() || tid.is_abandoned_mapped()
    }
}

/// # Safety
/// `page` must be valid. `theap` must be null or a valid thread heap for this allocator.
#[inline]
pub unsafe fn mi_page_set_theap(page: *mut mi_page_t, theap: *mut mi_theap_t) {
    // SAFETY[AUDIT:page-flag-owner-accessors]: raw page and owner inputs are
    // converted into typed witnesses before the owner transition is published.
    unsafe {
        let page = crate::core_types::PagePtr::new_unchecked(page);
        let owner = if theap.is_null() {
            None
        } else {
            let theap = crate::core_types::ThreadHeapPtr::new_unchecked(theap);
            Some(crate::core_types::ThreadHeapLocal::new_unchecked(theap))
        };
        mi_page_set_theap_typed(page, owner);
    }
}

/// # Safety
/// `page` must be valid. `owner` must be the local owner for `page`, or `None`
/// when transitioning the page to abandoned ownership.
#[inline]
unsafe fn mi_page_set_theap_typed(
    page: crate::core_types::PagePtr,
    owner: Option<crate::core_types::ThreadHeapLocal>,
) {
    // SAFETY[AUDIT:page-flag-owner-accessors]: `page` is a typed non-null page
    // witness and `owner` is either local ownership or abandoned ownership.
    unsafe {
        let theap = owner
            .map(|owner| owner.ptr().as_ptr())
            .unwrap_or(ptr::null_mut());
        (*page.as_ptr()).theap = theap;
        let owner = if let Some(owner) = owner {
            let tid = (*(*owner.ptr().as_ptr()).tld).thread_id;
            let thread_id = crate::core_types::ThreadId::new(tid).unwrap_or_else(|| {
                crate::internal::mi_assert_internal!(false);
                crate::core_types::ThreadId::new_unchecked(tid)
            });
            crate::core_types::PageOwnerId::Thread(thread_id)
        } else {
            crate::core_types::PageOwnerId::Abandoned
        };

        let atomic_tid = page.thread_id_cell();
        let mut xtid_old = atomic_tid.load();
        loop {
            let xtid = xtid_old.with_owner(owner);
            if atomic_tid.compare_exchange_owner(&mut xtid_old, xtid) {
                break;
            }
        }
    }
}

/// # Safety
/// `page` must be valid and must not be abandoned.
#[inline]
pub unsafe fn mi_page_theap(page: *const mi_page_t) -> *mut mi_theap_t {
    // SAFETY[AUDIT:page-flag-owner-accessors]: assertions preserve the
    // non-abandoned page and non-null owner contract before returning the heap.
    unsafe {
        crate::internal::mi_assert_internal!(!mi_page_is_abandoned(page));
        crate::internal::mi_assert_internal!(!(*page).theap.is_null());
        (*page).theap
    }
}
