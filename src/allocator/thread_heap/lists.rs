//! Thread-heap intrusive list helpers.
//!
//! Each initialized thread heap is linked into both its owning heap's list
//! (`hnext/hprev`) and its TLD's list (`tnext/tprev`). Keep the lock-protected
//! list protocols local to this module.

use core::ptr;

use crate::atomic::{mi_lock_acquire, mi_lock_release};
use crate::core_types::{HeapPtr, MiLockGuard, ThreadHeapPtr, TldPtr};

#[inline(always)]
unsafe fn heap_next(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).hnext) }
}

#[inline(always)]
unsafe fn set_heap_next(theap: ThreadHeapPtr, next: Option<ThreadHeapPtr>) {
    unsafe {
        (*theap.as_ptr()).hnext = next.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn heap_prev(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).hprev) }
}

#[inline(always)]
unsafe fn set_heap_prev(theap: ThreadHeapPtr, prev: Option<ThreadHeapPtr>) {
    unsafe {
        (*theap.as_ptr()).hprev = prev.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn tld_next(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).tnext) }
}

#[inline(always)]
unsafe fn set_tld_next(theap: ThreadHeapPtr, next: Option<ThreadHeapPtr>) {
    unsafe {
        (*theap.as_ptr()).tnext = next.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn tld_prev(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).tprev) }
}

#[inline(always)]
unsafe fn set_tld_prev(theap: ThreadHeapPtr, prev: Option<ThreadHeapPtr>) {
    unsafe {
        (*theap.as_ptr()).tprev = prev.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

/// # Safety
/// `tld` must own initialized TLD metadata, and `theap` must be initialized
/// with clear TLD-list links.
pub(super) unsafe fn push_tld_front(tld: TldPtr, theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    unsafe {
        mi_lock_acquire(tld.thread_heaps_lock());
        let first = tld.thread_heaps();
        set_tld_prev(theap, None);
        set_tld_next(theap, first);
        if let Some(first) = first {
            set_tld_prev(first, Some(theap));
        }
        tld.set_thread_heaps(Some(theap));
        mi_lock_release(tld.thread_heaps_lock());
        first
    }
}

/// # Safety
/// `heap` must own initialized heap metadata, and `theap` must be initialized
/// with clear heap-list links.
pub(super) unsafe fn push_heap_front(heap: HeapPtr, theap: ThreadHeapPtr) {
    unsafe {
        mi_lock_acquire(heap.thread_heaps_lock());
        let first = heap.thread_heaps();
        set_heap_prev(theap, None);
        set_heap_next(theap, first);
        if let Some(first) = first {
            set_heap_prev(first, Some(theap));
        }
        heap.set_thread_heaps(Some(theap));
        mi_lock_release(heap.thread_heaps_lock());
    }
}

/// The heap's thread-heap list, protected by that heap's borrowed lock.
pub(crate) struct LockedHeapList<'a> {
    heap: HeapPtr,
    _lock: MiLockGuard<'a>,
}

/// The TLD's thread-heap list, protected by that TLD's borrowed lock.
pub(crate) struct LockedTldList<'a> {
    tld: TldPtr,
    _lock: MiLockGuard<'a>,
}

pub(crate) enum HeldThreadHeapList<'a, 'lock> {
    Heap(&'a mut LockedHeapList<'lock>),
    Tld(&'a mut LockedTldList<'lock>),
}

/// # Safety
/// The heap metadata and initialized lock must remain live throughout `f`;
/// teardown of that storage must be excluded until the lock is released.
#[inline(always)]
pub(crate) unsafe fn with_heap_list<R>(
    heap: HeapPtr,
    f: impl FnOnce(&mut LockedHeapList<'_>) -> R,
) -> R {
    // SAFETY: the caller keeps this field live for the entire scoped borrow.
    let lock = unsafe { &*heap.thread_heaps_lock() };
    let mut list = LockedHeapList {
        heap,
        _lock: MiLockGuard::acquire(lock),
    };
    f(&mut list)
}

/// # Safety
/// The TLD metadata and initialized lock must remain live throughout `f`;
/// teardown of that storage must be excluded until the lock is released.
#[inline(always)]
pub(crate) unsafe fn with_tld_list<R>(
    tld: TldPtr,
    f: impl FnOnce(&mut LockedTldList<'_>) -> R,
) -> R {
    // SAFETY: the caller keeps this field live for the entire scoped borrow.
    let lock = unsafe { &*tld.thread_heaps_lock() };
    let mut list = LockedTldList {
        tld,
        _lock: MiLockGuard::acquire(lock),
    };
    f(&mut list)
}

impl LockedHeapList<'_> {
    /// # Safety
    /// `theap` must be a live node linked in this heap's thread-heap list.
    unsafe fn remove(&mut self, theap: ThreadHeapPtr) {
        let heap = self.heap;
        unsafe {
            let next = heap_next(theap);
            let prev = heap_prev(theap);
            if let Some(next) = next {
                set_heap_prev(next, prev);
            }
            if let Some(prev) = prev {
                set_heap_next(prev, next);
            } else {
                crate::internal::mi_assert_internal!(heap.thread_heaps() == Some(theap));
                heap.set_thread_heaps(next);
            }
            set_heap_next(theap, None);
            set_heap_prev(theap, None);
        }
    }
}

impl LockedTldList<'_> {
    /// # Safety
    /// `theap` must be a live node linked in this TLD's thread-heap list.
    unsafe fn remove(&mut self, theap: ThreadHeapPtr) {
        let tld = self.tld;
        unsafe {
            let next = tld_next(theap);
            let prev = tld_prev(theap);
            if let Some(next) = next {
                set_tld_prev(next, prev);
            }
            if let Some(prev) = prev {
                set_tld_next(prev, next);
            } else {
                crate::internal::mi_assert_internal!(tld.thread_heaps() == Some(theap));
                tld.set_thread_heaps(next);
            }
            set_tld_next(theap, None);
            set_tld_prev(theap, None);
        }
    }
}

impl HeldThreadHeapList<'_, '_> {
    /// # Safety
    /// `theap` must be a live node in both supplied owners' lists. The held
    /// capability must protect the corresponding owner, and the other owner's
    /// metadata and lock must remain live through this synchronous unlink.
    #[inline(always)]
    pub(super) unsafe fn unlink(self, heap: HeapPtr, tld: TldPtr, theap: ThreadHeapPtr) {
        unsafe {
            match self {
                Self::Heap(list) => {
                    crate::internal::mi_assert_internal!(list.heap == heap);
                    list.remove(theap);
                    with_tld_list(tld, |list| list.remove(theap));
                }
                Self::Tld(list) => {
                    crate::internal::mi_assert_internal!(list.tld == tld);
                    with_heap_list(heap, |list| list.remove(theap));
                    list.remove(theap);
                }
            }
        }
    }
}
