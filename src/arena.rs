//! Arena management

use core::ffi::{c_int, c_long, c_void};
use core::mem::{offset_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use crate::atomic::{
    EFAULT, mi_atomic_cas_ptr_strong_release, mi_atomic_cas_strong_acq_rel,
    mi_atomic_cas_weak_acq_rel, mi_atomic_casi64_strong_acq_rel, mi_atomic_guard_t,
    mi_atomic_load_explicit, mi_atomic_load_ptr_acquire, mi_atomic_loadi64_acquire,
    mi_atomic_loadi64_explicit, mi_atomic_store_ptr_release, mi_atomic_storei64_release,
    mi_lock_acquire, mi_lock_release, mi_memory_order_e,
};
use crate::bitmap::{
    _mi_bitmap_forall_set, _mi_bitmap_forall_setc_rangesn, MI_BCHUNK_BITS, MI_BCHUNK_FIELDS,
    MI_BCHUNK_SIZE, MI_BFIELD_BITS, MI_BITMAP_MAX_BIT_COUNT, mi_bbitmap_bsr_inv,
    mi_bbitmap_chunk_count, mi_bbitmap_debug_get_bin, mi_bbitmap_init, mi_bbitmap_is_clearN,
    mi_bbitmap_is_setN, mi_bbitmap_setN, mi_bbitmap_size, mi_bbitmap_t, mi_bbitmap_try_clearNC,
    mi_bbitmap_try_find_and_clearN, mi_bbitmap_unsafe_setN, mi_bchunk_t, mi_bchunkmap_t,
    mi_bfield_t, mi_bitmap_clear, mi_bitmap_clear_once_set, mi_bitmap_clearN, mi_bitmap_init,
    mi_bitmap_is_all_clear, mi_bitmap_is_clearN, mi_bitmap_is_set, mi_bitmap_is_setN,
    mi_bitmap_popcountN, mi_bitmap_set, mi_bitmap_setN, mi_bitmap_size, mi_bitmap_t,
    mi_bitmap_try_find_and_claim, mi_bitmap_unsafe_setN,
};
use crate::bits::{MI_INTPTR_BITS, MI_MiB};
use crate::internal::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt,
    __mi_stat_counter_increase, __mi_stat_counter_increase_mt, __mi_stat_decrease,
    __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt, _mi_align_up, _mi_clamp,
    _mi_divide_up, _mi_is_heap_main, _mi_memid_none, _mi_meta_free, _mi_meta_is_meta_page,
    _mi_os_commit, _mi_os_commit_ex, _mi_os_free, _mi_os_free_ex, _mi_os_has_overcommit,
    _mi_os_has_virtual_reserve, _mi_os_reuse, _mi_preloading, _mi_subproc, _mi_theap_heap,
    _mi_thread_id, _mi_toupper, MI_DEBUG, mi_mem_is_zero, mi_memid_t, mi_memkind_t,
    mi_mul_overflow, mi_page_all_free, mi_page_block_size, mi_page_claim_ownership,
    mi_page_clear_abandoned_mapped, mi_page_committed, mi_page_heap, mi_page_is_abandoned,
    mi_page_is_abandoned_mapped, mi_page_is_full, mi_page_is_owned, mi_page_is_singleton,
    mi_page_meta_is_separated, mi_page_slice_start, mi_page_start, mi_size_of_slices,
    mi_slice_count_of_size, mi_tf_block, mi_tf_create, mi_tf_is_owned,
};
use crate::mimalloc::{mi_heap_main, mi_heap_theap, mi_heap_zalloc_aligned};
use crate::options::{mi_option_get, mi_option_get_size, mi_option_is_enabled, mi_option_t};
use crate::os::{
    _mi_os_alloc_aligned, _mi_os_alloc_aligned_at_offset, _mi_os_alloc_huge_os_pages,
    _mi_os_minimal_purge_size, _mi_os_numa_node_count,
};
use crate::page::{
    _mi_bin_typed, _mi_page_free_collect, _mi_page_stats_bin, MI_LARGE_MAX_OBJ_SIZE,
    MI_LARGE_PAGE_SIZE, MI_MEDIUM_MAX_OBJ_SIZE, MI_MEDIUM_PAGE_SIZE, MI_PAGE_OSPAGE_BLOCK_ALIGN2,
    MI_SMALL_MAX_OBJ_SIZE, MI_SMALL_PAGE_SIZE, mi_page_set_theap,
};
use crate::page_map::{_mi_page_map_register, _mi_page_map_unregister, _mi_safe_ptr_page};
use crate::prim::_mi_page_associated_theap_peek;
use crate::stats::{MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_OTHER, MI_CBIN_SMALL};
use crate::theap::{_mi_heap_area_init, _mi_theap_area_visit_blocks};
use crate::track::{mi_track_mem_defined, mi_track_mem_undefined};
use crate::types::{
    MI_BIN_COUNT, MI_ENABLE_LARGE_PAGES, MI_PAGE_ALIGN, MI_PAGE_MAX_OVERALLOC_ALIGN,
    MI_PAGE_META_ALIGNED_FREE_SMALL, MI_PAGE_MIN_COMMIT_SIZE, MI_SMALL_SIZE_MAX,
    MI_THREADID_ABANDONED_MAPPED, mi_arena_id_t, mi_arena_t, mi_block_visit_fun, mi_commit_fun_t,
    mi_heap_area_t, mi_heap_t, mi_msecs_t, mi_page_t, mi_subproc_t, mi_theap_t, mi_thread_free_t,
    mi_thread_local_t, mi_tld_t,
};

pub const MI_ARENA_SLICE_SIZE: usize = 8192 * size_of::<usize>();
pub const MI_ARENA_SLICE_ALIGN: usize = MI_ARENA_SLICE_SIZE;
pub const MI_ARENA_BCHUNK_BITS: usize = 64 * size_of::<usize>();
pub const MI_ARENA_MIN_SIZE: usize = MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE;
const MI_ARENA_MAX_CHUNK_OBJ_SIZE: usize = MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE;
pub const MI_META_PAGE_SIZE: usize = MI_ARENA_SLICE_SIZE;
pub const MI_PAGE_BLOCK_START_MAX_OFFSET: usize = 8 * MI_INTPTR_BITS;
#[allow(non_upper_case_globals)]
pub const mi_forall_arenas_end: () = ();

const MI_MAX_ARENAS: usize = 160;
const MI_ARENA_BIN_COUNT: usize = MI_BIN_COUNT;
const MI_ARENA_MIN_OBJ_SIZE: usize = MI_ARENA_SLICE_SIZE;
const MI_ARENA_MAX_SIZE: usize = MI_BITMAP_MAX_BIT_COUNT * MI_ARENA_SLICE_SIZE;
const EAGAIN: c_int = 11;
const EINVAL: c_int = 22;
const ENOMEM: c_int = 12;

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_ansi_color_e {
    MI_BLACK = 30,
    MI_MAROON = 31,
    MI_DARKGREEN = 32,
    MI_ORANGE = 33,
    MI_NAVY = 34,
    MI_PURPLE = 35,
    MI_TEAL = 36,
    MI_GRAY = 37,
    MI_DARKGRAY = 90,
    MI_RED = 91,
    MI_GREEN = 92,
    MI_YELLOW = 93,
    MI_BLUE = 94,
    MI_MAGENTA = 95,
    MI_CYAN = 96,
    MI_WHITE = 97,
}

#[allow(non_camel_case_types)]
pub type mi_ansi_color_t = mi_ansi_color_e;

pub const MI_BLACK: mi_ansi_color_t = mi_ansi_color_e::MI_BLACK;
pub const MI_MAROON: mi_ansi_color_t = mi_ansi_color_e::MI_MAROON;
pub const MI_DARKGREEN: mi_ansi_color_t = mi_ansi_color_e::MI_DARKGREEN;
pub const MI_ORANGE: mi_ansi_color_t = mi_ansi_color_e::MI_ORANGE;
pub const MI_NAVY: mi_ansi_color_t = mi_ansi_color_e::MI_NAVY;
pub const MI_PURPLE: mi_ansi_color_t = mi_ansi_color_e::MI_PURPLE;
pub const MI_TEAL: mi_ansi_color_t = mi_ansi_color_e::MI_TEAL;
pub const MI_GRAY: mi_ansi_color_t = mi_ansi_color_e::MI_GRAY;
pub const MI_DARKGRAY: mi_ansi_color_t = mi_ansi_color_e::MI_DARKGRAY;
pub const MI_RED: mi_ansi_color_t = mi_ansi_color_e::MI_RED;
pub const MI_GREEN: mi_ansi_color_t = mi_ansi_color_e::MI_GREEN;
pub const MI_YELLOW: mi_ansi_color_t = mi_ansi_color_e::MI_YELLOW;
pub const MI_BLUE: mi_ansi_color_t = mi_ansi_color_e::MI_BLUE;
pub const MI_MAGENTA: mi_ansi_color_t = mi_ansi_color_e::MI_MAGENTA;
pub const MI_CYAN: mi_ansi_color_t = mi_ansi_color_e::MI_CYAN;
pub const MI_WHITE: mi_ansi_color_t = mi_ansi_color_e::MI_WHITE;

#[repr(C)]
struct MiHeapPrefix {
    subproc: *mut mi_subproc_t,
    heap_seq: usize,
    next: *mut mi_heap_t,
    prev: *mut mi_heap_t,
    theap: mi_thread_local_t,
    exclusive_arena: *mut mi_arena_t,
    numa_node: i32,
    _padding: u32,
    theaps: *mut mi_theap_t,
    theaps_lock: crate::atomic::mi_lock_t,
    abandoned_count: [AtomicUsize; MI_BIN_COUNT],
    os_abandoned_pages: *mut mi_page_t,
    os_abandoned_pages_lock: crate::atomic::mi_lock_t,
    arena_pages: [AtomicPtr<c_void>; MI_MAX_ARENAS],
    arena_pages_lock: crate::atomic::mi_lock_t,
}

#[repr(C)]
struct MiSubprocPrefix {
    subproc_seq: usize,
    next: *mut mi_subproc_t,
    prev: *mut mi_subproc_t,
    arena_count: AtomicUsize,
    arenas: [*mut mi_arena_t; MI_MAX_ARENAS],
    arena_reserve_lock: crate::atomic::mi_lock_t,
    purge_expire: AtomicI64,
    heap_main: *mut mi_heap_t,
    heaps: *mut mi_heap_t,
    heaps_lock: crate::atomic::mi_lock_t,
    thread_count: AtomicUsize,
    thread_total_count: AtomicUsize,
    heap_count: AtomicUsize,
}

#[repr(C)]
struct MiArenaPrefix {
    memid: crate::internal::mi_memid_t,
    subproc: *mut mi_subproc_t,
    arena_idx: usize,
    slice_count: usize,
    info_slices: usize,
    numa_node: i32,
    is_exclusive: bool,
    _padding: [u8; 7],
    purge_expire: AtomicI64,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    total_size: usize,
    parent: *mut mi_arena_t,
    slices_free: *mut crate::bitmap::mi_bbitmap_t,
    slices_committed: *mut crate::bitmap::mi_bitmap_t,
    slices_dirty: *mut crate::bitmap::mi_bitmap_t,
    slices_purge: *mut crate::bitmap::mi_bitmap_t,
    pages_meta: *mut mi_page_t,
    pages_main: MiArenaPagesPrefix,
}

#[repr(C)]
struct MiArenaPagesPrefix {
    pages: *mut mi_bitmap_t,
    pages_abandoned: [*mut mi_bitmap_t; MI_ARENA_BIN_COUNT],
}

#[allow(non_camel_case_types)]
type mi_arena_pages_t = MiArenaPagesPrefix;

#[allow(dead_code, non_camel_case_types)]
#[repr(C)]
struct mi_purge_visit_info_t {
    now: mi_msecs_t,
    delay: mi_msecs_t,
    all_purged: bool,
    any_purged: bool,
}

#[allow(dead_code, non_camel_case_types)]
#[repr(C)]
struct mi_heap_visit_info_t {
    heap: *mut mi_heap_t,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
    visit_blocks: bool,
}

#[allow(dead_code, non_camel_case_types)]
#[repr(C)]
struct mi_heap_delete_visit_info_t {
    heap_target: *mut mi_heap_t,
    theap_target: *mut mi_theap_t,
    theap: *mut mi_theap_t,
}

#[allow(dead_code)]
unsafe fn mi_heap_visit_page(page: *mut mi_page_t, vinfo: *mut mi_heap_visit_info_t) -> bool {
    let mut area = mi_heap_area_t::default();
    unsafe { _mi_heap_area_init(ptr::addr_of_mut!(area), page) };
    crate::mi_assert_internal!(unsafe { (*vinfo).heap == mi_page_heap(page) });
    if !unsafe {
        ((*vinfo).visitor)(
            (*vinfo).heap,
            ptr::addr_of!(area),
            ptr::null_mut(),
            area.block_size,
            (*vinfo).arg,
        )
    } {
        return false;
    }
    if unsafe { (*vinfo).visit_blocks } {
        unsafe {
            _mi_theap_area_visit_blocks(ptr::addr_of!(area), page, (*vinfo).visitor, (*vinfo).arg)
        }
    } else {
        true
    }
}

#[allow(dead_code)]
unsafe extern "C" fn mi_heap_visit_page_at(
    slice_index: usize,
    _slice_count: usize,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let vinfo = arg.cast::<mi_heap_visit_info_t>();
    let page = unsafe { mi_arena_page_at_slice(arena, slice_index) };
    unsafe { mi_heap_visit_page(page, vinfo) }
}

/// # Safety
/// Visits pages and optionally blocks for a heap using the C ABI callback contract.
pub(crate) unsafe extern "C" fn _mi_heap_visit_blocks(
    mut heap: *mut mi_heap_t,
    abandoned_only: bool,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    if heap.is_null() {
        heap = unsafe { mi_heap_main() };
    }

    let mut visit_info = mi_heap_visit_info_t {
        heap,
        visitor,
        arg,
        visit_blocks,
    };
    let mut ok = true;
    unsafe {
        mi_forall_arenas(heap, ptr::null_mut(), 0, |arena| {
            let arena_pages = mi_heap_arena_pages(heap, arena);
            if ok && !arena_pages.is_null() {
                if abandoned_only {
                    for bin in 0..MI_BIN_COUNT {
                        if !ok {
                            break;
                        }
                        if (*heap.cast::<MiHeapPrefix>()).abandoned_count[bin]
                            .load(Ordering::Relaxed)
                            > 0
                        {
                            ok = _mi_bitmap_forall_set(
                                (*arena_pages).pages_abandoned[bin],
                                mi_heap_visit_page_at,
                                arena,
                                ptr::addr_of_mut!(visit_info).cast(),
                            );
                        }
                    }
                } else {
                    ok = _mi_bitmap_forall_set(
                        (*arena_pages).pages,
                        mi_heap_visit_page_at,
                        arena,
                        ptr::addr_of_mut!(visit_info).cast(),
                    );
                }
            }
            ok
        });
    }
    if !ok {
        return false;
    }

    let page = unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(
            (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
        ));
        let page = (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages;
        mi_lock_release(ptr::addr_of_mut!(
            (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
        ));
        page
    };
    let mut page = page;
    while ok && !page.is_null() {
        let next = unsafe { (*page).next };
        ok = unsafe { mi_heap_visit_page(page, ptr::addr_of_mut!(visit_info)) };
        page = next;
    }

    ok
}

/// # Safety
/// Public C API wrapper over `_mi_heap_visit_blocks`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_visit_blocks(
    heap: *mut mi_heap_t,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    unsafe { _mi_heap_visit_blocks(heap, false, visit_blocks, visitor, arg) }
}

/// # Safety
/// Public C API wrapper over `_mi_heap_visit_blocks` for abandoned pages only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_visit_abandoned_blocks(
    heap: *mut mi_heap_t,
    visit_blocks: bool,
    visitor: mi_block_visit_fun,
    arg: *mut c_void,
) -> bool {
    unsafe { _mi_heap_visit_blocks(heap, true, visit_blocks, visitor, arg) }
}

