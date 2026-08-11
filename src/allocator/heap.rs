//! Heap algorithms

mod subproc_heaps;

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::Ordering;

use crate::allocator::arena::_mi_arena_from_id;
use crate::allocator::arena::{_mi_heap_destroy_pages, _mi_heap_move_pages};
use crate::allocator::init::{
    _mi_theap_cached_set, _mi_theap_default_safe, _mi_theap_empty_wrong, mi_heap_main,
};
use crate::allocator::options::{_mi_error_message_fmt, _mi_warning_message_fmt};
use crate::allocator::page_map::_mi_safe_ptr_page;
use crate::allocator::stats::{_mi_stats_merge_into, mi_stats_header_init};
use crate::allocator::thread_heap::{_mi_theap_create, _mi_theap_free};
use crate::allocator::threadlocal::{
    _mi_thread_local_create, _mi_thread_local_free, _mi_thread_local_get, _mi_thread_local_set,
};
use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release};
use crate::core_types::{
    BorrowedHeap, Errno, HeapPtr, MainHeap, OwnedHeap, PagePtr, SubprocPtr, ThreadHeapPtr,
};
use crate::internal::{__mi_stat_counter_increase_mt, _mi_is_heap_main, mi_page_heap};
use crate::mimalloc::{mi_free, mi_heap_zalloc};
use crate::os::_mi_os_numa_node_count;
use crate::prim::{_mi_heap_theap, _mi_prim_thread_yield};
use crate::types::{mi_arena_id_t, mi_heap_t, mi_page_t, mi_theap_t};

const MI_THEAP_TLS_BUSY: *mut mi_theap_t = core::ptr::dangling_mut::<mi_theap_t>();

/// # Safety
///
/// `heap` must be null or point to a live heap object accepted by the
/// mimalloc heap APIs. Non-null heaps must not be concurrently destroyed.
// SAFETY: raw heap-to-thread-heap lookup shims
// validate nullable heap metadata before returning a nullable C pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t {
    // SAFETY: upheld by the caller contract for
    // this C ABI boundary.
    unsafe { mi_heap_theap_typed(HeapPtr::new(heap)) }
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// `heap` must be `None` or point to a live heap object accepted by the
/// mimalloc heap APIs. Non-null heaps must not be concurrently destroyed.
// SAFETY: the typed helper forwards a validated
// optional heap pointer to the platform thread-heap accessor.
#[inline]
pub(crate) unsafe fn mi_heap_theap_typed(heap: Option<HeapPtr>) -> Option<ThreadHeapPtr> {
    let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    // SAFETY: `heap` is null or came from
    // `HeapPtr`; `_mi_heap_theap` preserves nullable lookup semantics.
    ThreadHeapPtr::new(unsafe { _mi_heap_theap(heap) })
}

/// # Safety
///
/// `heap` may be null to select the main heap. Otherwise it must point to a
/// live heap whose NUMA affinity may be mutated by the caller.
// SAFETY: the raw NUMA affinity shim parses the
// nullable heap argument once and routes mutation through `HeapPtr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_set_numa_affinity(heap: *mut mi_heap_t, numa_node: i32) {
    let heap = if heap.is_null() {
        // SAFETY: null heap selects the process
        // main heap by API contract.
        HeapPtr::new(unsafe { mi_heap_main() })
    } else {
        HeapPtr::new(heap)
    };
    // SAFETY: non-null heap pointers are live
    // and mutable by the caller contract.
    unsafe {
        let Some(heap) = heap else {
            return;
        };
        mi_heap_set_numa_affinity_typed(heap, crate::core_types::NumaNode::new(numa_node));
    }
}

/// # Safety
///
/// `heap` must point to live heap metadata whose NUMA affinity may be mutated.
// SAFETY: NUMA affinity mutation stays behind a
// typed heap witness while preserving the OS NUMA count query.
#[inline]
unsafe fn mi_heap_set_numa_affinity_typed(heap: HeapPtr, numa_node: crate::core_types::NumaNode) {
    let numa_count = _mi_os_numa_node_count();
    // SAFETY: `heap` is live and mutable by the
    // caller contract; `normalize` bounds the stored NUMA value.
    unsafe { heap.set_numa_node(numa_node.normalize(numa_count)) };
}

