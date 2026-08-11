//! Subprocess heap-list membership helpers.
//!
//! `mi_subproc_t::heaps` owns an intrusive list of heap metadata linked through
//! `mi_heap_t::next/prev`. Keep the lock-protected list protocol local here.

use core::ptr;
use core::sync::atomic::Ordering;

use crate::atomic::{mi_lock_acquire, mi_lock_release};
use crate::core_types::{HeapPtr, SubprocPtr};
use crate::internal::{__mi_stat_decrease_mt, __mi_stat_increase_mt};

#[inline(always)]
unsafe fn heap_next(heap: HeapPtr) -> Option<HeapPtr> {
    // SAFETY: callers provide heap metadata
    // linked through the subprocess heap-list protocol.
    unsafe { HeapPtr::new((*heap.as_ptr()).next) }
}

#[inline(always)]
unsafe fn set_heap_next(heap: HeapPtr, next: Option<HeapPtr>) {
    // SAFETY: caller owns mutation of the
    // heap's subprocess list links.
    unsafe {
        (*heap.as_ptr()).next = next.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

#[inline(always)]
unsafe fn heap_prev(heap: HeapPtr) -> Option<HeapPtr> {
    // SAFETY: callers provide heap metadata
    // linked through the subprocess heap-list protocol.
    unsafe { HeapPtr::new((*heap.as_ptr()).prev) }
}

#[inline(always)]
unsafe fn set_heap_prev(heap: HeapPtr, prev: Option<HeapPtr>) {
    // SAFETY: caller owns mutation of the
    // heap's subprocess list links.
    unsafe {
        (*heap.as_ptr()).prev = prev.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    }
}

/// # Safety
/// `subproc` must own initialized subprocess metadata, and `heap` must be a
/// freshly initialized heap whose subprocess list links are not yet published.
pub(super) unsafe fn push_front(subproc: SubprocPtr, heap: HeapPtr) {
    // SAFETY: this critical section owns head,
    // next, and prev updates for the subprocess heap list.
    unsafe {
        mi_lock_acquire(subproc.heaps_lock().as_ptr());
        let first = subproc.heaps();
        set_heap_prev(heap, None);
        set_heap_next(heap, first);
        if let Some(first) = first {
            set_heap_prev(first, Some(heap));
        }
        subproc.set_heaps(Some(heap));
        mi_lock_release(subproc.heaps_lock().as_ptr());

        subproc.heap_count_cell().fetch_add(1, Ordering::Relaxed);
        __mi_stat_increase_mt(ptr::addr_of_mut!((*subproc.stats()).heaps), 1);
    }
}

/// # Safety
/// `subproc` must own initialized subprocess metadata, and `heap` must be linked
/// in that subprocess heap list.
pub(super) unsafe fn remove(subproc: SubprocPtr, heap: HeapPtr) {
    // SAFETY: this critical section owns
    // unlinking the heap from the subprocess heap list.
    unsafe {
        subproc.heap_count_cell().fetch_sub(1, Ordering::Relaxed);
        __mi_stat_decrease_mt(ptr::addr_of_mut!((*subproc.stats()).heaps), 1);

        mi_lock_acquire(subproc.heaps_lock().as_ptr());
        let next = heap_next(heap);
        let prev = heap_prev(heap);
        if let Some(next) = next {
            set_heap_prev(next, prev);
        }
        if let Some(prev) = prev {
            set_heap_next(prev, next);
        } else {
            subproc.set_heaps(next);
        }
        mi_lock_release(subproc.heaps_lock().as_ptr());
    }
}