#[allow(dead_code)]
unsafe extern "C" fn mi_heap_delete_page(
    heap: *const mi_heap_t,
    area: *const mi_heap_area_t,
    _block: *mut c_void,
    _block_size: usize,
    arg: *mut c_void,
) -> bool {
    let info = arg.cast::<mi_heap_delete_visit_info_t>();
    let mut heap_target = unsafe { (*info).heap_target };
    let theap = ptr::null_mut::<mi_theap_t>();
    let page = unsafe { (*area).reserved1.cast::<mi_page_t>() };

    unsafe { mi_page_claim_ownership(page) };
    if unsafe { mi_page_is_abandoned(page) } {
        unsafe { _mi_arenas_page_unabandon(page, theap) };
    } else {
        unsafe {
            (*page).next = ptr::null_mut();
            (*page).prev = ptr::null_mut();
            mi_page_set_theap(page, ptr::null_mut());
        }
    }
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    if unsafe { (*page).used == 0 } {
        unsafe { _mi_arenas_page_free(page, theap) };
    } else if heap_target.is_null() {
        unsafe { (*page).used = 0 };
        unsafe { _mi_arenas_page_free(page, theap) };
    } else {
        let sbin = unsafe { _mi_page_stats_bin(page) };
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        let arena = unsafe {
            mi_page_arena_pages(page, &mut slice_index, &mut slice_count, &mut arena_pages)
        };
        crate::mi_assert_internal!(unsafe { mi_bitmap_is_set((*arena_pages).pages, slice_index) });
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        if !theap.is_null() {
            unsafe {
                __mi_stat_decrease(ptr::addr_of_mut!((*theap).stats.page_bins[sbin]), 1);
                __mi_stat_decrease(ptr::addr_of_mut!((*theap).stats.pages), 1);
            }
        } else {
            let heap = heap as *mut mi_heap_t;
            unsafe {
                __mi_stat_decrease_mt(&mut (*heap).stats.page_bins[sbin], 1);
                __mi_stat_decrease_mt(&mut (*heap).stats.pages, 1);
            }
        }
        let mut theap_target = unsafe { (*info).theap_target };

        let mut arena_pages_target = unsafe { mi_heap_ensure_arena_pages(heap_target, arena) };
        if arena_pages_target.is_null() {
            heap_target = unsafe { mi_heap_main() };
            theap_target = unsafe { mi_heap_theap(heap_target) };
            arena_pages_target = unsafe { mi_heap_ensure_arena_pages(heap_target, arena) };
            crate::mi_assert_internal!(!arena_pages_target.is_null());
        }
        crate::mi_assert_internal!(unsafe {
            !mi_bitmap_is_set((*arena_pages_target).pages, slice_index)
        });
        unsafe { mi_bitmap_set((*arena_pages_target).pages, slice_index) };
        unsafe { (*page).heap = heap_target };
        unsafe {
            __mi_stat_increase(ptr::addr_of_mut!((*theap_target).stats.page_bins[sbin]), 1);
            __mi_stat_increase(ptr::addr_of_mut!((*theap_target).stats.pages), 1);
        }
        unsafe { _mi_arenas_page_abandon(page, theap_target) };
    }
    true
}

#[allow(dead_code)]
unsafe fn mi_heap_delete_pages(heap: *mut mi_heap_t, heap_target: *mut mi_heap_t) {
    let theap_target = if heap_target.is_null() {
        ptr::null_mut()
    } else {
        unsafe { mi_heap_theap(heap_target) }
    };
    let mut info = mi_heap_delete_visit_info_t {
        heap_target,
        theap_target,
        theap: ptr::null_mut(),
    };
    let _ = unsafe {
        _mi_heap_visit_blocks(
            heap,
            false,
            false,
            mi_heap_delete_page,
            ptr::addr_of_mut!(info).cast(),
        )
    };
    if MI_DEBUG > 1 {
        for i in 0..MI_ARENA_BIN_COUNT {
            let arena_pages =
                unsafe { (*heap.cast::<MiHeapPrefix>()).arena_pages[i].load(Ordering::Acquire) }
                    .cast::<mi_arena_pages_t>();
            if !arena_pages.is_null() {
                crate::mi_assert_internal!(unsafe { mi_bitmap_is_all_clear((*arena_pages).pages) });
            }
        }
        unsafe {
            mi_lock_acquire(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
            ))
        };
        crate::mi_assert_internal!(unsafe {
            (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages.is_null()
        });
        unsafe {
            mi_lock_release(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
            ))
        };
        for i in 0..MI_BIN_COUNT {
            crate::mi_assert_internal!(unsafe {
                (*heap.cast::<MiHeapPrefix>()).abandoned_count[i].load(Ordering::Relaxed) == 0
            });
        }
    }
}

/// # Safety
/// Moves all pages from one heap to another following the original internal ABI.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_move_pages(
    heap_from: *mut mi_heap_t,
    mut heap_to: *mut mi_heap_t,
) {
    if unsafe { _mi_is_heap_main(heap_from) } {
        return;
    }
    if heap_to.is_null() {
        heap_to = unsafe { mi_heap_main() };
    }
    unsafe { mi_heap_delete_pages(heap_from, heap_to) };
}

/// # Safety
/// Destroys all pages owned by a non-main heap following the original internal ABI.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_heap_destroy_pages(heap_from: *mut mi_heap_t) {
    if unsafe { _mi_is_heap_main(heap_from) } {
        return;
    }
    unsafe { mi_heap_delete_pages(heap_from, ptr::null_mut()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_alignment() -> usize {
    MI_ARENA_SLICE_ALIGN
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_arena_min_size() -> usize {
    MI_ARENA_MIN_SIZE
}

/// # Safety
/// Matches the C internal API and returns the null arena-id sentinel.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arena_id_none() -> mi_arena_id_t {
    ptr::null_mut()
}

/// # Safety
/// `id` must be either null or a valid top-level `mi_arena_t*`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arena_from_id(id: mi_arena_id_t) -> *mut mi_arena_t {
    let arena = id.cast::<mi_arena_t>();
    crate::mi_assert_internal!(
        arena.is_null() || unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() }
    );
    arena
}

#[used]
static MI_ARENA_ID_NONE_KEEP: unsafe extern "C" fn() -> mi_arena_id_t = _mi_arena_id_none;

#[used]
static MI_ARENA_FROM_ID_KEEP: unsafe extern "C" fn(mi_arena_id_t) -> *mut mi_arena_t =
    _mi_arena_from_id;

/// # Safety
/// `arena` must be null or a valid top-level `mi_arena_t*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_id_from_arena(arena: *mut mi_arena_t) -> mi_arena_id_t {
    crate::mi_assert_internal!(
        arena.is_null() || unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() }
    );
    if arena.is_null() {
        unsafe { _mi_arena_id_none() }
    } else {
        arena.cast::<c_void>()
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_is_suitable(arena: *mut mi_arena_t, req_arena: *mut mi_arena_t) -> bool {
    if arena == req_arena {
        return true;
    }
    if arena.is_null() {
        return false;
    }
    if req_arena.is_null() && !unsafe { (*arena.cast::<MiArenaPrefix>()).is_exclusive } {
        return true;
    }
    !unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() }
        && unsafe { (*arena.cast::<MiArenaPrefix>()).parent == req_arena }
}

/// # Safety
/// `memid` and `request_arena` must follow the internal arena allocation invariants.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arena_memid_is_suitable(
    memid: mi_memid_t,
    request_arena: *mut mi_arena_t,
) -> bool {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(a) => unsafe {
            mi_arena_is_suitable(a.arena.as_ptr(), request_arena)
        },
        _ => unsafe { mi_arena_is_suitable(ptr::null_mut(), request_arena) },
    }
}

/// # Safety
/// `subproc` must point to a valid `mi_subproc_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arenas_get_count(subproc: *mut mi_subproc_t) -> usize {
    unsafe {
        (*subproc.cast::<MiSubprocPrefix>())
            .arena_count
            .load(Ordering::Relaxed)
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_info_slices(arena: *mut mi_arena_t) -> usize {
    unsafe { (*arena.cast::<MiArenaPrefix>()).info_slices }
}

#[allow(dead_code)]
unsafe fn mi_heap_arena_pages(
    heap: *mut mi_heap_t,
    arena: *mut mi_arena_t,
) -> *mut mi_arena_pages_t {
    crate::mi_assert_internal!(!arena.is_null());
    crate::mi_assert_internal!(!heap.is_null());
    let arena_idx = unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx };
    crate::mi_assert!(arena_idx < MI_MAX_ARENAS);
    unsafe { (*heap.cast::<MiHeapPrefix>()).arena_pages[arena_idx].load(Ordering::Acquire) }
        .cast::<mi_arena_pages_t>()
}

#[allow(dead_code)]
unsafe fn mi_page_arena_pages(
    page: *mut mi_page_t,
    slice_index: *mut usize,
    slice_count: *mut usize,
    parena_pages: *mut *mut mi_arena_pages_t,
) -> *mut mi_arena_t {
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    let arena = unsafe { mi_arena_from_memid((*page).memid, slice_index, slice_count) };
    crate::mi_assert_internal!(!arena.is_null());
    if !parena_pages.is_null() {
        let arena_pages = unsafe { mi_heap_arena_pages(mi_page_heap(page), arena) };
        crate::mi_assert_internal!(!arena_pages.is_null());
        crate::mi_assert_internal!(
            slice_index.is_null()
                || unsafe { mi_bitmap_is_setN((*arena_pages).pages, *slice_index, 1) }
        );
        unsafe { *parena_pages = arena_pages };
    }
    arena
}

#[inline]
unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool {
    matches!(
        crate::core_types::MemId::view(unsafe { (*page).memid }),
        crate::core_types::MemIdView::Arena(_)
    )
}

#[allow(dead_code)]
unsafe fn mi_heap_has_page(
    heap: *mut mi_heap_t,
    arena: *mut mi_arena_t,
    page: *mut mi_page_t,
) -> bool {
    let arena_idx = unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx };
    crate::mi_assert!(arena_idx < MI_MAX_ARENAS);
    let arena_pages =
        unsafe { (*heap.cast::<MiHeapPrefix>()).arena_pages[arena_idx].load(Ordering::Acquire) }
            .cast::<MiArenaPagesPrefix>();
    match crate::core_types::MemId::view(unsafe { (*page).memid }) {
        crate::core_types::MemIdView::Arena(a) => {
            a.arena.as_ptr() == arena
                && !arena_pages.is_null()
                && unsafe { mi_bitmap_is_setN((*arena_pages).pages, a.range.start().0, 1) }
        }
        _ => false,
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    unsafe { *__errno_location() = err };
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    unsafe { *__error() = err };
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

#[allow(dead_code)]
unsafe fn mi_arena_max_object_size() -> usize {
    let max_size = _mi_align_up(
        mi_option_get_size(mi_option_t::mi_option_arena_max_object_size),
        MI_ARENA_SLICE_SIZE,
    );
    if max_size <= MI_ARENA_MIN_OBJ_SIZE {
        MI_ARENA_MIN_OBJ_SIZE
    } else if max_size >= MI_ARENA_MAX_SIZE - MI_BCHUNK_SIZE {
        MI_ARENA_MAX_SIZE - MI_BCHUNK_SIZE
    } else {
        max_size
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_commit(
    arena: *mut mi_arena_t,
    start: *mut c_void,
    size: usize,
    is_zero: *mut bool,
    already_committed: usize,
) -> bool {
    if !arena.is_null() {
        let commit_fun = unsafe { (*arena.cast::<MiArenaPrefix>()).commit_fun };
        if let Some(commit_fun) = commit_fun {
            return unsafe {
                commit_fun(
                    true,
                    start,
                    size,
                    is_zero,
                    (*arena.cast::<MiArenaPrefix>()).commit_fun_arg,
                )
            };
        }
    }
    if already_committed > 0 {
        unsafe { _mi_os_commit_ex(start, size, is_zero, already_committed) }
    } else {
        unsafe { _mi_os_commit(start, size, is_zero) }
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_size(arena: *mut mi_arena_t) -> usize {
    mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count })
}

#[allow(dead_code)]
unsafe fn mi_arena_start(arena: *mut mi_arena_t) -> *mut u8 {
    arena.cast::<u8>()
}

#[allow(dead_code)]
#[inline(never)]
unsafe fn mi_arena_slice_start(arena: *mut mi_arena_t, slice_index: usize) -> *mut u8 {
    crate::mi_assert_internal!(
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    unsafe { mi_arena_start(arena).add(mi_size_of_slices(slice_index)) }
}

#[allow(dead_code)]
#[inline(never)]
unsafe fn mi_arena_page_at_slice(arena: *mut mi_arena_t, slice_index: usize) -> *mut mi_page_t {
    crate::mi_assert_internal!(
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
    if !pages_meta.is_null() {
        return unsafe { pages_meta.add(slice_index) };
    }
    unsafe { mi_arena_slice_start(arena, slice_index).cast::<mi_page_t>() }
}

/// # Safety
/// `arena_id` must be null or a valid top-level arena id; `size` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_area(arena_id: mi_arena_id_t, size: *mut usize) -> *mut c_void {
    if !size.is_null() {
        unsafe { *size = 0 };
    }
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        return ptr::null_mut();
    }
    if !size.is_null() {
        crate::mi_assert_internal!(
            mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count })
                <= unsafe { (*arena.cast::<MiArenaPrefix>()).total_size }
        );
        unsafe { *size = (*arena.cast::<MiArenaPrefix>()).total_size };
    }
    unsafe { mi_arena_start(arena).cast::<c_void>() }
}

#[allow(dead_code)]
unsafe fn mi_arena_strictly_contains(arena: *mut mi_arena_t, p: *const c_void) -> bool {
    let p = p.cast::<u8>();
    !arena.is_null() && {
        let start = unsafe { mi_arena_start(arena).cast_const() };
        let end = unsafe {
            start.add(mi_size_of_slices(
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            ))
        };
        start <= p && end > p
    }
}

#[allow(dead_code)]
unsafe fn mi_arenas_contain_ex(p: *const c_void, parent: *mut mi_arena_t) -> bool {
    let subproc = unsafe { _mi_subproc() };
    let max_arena = unsafe { mi_arenas_get_count(subproc) };
    for i in 0..max_arena {
        let arena = unsafe {
            mi_atomic_load_ptr_acquire(ptr::addr_of!(
                (*subproc.cast::<MiSubprocPrefix>()).arenas[i]
            ))
        };
        if !arena.is_null()
            && (parent.is_null()
                || arena == parent
                || unsafe { (*arena.cast::<MiArenaPrefix>()).parent == parent })
            && unsafe { mi_arena_strictly_contains(arena, p) }
        {
            return true;
        }
    }
    false
}

/// # Safety
/// `p` may be any pointer value.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_contain(p: *const c_void) -> bool {
    unsafe { mi_arenas_contain_ex(p, ptr::null_mut()) }
}

#[used]
static MI_ARENAS_CONTAIN_KEEP: unsafe extern "C" fn(*const c_void) -> bool = _mi_arenas_contain;

/// # Safety
/// `arena_id` must be null or a valid top-level arena id; `p` may be any pointer value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_contains(arena_id: mi_arena_id_t, p: *const c_void) -> bool {
    let arena = unsafe { _mi_arena_from_id(arena_id) };
    if arena.is_null() {
        false
    } else if unsafe { mi_arena_strictly_contains(arena, p) } {
        true
    } else {
        unsafe { mi_arenas_contain_ex(p, arena) }
    }
}

