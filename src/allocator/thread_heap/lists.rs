//! Thread-heap intrusive list helpers.
//!
//! Each initialized thread heap is linked into both its owning heap's list
//! (`hnext/hprev`) and its TLD's list (`tnext/tprev`). Keep the lock-protected
//! list protocols local to this module.

use core::ptr;

use crate::atomic::{mi_lock_acquire, mi_lock_release};
use crate::core_types::{HeapPtr, ThreadHeapPtr, TldPtr};

#[inline(always)]
unsafe fn heap_next(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    // SAFETY: callers provide thread-heap
    // metadata linked through the heap-local thread-heap list protocol.
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).hnext) }
}

#[inline(always)]
unsafe fn set_heap_next(theap: ThreadHeapPtr, next: Option<ThreadHeapPtr>) {
    // SAFETY: caller owns mutation of the
    // thread heap's heap-local list links.
    unsafe {
        (*theap.as_ptr()).hnext = next.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn heap_prev(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    // SAFETY: callers provide thread-heap
    // metadata linked through the heap-local thread-heap list protocol.
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).hprev) }
}

#[inline(always)]
unsafe fn set_heap_prev(theap: ThreadHeapPtr, prev: Option<ThreadHeapPtr>) {
    // SAFETY: caller owns mutation of the
    // thread heap's heap-local list links.
    unsafe {
        (*theap.as_ptr()).hprev = prev.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn tld_next(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    // SAFETY: callers provide thread-heap
    // metadata linked through the TLD-local thread-heap list protocol.
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).tnext) }
}

#[inline(always)]
unsafe fn set_tld_next(theap: ThreadHeapPtr, next: Option<ThreadHeapPtr>) {
    // SAFETY: caller owns mutation of the
    // thread heap's TLD-local list links.
    unsafe {
        (*theap.as_ptr()).tnext = next.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn tld_prev(theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    // SAFETY: callers provide thread-heap
    // metadata linked through the TLD-local thread-heap list protocol.
    unsafe { ThreadHeapPtr::new((*theap.as_ptr()).tprev) }
}

#[inline(always)]
unsafe fn set_tld_prev(theap: ThreadHeapPtr, prev: Option<ThreadHeapPtr>) {
    // SAFETY: caller owns mutation of the
    // thread heap's TLD-local list links.
    unsafe {
        (*theap.as_ptr()).tprev = prev.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

/// # Safety
/// `tld` must own initialized TLD metadata, and `theap` must be initialized
/// with clear TLD-list links.
pub(super) unsafe fn push_tld_front(tld: TldPtr, theap: ThreadHeapPtr) -> Option<ThreadHeapPtr> {
    // SAFETY: this critical section owns head,
    // next, and prev updates for the TLD-local thread-heap list.
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
    // SAFETY: this critical section owns head,
    // next, and prev updates for the heap-local thread-heap list.
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

/// # Safety
/// `heap` must own initialized heap metadata, and `theap` must be linked in
/// that heap's thread-heap list.
pub(super) unsafe fn remove_from_heap(heap: HeapPtr, theap: ThreadHeapPtr, acquire_lock: bool) {
    // SAFETY: caller selects whether the heap
    // thread-heap list lock is already held; this block owns unlinking.
    unsafe {
        if acquire_lock {
            mi_lock_acquire(heap.thread_heaps_lock());
        }
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
        if acquire_lock {
            mi_lock_release(heap.thread_heaps_lock());
        }
    }
}

/// # Safety
/// `tld` must own initialized TLD metadata, and `theap` must be linked in that
/// TLD's thread-heap list.
pub(super) unsafe fn remove_from_tld(tld: TldPtr, theap: ThreadHeapPtr, acquire_lock: bool) {
    // SAFETY: caller selects whether the TLD
    // thread-heap list lock is already held; this block owns unlinking.
    unsafe {
        if acquire_lock {
            mi_lock_acquire(tld.thread_heaps_lock());
        }
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
        if acquire_lock {
            mi_lock_release(tld.thread_heaps_lock());
        }
    }
}
