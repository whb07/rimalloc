//! Heap algorithms

#![allow(unsafe_op_in_unsafe_fn)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::Ordering;

use crate::arena::_mi_arena_from_id;
use crate::arena::{_mi_heap_destroy_pages, _mi_heap_move_pages};
use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release};
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
use crate::types::{EFAULT, mi_arena_id_t, mi_heap_t, mi_page_t, mi_theap_t};

const MI_THEAP_TLS_BUSY: *mut mi_theap_t = core::ptr::dangling_mut::<mi_theap_t>();

/// # Safety
///
/// `heap` must be null or point to a live heap object accepted by the
/// mimalloc heap APIs. Non-null heaps must not be concurrently destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t {
    _mi_heap_theap(heap)
}

/// # Safety
///
/// `heap` may be null to select the main heap. Otherwise it must point to a
/// live heap whose NUMA affinity may be mutated by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_set_numa_affinity(heap: *mut mi_heap_t, numa_node: i32) {
    let heap = if heap.is_null() { mi_heap_main() } else { heap };
    let numa_count = _mi_os_numa_node_count();
    (*heap).numa_node = if numa_node < 0 || numa_count <= 0 {
        -1
    } else {
        numa_node % numa_count
    };
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live and its stats must not be
/// concurrently mutated outside the allocator synchronization rules.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_stats_merge_to_main(heap: *mut mi_heap_t) {
    if heap.is_null() {
        return;
    }
    _mi_stats_merge_into(
        ptr::addr_of_mut!((*mi_heap_main()).stats),
        ptr::addr_of_mut!((*heap).stats),
    );
}

/// # Safety
///
/// The allocator runtime must be initialized or safe to initialize on this
/// thread. The returned heap, when non-null, must later be released through the
/// heap deletion or destruction APIs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_new_in_arena(exclusive_arena_id: mi_arena_id_t) -> *mut mi_heap_t {
    crate::mimalloc::mi_thread_init();
    let heap_main = mi_heap_main();
    if heap_main.is_null() {
        return ptr::null_mut();
    }
    let heap = mi_heap_zalloc(heap_main, core::mem::size_of::<mi_heap_t>()).cast::<mi_heap_t>();
    if heap.is_null() {
        return ptr::null_mut();
    }

    let theap_slot = _mi_thread_local_create();
    if theap_slot == 0 {
        _mi_error_message_fmt(
            EFAULT,
            format_args!("unable to dynamically create a thread local for a heap\n"),
        );
        mi_free(heap.cast::<c_void>());
        return ptr::null_mut();
    }

    (*heap).theap = theap_slot;
    (*heap).subproc = (*heap_main).subproc;
    (*heap).heap_seq = (*(*heap_main).subproc)
        .heap_total_count
        .fetch_add(1, Ordering::Relaxed);
    (*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);
    (*heap).numa_node = -1;
    mi_stats_header_init(ptr::addr_of_mut!((*heap).stats));
    mi_lock_init(ptr::addr_of_mut!((*heap).theaps_lock));
    mi_lock_init(ptr::addr_of_mut!((*heap).os_abandoned_pages_lock));
    mi_lock_init(ptr::addr_of_mut!((*heap).arena_pages_lock));

    let subproc = (*heap).subproc;
    mi_lock_acquire(ptr::addr_of_mut!((*subproc).heaps_lock));
    let head = (*subproc).heaps;
    (*heap).prev = ptr::null_mut();
    (*heap).next = head;
    if !head.is_null() {
        (*head).prev = heap;
    }
    (*subproc).heaps = heap;
    mi_lock_release(ptr::addr_of_mut!((*subproc).heaps_lock));

    (*subproc).heap_count.fetch_add(1, Ordering::Relaxed);
    __mi_stat_increase_mt(&mut (*subproc).stats.heaps, 1);
    heap
}

/// # Safety
///
/// The allocator runtime must be initialized or safe to initialize on this
/// thread. The returned heap, when non-null, must later be released through the
/// heap deletion or destruction APIs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_new() -> *mut mi_heap_t {
    mi_heap_new_in_arena(ptr::null_mut())
}