#[allow(dead_code)]
unsafe fn mi_memid_create_arena(
    arena: *mut mi_arena_t,
    range: crate::core_types::SliceRange,
) -> mi_memid_t {
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    crate::mi_assert_internal!(slice_count > 0);
    crate::mi_assert_internal!(
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    crate::core_types::MemId::<crate::core_types::ArenaMem>::arena(
        unsafe { crate::core_types::ArenaPtr::new_unchecked(arena) },
        range,
    )
    .raw
}

#[allow(dead_code)]
unsafe fn mi_arena_from_memid(
    memid: mi_memid_t,
    slice_index: *mut usize,
    slice_count: *mut usize,
) -> *mut mi_arena_t {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(a) => {
            if !slice_index.is_null() {
                unsafe { *slice_index = a.range.start().0 };
            }
            if !slice_count.is_null() {
                unsafe { *slice_count = a.range.count().0.get() as usize };
            }
            a.arena.as_ptr()
        }
        _ => {
            crate::mi_assert_internal!(false);
            core::ptr::null_mut()
        }
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_range_from_memid(
    memid: mi_memid_t,
) -> Option<(crate::core_types::ArenaPtr, crate::core_types::SliceRange)> {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(a) => Some((a.arena, a.range)),
        _ => {
            crate::mi_assert_internal!(false);
            None
        }
    }
}

#[allow(dead_code)]
fn mi_memid_is_os(memid: mi_memid_t) -> bool {
    matches!(
        crate::core_types::MemId::view(memid),
        crate::core_types::MemIdView::Os(_)
    )
}

#[allow(dead_code)]
unsafe fn mi_page_full_size(page: *mut mi_page_t) -> usize {
    let memid = unsafe { (*page).memid };
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Arena(a) => {
            a.range.count().0.get() as usize * MI_ARENA_SLICE_SIZE
        }
        crate::core_types::MemIdView::Os(os) => {
            let base = os
                .base
                .map(|p| p.as_ptr())
                .unwrap_or(core::ptr::null_mut())
                .cast::<u8>();
            let os_size = os.size.0;

            let page_ptr = page.cast::<u8>();
            crate::mi_assert_internal!(base <= page_ptr);
            let presize = unsafe { page_ptr.offset_from(base) };
            crate::mi_assert_internal!(os_size as isize >= presize);
            if presize > os_size as isize {
                0
            } else {
                os_size - presize as usize
            }
        }
        crate::core_types::MemIdView::External(external) => {
            let base = external
                .base
                .map(|p| p.as_ptr())
                .unwrap_or(core::ptr::null_mut())
                .cast::<u8>();
            let os_size = external.size.0;

            let page_ptr = page.cast::<u8>();
            crate::mi_assert_internal!(base <= page_ptr);
            let presize = unsafe { page_ptr.offset_from(base) };
            crate::mi_assert_internal!(os_size as isize >= presize);
            if presize > os_size as isize {
                0
            } else {
                os_size - presize as usize
            }
        }
        _ => 0,
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_try_alloc_at(
    arena: *mut mi_arena_t,
    slice_count: crate::core_types::SliceCount,
    commit: bool,
    tseq: usize,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    let slice_count = slice_count.get() as usize;
    let mut slice_index = 0usize;
    if !unsafe {
        mi_bbitmap_try_find_and_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            tseq,
            slice_count,
            &mut slice_index,
        )
    } {
        return ptr::null_mut();
    }

    let p = unsafe { mi_arena_slice_start(arena, slice_index).cast::<c_void>() };
    let Some(range) = crate::core_types::SliceRange::new(slice_index, slice_count, unsafe {
        crate::core_types::ArenaPtr::new_unchecked(arena)
    })
    .ok() else {
        crate::mi_assert_internal!(false);
        unsafe {
            mi_bbitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        };
        return ptr::null_mut();
    };
    unsafe { *memid = mi_memid_create_arena(arena, range) };
    unsafe { (*memid).is_pinned = (*arena.cast::<MiArenaPrefix>()).memid.is_pinned };

    let mut touched_slices = slice_count;
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.initially_zero } {
        let mut already_dirty = 0usize;
        unsafe {
            (*memid).initially_zero = mi_bitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index,
                slice_count,
                &mut already_dirty,
            )
        };
        crate::mi_assert_internal!(already_dirty <= touched_slices);
        touched_slices -= already_dirty;
    }

    if commit {
        let already_committed = unsafe {
            mi_bitmap_popcountN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
        if already_committed < slice_count {
            let mut commit_zero = false;
            if !unsafe {
                mi_arena_commit(
                    arena,
                    p.cast(),
                    mi_size_of_slices(slice_count),
                    &mut commit_zero,
                    mi_size_of_slices(slice_count - already_committed),
                )
            } {
                unsafe {
                    mi_bbitmap_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_free,
                        slice_index,
                        slice_count,
                    )
                };
                return ptr::null_mut();
            }
            if commit_zero {
                unsafe { (*memid).initially_zero = true };
            }
            unsafe {
                mi_bitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                    ptr::null_mut(),
                )
            };
            if MI_DEBUG > 1
                && unsafe { (*memid).initially_zero }
                && !unsafe { mi_mem_is_zero(p, mi_size_of_slices(slice_count)) }
            {
                crate::options::_mi_error_message_fmt(
                    EFAULT,
                    format_args!("internal error: arena allocation was not zero-initialized!\n"),
                );
                unsafe { (*memid).initially_zero = false };
            }
        } else {
            unsafe { _mi_os_reuse(p, mi_size_of_slices(slice_count)) };
            if unsafe { _mi_os_has_overcommit() }
                && touched_slices > 0
                && !unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned }
            {
                unsafe {
                    __mi_stat_increase_mt(
                        &mut (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed,
                        mi_size_of_slices(touched_slices),
                    )
                };
            }
        }

        crate::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        });
        unsafe { (*memid).initially_committed = true };

        if unsafe { (*memid).initially_zero } {
            unsafe { mi_track_mem_defined(p, slice_count * MI_ARENA_SLICE_SIZE) };
        } else {
            unsafe { mi_track_mem_undefined(p, slice_count * MI_ARENA_SLICE_SIZE) };
        }
    } else {
        unsafe {
            (*memid).initially_committed = mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
        if !unsafe { (*memid).initially_committed } {
            let mut already_committed_count = 0usize;
            unsafe {
                mi_bitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                    &mut already_committed_count,
                )
            };
            unsafe {
                mi_bitmap_clearN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            };
            unsafe {
                __mi_stat_adjust_decrease_mt(
                    ptr::addr_of_mut!((*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed),
                    mi_size_of_slices(already_committed_count),
                )
            };
        }
    }

    crate::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });
    if commit {
        crate::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        });
        crate::mi_assert_internal!(unsafe { (*memid).initially_committed });
    }
    crate::mi_assert_internal!(unsafe {
        mi_bitmap_is_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_dirty,
            slice_index,
            slice_count,
        )
    });

    p
}

/// # Safety
/// `subproc` must point to a valid `mi_subproc_t`, and `idx` must be in range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arena_from_index(
    subproc: *mut mi_subproc_t,
    idx: usize,
) -> *mut mi_arena_t {
    crate::mi_assert_internal!(idx < unsafe { mi_arenas_get_count(subproc) });
    unsafe {
        mi_atomic_load_ptr_acquire(ptr::addr_of!(
            (*subproc.cast::<MiSubprocPrefix>()).arenas[idx]
        ))
    }
}

#[allow(dead_code)]
unsafe fn mi_arena_start_idx(heap: *mut mi_heap_t, tseq: usize, arena_cycle: usize) -> usize {
    let heap = heap.cast::<MiHeapPrefix>();
    let subproc = unsafe { (*heap).subproc.cast::<MiSubprocPrefix>() };
    let hseq = unsafe { (*heap).heap_seq };
    let hcount = unsafe { (*subproc).heap_count.load(Ordering::Relaxed) };
    if arena_cycle <= 1 {
        return 0;
    }
    if hseq == 0 || hcount <= 1 || arena_cycle > 0x8FF {
        return tseq % arena_cycle;
    }

    crate::mi_assert_internal!(arena_cycle <= 0x8FF);
    let frac = (arena_cycle * 256) / hcount;
    let start = if frac == 0 {
        hseq % arena_cycle
    } else {
        let hspot = hseq % hcount;
        let mut start = (frac * hspot) / 256;
        if frac >= 512 {
            start += tseq % (frac / 256);
        }
        start
    };
    crate::mi_assert_internal!(start < arena_cycle);
    start
}

#[allow(dead_code)]
unsafe fn mi_arena_is_suitable_ex(
    arena: *mut mi_arena_t,
    req_arena: *mut mi_arena_t,
    match_numa: bool,
    numa_node: i32,
    allow_pinned: bool,
) -> bool {
    if !allow_pinned && unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return false;
    }
    if unsafe { !mi_arena_is_suitable(arena, req_arena) } {
        return false;
    }
    if req_arena.is_null() {
        let arena_numa = unsafe { (*arena.cast::<MiArenaPrefix>()).numa_node };
        let numa_suitable = numa_node < 0 || arena_numa < 0 || arena_numa == numa_node;
        if match_numa {
            if !numa_suitable {
                return false;
            }
        } else if numa_suitable {
            return false;
        }
    }
    true
}

#[allow(dead_code)]
unsafe fn mi_forall_arenas<F>(
    heap: *mut mi_heap_t,
    req_arena: *mut mi_arena_t,
    tseq: usize,
    mut visit: F,
) where
    F: FnMut(*mut mi_arena_t) -> bool,
{
    let subproc = unsafe { (*heap.cast::<MiHeapPrefix>()).subproc };
    let arena_count = unsafe { mi_arenas_get_count(subproc) };
    let arena_cycle = arena_count.saturating_sub(1);
    let start = unsafe { mi_arena_start_idx(heap, tseq, arena_cycle) };
    for i in 0..arena_count {
        let arena = if !req_arena.is_null() {
            if i > 0 {
                break;
            }
            req_arena
        } else {
            let idx = if i < arena_cycle {
                let mut idx = i + start;
                if idx >= arena_cycle {
                    idx -= arena_cycle;
                }
                idx
            } else {
                i
            };
            unsafe { mi_arena_from_index(subproc, idx) }
        };
        if !arena.is_null() && !visit(arena) {
            break;
        }
    }
}

#[allow(dead_code)]
unsafe fn mi_forall_suitable_arenas<F>(
    heap: *mut mi_heap_t,
    req_arena: *mut mi_arena_t,
    tseq: usize,
    match_numa: bool,
    numa_node: i32,
    allow_large: bool,
    mut visit: F,
) where
    F: FnMut(*mut mi_arena_t) -> bool,
{
    unsafe {
        mi_forall_arenas(heap, req_arena, tseq, |arena| {
            !mi_arena_is_suitable_ex(arena, req_arena, match_numa, numa_node, allow_large)
                || visit(arena)
        })
    };
}

#[allow(clippy::too_many_arguments)]
unsafe fn mi_arenas_try_find_free(
    heap: *mut mi_heap_t,
    slice_count: crate::core_types::SliceCount,
    alignment: usize,
    commit: bool,
    allow_large: bool,
    req_arena: *mut mi_arena_t,
    tseq: usize,
    numa_node: c_int,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    crate::mi_assert!(alignment <= MI_ARENA_SLICE_ALIGN);
    if alignment > MI_ARENA_SLICE_ALIGN {
        return ptr::null_mut();
    }

    let mut found = ptr::null_mut();
    unsafe {
        mi_forall_suitable_arenas(
            heap,
            req_arena,
            tseq,
            true,
            numa_node,
            allow_large,
            |arena| {
                let p = mi_arena_try_alloc_at(arena, slice_count, commit, tseq, memid);
                if !p.is_null() {
                    found = p;
                    false
                } else {
                    true
                }
            },
        );
    }
    if !found.is_null() || numa_node < 0 {
        return found;
    }

    unsafe {
        mi_forall_suitable_arenas(
            heap,
            req_arena,
            tseq,
            false,
            numa_node,
            allow_large,
            |arena| {
                let p = mi_arena_try_alloc_at(arena, slice_count, commit, tseq, memid);
                if !p.is_null() {
                    found = p;
                    false
                } else {
                    true
                }
            },
        );
    }
    found
}

#[allow(dead_code)]
unsafe fn mi_arena_pages_size(slice_count: usize, bitmap_base: *mut usize) -> usize {
    let slice_count = if slice_count == 0 {
        MI_ARENA_BCHUNK_BITS
    } else {
        slice_count
    };
    crate::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_pages_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 1 + MI_ARENA_BIN_COUNT;
    let bitmaps_size = bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) };
    let size = base_size + bitmaps_size;
    if !bitmap_base.is_null() {
        unsafe { *bitmap_base = base_size };
    }
    size
}

#[allow(dead_code)]
unsafe fn mi_arena_info_slices_needed(slice_count: usize, bitmap_base: *mut usize) -> usize {
    let slice_count = if slice_count == 0 {
        MI_ARENA_BCHUNK_BITS
    } else {
        slice_count
    };
    crate::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 4 + MI_ARENA_BIN_COUNT;
    let bitmaps_size = (bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) })
        + unsafe { mi_bbitmap_size(slice_count, ptr::null_mut()) };
    let pages_size = if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        slice_count * size_of::<mi_page_t>()
    } else {
        0
    };
    let size = base_size + bitmaps_size + pages_size;
    let info_size = _mi_align_up(size, unsafe { crate::os::_mi_os_page_size() })
        + unsafe { crate::os::_mi_os_secure_guard_page_size() };
    let info_slices = mi_slice_count_of_size(info_size);
    if !bitmap_base.is_null() {
        unsafe { *bitmap_base = base_size };
    }
    info_slices
}

#[allow(dead_code)]
unsafe fn mi_arena_bitmap_init(slice_count: usize, base: *mut *mut u8) -> *mut mi_bitmap_t {
    let bitmap = unsafe { *base }.cast::<mi_bitmap_t>();
    let size = unsafe { mi_bitmap_init(bitmap, slice_count, true) };
    unsafe { *base = (*base).add(size) };
    bitmap
}

#[allow(dead_code)]
unsafe fn mi_arena_bbitmap_init(slice_count: usize, base: *mut *mut u8) -> *mut mi_bbitmap_t {
    let bbitmap = unsafe { *base }.cast::<mi_bbitmap_t>();
    let size = unsafe { mi_bbitmap_init(bbitmap, slice_count, true) };
    unsafe { *base = (*base).add(size) };
    bbitmap
}

#[allow(dead_code)]
unsafe fn mi_arena_pages_alloc(arena: *mut mi_arena_t) -> *mut mi_arena_pages_t {
    let slice_count = unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count };
    let mut bitmap_base = 0usize;
    let size = unsafe { mi_arena_pages_size(slice_count, &mut bitmap_base) };
    let arena_pages = unsafe { mi_heap_zalloc_aligned(mi_heap_main(), size, MI_BCHUNK_SIZE) }
        .cast::<mi_arena_pages_t>();
    if arena_pages.is_null() {
        return ptr::null_mut();
    }
    let mut base = unsafe { arena_pages.cast::<u8>().add(bitmap_base) };
    crate::mi_assert_internal!((base as usize).is_multiple_of(MI_BCHUNK_SIZE));
    unsafe { (*arena_pages).pages = mi_arena_bitmap_init(slice_count, &mut base) };
    for i in 0..MI_ARENA_BIN_COUNT {
        unsafe { (*arena_pages).pages_abandoned[i] = mi_arena_bitmap_init(slice_count, &mut base) };
    }
    arena_pages
}

#[allow(dead_code)]
unsafe fn mi_arenas_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::mi_assert_internal!(!subproc.is_null());
    let arena_count = unsafe { mi_arenas_get_count(subproc) };
    for i in 0..arena_count {
        let arena = unsafe {
            mi_atomic_load_ptr_acquire(ptr::addr_of!(
                (*subproc.cast::<MiSubprocPrefix>()).arenas[i]
            ))
        };
        if !arena.is_null() {
            unsafe {
                mi_atomic_store_ptr_release(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arenas[i]),
                    ptr::null_mut(),
                );
            }
            if mi_memid_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid }) {
                unsafe {
                    crate::internal::_mi_os_free_ex(
                        mi_arena_start(arena).cast::<c_void>(),
                        mi_arena_size(arena),
                        true,
                        (*arena.cast::<MiArenaPrefix>()).memid,
                        subproc,
                    );
                }
            }
        }
    }
    let mut expected = arena_count;
    let _ = unsafe {
        mi_atomic_cas_strong_acq_rel(
            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
            &mut expected,
            0,
        )
    };
}

