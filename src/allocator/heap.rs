//! Heap algorithms

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::Ordering;

use crate::arena::_mi_arena_from_id;
use crate::arena::{_mi_heap_destroy_pages, _mi_heap_move_pages};
use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release};
use crate::core_types::{
    BorrowedHeap, Errno, HeapPtr, MainHeap, OwnedHeap, PagePtr, ThreadHeapPtr,
};
use crate::init::{
    _mi_theap_cached_set, _mi_theap_default_safe, _mi_theap_empty_wrong, mi_heap_main,
};
use crate::internal::{
    __mi_stat_counter_increase_mt, __mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_is_heap_main,
    mi_page_heap,
};
use crate::mimalloc::{mi_free, mi_heap_zalloc};
use crate::options::{_mi_error_message_fmt, _mi_warning_message_fmt};
use crate::os::_mi_os_numa_node_count;
use crate::page_map::_mi_safe_ptr_page;
use crate::prim::{_mi_heap_theap, _mi_prim_thread_yield};
use crate::stats::{_mi_stats_merge_into, mi_stats_header_init};
use crate::theap::{_mi_theap_create, _mi_theap_free};
use crate::threadlocal::{
    _mi_thread_local_create, _mi_thread_local_free, _mi_thread_local_get, _mi_thread_local_set,
};
use crate::types::{mi_arena_id_t, mi_heap_t, mi_page_t, mi_theap_t};

const MI_THEAP_TLS_BUSY: *mut mi_theap_t = core::ptr::dangling_mut::<mi_theap_t>();

/// # Safety
///
/// `heap` must be null or point to a live heap object accepted by the
/// mimalloc heap APIs. Non-null heaps must not be concurrently destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t {
    // SAFETY: upheld by the caller contract for this C ABI boundary.
    unsafe { mi_heap_theap_typed(HeapPtr::new(heap)) }
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// `heap` must be `None` or point to a live heap object accepted by the
/// mimalloc heap APIs. Non-null heaps must not be concurrently destroyed.
#[inline]
pub(crate) unsafe fn mi_heap_theap_typed(heap: Option<HeapPtr>) -> Option<ThreadHeapPtr> {
    let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    ThreadHeapPtr::new(unsafe { _mi_heap_theap(heap) })
}

/// # Safety
///
/// `heap` may be null to select the main heap. Otherwise it must point to a
/// live heap whose NUMA affinity may be mutated by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_set_numa_affinity(heap: *mut mi_heap_t, numa_node: i32) {
    let heap = if heap.is_null() {
        // SAFETY: null heap selects the process main heap by API contract.
        HeapPtr::new(unsafe { mi_heap_main() })
    } else {
        HeapPtr::new(heap)
    };
    // SAFETY: non-null heap pointers are live and mutable by the caller contract.
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
#[inline]
unsafe fn mi_heap_set_numa_affinity_typed(heap: HeapPtr, numa_node: crate::core_types::NumaNode) {
    let numa_count = unsafe { _mi_os_numa_node_count() };
    unsafe { heap.set_numa_node(numa_node.normalize(numa_count)) };
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live and its stats must not be
/// concurrently mutated outside the allocator synchronization rules.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_heap_stats_merge_to_main(heap: *mut mi_heap_t) {
    unsafe {
        let Some(heap) = BorrowedHeap::from_lookup_raw(heap) else {
            return;
        };
        mi_heap_stats_merge_to_main_typed(heap);
    }
}

#[inline]
unsafe fn mi_heap_stats_merge_to_main_typed(heap: BorrowedHeap) {
    unsafe {
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_new_in_arena(exclusive_arena_id: mi_arena_id_t) -> *mut mi_heap_t {
    // SAFETY: heap creation is a C ABI boundary; initialization and raw heap mutations follow
    // mimalloc's allocator synchronization rules.
    unsafe { crate::mimalloc::mi_thread_init() };
    let heap_main = unsafe { mi_heap_main() };
    if heap_main.is_null() {
        return ptr::null_mut();
    }
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
        unsafe { mi_free(heap.cast::<c_void>()) };
        return ptr::null_mut();
    }

    // SAFETY: `heap` is freshly allocated and `heap_main` is the initialized main heap.
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

        let subproc = (*heap).subproc;
        mi_lock_acquire(ptr::addr_of_mut!((*subproc).heaps_lock));
        let heap_head = (*subproc).heaps;
        (*heap).prev = ptr::null_mut();
        (*heap).next = heap_head;
        if !heap_head.is_null() {
            (*heap_head).prev = heap;
        }
        (*subproc).heaps = heap;
        mi_lock_release(ptr::addr_of_mut!((*subproc).heaps_lock));

        (*subproc).heap_count.fetch_add(1, Ordering::Relaxed);
        __mi_stat_increase_mt(&mut (*subproc).stats.heaps, 1);
    }
    heap
}

/// # Safety
///
/// The allocator runtime must be initialized or safe to initialize on this
/// thread. The returned heap, when non-null, must later be released through the
/// heap deletion or destruction APIs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_new() -> *mut mi_heap_t {
    // SAFETY: forwards the heap creation contract with no exclusive arena.
    unsafe { mi_heap_new_in_arena(ptr::null_mut()) }
}