/// # Safety
///
/// `heap` may be null. A non-null heap and its initialized statistics must
/// remain live, and the caller must own exclusive access to its source counters
/// for this destructive merge/reset. The live main heap's destination statistics
/// must remain available through the allocator statistics protocol. Heap/main
/// teardown and concurrent source-stat updates must not race.
// SAFETY: stats merge accepts nullable heap
// metadata and immediately converts it to a borrowed heap witness.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_heap_stats_merge_to_main(heap: *mut mi_heap_t) {
    // SAFETY: the caller keeps the optional heap and its stats live under the
    // allocator stats protocol; the main heap remains live for the same merge.
    unsafe {
        let Some(heap) = BorrowedHeap::from_lookup_raw(heap) else {
            return;
        };
        let Some(main) = HeapPtr::new(mi_heap_main()) else {
            return;
        };
        _mi_stats_merge_into(main.stats(), heap.ptr().stats());
    }
}

/// # Safety
///
/// The allocator runtime must be initialized or safe to initialize on this
/// thread. The returned heap, when non-null, must later be released through the
/// heap deletion or destruction APIs.
// SAFETY: heap creation initializes allocator
// state, allocates raw heap metadata, and registers it with the subproc list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_new_in_arena(exclusive_arena_id: mi_arena_id_t) -> *mut mi_heap_t {
    crate::allocator::init::mi_thread_init();
    // SAFETY: after thread initialization, the
    // process main heap is the allocation owner for heap metadata.
    let heap_main = unsafe { mi_heap_main() };
    if heap_main.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: allocate zeroed storage for one
    // `mi_heap_t` from the initialized main heap.
    let heap =
        unsafe { mi_heap_zalloc(heap_main, core::mem::size_of::<mi_heap_t>()) }.cast::<mi_heap_t>();
    if heap.is_null() {
        return ptr::null_mut();
    }

    let theap_slot = _mi_thread_local_create();
    if theap_slot == 0 {
        _mi_error_message_fmt(
            Errno::EFAULT,
            format_args!("unable to dynamically create a thread local for a heap\n"),
        );
        // SAFETY: the heap metadata was just
        // allocated and has not been registered with the subproc list.
        unsafe { mi_free(heap.cast::<c_void>()) };
        return ptr::null_mut();
    }

    // SAFETY: `heap` is freshly allocated,
    // `heap_main` is initialized, and subproc list insertion is lock-protected.
    unsafe {
        (*heap).theap = theap_slot;
        (*heap).subproc = (*heap_main).subproc;
        (*heap).heap_seq = crate::core_types::HeapSeq::new(
            (*(*heap_main).subproc)
                .heap_total_count
                .fetch_add(1, Ordering::Relaxed),
        )
        .get();
        (*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);
        (*heap).numa_node = crate::core_types::NumaNode::any().get();
        mi_stats_header_init(ptr::addr_of_mut!((*heap).stats));
        mi_lock_init(ptr::addr_of_mut!((*heap).theaps_lock));
        mi_lock_init(ptr::addr_of_mut!((*heap).os_abandoned_pages_lock));
        mi_lock_init(ptr::addr_of_mut!((*heap).arena_pages_lock));

        let subproc = SubprocPtr::new_unchecked((*heap).subproc);
        subproc_heaps::push_front(subproc, HeapPtr::new_unchecked(heap));
    }
    heap
}

/// # Safety
///
/// The allocator runtime must be initialized or safe to initialize on this
/// thread. The returned heap, when non-null, must later be released through the
/// heap deletion or destruction APIs.
// SAFETY: public heap creation forwards to the
// arena-aware constructor with no exclusive arena.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_new() -> *mut mi_heap_t {
    // SAFETY: forwards the heap creation
    // contract with no exclusive arena.
    unsafe { mi_heap_new_in_arena(ptr::null_mut()) }
}