/// # Safety
/// Caller must ensure `subproc` is valid and destruction is only used in the same narrow cases
/// as the original C implementation.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_unsafe_destroy_all(subproc: *mut mi_subproc_t) {
    unsafe { mi_arenas_unsafe_destroy(subproc) };
}

#[allow(dead_code)]
unsafe fn mi_arenas_add(
    subproc: *mut mi_subproc_t,
    arena: *mut mi_arena_t,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    crate::mi_assert_internal!(!arena.is_null());
    crate::mi_assert_internal!(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count > 0 });
    if !arena_id.is_null() {
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    let mut count = unsafe { mi_arenas_get_count(subproc) };
    for i in 0..count {
        if unsafe { mi_arena_from_index(subproc, i) }.is_null() {
            unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx = i };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            if unsafe {
                mi_atomic_cas_ptr_strong_release(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arenas[i]),
                    &mut expected,
                    arena,
                )
            } {
                if !arena_id.is_null() {
                    unsafe { *arena_id = mi_arena_id_from_arena(arena) };
                }
                return true;
            }
        }
    }

    while count < MI_MAX_ARENAS {
        if unsafe {
            mi_atomic_cas_strong_acq_rel(
                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
                &mut count,
                count + 1,
            )
        } {
            unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx = count };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            if unsafe {
                mi_atomic_cas_ptr_strong_release(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arenas[count]),
                    &mut expected,
                    arena,
                )
            } {
                unsafe {
                    __mi_stat_counter_increase(
                        ptr::addr_of_mut!(
                            (*(*arena.cast::<MiArenaPrefix>()).subproc)
                                .stats
                                .arena_count
                        ),
                        1,
                    )
                };
                if !arena_id.is_null() {
                    unsafe { *arena_id = mi_arena_id_from_arena(arena) };
                }
                return true;
            }
        }
    }

    unsafe {
        (*arena.cast::<MiArenaPrefix>()).arena_idx = 0;
        (*arena.cast::<MiArenaPrefix>()).subproc = ptr::null_mut();
    }
    false
}

#[allow(dead_code, clippy::too_many_arguments)]
unsafe fn mi_arena_initialize(
    subproc: *mut mi_subproc_t,
    start: *mut c_void,
    slice_count: usize,
    parent: *mut mi_arena_t,
    total_size: usize,
    numa_node: c_int,
    exclusive: bool,
    memid: mi_memid_t,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> *mut mi_arena_t {
    crate::mi_assert_internal!((start as usize).is_multiple_of(MI_ARENA_SLICE_ALIGN));
    crate::mi_assert_internal!(mi_size_of_slices(slice_count) >= MI_ARENA_MIN_SIZE);

    if slice_count > MI_BITMAP_MAX_BIT_COUNT {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is too large (size {} MiB, maximum is {} MiB)",
            mi_size_of_slices(slice_count) / (1024 * 1024),
            mi_size_of_slices(MI_BITMAP_MAX_BIT_COUNT) / (1024 * 1024)
        ));
        return ptr::null_mut();
    }

    let mut bitmap_base = 0usize;
    let info_slices = unsafe { mi_arena_info_slices_needed(slice_count, &mut bitmap_base) };
    if slice_count < info_slices + 1 {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is not large enough (size {} KiB, minimum required is {} KiB)",
            mi_size_of_slices(slice_count) / 1024,
            mi_size_of_slices(info_slices + 1) / 1024
        ));
        return ptr::null_mut();
    } else if info_slices >= MI_ARENA_BCHUNK_BITS {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is too large with respect to the maximum object size (size {} MiB, meta-info slices {}, maximum object slices are {})",
            mi_size_of_slices(slice_count) / (1024 * 1024),
            info_slices,
            MI_ARENA_BCHUNK_BITS
        ));
        return ptr::null_mut();
    }

    let arena = start.cast::<mi_arena_t>();
    if !memid.initially_committed {
        let mut commit_size = mi_size_of_slices(info_slices);
        if !memid.is_pinned {
            commit_size -= unsafe { crate::os::_mi_os_secure_guard_page_size() };
        }
        let ok = if let Some(commit_fun) = commit_fun {
            unsafe {
                commit_fun(
                    true,
                    arena.cast::<c_void>(),
                    commit_size,
                    ptr::null_mut(),
                    commit_fun_arg,
                )
            }
        } else {
            unsafe { _mi_os_commit(arena.cast::<c_void>(), commit_size, ptr::null_mut()) }
        };
        if !ok {
            crate::options::_mi_warning_message_fmt(format_args!(
                "unable to commit meta-data for OS memory"
            ));
            return ptr::null_mut();
        }
    } else if !memid.is_pinned {
        unsafe {
            crate::os::_mi_os_secure_guard_page_set_before(
                arena
                    .cast::<u8>()
                    .add(mi_size_of_slices(info_slices))
                    .cast::<c_void>(),
                memid,
            )
        };
    }
    if !memid.initially_zero {
        unsafe {
            crate::internal::_mi_memzero(
                arena.cast::<c_void>(),
                mi_size_of_slices(info_slices) - crate::os::_mi_os_secure_guard_page_size(),
            )
        };
    }

    unsafe {
        (*arena.cast::<MiArenaPrefix>()).subproc = subproc;
        (*arena.cast::<MiArenaPrefix>()).memid = memid;
        (*arena.cast::<MiArenaPrefix>()).is_exclusive = exclusive;
        (*arena.cast::<MiArenaPrefix>()).slice_count = slice_count;
        (*arena.cast::<MiArenaPrefix>()).info_slices = info_slices;
        (*arena.cast::<MiArenaPrefix>()).numa_node =
            if numa_node < 0 && mi_option_is_enabled(mi_option_t::mi_option_arena_is_numa_local) {
                crate::os::_mi_os_numa_node()
            } else {
                numa_node
            };
        (*arena.cast::<MiArenaPrefix>())
            .purge_expire
            .store(0, Ordering::Relaxed);
        (*arena.cast::<MiArenaPrefix>()).commit_fun = commit_fun;
        (*arena.cast::<MiArenaPrefix>()).commit_fun_arg = commit_fun_arg;
        (*arena.cast::<MiArenaPrefix>()).parent = parent;
        (*arena.cast::<MiArenaPrefix>()).total_size = total_size;
    }

    let mut base = unsafe { mi_arena_start(arena).add(bitmap_base) };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_free = mi_arena_bbitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_committed =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_dirty = mi_arena_bitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_purge = mi_arena_bitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).pages_main.pages =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    for i in 0..MI_ARENA_BIN_COUNT {
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_main.pages_abandoned[i] =
                mi_arena_bitmap_init(slice_count, &mut base)
        };
    }
    if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_meta = base.cast::<mi_page_t>();
            base = base.add(slice_count * size_of::<mi_page_t>());
        }
    } else {
        unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta = ptr::null_mut() };
    }
    crate::mi_assert_internal!(
        mi_size_of_slices(info_slices)
            >= unsafe { base.offset_from(mi_arena_start(arena)) as usize }
    );

    unsafe {
        mi_bbitmap_unsafe_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            info_slices,
            (*arena.cast::<MiArenaPrefix>()).slice_count - info_slices,
        )
    };
    if memid.initially_committed {
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }
    if !memid.initially_zero {
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }

    if !unsafe { mi_arenas_add(subproc, arena, arena_id) } {
        return ptr::null_mut();
    }
    arena
}

#[allow(dead_code, clippy::too_many_arguments)]
unsafe fn mi_arenas_try_alloc(
    heap: *mut mi_heap_t,
    slice_count: crate::core_types::SliceCount,
    alignment: usize,
    commit: bool,
    allow_large: bool,
    req_arena: *mut mi_arena_t,
    tseq: usize,
    numa_node: c_int,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    crate::mi_assert!(alignment <= MI_ARENA_SLICE_ALIGN);
    if (slice_count.get() as usize).saturating_mul(MI_ARENA_SLICE_SIZE) > MI_ARENA_MAX_SIZE {
        return ptr::null_mut();
    }

    let mut p = unsafe {
        mi_arenas_try_find_free(
            heap,
            slice_count,
            alignment,
            commit,
            allow_large,
            req_arena,
            tseq,
            numa_node,
            memid,
        )
    };
    if !p.is_null() {
        return p;
    }
    if !req_arena.is_null() || _mi_preloading() {
        return ptr::null_mut();
    }
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc) {
        return ptr::null_mut();
    }

    let subproc = unsafe { (*heap.cast::<MiHeapPrefix>()).subproc };
    let arena_count = unsafe { mi_arenas_get_count(subproc) };
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(
            (*subproc.cast::<MiSubprocPrefix>()).arena_reserve_lock
        ))
    };
    if arena_count == unsafe { mi_arenas_get_count(subproc) } {
        let mut arena_id = unsafe { _mi_arena_id_none() };
        let _ = unsafe {
            mi_arena_reserve(
                subproc,
                mi_size_of_slices(slice_count.get() as usize),
                allow_large,
                &mut arena_id,
            )
        };
    }
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(
            (*subproc.cast::<MiSubprocPrefix>()).arena_reserve_lock
        ))
    };

    p = unsafe {
        mi_arenas_try_find_free(
            heap,
            slice_count,
            alignment,
            commit,
            allow_large,
            req_arena,
            tseq,
            numa_node,
            memid,
        )
    };
    p
}

#[allow(dead_code)]
unsafe fn mi_arena_reserve(
    subproc: *mut mi_subproc_t,
    req_size: usize,
    allow_large: bool,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let arena_count = unsafe { mi_arenas_get_count(subproc) };
    if arena_count > (MI_MAX_ARENAS - 4) {
        return false;
    }

    let mut arena_reserve = mi_option_get_size(mi_option_t::mi_option_arena_reserve);
    if arena_reserve == 0 {
        return false;
    }

    if !unsafe { _mi_os_has_virtual_reserve() } {
        arena_reserve /= 4;
    }
    arena_reserve = _mi_align_up(arena_reserve, MI_ARENA_SLICE_SIZE);

    if (1..=128).contains(&arena_count) {
        let multiplier = 1usize << _mi_clamp(arena_count / 8, 0, 16);
        let mut reserve = 0usize;
        if !mi_mul_overflow(multiplier, arena_reserve, &mut reserve) {
            arena_reserve = reserve;
        }
    }

    let req_size = _mi_align_up(
        req_size.saturating_add(MI_ARENA_MAX_CHUNK_OBJ_SIZE),
        MI_ARENA_MAX_CHUNK_OBJ_SIZE,
    );
    if arena_reserve < req_size {
        arena_reserve = req_size;
    }

    let min_reserve = MI_ARENA_MIN_SIZE;
    let max_reserve = MI_ARENA_MAX_SIZE;
    if arena_reserve < min_reserve {
        arena_reserve = min_reserve;
    } else if arena_reserve > max_reserve {
        arena_reserve = max_reserve;
    }
    if arena_reserve < req_size {
        return false;
    }

    let overcommit = unsafe { _mi_os_has_overcommit() };
    let mut arena_commit = false;
    if mi_option_get(mi_option_t::mi_option_arena_eager_commit) == 2 {
        arena_commit =
            overcommit || mi_option_is_enabled(mi_option_t::mi_option_allow_large_os_pages);
    } else if mi_option_get(mi_option_t::mi_option_arena_eager_commit) == 1 {
        arena_commit = true;
    }

    let adjust = overcommit && arena_commit;
    if adjust {
        unsafe {
            __mi_stat_adjust_decrease_mt(
                ptr::addr_of_mut!((*subproc).stats.committed),
                arena_reserve,
            );
        }
    }

    let mut err = unsafe {
        mi_reserve_os_memory_ex2(
            subproc,
            arena_reserve,
            arena_commit,
            allow_large,
            false,
            arena_id,
        )
    };
    if err != 0 {
        if adjust {
            unsafe {
                __mi_stat_adjust_increase_mt(
                    ptr::addr_of_mut!((*subproc).stats.committed),
                    arena_reserve,
                );
            }
        }
        let small_arena_reserve = 4 * MI_ARENA_MIN_SIZE;
        if arena_reserve > small_arena_reserve && small_arena_reserve > req_size {
            if adjust {
                unsafe {
                    __mi_stat_adjust_decrease_mt(
                        ptr::addr_of_mut!((*subproc).stats.committed),
                        small_arena_reserve,
                    );
                }
            }
            err = unsafe {
                mi_reserve_os_memory_ex2(
                    subproc,
                    small_arena_reserve,
                    arena_commit,
                    allow_large,
                    false,
                    arena_id,
                )
            };
            if err != 0 && adjust {
                unsafe {
                    __mi_stat_adjust_increase_mt(
                        ptr::addr_of_mut!((*subproc).stats.committed),
                        small_arena_reserve,
                    );
                }
            }
        }
    }
    err == 0
}

#[allow(dead_code)]
unsafe fn mi_arena_os_alloc_aligned(
    size: usize,
    alignment: usize,
    align_offset: usize,
    commit: bool,
    allow_large: bool,
    req_arena_id: mi_arena_id_t,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc)
        || req_arena_id != unsafe { _mi_arena_id_none() }
    {
        unsafe { mi_set_errno(ENOMEM) };
        return ptr::null_mut();
    }

    if align_offset > 0 {
        unsafe {
            _mi_os_alloc_aligned_at_offset(
                size,
                alignment,
                align_offset,
                commit,
                allow_large,
                memid,
            )
        }
    } else {
        unsafe { _mi_os_alloc_aligned(size, alignment, commit, allow_large, memid) }
    }
}

/// # Safety
/// Internal arena allocator wrapper following the C ABI.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_alloc_aligned(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
    align_offset: usize,
    commit: bool,
    allow_large: bool,
    req_arena: *mut mi_arena_t,
    tseq: usize,
    numa_node: c_int,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    crate::mi_assert_internal!(!memid.is_null());
    crate::mi_assert_internal!(size > 0);

    if !mi_option_is_enabled(mi_option_t::mi_option_disallow_arena_alloc)
        && size >= MI_ARENA_MIN_OBJ_SIZE
        && size <= unsafe { mi_arena_max_object_size() }
        && alignment <= MI_ARENA_SLICE_ALIGN
        && align_offset == 0
    {
        let slice_count = mi_slice_count_of_size(size);
        if let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) {
            let p = unsafe {
                mi_arenas_try_alloc(
                    heap,
                    slice_count,
                    alignment,
                    commit,
                    allow_large,
                    req_arena,
                    tseq,
                    numa_node,
                    memid,
                )
            };
            if !p.is_null() {
                return p;
            }
        }
    }

    unsafe {
        mi_arena_os_alloc_aligned(
            size,
            alignment,
            align_offset,
            commit,
            allow_large,
            req_arena.cast::<c_void>(),
            memid,
        )
    }
}

/// # Safety
/// Internal arena allocator wrapper following the C ABI.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_alloc(
    heap: *mut mi_heap_t,
    size: usize,
    commit: bool,
    allow_large: bool,
    req_arena: *mut mi_arena_t,
    tseq: usize,
    numa_node: c_int,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    unsafe {
        _mi_arenas_alloc_aligned(
            heap,
            size,
            MI_ARENA_SLICE_SIZE,
            0,
            commit,
            allow_large,
            req_arena,
            tseq,
            numa_node,
            memid,
        )
    }
}