unsafe fn mi_heap_free_theaps_in(heap: HeapPtr) {
    let heap = heap.as_ptr();
    // SAFETY: callers pass a live heap whose thread heaps are being deleted/destroyed.
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
                crate::mi_assert_internal!((*heap).theaps.is_null());
                break;
            }

            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*heap).stats.heaps_delete_wait), 1);
            _mi_prim_thread_yield();
        }
    }
}

unsafe fn mi_heap_free_theaps(heap: OwnedHeap) {
    unsafe { mi_heap_free_theaps_in(heap.ptr()) };
}

unsafe fn mi_heap_free(heap: OwnedHeap) {
    let heap = heap.ptr().as_ptr();
    // SAFETY: callers pass a non-main heap that is no longer reachable by mutators.
    unsafe {
        crate::mi_assert_internal!(!heap.is_null() && !_mi_is_heap_main(heap));

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

        let subproc = (*heap).subproc;
        (*subproc).heap_count.fetch_sub(1, Ordering::Relaxed);
        __mi_stat_decrease_mt(&mut (*subproc).stats.heaps, 1);

        mi_lock_acquire(ptr::addr_of_mut!((*subproc).heaps_lock));
        if !(*heap).next.is_null() {
            (*(*heap).next).prev = (*heap).prev;
        }
        if !(*heap).prev.is_null() {
            (*(*heap).prev).next = (*heap).next;
        } else {
            (*subproc).heaps = (*heap).next;
        }
        mi_lock_release(ptr::addr_of_mut!((*subproc).heaps_lock));

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
/// and must not be used again after this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_delete(heap: *mut mi_heap_t) {
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    // SAFETY: non-null heap is live by the caller contract.
    let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else {
        _mi_warning_message_fmt(format_args!("cannot delete the main heap\n"));
        return;
    };
    // SAFETY: heap deletion owns the non-main heap teardown sequence.
    unsafe { mi_heap_delete_typed(heap) };
}

#[inline]
unsafe fn mi_heap_delete_typed(heap: OwnedHeap) {
    let heap_raw = heap.ptr().as_ptr();
    // SAFETY: heap deletion owns the non-main heap teardown sequence.
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_force_destroy(heap: *mut mi_heap_t) {
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    // SAFETY: force-destroy owns teardown of the live heap pointer.
    unsafe {
        if let Some(heap) = MainHeap::new(heap) {
            mi_heap_force_destroy_main(heap);
        } else if let Some(heap) = OwnedHeap::new(heap) {
            mi_heap_force_destroy_owned(heap);
        }
    }
}

#[inline]
unsafe fn mi_heap_force_destroy_main(heap: MainHeap) {
    let heap = heap.ptr();
    // SAFETY: force-destroy owns teardown of this main heap.
    unsafe {
        mi_heap_free_theaps_in(heap);
        _mi_heap_destroy_pages(heap.as_ptr());
    }
}

#[inline]
unsafe fn mi_heap_force_destroy_owned(heap: OwnedHeap) {
    let heap_ptr = heap.ptr();
    // SAFETY: force-destroy owns teardown of this non-main heap.
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_destroy(heap: *mut mi_heap_t) {
    let Some(heap) = HeapPtr::new(heap) else {
        return;
    };
    // SAFETY: non-null heap is live by the caller contract.
    let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else {
        _mi_warning_message_fmt(format_args!("cannot destroy the main heap\n"));
        return;
    };
    // SAFETY: non-main heap is live and destroy owns teardown.
    unsafe { mi_heap_force_destroy_owned(heap) };
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_of(p: *const c_void) -> *mut mi_heap_t {
    // SAFETY: caller permits allocator page lookup for `p`.
    let page: *mut mi_page_t = unsafe { _mi_safe_ptr_page(p) };
    if page.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: non-null page came from the allocator page map.
    unsafe { mi_page_heap(page) }
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_any_heap_contains(p: *const c_void) -> bool {
    // SAFETY: forwards this API's ownership-inspection contract.
    unsafe { !mi_heap_of(p).is_null() }
}

/// # Safety
///
/// `heap` may be null to select the main heap. Non-null heap and pointer
/// arguments must remain valid for allocator ownership inspection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_contains(heap: *const mi_heap_t, p: *const c_void) -> bool {
    let heap = if heap.is_null() {
        // SAFETY: null heap selects the main heap by API contract.
        unsafe { mi_heap_main() }
    } else {
        heap.cast_mut()
    };
    // SAFETY: forwards this API's ownership-inspection contract.
    core::ptr::eq(heap.cast_const(), unsafe { mi_heap_of(p) })
}

/// # Safety
///
/// `heap` may be null to select the main heap. Non-null heap and pointer
/// arguments must remain valid for allocator ownership inspection.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_heap_check_owned(heap: *mut mi_heap_t, p: *const c_void) -> bool {
    let heap = if heap.is_null() {
        // SAFETY: null heap selects the main heap by API contract.
        unsafe { mi_heap_main() }
    } else {
        heap
    };
    // SAFETY: forwards this API's ownership-inspection contract.
    let Some(heap) = (unsafe { BorrowedHeap::from_lookup_raw(heap) }) else {
        return false;
    };
    // SAFETY: heap and pointer arguments remain valid for ownership inspection.
    unsafe { mi_heap_check_owned_typed(heap, p) }
}

#[inline]
unsafe fn mi_heap_check_owned_typed(heap: BorrowedHeap, p: *const c_void) -> bool {
    // SAFETY: `heap` is a live heap borrowed for this ownership check.
    if let Some(heap) = unsafe { OwnedHeap::new(heap.ptr()) } {
        // SAFETY: non-main heap and pointer remain valid for ownership inspection.
        unsafe { mi_heap_check_owned_owned(heap, p) }
    } else {
        // SAFETY: main heap and pointer remain valid for ownership inspection.
        unsafe { mi_heap_check_owned_borrowed(heap, p) }
    }
}

#[inline]
unsafe fn mi_heap_check_owned_owned(heap: OwnedHeap, p: *const c_void) -> bool {
    // SAFETY: forwards this API's ownership-inspection contract.
    core::ptr::eq(heap.ptr().as_ptr().cast_const(), unsafe { mi_heap_of(p) })
}

#[inline]
unsafe fn mi_heap_check_owned_borrowed(heap: BorrowedHeap, p: *const c_void) -> bool {
    // SAFETY: forwards this API's ownership-inspection contract.
    core::ptr::eq(heap.ptr().as_ptr().cast_const(), unsafe { mi_heap_of(p) })
}

#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_heap_t) = mi_heap_stats_merge_to_main;

#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_heap_t, *const c_void) -> bool = mi_heap_check_owned;

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_check_owned(p: *const c_void) -> bool {
    // SAFETY: forwards this API's ownership-inspection contract.
    unsafe { mi_any_heap_contains(p) }
}

/// # Safety
///
/// `heap` may be null. A non-null `p` must be suitable for mimalloc's safe page
/// lookup, and the owning page and heap must not be concurrently destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_unsafe_heap_page_is_under_utilized(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    perc_threshold: usize,
) -> bool {
    if p.is_null() {
        return false;
    }
    // SAFETY: non-null `p` is suitable for allocator page lookup by API contract.
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

#[inline]
unsafe fn mi_unsafe_heap_page_is_under_utilized_typed(
    heap: Option<BorrowedHeap>,
    page: PagePtr,
    perc_threshold: usize,
) -> bool {
    unsafe {
        let page_raw = page.as_ptr();
        let used = crate::page::mi_page_used_count(page_raw);
        let capacity = crate::page::mi_page_capacity_count(page_raw);
        let reserved = crate::page::mi_page_reserved_count(page_raw);
        if used.get() == capacity.get() || capacity.get() < reserved.get() {
            return false;
        }
        if (*page_raw).prev.is_null() {
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
#[inline(never)]
unsafe extern "C" fn mi_heap_init_theap(heap: *const mi_heap_t) -> *mut mi_theap_t {
    let heap = heap.cast_mut();
    crate::mi_assert_internal!(!heap.is_null());

    // SAFETY: caller passes a live heap pointer with initialized heap-local metadata.
    unsafe {
        if _mi_is_heap_main(heap) {
            let theap = crate::init::_mi_theap_main_safe();
            crate::mi_assert_internal!(
                !theap.is_null() && _mi_is_heap_main(crate::internal::_mi_theap_heap(theap))
            );
            return theap;
        }

        crate::mi_assert_internal!((*heap).theap != 0);
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t {
    // SAFETY: caller passes null or a live heap pointer with initialized thread-local metadata.
    unsafe {
        if heap.is_null() || _mi_is_heap_main(heap) {
            crate::init::_mi_theap_main_safe()
        } else {
            _mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()
        }
    }
}

/// # Safety
///
/// `heap` may be null or point to a live heap. Non-main heaps must have a valid
/// thread-local slot and must not be concurrently destroyed.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_theap_get_or_init(
    heap: *const mi_heap_t,
) -> *mut mi_theap_t {
    // SAFETY: forwards this API's live-heap/thread-local metadata contract.
    let mut theap = unsafe { _mi_heap_theap_get_peek(heap) };
    if theap.is_null() {
        // SAFETY: same contract as above; initializes the heap-local thread heap if needed.
        theap = unsafe { mi_heap_init_theap(heap) };
        if theap.is_null() {
            return ptr::addr_of!(_mi_theap_empty_wrong).cast_mut();
        }
    }
    // SAFETY: `theap` is either an initialized thread heap or the allocator empty sentinel.
    unsafe { _mi_theap_cached_set(theap) };
    theap
}

#[used]
static MI_HEAP_INIT_THEAP_KEEP: unsafe extern "C" fn(*const mi_heap_t) -> *mut mi_theap_t =
    mi_heap_init_theap;

#[used]
static MI_HEAP_THEAP_GET_OR_INIT_KEEP: unsafe extern "C" fn(*const mi_heap_t) -> *mut mi_theap_t =
    _mi_heap_theap_get_or_init;