/// Drain and free every thread heap currently attached to `heap`.
///
/// # Safety
/// `heap` must remain live with initialized thread-heap list, lock, statistics,
/// and list nodes. The caller must own the heap delete/destroy epoch: no new
/// thread heap may attach and no competing teardown or list relink may occur.
/// Existing users may finish only through the thread-heap refcount protocol;
/// this function waits until those references are released.
unsafe fn mi_heap_free_theaps_in(heap: HeapPtr) {
    let heap = heap.as_ptr();
    // SAFETY: the exclusive teardown epoch keeps the list, nodes, lock, and
    // counter live. The lock serializes each traversal while refcount failures
    // are retried only after releasing it.
    unsafe {
        loop {
            let mut all_freed = true;
            mi_lock_acquire(ptr::addr_of_mut!((*heap).theaps_lock));
            let mut theap = (*heap).theaps;
            while !theap.is_null() {
                let next = (*theap).hnext;
                if !_mi_theap_free(theap, false, true) {
                    all_freed = false;
                }
                theap = next;
            }
            mi_lock_release(ptr::addr_of_mut!((*heap).theaps_lock));

            if all_freed {
                crate::internal::mi_assert_internal!((*heap).theaps.is_null());
                break;
            }

            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*heap).stats.heaps_delete_wait), 1);
            _mi_prim_thread_yield();
        }
    }
}

/// Drain all thread heaps attached to an owned non-main heap.
///
/// # Safety
/// The owned heap must remain live with initialized thread-heap metadata, and
/// the caller must hold its exclusive delete/destroy epoch. No new thread heap,
/// competing teardown, or non-protocol list mutation may race; existing users
/// may finish through the thread-heap refcount protocol.
unsafe fn mi_heap_free_theaps(heap: OwnedHeap) {
    // SAFETY: the caller supplies the exclusive teardown epoch; `OwnedHeap`
    // additionally excludes the main-heap lifetime policy.
    unsafe { mi_heap_free_theaps_in(heap.ptr()) };
}

/// Unregister and release an already-drained owned heap.
///
/// # Safety
/// `heap` must be live, non-main, and exclusively owned for final teardown. Its
/// thread heaps must already be drained and its pages moved or destroyed. The
/// arena-page table/lock, statistics, TLS slot, locks, subprocess association,
/// and subprocess heap-list links must remain initialized and valid. The main
/// heap and subprocess must remain live, with no concurrent heap relink,
/// teardown, arena-table access, or other metadata use.
unsafe fn mi_heap_free(heap: OwnedHeap) {
    let heap = heap.ptr().as_ptr();
    // SAFETY: the caller's final-teardown epoch keeps every projected field,
    // lock, registry link, and allocator target live until their ordered release;
    // metadata is freed only after all dependent state is detached or destroyed.
    unsafe {
        crate::internal::mi_assert_internal!(!heap.is_null() && !_mi_is_heap_main(heap));

        mi_lock_acquire(ptr::addr_of_mut!((*heap).arena_pages_lock));
        for arena_pages in &mut (*heap).arena_pages {
            let pages = arena_pages.swap(ptr::null_mut(), Ordering::Relaxed);
            if !pages.is_null() {
                mi_free(pages);
            }
        }
        mi_lock_release(ptr::addr_of_mut!((*heap).arena_pages_lock));

        _mi_stats_merge_into(
            ptr::addr_of_mut!((*mi_heap_main()).stats),
            ptr::addr_of_mut!((*heap).stats),
        );

        let subproc = SubprocPtr::new_unchecked((*heap).subproc);
        subproc_heaps::remove(subproc, HeapPtr::new_unchecked(heap));

        _mi_thread_local_free((*heap).theap);
        mi_lock_done(ptr::addr_of_mut!((*heap).theaps_lock));
        mi_lock_done(ptr::addr_of_mut!((*heap).os_abandoned_pages_lock));
        mi_lock_done(ptr::addr_of_mut!((*heap).arena_pages_lock));
        mi_free(heap.cast::<c_void>());
    }
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live, must not be the main heap,
/// and the caller must own its exclusive delete epoch from call entry: no
/// concurrent allocation, thread-heap attachment, page/queue/list relink,
/// teardown, or other heap use may occur. Its metadata, locks, TLS slot,
/// subprocess links, pages, and backing must remain initialized until released.
/// The allocator runtime, subprocess, and main heap must remain live throughout.
// SAFETY: heap deletion rejects main heaps,
// moves pages to main heap, and then frees owned heap metadata.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_delete(heap: *mut mi_heap_t) {
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    // SAFETY: non-null heap is live by the
    // caller contract and `OwnedHeap` rejects the process main heap.
    let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else {
        _mi_warning_message_fmt(format_args!("cannot delete the main heap\n"));
        return;
    };
    let heap_raw = heap.ptr().as_ptr();
    // SAFETY: the caller grants exclusive delete ownership of this live
    // non-main heap. Thread heaps are drained before pages move to the live main
    // heap, and metadata is freed only after both transitions complete.
    unsafe {
        mi_heap_free_theaps(heap);
        _mi_heap_move_pages(heap_raw, mi_heap_main());
        mi_heap_free(heap);
    }
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live and must not be accessed
/// after destruction starts, except through allocator teardown internals.
// SAFETY: force destroy classifies live heap
// metadata into main or owned teardown witnesses before destroying pages.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_force_destroy(heap: *mut mi_heap_t) {
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    // SAFETY: force-destroy owns teardown of
    // the live heap pointer.
    unsafe {
        if let Some(heap) = MainHeap::new(heap) {
            mi_heap_force_destroy_main(heap);
        } else if let Some(heap) = OwnedHeap::new(heap) {
            mi_heap_force_destroy_owned(heap);
        }
    }
}