#[allow(dead_code)]
unsafe fn mi_abandoned_page_unown(page: *mut mi_page_t, current_theap: *mut mi_theap_t) -> bool {
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(unsafe { _mi_thread_id() == (*(*current_theap).tld).thread_id });

    let mut tf_old: mi_thread_free_t = unsafe {
        mi_atomic_load_explicit(
            ptr::addr_of!((*page).xthread_free).cast(),
            mi_memory_order_e::mi_memory_order_relaxed,
        )
    };
    loop {
        crate::mi_assert_internal!(mi_tf_is_owned(tf_old));
        while !mi_tf_block(tf_old).is_null() {
            unsafe { _mi_page_free_collect(page, false) };
            if unsafe { mi_page_all_free(page) } {
                unsafe { _mi_arenas_page_unabandon(page, current_theap) };
                unsafe { _mi_arenas_page_free(page, current_theap) };
                return true;
            }
            tf_old = unsafe {
                mi_atomic_load_explicit(
                    ptr::addr_of!((*page).xthread_free).cast(),
                    mi_memory_order_e::mi_memory_order_relaxed,
                )
            };
        }
        crate::mi_assert_internal!(mi_tf_block(tf_old).is_null());
        let tf_new = mi_tf_create(ptr::null_mut(), false);
        let mut expected = tf_old;
        if unsafe {
            mi_atomic_cas_weak_acq_rel(
                ptr::addr_of_mut!((*page).xthread_free).cast(),
                &mut expected,
                tf_new,
            )
        } {
            return false;
        }
        tf_old = expected;
    }
}

#[allow(dead_code)]
unsafe extern "C" fn mi_arena_try_claim_abandoned(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_abandoned: *mut bool,
) -> bool {
    crate::mi_assert_internal!(
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let page = {
        let index = slice_index;
        let page_addr: usize;
        unsafe {
            core::arch::asm!(
                "mov {page_addr}, qword ptr [{arena} + {pages_meta_offset}]",
                "test {page_addr}, {page_addr}",
                "je 2f",
                "imul {index}, {index}, {page_size}",
                "add {page_addr}, {index}",
                "jmp 3f",
                "2:",
                "shl {index}, {slice_shift}",
                "lea {page_addr}, [{arena} + {index}]",
                "3:",
                arena = in(reg) arena,
                index = inout(reg) index => _,
                page_addr = out(reg) page_addr,
                pages_meta_offset = const offset_of!(MiArenaPrefix, pages_meta),
                page_size = const size_of::<mi_page_t>(),
                slice_shift = const MI_ARENA_SLICE_SIZE.trailing_zeros(),
                options(nostack, readonly)
            );
        }
        page_addr as *mut mi_page_t
    };
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    let page = {
        let pages_meta = unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta };
        if !pages_meta.is_null() {
            unsafe { pages_meta.add(slice_index) }
        } else {
            unsafe {
                arena
                    .cast::<u8>()
                    .add(slice_index * MI_ARENA_SLICE_SIZE)
                    .cast::<mi_page_t>()
            }
        }
    };
    if !unsafe { mi_page_claim_ownership(page) } {
        unsafe { *keep_abandoned = true };
        false
    } else {
        unsafe { *keep_abandoned = false };
        true
    }
}

#[allow(dead_code)]
unsafe fn mi_heap_ensure_arena_pages(
    heap: *mut mi_heap_t,
    arena: *mut mi_arena_t,
) -> *mut mi_arena_pages_t {
    crate::mi_assert_internal!(!arena.is_null());
    crate::mi_assert_internal!(!heap.is_null());
    let arena_idx = unsafe { (*arena.cast::<MiArenaPrefix>()).arena_idx };
    crate::mi_assert!(arena_idx < MI_MAX_ARENAS);

    let mut arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
    if arena_pages.is_null() {
        unsafe {
            mi_lock_acquire(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).arena_pages_lock
            ))
        };
        arena_pages = unsafe { mi_heap_arena_pages(heap, arena) };
        if arena_pages.is_null() {
            if unsafe { _mi_is_heap_main(heap) } {
                arena_pages =
                    unsafe { ptr::addr_of_mut!((*arena.cast::<MiArenaPrefix>()).pages_main) };
            } else {
                arena_pages = unsafe { mi_arena_pages_alloc(arena) };
            }
            unsafe {
                (*heap.cast::<MiHeapPrefix>()).arena_pages[arena_idx]
                    .store(arena_pages.cast::<c_void>(), Ordering::Release)
            };
        }
        unsafe {
            mi_lock_release(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).arena_pages_lock
            ))
        };
    }
    if unsafe { _mi_is_heap_main(heap) } {
        crate::mi_assert!(!arena_pages.is_null());
    }
    arena_pages
}

#[inline]
unsafe fn mi_page_is_full_local(page: *const mi_page_t) -> bool {
    let full = unsafe { (*page).reserved == (*page).used };
    crate::mi_assert_internal!(!full || unsafe { (*page).free.is_null() });
    full
}

unsafe fn mi_arenas_page_try_find_abandoned(
    theap: *mut mi_theap_t,
    slice_count: usize,
    block_size: usize,
) -> *mut mi_page_t {
    let heap = unsafe { _mi_theap_heap(theap) };
    let tseq = unsafe { (*(*theap).tld).thread_seq };
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let _ = slice_count;

    let bin = _mi_bin_typed(block_size).as_usize();
    crate::mi_assert_internal!(bin < MI_BIN_COUNT);
    if unsafe { (*heap.cast::<MiHeapPrefix>()).abandoned_count[bin].load(Ordering::Relaxed) } == 0 {
        return ptr::null_mut();
    }

    let any_numa = -1;
    let mut result = ptr::null_mut();
    let mut claim_fun: crate::types::mi_claim_fun_t = mi_arena_try_claim_abandoned;
    unsafe {
        mi_forall_suitable_arenas(heap, req_arena, tseq, true, any_numa, true, |arena| {
            let arena_pages = mi_heap_arena_pages(heap, arena);
            if arena_pages.is_null() {
                return true;
            }
            let mut slice_index = 0usize;
            let bitmap = (*arena_pages).pages_abandoned[bin];
            if mi_bitmap_try_find_and_claim(
                bitmap,
                tseq,
                &mut slice_index,
                &mut claim_fun,
                arena.cast(),
            ) {
                let page = mi_arena_page_at_slice(arena, slice_index);
                crate::mi_assert_internal!(mi_page_is_owned(page));
                crate::mi_assert_internal!(mi_page_is_abandoned(page));
                crate::mi_assert_internal!(mi_heap_has_page(heap, arena, page));
                (*heap.cast::<MiHeapPrefix>()).abandoned_count[bin].fetch_sub(1, Ordering::Relaxed);
                (*theap).stats.pages_reclaim_on_alloc.total += 1;
                crate::internal::__mi_stat_decrease(
                    ptr::addr_of_mut!((*theap).stats.pages_abandoned),
                    1,
                );
                _mi_page_free_collect(page, false);
                crate::mi_assert_internal!(mi_bbitmap_is_clearN(
                    (*arena.cast::<MiArenaPrefix>()).slices_free,
                    slice_index,
                    slice_count,
                ));
                crate::mi_assert_internal!(
                    (*page).slice_committed > 0
                        || mi_bitmap_is_setN(
                            (*arena.cast::<MiArenaPrefix>()).slices_committed,
                            slice_index,
                            slice_count,
                        )
                );
                crate::mi_assert_internal!(mi_bitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                    slice_index,
                    slice_count,
                ));
                crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
                    mi_page_slice_start(page).cast(),
                    MI_PAGE_ALIGN
                ));
                crate::mi_assert_internal!(
                    crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
                );
                crate::mi_assert_internal!(mi_page_block_size(page) == block_size);
                crate::mi_assert_internal!(!mi_page_is_full_local(page));
                result = page;
                false
            } else {
                true
            }
        })
    };
    result
}

unsafe fn mi_arenas_page_alloc_fresh_area(
    theap: *mut mi_theap_t,
    slice_count: usize,
    block_size: usize,
    block_alignment: usize,
    os_align: bool,
    commit: bool,
    memid: *mut mi_memid_t,
) -> *mut u8 {
    let _ = block_size;
    let allow_large = true;
    let page_alignment = MI_ARENA_SLICE_ALIGN;
    let heap = unsafe { _mi_theap_heap(theap) };
    let tld = unsafe { (*theap).tld };
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let numa_node = if unsafe { (*heap.cast::<MiHeapPrefix>()).numa_node } >= 0 {
        unsafe { (*heap.cast::<MiHeapPrefix>()).numa_node }
    } else {
        unsafe { (*tld).numa_node }
    };

    let mut start = ptr::null_mut::<u8>();
    unsafe { *memid = _mi_memid_none() };
    let alloc_size = mi_size_of_slices(slice_count);
    if !mi_option_is_enabled(mi_option_t::mi_option_disallow_arena_alloc)
        && !os_align
        && slice_count <= unsafe { mi_arena_max_object_size() } / MI_ARENA_SLICE_SIZE
    {
        if let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count) {
            start = unsafe {
                mi_arenas_try_alloc(
                    heap,
                    typed_slice_count,
                    page_alignment,
                    commit,
                    allow_large,
                    req_arena,
                    (*tld).thread_seq,
                    numa_node,
                    memid,
                )
                .cast::<u8>()
            };
        }
        if !start.is_null() {
            match crate::core_types::MemId::view(unsafe { *memid }) {
                crate::core_types::MemIdView::Arena(a) => {
                    let arena = a.arena.as_ptr();
                    let arena_pages = unsafe { mi_heap_ensure_arena_pages(heap, arena) };
                    if arena_pages.is_null() {
                        unsafe {
                            _mi_arenas_free(start.cast(), mi_size_of_slices(slice_count), *memid)
                        };
                        start = ptr::null_mut();
                    } else {
                        let slice_idx = a.range.start().0;
                        crate::mi_assert_internal!(unsafe {
                            !mi_bitmap_is_setN((*arena_pages).pages, slice_idx, 1)
                        });
                        unsafe { mi_bitmap_set((*arena_pages).pages, slice_idx) };
                    }
                }
                _ => {
                    // Fallback for non-arena allocations that bypassed the arena path above.
                }
            }
        }
    }

    if start.is_null() {
        start = if os_align {
            unsafe {
                mi_arena_os_alloc_aligned(
                    alloc_size,
                    block_alignment,
                    page_alignment,
                    commit,
                    allow_large,
                    req_arena.cast(),
                    memid,
                )
                .cast::<u8>()
            }
        } else {
            unsafe {
                mi_arena_os_alloc_aligned(
                    alloc_size,
                    page_alignment,
                    0,
                    commit,
                    allow_large,
                    req_arena.cast(),
                    memid,
                )
                .cast::<u8>()
            }
        };
    }

    if start.is_null() {
        return ptr::null_mut();
    }
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(start.cast(), MI_PAGE_ALIGN));
    crate::mi_assert_internal!(
        !os_align
            || crate::internal::_mi_is_aligned(
                unsafe { start.add(page_alignment) }.cast(),
                block_alignment
            )
    );
    start
}

unsafe fn mi_page_block_start(block_size: usize, os_align: bool) -> usize {
    if os_align {
        MI_PAGE_ALIGN
    } else if crate::internal::_mi_is_power_of_two(block_size)
        && block_size <= MI_PAGE_BLOCK_START_MAX_OFFSET
    {
        _mi_align_up(crate::internal::mi_page_info_size(), block_size)
    } else if block_size != 0 && block_size.is_multiple_of(MI_PAGE_OSPAGE_BLOCK_ALIGN2) {
        _mi_align_up(
            crate::internal::mi_page_info_size(),
            MI_PAGE_OSPAGE_BLOCK_ALIGN2,
        )
    } else {
        crate::internal::mi_page_info_size()
    }
}

unsafe fn mi_arenas_page_alloc_fresh(
    theap: *mut mi_theap_t,
    slice_count: usize,
    block_size: usize,
    block_alignment: usize,
    commit: bool,
) -> *mut mi_page_t {
    let os_align = block_alignment > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let alloc_size = mi_size_of_slices(slice_count);
    let mut memid = unsafe { _mi_memid_none() };
    let slice_start = unsafe {
        mi_arenas_page_alloc_fresh_area(
            theap,
            slice_count,
            block_size,
            block_alignment,
            os_align,
            commit,
            &mut memid,
        )
    };
    if slice_start.is_null() {
        return ptr::null_mut();
    }

    let page_noguard_size = alloc_size;
    let mut page = ptr::null_mut::<mi_page_t>();
    let mut page_meta_is_separate = false;
    let mut block_start = 0usize;

    if let crate::core_types::MemIdView::Arena(a) = crate::core_types::MemId::view(memid) {
        let arena = a.arena.as_ptr();
        if unsafe { !(*arena.cast::<MiArenaPrefix>()).pages_meta.is_null() } {
            let page_meta = unsafe {
                (*arena.cast::<MiArenaPrefix>())
                    .pages_meta
                    .add(a.range.start().0)
            };
            crate::mi_assert_internal!(unsafe { (*page_meta).block_size == 0 });
            if block_size > MI_SMALL_SIZE_MAX || MI_PAGE_META_ALIGNED_FREE_SMALL == 0 {
                page = page_meta;
                page_meta_is_separate = true;
                block_start = 0;
                if block_size >= size_of::<usize>()
                    && block_size <= MI_PAGE_BLOCK_START_MAX_OFFSET
                    && crate::internal::_mi_is_power_of_two(block_size)
                {
                    block_start += block_size;
                }
                unsafe {
                    crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>())
                };
            }
        }
    }
    if page.is_null() {
        page = slice_start.cast::<mi_page_t>();
        block_start = unsafe { mi_page_block_start(block_size, os_align) };
    }

    let mut commit_size = 0usize;
    if !memid.initially_committed {
        commit_size = _mi_align_up(block_start + block_size, MI_PAGE_MIN_COMMIT_SIZE);
        if commit_size > page_noguard_size {
            commit_size = page_noguard_size;
        }
        let mut is_zero = false;
        if !unsafe {
            let arena_ptr = match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => a.arena.as_ptr(),
                _ => ptr::null_mut(),
            };
            mi_arena_commit(arena_ptr, slice_start.cast(), commit_size, &mut is_zero, 0)
        } {
            unsafe { _mi_arenas_free(slice_start.cast(), alloc_size, memid) };
            return ptr::null_mut();
        }
    }
    if !memid.initially_zero && !page_meta_is_separate {
        unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
    }

    if memid.initially_committed {
        if memid.initially_zero {
            unsafe { mi_track_mem_defined(slice_start.cast(), slice_count * MI_ARENA_SLICE_SIZE) };
        } else {
            unsafe {
                mi_track_mem_undefined(slice_start.cast(), slice_count * MI_ARENA_SLICE_SIZE)
            };
        }
    }
    if MI_DEBUG > 1
        && memid.initially_zero
        && memid.initially_committed
        && !unsafe { mi_mem_is_zero(slice_start.cast(), page_noguard_size) }
    {
        crate::options::_mi_error_message_fmt(
            EFAULT,
            format_args!("internal error: page memory was not zero initialized.\n"),
        );
        memid.initially_zero = false;
        if block_start > 0 {
            unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
        }
    }
    let reserved = if os_align {
        1
    } else {
        (page_noguard_size - block_start) / block_size
    };
    crate::mi_assert_internal!(reserved > 0 && u16::try_from(reserved).is_ok());

    unsafe { (*page).reserved = reserved as u16 };
    unsafe { (*page).page_start = slice_start.add(block_start) };
    unsafe { (*page).block_size = block_size };
    unsafe { (*page).slice_committed = commit_size };
    unsafe { (*page).memid = memid };
    unsafe { (*page).free_is_zero = memid.initially_zero };
    crate::mi_assert_internal!(unsafe { (*page).free.is_null() });
    crate::mi_assert_internal!(page_meta_is_separate == unsafe { mi_page_meta_is_separated(page) });
    crate::mi_assert_internal!(unsafe { mi_page_slice_start(page) } == slice_start);

    unsafe { (*page).xthread_free = 0 };
    unsafe { mi_page_claim_ownership(page) };
    if !unsafe { _mi_page_map_register(page) } {
        unsafe { _mi_arenas_free(slice_start.cast(), alloc_size, memid) };
        return ptr::null_mut();
    }

    unsafe { __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.pages), 1) };
    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    unsafe { __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.page_bins[stats_bin]), 1) };

    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::mi_assert_internal!(unsafe { mi_page_block_size(page) } == block_size);
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    page
}

