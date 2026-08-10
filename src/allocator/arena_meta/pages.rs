//! Published meta-page stack helpers.

use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use super::mi_meta_page_t;

static MI_META_PAGES: AtomicPtr<mi_meta_page_t> = AtomicPtr::new(ptr::null_mut());

#[inline]
pub(super) fn mi_meta_page_list_head() -> *mut mi_meta_page_t {
    MI_META_PAGES.load(Ordering::Acquire)
}

#[inline]
pub(super) fn mi_meta_page_list_changed_since(head: *mut mi_meta_page_t) -> bool {
    mi_meta_page_list_head() != head
}

/// # Safety
///
/// `mpage` must point to a live meta page from the published stack.
#[inline]
pub(super) unsafe fn mi_meta_page_next(mpage: *mut mi_meta_page_t) -> *mut mi_meta_page_t {
    // SAFETY: callers provide a live meta-page
    // header loaded from the acquire-published meta-page stack.
    unsafe { (*mpage).next }
}

/// # Safety
///
/// `mpage` must be a fully initialized meta page that has not yet been
/// published on the global meta-page stack.
#[inline]
pub(super) unsafe fn mi_meta_page_publish(mpage: *mut mi_meta_page_t) -> *mut mi_meta_page_t {
    // SAFETY: `mpage` is fully initialized before
    // publication; the release CAS publishes its header and next link to
    // acquire-loading traversals.
    unsafe {
        let mut old = mi_meta_page_list_head();
        loop {
            (*mpage).next = old;
            match MI_META_PAGES.compare_exchange_weak(
                old,
                mpage,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return mpage,
                Err(actual) => old = actual,
            }
        }
    }
}