/// Force-destroy all allocations attached to the main heap without freeing its
/// static heap metadata.
///
/// # Safety
/// The main heap and all attached thread heaps, pages, queues, locks, and page
/// backing must remain live and initialized. The caller must own an exclusive
/// process/subprocess teardown epoch: no new attachment, allocation, queue/list
/// relink, or competing destroy may occur. Protocol remote-free publication may
/// complete only as permitted by the page-destroy traversal contract.
#[inline]
unsafe fn mi_heap_force_destroy_main(heap: MainHeap) {
    let heap = heap.ptr();
    // SAFETY: the exclusive process teardown first drains thread heaps, then
    // destroys pages while the permanent main-heap metadata remains live.
    unsafe {
        mi_heap_free_theaps_in(heap);
        _mi_heap_destroy_pages(heap.as_ptr());
    }
}

/// Force-destroy allocations and then release an owned non-main heap.
///
/// # Safety
/// The owned heap and all attached thread heaps, pages, queues, locks, backing,
/// TLS state, and subprocess links must remain live and initialized. The caller
/// must own its exclusive destroy epoch: no new attachment, allocation,
/// queue/list relink, or competing teardown may occur. Protocol remote-free
/// publication may complete only as permitted by page destruction.
#[inline]
unsafe fn mi_heap_force_destroy_owned(heap: OwnedHeap) {
    let heap_ptr = heap.ptr();
    // SAFETY: the exclusive epoch orders thread-heap drain before page
    // destruction and delays metadata release until both are complete.
    unsafe {
        mi_heap_free_theaps(heap);
        _mi_heap_destroy_pages(heap_ptr.as_ptr());
        mi_heap_free(heap);
    }
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live, must not be the main heap,
/// and must not be used again after this call returns.
// SAFETY: heap destruction rejects main heaps
// before entering the owned force-destroy path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_destroy(heap: *mut mi_heap_t) {
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    // SAFETY: non-null heap is live by the
    // caller contract and `OwnedHeap` rejects the process main heap.
    let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else {
        _mi_warning_message_fmt(format_args!("cannot destroy the main heap\n"));
        return;
    };
    // SAFETY: non-main heap is live and destroy
    // owns teardown.
    unsafe { mi_heap_force_destroy_owned(heap) };
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
// SAFETY: ownership lookup translates an
// allocation pointer to its allocator page and heap without taking ownership.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_of(p: *const c_void) -> *mut mi_heap_t {
    // SAFETY: caller permits allocator page
    // lookup for `p`.
    let page: *mut mi_page_t = unsafe { _mi_safe_ptr_page(p) };
    if page.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: non-null page came from the
    // allocator page map.
    unsafe { mi_page_heap(page) }
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
// SAFETY: heap containment is a nullable
// ownership lookup and does not retain pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_any_heap_contains(p: *const c_void) -> bool {
    // SAFETY: forwards this API's
    // ownership-inspection contract.
    unsafe { !mi_heap_of(p).is_null() }
}