unsafe fn mi_arenas_page_regular_alloc(
    theap: *mut mi_theap_t,
    slice_count: usize,
    block_size: usize,
) -> *mut mi_page_t {
    let mut page = unsafe { mi_arenas_page_try_find_abandoned(theap, slice_count, block_size) };
    if !page.is_null() {
        return page;
    }

    let commit_on_demand =
        crate::options::mi_option_get(mi_option_t::mi_option_page_commit_on_demand);
    let commit = slice_count <= mi_slice_count_of_size(MI_PAGE_MIN_COMMIT_SIZE)
        || (commit_on_demand == 2 && unsafe { _mi_os_has_overcommit() })
        || commit_on_demand == 0;
    page = unsafe { mi_arenas_page_alloc_fresh(theap, slice_count, block_size, 1, commit) };
    if page.is_null() {
        return ptr::null_mut();
    }
    crate::mi_assert_internal!(
        match crate::core_types::MemId::view(unsafe { (*page).memid }) {
            crate::core_types::MemIdView::Arena(a) =>
                a.range.count().0.get() as usize == slice_count,
            _ => true, // Non-arena allocations trivially pass this slice_count assert
        }
    );
    if !unsafe { crate::page::_mi_page_init(theap, page) } {
        unsafe { _mi_arenas_free(page.cast(), mi_page_full_size(page), (*page).memid) };
        return ptr::null_mut();
    }
    page
}

#[inline(never)]
unsafe fn mi_arenas_page_singleton_alloc(
    theap: *mut mi_theap_t,
    block_size: usize,
    block_alignment: usize,
) -> *mut mi_page_t {
    let os_align = block_alignment > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let info_size = if os_align {
        MI_PAGE_ALIGN
    } else {
        crate::internal::mi_page_info_size()
    };
    let slice_count = mi_slice_count_of_size(info_size + block_size);
    let page = unsafe {
        mi_arenas_page_alloc_fresh(theap, slice_count, block_size, block_alignment, true)
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    crate::mi_assert_internal!(unsafe { (*page).reserved == 1 });
    if !unsafe { crate::page::_mi_page_init(theap, page) } {
        unsafe { _mi_arenas_free(page.cast(), mi_page_full_size(page), (*page).memid) };
        return ptr::null_mut();
    }
    page
}

/// # Safety
/// Internal arena page allocator matching the C ABI.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_page_alloc(
    theap: *mut mi_theap_t,
    block_size: usize,
    block_alignment: usize,
) -> *mut mi_page_t {
    let page = if block_alignment > MI_PAGE_MAX_OVERALLOC_ALIGN {
        crate::mi_assert_internal!(crate::internal::_mi_is_power_of_two(block_alignment));
        unsafe { mi_arenas_page_singleton_alloc(theap, block_size, block_alignment) }
    } else if block_size <= MI_SMALL_MAX_OBJ_SIZE {
        unsafe {
            mi_arenas_page_regular_alloc(
                theap,
                mi_slice_count_of_size(MI_SMALL_PAGE_SIZE),
                block_size,
            )
        }
    } else if block_size <= MI_MEDIUM_MAX_OBJ_SIZE {
        unsafe {
            mi_arenas_page_regular_alloc(
                theap,
                mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE),
                block_size,
            )
        }
    } else if MI_ENABLE_LARGE_PAGES != 0 && block_size <= MI_LARGE_MAX_OBJ_SIZE {
        unsafe {
            mi_arenas_page_regular_alloc(
                theap,
                mi_slice_count_of_size(MI_LARGE_PAGE_SIZE),
                block_size,
            )
        }
    } else {
        unsafe { mi_arenas_page_singleton_alloc(theap, block_size, block_alignment) }
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::mi_assert_internal!(
        block_alignment <= MI_PAGE_MAX_OVERALLOC_ALIGN
            || crate::internal::_mi_is_aligned(
                unsafe { mi_page_start(page) }.cast(),
                block_alignment
            )
    );
    page
}

/// # Safety
/// Internal arena page free path matching the C ABI.
pub(crate) unsafe extern "C" fn _mi_arenas_page_free(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_all_free(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(unsafe { (*page).next.is_null() && (*page).prev.is_null() });

    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    if !current_theapx.is_null() {
        unsafe {
            crate::internal::__mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.page_bins[stats_bin]),
                1,
            )
        };
        unsafe {
            crate::internal::__mi_stat_decrease(ptr::addr_of_mut!((*current_theapx).stats.pages), 1)
        };
    } else {
        let heap = unsafe { mi_page_heap(page) };
        unsafe {
            crate::internal::__mi_stat_decrease_mt(&mut (*heap).stats.page_bins[stats_bin], 1)
        };
        unsafe { crate::internal::__mi_stat_decrease_mt(&mut (*heap).stats.pages, 1) };
    }

    if MI_DEBUG > 1
        && unsafe { mi_page_memid_is_arena(page) }
        && !unsafe { mi_page_is_full_local(page) }
    {
        let bin = _mi_bin_typed(unsafe { mi_page_block_size(page) }).as_usize();
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        let arena = unsafe {
            mi_page_arena_pages(page, &mut slice_index, &mut slice_count, &mut arena_pages)
        };
        crate::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        crate::mi_assert_internal!(
            unsafe { (*page).slice_committed > 0 }
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        crate::mi_assert_internal!(!unsafe {
            mi_bitmap_is_setN((*arena_pages).pages_abandoned[bin], slice_index, 1)
        });
        crate::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN((*arena_pages).pages, slice_index, 1)
        });
    }

    unsafe { _mi_page_map_unregister(page) };
    if unsafe { mi_page_memid_is_arena(page) } {
        let mut arena_pages = ptr::null_mut();
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let arena = unsafe {
            mi_page_arena_pages(page, &mut slice_index, &mut slice_count, &mut arena_pages)
        };
        crate::mi_assert_internal!(!arena_pages.is_null());
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        if unsafe { (*page).slice_committed > 0 } {
            let total_slices = unsafe { (*page).slice_committed / MI_ARENA_SLICE_SIZE };
            crate::mi_assert_internal!(slice_count >= total_slices);
            if total_slices > 0 {
                unsafe {
                    mi_bitmap_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        total_slices,
                        ptr::null_mut(),
                    )
                };
            }
            let extra = unsafe { (*page).slice_committed % MI_ARENA_SLICE_SIZE };
            if extra > 0 {
                unsafe {
                    crate::internal::__mi_stat_decrease_mt(
                        &mut (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed,
                        extra,
                    )
                };
            }
        } else {
            crate::mi_assert_internal!(unsafe {
                mi_bitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            });
        }
    }
    if unsafe { mi_page_meta_is_separated(page) } {
        unsafe { (*page).block_size = 0 };
    }
    unsafe {
        _mi_arenas_free(
            mi_page_slice_start(page).cast(),
            mi_page_full_size(page),
            (*page).memid,
        )
    };
}

/// # Safety
/// Internal arena abandon path matching the C ABI.
pub(crate) unsafe extern "C" fn _mi_arenas_page_abandon(
    page: *mut mi_page_t,
    current_theap: *mut mi_theap_t,
) {
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(!unsafe { mi_page_all_free(page) });
    crate::mi_assert_internal!(unsafe { (*page).next.is_null() && (*page).prev.is_null() });

    let heap = unsafe { mi_page_heap(page) };
    crate::mi_assert_internal!(heap == unsafe { _mi_theap_heap(current_theap) });
    if unsafe { mi_page_memid_is_arena(page) } && !unsafe { mi_page_is_full_local(page) } {
        let bin = _mi_bin_typed(unsafe { mi_page_block_size(page) }).as_usize();
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        let arena = unsafe {
            mi_page_arena_pages(page, &mut slice_index, &mut slice_count, &mut arena_pages)
        };
        let _ = arena;
        crate::mi_assert_internal!(!unsafe { mi_page_is_singleton(page) });
        crate::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        crate::mi_assert_internal!(
            unsafe { (*page).slice_committed > 0 }
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        crate::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index,
                slice_count,
            )
        });

        // Preserve current page flags while switching the abandoned thread id state.
        let flags = unsafe { crate::internal::mi_page_flags(page) };
        unsafe { (*page).xthread_id = MI_THREADID_ABANDONED_MAPPED | flags };
        let was_clear = unsafe { mi_bitmap_set((*arena_pages).pages_abandoned[bin], slice_index) };
        crate::MI_UNUSED!(was_clear);
        crate::mi_assert_internal!(was_clear);
        unsafe {
            (*heap.cast::<MiHeapPrefix>()).abandoned_count[bin].fetch_add(1, Ordering::Relaxed)
        };
        unsafe { __mi_stat_increase(ptr::addr_of_mut!((*current_theap).stats.pages_abandoned), 1) };
    } else {
        if !unsafe { mi_page_memid_is_arena(page) }
            && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned)
        {
            unsafe {
                mi_lock_acquire(ptr::addr_of_mut!(
                    (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
                ))
            };
            unsafe {
                (*page).prev = ptr::null_mut();
                (*page).next = (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages;
                if !(*page).next.is_null() {
                    (*(*page).next).prev = page;
                }
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages = page;
            }
            unsafe {
                mi_lock_release(ptr::addr_of_mut!(
                    (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
                ))
            };
        }
        unsafe { __mi_stat_increase(ptr::addr_of_mut!((*current_theap).stats.pages_abandoned), 1) };
    }
    unsafe { mi_abandoned_page_unown(page, current_theap) };
}

/// # Safety
/// Called from the free path on pages already associated with an arena.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_page_try_reabandon_to_mapped(
    page: *mut mi_page_t,
) -> bool {
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(!unsafe { mi_page_is_abandoned_mapped(page) });
    crate::mi_assert_internal!(!unsafe { mi_page_is_full_local(page) });
    crate::mi_assert_internal!(!unsafe { mi_page_all_free(page) });
    crate::mi_assert_internal!(!unsafe { mi_page_is_singleton(page) });

    if unsafe { mi_page_is_full_local(page) }
        || unsafe { mi_page_is_abandoned_mapped(page) }
        || !unsafe { mi_page_memid_is_arena(page) }
    {
        return false;
    }

    let theap = unsafe { _mi_page_associated_theap_peek(page) };
    if theap.is_null() {
        return false;
    }

    unsafe {
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reabandon_full), 1)
    };
    unsafe { __mi_stat_adjust_decrease(ptr::addr_of_mut!((*theap).stats.pages_abandoned), 1) };
    unsafe { _mi_arenas_page_abandon(page, theap) };
    true
}

/// # Safety
/// Called from `free` paths while racing with abandoned-page readers.
pub(crate) unsafe extern "C" fn _mi_arenas_page_unabandon(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::mi_assert_internal!(
        current_theapx.is_null()
            || unsafe { _mi_thread_id() == (*(*current_theapx).tld).thread_id }
    );

    let heap = unsafe { mi_page_heap(page) };
    if unsafe { mi_page_is_abandoned_mapped(page) } {
        crate::mi_assert_internal!(unsafe { mi_page_memid_is_arena(page) });
        let bin = _mi_bin_typed(unsafe { mi_page_block_size(page) }).as_usize();
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        let arena = unsafe {
            mi_page_arena_pages(page, &mut slice_index, &mut slice_count, &mut arena_pages)
        };
        let _ = arena;
        crate::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        crate::mi_assert_internal!(
            unsafe { (*page).slice_committed > 0 }
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        unsafe { mi_bitmap_clear_once_set((*arena_pages).pages_abandoned[bin], slice_index) };
        unsafe { mi_page_clear_abandoned_mapped(page) };
        unsafe {
            (*heap.cast::<MiHeapPrefix>()).abandoned_count[bin].fetch_sub(1, Ordering::Relaxed)
        };
    } else if !unsafe { mi_page_memid_is_arena(page) }
        && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned)
    {
        unsafe {
            mi_lock_acquire(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
            ))
        };
        unsafe {
            if !(*page).prev.is_null() {
                (*(*page).prev).next = (*page).next;
            }
            if !(*page).next.is_null() {
                (*(*page).next).prev = (*page).prev;
            }
            if (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages == page {
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages = (*page).next;
            }
            (*page).next = ptr::null_mut();
            (*page).prev = ptr::null_mut();
        };
        unsafe {
            mi_lock_release(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
            ))
        };
    }

    if !current_theapx.is_null() {
        unsafe {
            __mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.pages_abandoned),
                1,
            )
        };
    } else {
        unsafe { __mi_stat_decrease_mt(&mut (*heap).stats.pages_abandoned, 1) };
    }
}

unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) {
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    let delay = unsafe { mi_arena_purge_delay() };
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } || delay < 0 || _mi_preloading()
    {
        return;
    }
    crate::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });
    if delay == 0 {
        unsafe { mi_arena_purge(arena, range) };
        return;
    }

    let expire = crate::stats::_mi_clock_now() + delay as i64;
    let mut expire0 = 0i64;
    if unsafe {
        mi_atomic_casi64_strong_acq_rel(
            ptr::addr_of_mut!((*arena.cast::<MiArenaPrefix>()).purge_expire).cast::<i64>(),
            &mut expire0,
            expire,
        )
    } {
        crate::mi_assert_internal!(expire0 == 0);
        let subproc = unsafe { (*arena.cast::<MiArenaPrefix>()).subproc };
        unsafe {
            mi_atomic_casi64_strong_acq_rel(
                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire).cast::<i64>(),
                &mut expire0,
                expire,
            )
        };
    }
    unsafe {
        mi_bitmap_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_purge,
            slice_index,
            slice_count,
            ptr::null_mut(),
        )
    };
}

/// # Safety
/// Internal arena free path matching the C ABI.
pub(crate) unsafe extern "C" fn _mi_arenas_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    if p.is_null() || size == 0 {
        return;
    }

    unsafe { mi_track_mem_undefined(p, size) };
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Os(_) => {
            unsafe { _mi_os_free(p, size, memid) };
        }
        crate::core_types::MemIdView::Arena(arena_memid) => {
            let arena = arena_memid.arena.as_ptr();
            let range = arena_memid.range;
            let slice_index = range.start_usize();
            let slice_count = range.count_usize();
            crate::mi_assert_internal!(size.is_multiple_of(MI_ARENA_SLICE_SIZE));
            crate::mi_assert_internal!(slice_count * MI_ARENA_SLICE_SIZE == size);
            crate::mi_assert_internal!(
                slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
            );
            crate::mi_assert_internal!(slice_index >= unsafe { mi_arena_info_slices(arena) });
            if slice_index < unsafe { mi_arena_info_slices(arena) }
                || slice_index >= unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
            {
                crate::options::_mi_error_message_fmt(
                    EINVAL,
                    format_args!(
                        "trying to free from an invalid arena block: {:p}, size {}\n",
                        p, size
                    ),
                );
                return;
            }

            if !unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
                unsafe { mi_arena_schedule_purge(arena, range) };
            }

            let all_inuse = unsafe {
                mi_bbitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_free,
                    slice_index,
                    slice_count,
                )
            };
            if !all_inuse {
                crate::options::_mi_error_message_fmt(
                    EAGAIN,
                    format_args!(
                        "trying to free an already freed arena block: {:p}, size {}\n",
                        p, size
                    ),
                );
            }
        }
        crate::core_types::MemIdView::Meta(_) => {
            unsafe { _mi_meta_free(p, size, memid) };
        }
        crate::core_types::MemIdView::None(_)
        | crate::core_types::MemIdView::External(_)
        | crate::core_types::MemIdView::Static(_) => {}
        crate::core_types::MemIdView::HeapMain(_) | crate::core_types::MemIdView::Unknown(_) => {
            crate::mi_assert_internal!(false);
        }
    }
}

