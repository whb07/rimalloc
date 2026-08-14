//! Heap-owned OS abandoned-page list helpers.
//!
//! Non-arena abandoned pages use `mi_page_t.next/prev` as a per-heap list while
//! `MiHeapPrefix::os_abandoned_pages_lock` is held. Keep that raw list protocol
//! local to this module.

use super::*;

#[inline(always)]
unsafe fn lock(heap: HeapPtr) -> *mut crate::atomic::mi_lock_t {
    unsafe { ptr::addr_of_mut!((*heap.as_ptr().cast::<MiHeapPrefix>()).os_abandoned_pages_lock) }
}

#[inline(always)]
unsafe fn head(heap: HeapPtr) -> Option<PagePtr> {
    unsafe { PagePtr::new((*heap.as_ptr().cast::<MiHeapPrefix>()).os_abandoned_pages) }
}

#[inline(always)]
unsafe fn set_head(heap: HeapPtr, page: Option<PagePtr>) {
    unsafe {
        (*heap.as_ptr().cast::<MiHeapPrefix>()).os_abandoned_pages =
            page.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
pub(super) unsafe fn page_next(page: PagePtr) -> Option<PagePtr> {
    unsafe { PagePtr::new((*page.as_ptr()).next) }
}

#[inline(always)]
unsafe fn set_page_next(page: PagePtr, next: Option<PagePtr>) {
    unsafe {
        (*page.as_ptr()).next = next.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn page_prev(page: PagePtr) -> Option<PagePtr> {
    unsafe { PagePtr::new((*page.as_ptr()).prev) }
}

#[inline(always)]
unsafe fn set_page_prev(page: PagePtr, prev: Option<PagePtr>) {
    unsafe {
        (*page.as_ptr()).prev = prev.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

/// # Safety
/// `heap` must wrap valid heap metadata, and `page` must be an abandoned non-arena
/// page whose list links are currently clear.
pub(super) unsafe fn push_front(heap: HeapPtr, page: PagePtr) {
    unsafe {
        mi_lock_acquire(lock(heap));
        let first = head(heap);
        set_page_prev(page, None);
        set_page_next(page, first);
        if let Some(first) = first {
            set_page_prev(first, Some(page));
        }
        set_head(heap, Some(page));
        mi_lock_release(lock(heap));
    }
}

/// # Safety
/// `heap` must wrap valid heap metadata, and `page` must be linked in that
/// heap's OS abandoned-page list or have already-clear list links.
pub(super) unsafe fn remove(heap: HeapPtr, page: PagePtr) {
    unsafe {
        mi_lock_acquire(lock(heap));
        let prev = page_prev(page);
        let next = page_next(page);
        if let Some(prev) = prev {
            set_page_next(prev, next);
        }
        if let Some(next) = next {
            set_page_prev(next, prev);
        }
        if head(heap) == Some(page) {
            set_head(heap, next);
        }
        set_page_next(page, None);
        set_page_prev(page, None);
        mi_lock_release(lock(heap));
    }
}

/// # Safety
/// `heap` must wrap valid heap metadata. The returned page is a snapshot of
/// the list head after acquiring and releasing the heap abandoned-page lock.
pub(super) unsafe fn first_snapshot(heap: HeapPtr) -> Option<PagePtr> {
    unsafe {
        mi_lock_acquire(lock(heap));
        let page = head(heap);
        mi_lock_release(lock(heap));
        page
    }
}

/// # Safety
/// `heap` must wrap valid heap metadata.
pub(super) unsafe fn is_empty(heap: HeapPtr) -> bool {
    unsafe {
        mi_lock_acquire(lock(heap));
        let empty = head(heap).is_none();
        mi_lock_release(lock(heap));
        empty
    }
}