/// # Safety
///
/// `heap` may be null to select the main heap. Non-null heap and pointer
/// arguments must remain valid for allocator ownership inspection.
// SAFETY: heap-specific containment selects the
// main heap for null and compares against allocator page ownership.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_contains(heap: *const mi_heap_t, p: *const c_void) -> bool {
    let heap = if heap.is_null() {
        // SAFETY: null heap selects the main
        // heap by API contract.
        unsafe { mi_heap_main() }
    } else {
        heap.cast_mut()
    };
    // SAFETY: forwards this API's
    // ownership-inspection contract.
    core::ptr::eq(heap.cast_const(), unsafe { mi_heap_of(p) })
}

/// # Safety
///
/// `heap` may be null to select the main heap. Non-null heap and pointer
/// arguments must remain valid for allocator ownership inspection.
// SAFETY: checked ownership parses heap metadata
// into a borrowed witness before comparing allocator page ownership.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_heap_check_owned(heap: *mut mi_heap_t, p: *const c_void) -> bool {
    let heap = if heap.is_null() {
        // SAFETY: null heap selects the main
        // heap by API contract.
        unsafe { mi_heap_main() }
    } else {
        heap
    };
    // SAFETY: forwards this API's
    // ownership-inspection contract.
    let Some(heap) = (unsafe { BorrowedHeap::from_lookup_raw(heap) }) else {
        return false;
    };
    // SAFETY: the public contract keeps `p` suitable for allocator page-map
    // lookup and stabilizes the resolved page ownership and borrowed heap.
    core::ptr::eq(heap.ptr().as_ptr().cast_const(), unsafe { mi_heap_of(p) })
}

#[cfg(feature = "c-abi")]
// SAFETY: const ABI witness preserves the raw C
// signature for feature-gated stats merge exports.
const _: unsafe extern "C" fn(*mut mi_heap_t) = mi_heap_stats_merge_to_main;

#[cfg(feature = "c-abi")]
// SAFETY: const ABI witness preserves the raw C
// signature for feature-gated heap ownership exports.
const _: unsafe extern "C" fn(*mut mi_heap_t, *const c_void) -> bool = mi_heap_check_owned;

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
// SAFETY: global ownership check is a raw C ABI
// shim over the same allocator page lookup as `mi_any_heap_contains`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_check_owned(p: *const c_void) -> bool {
    // SAFETY: forwards this API's
    // ownership-inspection contract.
    unsafe { mi_any_heap_contains(p) }
}

/// # Safety
///
/// `heap` may be null. A non-null `p` must be suitable for mimalloc's safe page
/// lookup. The resolved page must remain live and initialized with stable
/// used/capacity/reserved counts, queue links, and heap association. A non-null
/// `heap` must remain live and initialized. Page/heap teardown, reassignment,
/// queue relink, and non-protocol metadata mutation must not race; protocol
/// remote-free publication is permitted while these inspected fields stay stable.
// SAFETY: under-utilized inspection converts
// raw heap/page metadata to typed witnesses before reading page counters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_unsafe_heap_page_is_under_utilized(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    perc_threshold: usize,
) -> bool {
    if p.is_null() {
        return false;
    }
    // SAFETY: non-null `p` is suitable for
    // allocator page lookup by API contract.
    unsafe {
        let heap = if heap.is_null() {
            None
        } else {
            BorrowedHeap::from_lookup_raw(heap)
        };
        let page = _mi_safe_ptr_page(p);
        let Some(page) = PagePtr::new(page) else {
            return false;
        };
        mi_unsafe_heap_page_is_under_utilized_typed(heap, page, perc_threshold)
    }
}