unsafe fn mi_heap_free_theaps(heap: *mut mi_heap_t) {
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

unsafe fn mi_heap_free(heap: *mut mi_heap_t) {
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

/// # Safety
///
/// `heap` may be null. A non-null heap must be live, must not be the main heap,
/// and must not be used again after this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_delete(heap: *mut mi_heap_t) {
    if heap.is_null() {
        return;
    }
    if _mi_is_heap_main(heap) {
        _mi_warning_message_fmt(format_args!("cannot delete the main heap\n"));
        return;
    }
    mi_heap_free_theaps(heap);
    _mi_heap_move_pages(heap, mi_heap_main());
    mi_heap_free(heap);
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live and must not be accessed
/// after destruction starts, except through allocator teardown internals.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_force_destroy(heap: *mut mi_heap_t) {
    if heap.is_null() {
        return;
    }
    mi_heap_free_theaps(heap);
    _mi_heap_destroy_pages(heap);
    if !_mi_is_heap_main(heap) {
        mi_heap_free(heap);
    }
}

/// # Safety
///
/// `heap` may be null. A non-null heap must be live, must not be the main heap,
/// and must not be used again after this call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_destroy(heap: *mut mi_heap_t) {
    if heap.is_null() {
        return;
    }
    if _mi_is_heap_main(heap) {
        _mi_warning_message_fmt(format_args!("cannot destroy the main heap\n"));
        return;
    }
    _mi_heap_force_destroy(heap);
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_of(p: *const c_void) -> *mut mi_heap_t {
    let page: *mut mi_page_t = _mi_safe_ptr_page(p);
    if page.is_null() {
        return ptr::null_mut();
    }
    mi_page_heap(page)
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_any_heap_contains(p: *const c_void) -> bool {
    !mi_heap_of(p).is_null()
}

/// # Safety
///
/// `heap` may be null to select the main heap. Non-null heap and pointer
/// arguments must remain valid for allocator ownership inspection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_contains(heap: *const mi_heap_t, p: *const c_void) -> bool {
    let heap = if heap.is_null() { mi_heap_main() } else { heap };
    core::ptr::eq(heap, mi_heap_of(p))
}

/// # Safety
///
/// `heap` may be null to select the main heap. Non-null heap and pointer
/// arguments must remain valid for allocator ownership inspection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_check_owned(heap: *mut mi_heap_t, p: *const c_void) -> bool {
    mi_heap_contains(heap.cast_const(), p)
}

/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc's safe
/// page lookup, such as an allocation pointer or supported interior pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_check_owned(p: *const c_void) -> bool {
    mi_any_heap_contains(p)
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
    let page = _mi_safe_ptr_page(p);
    if page.is_null() || (*page).used == (*page).capacity || (*page).capacity < (*page).reserved {
        return false;
    }
    if (*page).prev.is_null() {
        return false;
    }

    let page_heap = mi_page_heap(page);
    if page_heap.is_null() {
        return false;
    }
    if !heap.is_null() && !core::ptr::eq(page_heap, heap) {
        return false;
    }

    if (*page).capacity == 0 {
        return false;
    }
    if perc_threshold >= 100 {
        return true;
    }
    perc_threshold >= ((100usize * (*page).used as usize) / (*page).capacity as usize)
}

/// # Safety
///
/// `heap` must be a live heap pointer with initialized thread-local metadata.
#[allow(dead_code)]
#[inline(never)]
unsafe extern "C" fn mi_heap_init_theap(heap: *const mi_heap_t) -> *mut mi_theap_t {
    let heap = heap.cast_mut();
    crate::mi_assert_internal!(!heap.is_null());

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
            EFAULT,
            format_args!("no thread-local reserved for heap ({:p})\n", heap),
        );
        return ptr::null_mut();
    }

    let mut theap = _mi_thread_local_get((*heap).theap).cast::<mi_theap_t>();
    if theap.is_null() {
        if !_mi_thread_local_set((*heap).theap, MI_THEAP_TLS_BUSY.cast::<c_void>()) {
            _mi_error_message_fmt(
                EFAULT,
                format_args!("unable to allocate memory for thread local storage\n"),
            );
            return ptr::null_mut();
        }
        theap = _mi_theap_create(heap, (*_mi_theap_default_safe()).tld);
        _mi_thread_local_set((*heap).theap, theap.cast::<c_void>());
        if theap.is_null() {
            _mi_error_message_fmt(
                EFAULT,
                format_args!("unable to allocate memory for a thread local heap\n"),
            );
            return ptr::null_mut();
        }
    }
    theap
}

/// # Safety
///
/// `heap` may be null or point to a live heap. Non-main heaps must have a valid
/// thread-local slot and must not be concurrently destroyed.
#[allow(dead_code)]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t {
    if heap.is_null() || _mi_is_heap_main(heap) {
        crate::init::_mi_theap_main_safe()
    } else {
        _mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()
    }
}

/// # Safety
///
/// `heap` may be null or point to a live heap. Non-main heaps must have a valid
/// thread-local slot and must not be concurrently destroyed.
#[allow(dead_code)]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_theap_get_or_init(
    heap: *const mi_heap_t,
) -> *mut mi_theap_t {
    let mut theap = _mi_heap_theap_get_peek(heap);
    if theap.is_null() {
        theap = mi_heap_init_theap(heap);
        if theap.is_null() {
            return ptr::addr_of!(_mi_theap_empty_wrong).cast_mut();
        }
    }
    _mi_theap_cached_set(theap);
    theap
}

#[used]
static MI_HEAP_INIT_THEAP_KEEP: unsafe extern "C" fn(*const mi_heap_t) -> *mut mi_theap_t =
    mi_heap_init_theap;

#[used]
static MI_HEAP_THEAP_GET_OR_INIT_KEEP: unsafe extern "C" fn(*const mi_heap_t) -> *mut mi_theap_t =
    _mi_heap_theap_get_or_init;