unsafe fn mi_arena_purge_delay() -> c_long {
    mi_option_get(mi_option_t::mi_option_purge_delay)
        * mi_option_get(mi_option_t::mi_option_arena_purge_mult)
}

#[inline(never)]
unsafe fn mi_arena_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) -> bool {
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    crate::mi_assert_internal!(!unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned });
    crate::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    });

    let size = mi_size_of_slices(slice_count);
    let p = unsafe { mi_arena_slice_start(arena, slice_index).cast::<c_void>() };
    let mut already_committed = 0usize;
    unsafe {
        mi_bitmap_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_committed,
            slice_index,
            slice_count,
            &mut already_committed,
        )
    };
    let all_committed = already_committed == slice_count;
    let needs_recommit = unsafe {
        crate::internal::_mi_os_purge_ex(
            p,
            size,
            all_committed,
            mi_size_of_slices(already_committed),
            (*arena.cast::<MiArenaPrefix>()).commit_fun,
            (*arena.cast::<MiArenaPrefix>()).commit_fun_arg,
        )
    };

    if needs_recommit || !all_committed {
        unsafe {
            mi_bitmap_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
    }

    needs_recommit
}

#[allow(dead_code)]
unsafe fn mi_arena_try_purge_range(
    arena: *mut mi_arena_t,
    range: crate::core_types::SliceRange,
) -> bool {
    let slice_index = range.start_usize();
    let slice_count = range.count_usize();
    crate::mi_assert!(slice_count < MI_BCHUNK_BITS);
    if unsafe {
        mi_bbitmap_try_clearNC(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            slice_index,
            slice_count,
        )
    } {
        let decommitted = unsafe { mi_arena_purge(arena, range) };
        crate::mi_assert_internal!(
            !decommitted
                || unsafe {
                    mi_bitmap_is_clearN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        unsafe {
            mi_bbitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        };
        true
    } else {
        false
    }
}

#[allow(dead_code)]
unsafe extern "C" fn mi_arena_try_purge_visitor(
    slice_index: usize,
    slice_count: usize,
    arena: *mut mi_arena_t,
    arg: *mut c_void,
) -> bool {
    let vinfo = arg.cast::<mi_purge_visit_info_t>();
    let Some(range) = crate::core_types::SliceRange::new(slice_index, slice_count, unsafe {
        crate::core_types::ArenaPtr::new_unchecked(arena)
    })
    .ok() else {
        crate::mi_assert_internal!(false);
        return true;
    };
    if unsafe { mi_arena_try_purge_range(arena, range) } {
        unsafe {
            (*vinfo).any_purged = true;
            (*vinfo).all_purged = true;
        }
    } else if slice_count > 1 {
        for i in 0..slice_count {
            let Some(single) = crate::core_types::SliceRange::new(slice_index + i, 1, unsafe {
                crate::core_types::ArenaPtr::new_unchecked(arena)
            })
            .ok() else {
                crate::mi_assert_internal!(false);
                unsafe { (*vinfo).all_purged = false };
                continue;
            };
            let purged = unsafe { mi_arena_try_purge_range(arena, single) };
            unsafe {
                (*vinfo).any_purged = (*vinfo).any_purged || purged;
                (*vinfo).all_purged = (*vinfo).all_purged && purged;
            }
        }
    }
    true
}

#[allow(dead_code)]
unsafe fn mi_arena_try_purge(arena: *mut mi_arena_t, now: mi_msecs_t, force: bool) -> c_int {
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return -1;
    }

    let expire = unsafe {
        mi_atomic_loadi64_explicit(
            ptr::addr_of!((*arena.cast::<MiArenaPrefix>()).purge_expire).cast(),
            mi_memory_order_e::mi_memory_order_relaxed,
        )
    };
    if !force {
        if expire == 0 {
            return -1;
        }
        if expire > now {
            return 0;
        }
    }

    unsafe {
        mi_atomic_storei64_release(
            ptr::addr_of_mut!((*arena.cast::<MiArenaPrefix>()).purge_expire).cast(),
            0,
        )
    };
    unsafe {
        __mi_stat_counter_increase_mt(
            ptr::addr_of_mut!(
                (*(*arena.cast::<MiArenaPrefix>()).subproc)
                    .stats
                    .arena_purges
            ),
            1,
        )
    };

    let mut vinfo = mi_purge_visit_info_t {
        now,
        delay: unsafe { mi_arena_purge_delay() },
        all_purged: true,
        any_purged: false,
    };
    let minslices = mi_slice_count_of_size(unsafe { _mi_os_minimal_purge_size() });
    unsafe {
        _mi_bitmap_forall_setc_rangesn(
            (*arena.cast::<MiArenaPrefix>()).slices_purge,
            minslices,
            mi_arena_try_purge_visitor,
            arena,
            ptr::addr_of_mut!(vinfo).cast(),
        )
    };

    if vinfo.any_purged { 1 } else { -1 }
}

#[allow(dead_code)]
unsafe fn mi_arenas_try_purge(
    force: bool,
    visit_all: bool,
    subproc: *mut mi_subproc_t,
    tseq: usize,
) {
    let delay = unsafe { mi_arena_purge_delay() };
    if _mi_preloading() || delay <= 0 {
        return;
    }

    let now = crate::stats::_mi_clock_now();
    let arenas_expire = unsafe {
        mi_atomic_loadi64_acquire(
            ptr::addr_of!((*subproc.cast::<MiSubprocPrefix>()).purge_expire).cast(),
        )
    };
    if !visit_all && !force && (arenas_expire == 0 || arenas_expire > now) {
        return;
    }

    let max_arena = unsafe { mi_arenas_get_count(subproc) };
    if max_arena == 0 {
        return;
    }

    static PURGE_GUARD: mi_atomic_guard_t = AtomicUsize::new(0);
    crate::mi_atomic_guard!(ptr::addr_of!(PURGE_GUARD).cast_mut(), {
        if arenas_expire > now {
            unsafe {
                mi_atomic_storei64_release(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire).cast(),
                    now + ((delay as mi_msecs_t) / 10),
                )
            };
        }
        let arena_start = tseq % max_arena;
        let mut max_purge_count = if visit_all {
            max_arena
        } else {
            (max_arena / 4) + 1
        };
        let mut all_visited = true;
        let mut any_purged = false;
        for arena_offset in 0..max_arena {
            let mut i = arena_offset + arena_start;
            if i >= max_arena {
                i -= max_arena;
            }
            let arena = unsafe { mi_arena_from_index(subproc, i) };
            if !arena.is_null() {
                let purged = unsafe { mi_arena_try_purge(arena, now, force) };
                if purged >= 0 {
                    any_purged = true;
                    if purged >= 1 {
                        if max_purge_count <= 1 {
                            all_visited = false;
                            break;
                        }
                        max_purge_count -= 1;
                    }
                }
            }
        }
        if all_visited && !any_purged {
            unsafe {
                mi_atomic_storei64_release(
                    ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).purge_expire).cast(),
                    0,
                )
            };
        }
    });
}

/// # Safety
/// Purges scheduled arena ranges for the current thread-local context.
pub(crate) unsafe extern "C" fn _mi_arenas_collect(
    force_purge: bool,
    visit_all: bool,
    tld: *mut mi_tld_t,
) {
    unsafe { mi_arenas_try_purge(force_purge, visit_all, (*tld).subproc, (*tld).thread_seq) };
}

unsafe fn mi_reserve_os_memory_ex2(
    subproc: *mut mi_subproc_t,
    size: usize,
    commit: bool,
    allow_large: bool,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    if !arena_id.is_null() {
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    let size = _mi_align_up(size, MI_ARENA_SLICE_SIZE);
    let mut memid = unsafe { _mi_memid_none() };
    let start = unsafe {
        _mi_os_alloc_aligned(size, MI_ARENA_SLICE_ALIGN, commit, allow_large, &mut memid)
    };
    if start.is_null() {
        return ENOMEM;
    }

    if unsafe {
        mi_manage_os_memory_ex2(
            subproc,
            start,
            size,
            -1,
            exclusive,
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    } {
        crate::options::_mi_verbose_message(format_args!(
            "reserved {} KiB memory{}\n",
            _mi_divide_up(size, 1024),
            if memid.is_pinned {
                " (in large os pages)"
            } else {
                ""
            }
        ));
        0
    } else {
        unsafe { _mi_os_free_ex(start, size, commit, memid, ptr::null_mut()) };
        crate::options::_mi_verbose_message(format_args!(
            "failed to reserve {} KiB memory\n",
            _mi_divide_up(size, 1024)
        ));
        ENOMEM
    }
}

/// # Safety
/// Matches the C API and may mutate global arena state through the active sub-process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_os_memory_ex(
    size: usize,
    commit: bool,
    allow_large: bool,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    unsafe {
        mi_reserve_os_memory_ex2(
            _mi_subproc(),
            size,
            commit,
            allow_large,
            exclusive,
            arena_id,
        )
    }
}

/// # Safety
/// Matches the C API and may mutate global arena state through the active sub-process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_os_memory(
    size: usize,
    commit: bool,
    allow_large: bool,
) -> c_int {
    unsafe { mi_reserve_os_memory_ex(size, commit, allow_large, false, ptr::null_mut()) }
}

/// # Safety
/// Matches the C API and forwards to the current arena-management entrypoint.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_manage_os_memory(
    start: *mut c_void,
    size: usize,
    is_committed: bool,
    is_large: bool,
    is_zero: bool,
    numa_node: c_int,
) -> bool {
    unsafe {
        mi_manage_os_memory_ex(
            start,
            size,
            is_committed,
            is_large,
            is_zero,
            numa_node,
            false,
            ptr::null_mut(),
        )
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn mi_manage_os_memory_ex2(
    subproc: *mut mi_subproc_t,
    mut start: *mut c_void,
    mut size: usize,
    numa_node: c_int,
    exclusive: bool,
    mut memid: mi_memid_t,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    if !arena_id.is_null() {
        unsafe { *arena_id = _mi_arena_id_none() };
    }
    if start.is_null() {
        return false;
    }
    if !(start as usize).is_multiple_of(MI_ARENA_SLICE_SIZE) {
        let aligned_start = _mi_align_up(start as usize, MI_ARENA_SLICE_SIZE) as *mut c_void;
        let diff = aligned_start as usize - start as usize;
        if diff >= size || (size - diff) < MI_ARENA_SLICE_SIZE {
            crate::options::_mi_warning_message_fmt(format_args!(
                "after alignment, the size of the arena becomes too small (memory at {:p} with size {})\n",
                start, size
            ));
            return false;
        }
        start = aligned_start;
        size -= diff;
    }

    let mut total_slice_count = (size / MI_ARENA_SLICE_SIZE / MI_BCHUNK_BITS) * MI_BCHUNK_BITS;
    let total_size = mi_size_of_slices(total_slice_count);
    if total_size < MI_ARENA_MIN_SIZE {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is not large enough (size {} KiB, minimum required is {} KiB)",
            size / crate::bits::MI_KiB,
            MI_ARENA_MIN_SIZE / crate::bits::MI_KiB
        ));
        return false;
    }

    let mut parent: *mut mi_arena_t = ptr::null_mut();
    while total_slice_count > 0 {
        let mut slice_count = total_slice_count;
        if slice_count > MI_BITMAP_MAX_BIT_COUNT {
            slice_count = MI_BITMAP_MAX_BIT_COUNT;
        }

        let arena = unsafe {
            mi_arena_initialize(
                subproc,
                start,
                slice_count,
                parent,
                if parent.is_null() { total_size } else { 0 },
                numa_node,
                exclusive,
                memid,
                commit_fun,
                commit_fun_arg,
                if parent.is_null() {
                    arena_id
                } else {
                    ptr::null_mut()
                },
            )
        };
        if arena.is_null() {
            if parent.is_null() {
                return false;
            }
            crate::mi_assert!(
                mi_size_of_slices(total_slice_count)
                    <= unsafe { (*parent.cast::<MiArenaPrefix>()).total_size }
            );
            unsafe {
                (*parent.cast::<MiArenaPrefix>()).total_size -=
                    mi_size_of_slices(total_slice_count);
            }
            return true;
        }

        if parent.is_null() {
            parent = arena;
            memid.memkind = mi_memkind_t::MI_MEM_NONE;
        }
        crate::mi_assert!(slice_count <= total_slice_count);
        total_slice_count -= slice_count;
        start = unsafe {
            start
                .cast::<u8>()
                .add(mi_size_of_slices(slice_count))
                .cast()
        };
    }

    true
}

/// # Safety
/// Matches the C API and may mutate arena state for externally managed memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_manage_os_memory_ex(
    start: *mut c_void,
    size: usize,
    is_committed: bool,
    is_pinned: bool,
    is_zero: bool,
    numa_node: c_int,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let memid = crate::core_types::MemId::<crate::core_types::ExternalMem>::external(
        crate::core_types::OsMemPtr::new(start),
        crate::core_types::ReservedBytes(size),
        crate::core_types::InitiallyCommitted(is_committed),
        crate::core_types::InitiallyZero(is_zero),
        crate::core_types::Pinned(is_pinned),
    )
    .raw;
    unsafe {
        mi_manage_os_memory_ex2(
            _mi_subproc(),
            start,
            size,
            numa_node,
            exclusive,
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    }
}

#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
/// # Safety
/// `start..start+size` must describe caller-owned memory suitable for arena management.
/// `commit_fun` and `arena_id`, when present, must follow the C ABI contract.
pub unsafe extern "C" fn mi_manage_memory(
    start: *mut c_void,
    size: usize,
    is_committed: bool,
    is_pinned: bool,
    is_zero: bool,
    numa_node: c_int,
    exclusive: bool,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let memid = crate::core_types::MemId::<crate::core_types::ExternalMem>::external(
        crate::core_types::OsMemPtr::new(start),
        crate::core_types::ReservedBytes(size),
        crate::core_types::InitiallyCommitted(is_committed),
        crate::core_types::InitiallyZero(is_zero),
        crate::core_types::Pinned(is_pinned),
    )
    .raw;
    unsafe {
        mi_manage_os_memory_ex2(
            _mi_subproc(),
            start,
            size,
            numa_node,
            exclusive,
            memid,
            commit_fun,
            commit_fun_arg,
            arena_id,
        )
    }
}

unsafe fn mi_arena_used_slices(arena: *mut mi_arena_t) -> usize {
    let mut idx = 0usize;
    if unsafe { mi_bbitmap_bsr_inv((*arena.cast::<MiArenaPrefix>()).slices_free, &mut idx) } {
        idx + 1
    } else {
        unsafe { mi_arena_info_slices(arena) }
    }
}

unsafe fn mi_debug_show_bfield(field: mi_bfield_t, buf: *mut u8, k: *mut usize) -> usize {
    let mut bit_set_count = 0usize;
    for bit in 0..MI_BFIELD_BITS {
        let is_set = ((1usize << bit) & field) != 0;
        if is_set {
            bit_set_count += 1;
        }
        unsafe {
            *buf.add(*k) = if is_set { b'x' } else { b'.' };
            *k += 1;
        }
    }
    bit_set_count
}

unsafe fn mi_debug_color(buf: *mut u8, k: *mut usize, color: mi_ansi_color_t) {
    let code = color as i32;
    let tens = ((code / 10) as u8) + b'0';
    let ones = ((code % 10) as u8) + b'0';
    unsafe {
        *buf.add(*k) = 0x1B;
        *buf.add(*k + 1) = b'[';
        *buf.add(*k + 2) = tens;
        *buf.add(*k + 3) = ones;
        *buf.add(*k + 4) = b'm';
        *k += 5;
    }
}