/// Inspect utilization and optional heap ownership for a mapped page.
///
/// # Safety
/// `page` must remain live and initialized with stable used/capacity/reserved
/// counts, queue links, and heap association satisfying allocator invariants.
/// If present, `heap` must remain live and initialized. Page/heap teardown,
/// reassignment, queue relink, and non-protocol metadata mutation must not race;
/// protocol remote-free publication is permitted while these fields remain
/// stable. Neither witness is retained.
#[inline]
unsafe fn mi_unsafe_heap_page_is_under_utilized_typed(
    heap: Option<BorrowedHeap>,
    page: PagePtr,
    perc_threshold: usize,
) -> bool {
    // SAFETY: the caller stabilizes the initialized page counters, queue links,
    // and heap association, plus the optional borrowed heap, for this one
    // synchronous classification.
    unsafe {
        let page_raw = page.as_ptr();
        let used = crate::allocator::page::mi_page_used_count(page_raw);
        let capacity = crate::allocator::page::mi_page_capacity_count(page_raw);
        let reserved = crate::allocator::page::mi_page_reserved_count(page_raw);
        if used.get() == capacity.get() || capacity.get() < reserved.get() {
            return false;
        }
        if page.queue_prev().is_none() {
            return false;
        }

        let Some(page_heap) = HeapPtr::new(mi_page_heap(page_raw)) else {
            return false;
        };
        if let Some(heap) = heap
            && !core::ptr::eq(page_heap.as_ptr(), heap.ptr().as_ptr())
        {
            return false;
        }

        if perc_threshold >= 100 {
            return true;
        }
        perc_threshold >= ((100usize * used.as_usize()) / capacity.as_usize())
    }
}

/// # Safety
///
/// `heap` must be a live heap pointer with initialized thread-local metadata.
// SAFETY: lazy thread-heap initialization uses
// the heap's reserved TLS key and publishes a per-thread theap for that heap.
#[inline(never)]
unsafe extern "C" fn mi_heap_init_theap(heap: *const mi_heap_t) -> *mut mi_theap_t {
    let heap = heap.cast_mut();
    crate::internal::mi_assert_internal!(!heap.is_null());

    // SAFETY: caller passes a live heap pointer
    // with initialized heap-local metadata.
    unsafe {
        if _mi_is_heap_main(heap) {
            let theap = crate::allocator::init::_mi_theap_main_safe();
            crate::internal::mi_assert_internal!(
                !theap.is_null() && _mi_is_heap_main(crate::internal::_mi_theap_heap(theap))
            );
            return theap;
        }

        crate::internal::mi_assert_internal!((*heap).theap != 0);
        if (*heap).theap == 0 {
            _mi_error_message_fmt(
                Errno::EFAULT,
                format_args!("no thread-local reserved for heap ({:p})\n", heap),
            );
            return ptr::null_mut();
        }

        let mut theap = _mi_thread_local_get((*heap).theap).cast::<mi_theap_t>();
        if theap.is_null() {
            if !_mi_thread_local_set((*heap).theap, MI_THEAP_TLS_BUSY.cast::<c_void>()) {
                _mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!("unable to allocate memory for thread local storage\n"),
                );
                return ptr::null_mut();
            }
            theap = _mi_theap_create(heap, (*_mi_theap_default_safe()).tld);
            _mi_thread_local_set((*heap).theap, theap.cast::<c_void>());
            if theap.is_null() {
                _mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!("unable to allocate memory for a thread local heap\n"),
                );
                return ptr::null_mut();
            }
        }
        theap
    }
}

/// # Safety
///
/// `heap` may be null or point to a live heap. Non-main heaps must have a valid
/// thread-local slot and must not be concurrently destroyed.
// SAFETY: peek returns the current thread's
// cached heap-local theap or the main theap without allocating.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t {
    // SAFETY: caller passes null or a live heap
    // pointer with initialized thread-local metadata.
    unsafe {
        if heap.is_null() || _mi_is_heap_main(heap) {
            crate::allocator::init::_mi_theap_main_safe()
        } else {
            _mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()
        }
    }
}

/// # Safety
///
/// `heap` may be null or point to a live heap. Non-main heaps must have a valid
/// thread-local slot and must not be concurrently destroyed.
// SAFETY: get-or-init preserves the no-allocation
// fast peek path and initializes only on a missing heap-local theap.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_theap_get_or_init(
    heap: *const mi_heap_t,
) -> *mut mi_theap_t {
    // SAFETY: forwards this API's
    // live-heap/thread-local metadata contract.
    let mut theap = unsafe { _mi_heap_theap_get_peek(heap) };
    if theap.is_null() {
        // SAFETY: same contract as above;
        // initializes the heap-local thread heap if needed.
        theap = unsafe { mi_heap_init_theap(heap) };
        if theap.is_null() {
            return ptr::addr_of!(_mi_theap_empty_wrong).cast_mut();
        }
    }
    // SAFETY: `theap` is either an initialized
    // thread heap or the allocator empty sentinel.
    unsafe { _mi_theap_cached_set(theap) };
    theap
}