unsafe fn mi_page_commit_usage(page: *mut mi_page_t) -> c_int {
    let committed_size = unsafe { mi_page_committed(page) };
    let used_size = unsafe { (*page).used as usize * mi_page_block_size(page) };
    (used_size * 100 / committed_size) as c_int
}

unsafe fn mi_debug_show_page_bfield(
    buf: *mut u8,
    k: *mut usize,
    arena: *mut mi_arena_t,
    slice_index: usize,
    pbit_of_page: *mut isize,
    pcolor_of_page: *mut mi_ansi_color_t,
) -> usize {
    let mut bit_set_count = 0usize;
    let mut bit_of_page = unsafe { *pbit_of_page };
    let mut color = unsafe { *pcolor_of_page };
    let mut prev_color = MI_GRAY;
    for bit in 0..MI_BFIELD_BITS {
        let start = unsafe { mi_arena_slice_start(arena, slice_index + bit).cast::<c_void>() };
        let page = unsafe { _mi_safe_ptr_page(start) };
        if !page.is_null() && start == unsafe { mi_page_slice_start(page) }.cast::<c_void>() {
            crate::mi_assert_internal!(bit_of_page <= 0);
            bit_set_count += 1;
            let mut c = b'p';
            if unsafe { mi_page_is_singleton(page) } {
                c = b's';
            } else if unsafe { mi_page_is_full(page) } {
                c = b'f';
            }
            if !unsafe { mi_page_is_abandoned(page) } {
                c = _mi_toupper(c as i8) as u8;
            }
            let commit_usage = unsafe { mi_page_commit_usage(page) };
            if commit_usage < 25 {
                color = MI_MAROON;
            } else if commit_usage < 50 {
                color = MI_ORANGE;
            } else if commit_usage < 75 {
                color = MI_TEAL;
            } else {
                color = MI_DARKGREEN;
            }
            bit_of_page = match crate::core_types::MemId::view(unsafe { (*page).memid }) {
                crate::core_types::MemIdView::Arena(a) => a.range.count_usize() as isize,
                _ => {
                    crate::mi_assert_internal!(false);
                    0
                }
            };
            if color != prev_color {
                unsafe { mi_debug_color(buf, k, color) };
                prev_color = color;
            }
            unsafe {
                *buf.add(*k) = c;
                *k += 1;
            }
        } else {
            let mut c = b'?';
            if bit_of_page > 0 {
                c = b'-';
            } else if unsafe { _mi_meta_is_meta_page(start) } {
                c = b'm';
                color = MI_GRAY;
            } else if slice_index + bit < unsafe { (*arena.cast::<MiArenaPrefix>()).info_slices } {
                c = b'i';
                color = MI_GRAY;
            } else if unsafe {
                mi_bbitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_free,
                    slice_index + bit,
                    1,
                )
            } {
                if unsafe {
                    mi_bitmap_is_set(
                        (*arena.cast::<MiArenaPrefix>()).slices_purge,
                        slice_index + bit,
                    )
                } {
                    c = b'~';
                    color = MI_ORANGE;
                } else if unsafe {
                    mi_bitmap_is_set(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index + bit,
                    )
                } {
                    c = b'_';
                    color = MI_GRAY;
                } else {
                    c = b'.';
                    color = MI_GRAY;
                }
            }
            if bit == MI_BFIELD_BITS - 1 && bit_of_page > 1 {
                c = b'>';
            }
            if color != prev_color {
                unsafe { mi_debug_color(buf, k, color) };
                prev_color = color;
            }
            unsafe {
                *buf.add(*k) = c;
                *k += 1;
            }
        }
        bit_of_page -= 1;
    }
    unsafe { mi_debug_color(buf, k, MI_GRAY) };
    unsafe {
        *pbit_of_page = bit_of_page;
        *pcolor_of_page = color;
    }
    bit_set_count
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
unsafe fn mi_debug_show_chunks(
    header1: &str,
    header2: &str,
    header3: &str,
    slice_count: usize,
    chunk_count: usize,
    chunks: *mut mi_bchunk_t,
    chunk_bins: *mut mi_bchunkmap_t,
    invert: bool,
    arena: *mut mi_arena_t,
    narrow: bool,
) -> usize {
    crate::options::_mi_raw_message(format_args!(
        "\x1B[37m{}{}{} (use/commit: \x1B[31m0 - 25%\x1B[33m - 50%\x1B[36m - 75%\x1B[32m - 100%\x1B[0m)\n",
        header1, header2, header3
    ));
    let fields_per_line = if narrow { 2 } else { 4 };
    let used_slice_count = unsafe { mi_arena_used_slices(arena) };
    let mut bit_count = 0usize;
    let mut bit_set_count = 0usize;
    let mut bit_of_page = 0isize;
    let mut color_of_page = MI_GRAY;
    let mut i = 0usize;
    while i < chunk_count {
        if bit_count >= slice_count {
            break;
        }
        let mut buf = [0u8; 5 * MI_BCHUNK_BITS + 64];
        if bit_count > used_slice_count && i + 2 < chunk_count {
            let diff = chunk_count - 1 - i;
            bit_count += diff * MI_BCHUNK_BITS;
            crate::options::_mi_raw_message(format_args!("  |\n"));
            i = chunk_count - 1;
        }

        let mut k = 0usize;
        if i < 10 {
            buf[k] = b'0' + i as u8;
            buf[k + 1] = b' ';
            buf[k + 2] = b' ';
            k += 3;
        } else if i < 100 {
            buf[k] = b'0' + (i / 10) as u8;
            buf[k + 1] = b'0' + (i % 10) as u8;
            buf[k + 2] = b' ';
            k += 3;
        } else if i < 1000 {
            buf[k] = b'0' + (i / 100) as u8;
            buf[k + 1] = b'0' + ((i % 100) / 10) as u8;
            buf[k + 2] = b'0' + (i % 10) as u8;
            k += 3;
        }

        let mut chunk_kind = b' ';
        if !chunk_bins.is_null() {
            chunk_kind = match unsafe { mi_bbitmap_debug_get_bin(chunk_bins, i) } {
                MI_CBIN_SMALL => b'S',
                MI_CBIN_MEDIUM => b'M',
                MI_CBIN_LARGE => b'L',
                MI_CBIN_HUGE => b'H',
                MI_CBIN_OTHER => b'X',
                _ => b' ',
            };
        }
        buf[k] = chunk_kind;
        buf[k + 1] = b' ';
        k += 2;

        for j in 0..MI_BCHUNK_FIELDS {
            if j > 0 && (j % fields_per_line) == 0 {
                let line = unsafe { core::str::from_utf8_unchecked(&buf[..k]) };
                crate::options::_mi_raw_message(format_args!("  {}\n\x1B[37m", line));
                buf.fill(0);
                buf[..5].fill(b' ');
                k = 5;
            }
            if bit_count < slice_count {
                let mut bfield = 0usize;
                if !chunks.is_null() {
                    bfield = unsafe { (*chunks.add(i)).bfields[j].load(Ordering::Relaxed) };
                }
                if invert {
                    bfield = !bfield;
                }
                let mut line_k = k;
                let xcount = if chunks.is_null() {
                    unsafe {
                        mi_debug_show_page_bfield(
                            buf.as_mut_ptr(),
                            &mut line_k,
                            arena,
                            bit_count,
                            &mut bit_of_page,
                            &mut color_of_page,
                        )
                    }
                } else {
                    unsafe { mi_debug_show_bfield(bfield, buf.as_mut_ptr(), &mut line_k) }
                };
                k = line_k;
                bit_set_count += if invert {
                    MI_BFIELD_BITS - xcount
                } else {
                    xcount
                };
                buf[k] = b' ';
                k += 1;
            } else {
                buf[k..k + MI_BFIELD_BITS].fill(b'o');
                k += MI_BFIELD_BITS;
            }
            bit_count += MI_BFIELD_BITS;
        }
        let line = unsafe { core::str::from_utf8_unchecked(&buf[..k]) };
        crate::options::_mi_raw_message(format_args!("  {}\n\x1B[37m", line));
        i += 1;
    }
    crate::options::_mi_raw_message(format_args!("\x1B[0m  total pages: {}\n", bit_set_count));
    bit_set_count
}

unsafe fn mi_debug_show_arenas_ex(heap: *mut mi_heap_t, show_pages: bool, narrow: bool) {
    let subproc = unsafe { (*heap.cast::<MiHeapPrefix>()).subproc };
    let max_arenas = unsafe { mi_arenas_get_count(subproc) };
    let mut page_total = 0usize;
    for i in 0..max_arenas {
        let arena = unsafe { mi_arena_from_index(subproc, i) };
        if arena.is_null() {
            break;
        }
        crate::mi_assert!(unsafe { (*arena.cast::<MiArenaPrefix>()).subproc == subproc });
        crate::options::_mi_raw_message(format_args!(
            "{}arena {} at {:p}: {} slices ({} MiB){}{}, subproc: {:p}, numa: {}\n",
            if unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() } {
                ""
            } else {
                "(sub)"
            },
            i,
            arena,
            unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count },
            mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }) / MI_MiB,
            if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
                ", pinned"
            } else {
                ""
            },
            if unsafe { (*arena.cast::<MiArenaPrefix>()).is_exclusive } {
                ", exclusive"
            } else {
                ""
            },
            unsafe { (*arena.cast::<MiArenaPrefix>()).subproc },
            unsafe { (*arena.cast::<MiArenaPrefix>()).numa_node },
        ));
        if show_pages {
            let header1 = "chunks (p:page, f:full, s:singleton, P,F,S:not abandoned, i:arena-info, m:meta-data, ~:free-purgable, _:free-committed, .:free-reserved)";
            let header2 = if narrow { "\n       " } else { " " };
            let header3 = "(chunk bin: S:small, M : medium, L : large, X : other)";
            let slices_free = unsafe { (*arena.cast::<MiArenaPrefix>()).slices_free };
            page_total += unsafe {
                mi_debug_show_chunks(
                    header1,
                    header2,
                    header3,
                    (*arena.cast::<MiArenaPrefix>()).slice_count,
                    mi_bbitmap_chunk_count(slices_free),
                    ptr::null_mut(),
                    (*slices_free).chunkmap_bins.as_mut_ptr(),
                    false,
                    arena,
                    narrow,
                )
            };
        }
    }
    if show_pages {
        crate::options::_mi_raw_message(format_args!("total pages in arenas: {}\n", page_total));
    }
}

/// # Safety
/// Matches the C API and reads the current sub-process arena registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_debug_show_arenas() {
    unsafe { mi_debug_show_arenas_ex(mi_heap_main(), true, true) }
}

/// # Safety
/// Matches the C API and forwards to the current arena debug printer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arenas_print() {
    unsafe { mi_debug_show_arenas() }
}

/// # Safety
/// Matches the C API and may reserve/manage huge-page backed arena memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages_at_ex(
    pages: usize,
    mut numa_node: c_int,
    timeout_msecs: usize,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    if !arena_id.is_null() {
        unsafe { *arena_id = ptr::null_mut() };
    }
    if pages == 0 {
        return 0;
    }
    if numa_node < -1 {
        numa_node = -1;
    }
    if numa_node >= 0 {
        numa_node %= unsafe { _mi_os_numa_node_count() };
    }
    let mut hsize = 0usize;
    let mut pages_reserved = 0usize;
    let mut memid = unsafe { _mi_memid_none() };
    let p = unsafe {
        _mi_os_alloc_huge_os_pages(
            pages,
            numa_node,
            timeout_msecs as i64,
            &mut pages_reserved,
            &mut hsize,
            &mut memid,
        )
    };
    if p.is_null() || pages_reserved == 0 {
        crate::options::_mi_warning_message_fmt(format_args!(
            "failed to reserve {} GiB huge pages\n",
            pages
        ));
        return ENOMEM;
    }
    crate::options::_mi_verbose_message(format_args!(
        "numa node {}: reserved {} GiB huge pages (of the {} GiB requested)\n",
        numa_node, pages_reserved, pages
    ));

    if !unsafe {
        mi_manage_os_memory_ex2(
            _mi_subproc(),
            p,
            hsize,
            numa_node,
            exclusive,
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    } {
        unsafe { _mi_os_free(p, hsize, memid) };
        return ENOMEM;
    }
    0
}

/// # Safety
/// Matches the C API and reserves huge-page backed arena memory on a specific NUMA node.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages_at(
    pages: usize,
    numa_node: c_int,
    timeout_msecs: usize,
) -> c_int {
    unsafe {
        mi_reserve_huge_os_pages_at_ex(pages, numa_node, timeout_msecs, false, ptr::null_mut())
    }
}

/// # Safety
/// Matches the C API and reserves huge-page backed arena memory interleaved across NUMA nodes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages_interleave(
    mut pages: usize,
    numa_nodes: usize,
    timeout_msecs: usize,
) -> c_int {
    if pages == 0 {
        return 0;
    }

    let mut numa_count = if numa_nodes > 0 && numa_nodes <= c_int::MAX as usize {
        numa_nodes as c_int
    } else {
        unsafe { _mi_os_numa_node_count() }
    };
    if numa_count <= 0 {
        numa_count = 1;
    }
    let pages_per = pages / numa_count as usize;
    let pages_mod = pages % numa_count as usize;
    let timeout_per = if timeout_msecs == 0 {
        0
    } else {
        (timeout_msecs / numa_count as usize) + 50
    };

    for numa_node in 0..numa_count {
        if pages == 0 {
            break;
        }
        let mut node_pages = pages_per;
        if (numa_node as usize) < pages_mod {
            node_pages += 1;
        }
        let err = unsafe { mi_reserve_huge_os_pages_at(node_pages, numa_node, timeout_per) };
        if err != 0 {
            return err;
        }
        if pages < node_pages {
            pages = 0;
        } else {
            pages -= node_pages;
        }
    }

    0
}

/// # Safety
/// Matches the deprecated C API and forwards to the interleaved huge-page reservation path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages(
    pages: usize,
    max_secs: f64,
    pages_reserved: *mut usize,
) -> c_int {
    crate::options::_mi_warning_message_fmt(format_args!(
        "mi_reserve_huge_os_pages is deprecated: use mi_reserve_huge_os_pages_interleave/at instead\n"
    ));
    if !pages_reserved.is_null() {
        unsafe { *pages_reserved = 0 };
    }
    let err =
        unsafe { mi_reserve_huge_os_pages_interleave(pages, 0, (max_secs * 1000.0) as usize) };
    if err == 0 && !pages_reserved.is_null() {
        unsafe { *pages_reserved = pages };
    }
    err
}

type MiDebugShowChunksFunT = unsafe fn(
    &str,
    &str,
    &str,
    usize,
    usize,
    *mut mi_bchunk_t,
    *mut mi_bchunkmap_t,
    bool,
    *mut mi_arena_t,
    bool,
) -> usize;

#[used]
static MI_ARENA_SLICE_START_KEEP: unsafe fn(*mut mi_arena_t, usize) -> *mut u8 =
    mi_arena_slice_start;
#[used]
static MI_ARENA_PAGE_AT_SLICE_KEEP: unsafe fn(*mut mi_arena_t, usize) -> *mut mi_page_t =
    mi_arena_page_at_slice;
#[used]
static MI_ARENAS_PAGE_SINGLETON_ALLOC_KEEP: unsafe fn(
    *mut mi_theap_t,
    usize,
    usize,
) -> *mut mi_page_t = mi_arenas_page_singleton_alloc;
#[used]
static MI_ARENAS_PAGE_ALLOC_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    usize,
    usize,
) -> *mut mi_page_t = _mi_arenas_page_alloc;
#[used]
static MI_ARENA_PURGE_KEEP: unsafe fn(*mut mi_arena_t, crate::core_types::SliceRange) -> bool =
    mi_arena_purge;
#[used]
static MI_DEBUG_SHOW_CHUNKS_KEEP: MiDebugShowChunksFunT = mi_debug_show_chunks;
