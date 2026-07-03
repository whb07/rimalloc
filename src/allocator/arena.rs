//! Arena management

use core::ffi::{c_int, c_long, c_void};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use core::mem::offset_of;
use core::mem::{MaybeUninit, size_of};
use core::ptr;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use crate::atomic::{mi_atomic_guard_t, mi_lock_acquire, mi_lock_release};
use crate::bitmap::{
    _mi_bitmap_forall_setc_rangesn, MI_BCHUNK_BITS, MI_BCHUNK_FIELDS, MI_BCHUNK_SIZE,
    MI_BFIELD_BITS, MI_BITMAP_MAX_BIT_COUNT, mi_bbitmap_bsr_inv, mi_bbitmap_chunk_count,
    mi_bbitmap_debug_get_bin, mi_bbitmap_init, mi_bbitmap_is_clearN, mi_bbitmap_is_setN,
    mi_bbitmap_setN, mi_bbitmap_size, mi_bbitmap_t, mi_bbitmap_try_clearNC,
    mi_bbitmap_try_find_and_clearN, mi_bbitmap_unsafe_setN, mi_bchunk_t, mi_bchunkmap_t,
    mi_bfield_t, mi_bitmap_clear, mi_bitmap_clear_once_set, mi_bitmap_clearN,
    mi_bitmap_forall_set_typed, mi_bitmap_init, mi_bitmap_is_all_clear, mi_bitmap_is_clearN,
    mi_bitmap_is_set, mi_bitmap_is_setN, mi_bitmap_popcountN, mi_bitmap_set, mi_bitmap_setN,
    mi_bitmap_size, mi_bitmap_t, mi_bitmap_try_find_and_claim, mi_bitmap_unsafe_setN,
};
use crate::bits::{MI_INTPTR_BITS, MI_MiB};
use crate::core_types::{
    ArenaIndex, ArenaPtr, BlockVisitCallback, BlockVisitCallbackArg, BorrowedHeap, Errno, HeapPtr,
    InteriorUserPtr, MiLockGuard, OsMemPtr, OwnedHeap, PagePtr, ReservedBytes, SliceCount,
    SliceIndex, SubprocPtr, ThreadLocalClaimCallback,
};
use crate::internal::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt,
    __mi_stat_counter_increase, __mi_stat_counter_increase_mt, __mi_stat_decrease,
    __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt, _mi_align_up, _mi_clamp,
    _mi_divide_up, _mi_is_heap_main, _mi_memid_none, _mi_meta_free, _mi_meta_is_meta_page,
    _mi_os_commit, _mi_os_commit_ex, _mi_os_free, _mi_os_free_ex, _mi_os_has_overcommit,
    _mi_os_has_virtual_reserve, _mi_os_reuse, _mi_preloading, _mi_subproc, _mi_theap_heap,
    _mi_thread_id, _mi_toupper, MI_DEBUG, mi_mem_is_zero, mi_memid_t, mi_mul_overflow,
    mi_page_all_free, mi_page_block_size, mi_page_block_size_typed, mi_page_claim_ownership,
    mi_page_clear_abandoned_mapped, mi_page_committed_typed, mi_page_heap, mi_page_is_abandoned,
    mi_page_is_abandoned_mapped, mi_page_is_full, mi_page_is_owned, mi_page_is_singleton,
    mi_page_meta_is_separated, mi_page_slice_start, mi_page_start, mi_size_of_slices,
    mi_slice_count_of_size,
};
use crate::mimalloc::{mi_heap_main, mi_heap_theap, mi_heap_zalloc_aligned};
use crate::options::{mi_option_get, mi_option_get_size, mi_option_is_enabled, mi_option_t};
use crate::os::{
    _mi_os_alloc_aligned, _mi_os_alloc_aligned_at_offset, _mi_os_alloc_huge_os_pages,
    _mi_os_minimal_purge_size, _mi_os_numa_node_count,
};
use crate::page::{
    _mi_page_free_collect, _mi_page_stats_bin, MI_LARGE_MAX_OBJ_SIZE, MI_LARGE_PAGE_SIZE,
    MI_MEDIUM_MAX_OBJ_SIZE, MI_MEDIUM_PAGE_SIZE, MI_PAGE_OSPAGE_BLOCK_ALIGN2,
    MI_SMALL_MAX_OBJ_SIZE, MI_SMALL_PAGE_SIZE, mi_bin_from_block_size, mi_page_set_theap,
};
use crate::page_map::{_mi_page_map_register, _mi_page_map_unregister, _mi_safe_ptr_page};
use crate::prim::_mi_page_associated_theap_peek;
use crate::stats::{MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_OTHER, MI_CBIN_SMALL};
use crate::theap::{_mi_heap_area_init, _mi_theap_area_visit_blocks};
use crate::track::{mi_track_mem_defined, mi_track_mem_undefined};
use crate::types::{
    MI_BIN_COUNT, MI_ENABLE_LARGE_PAGES, MI_PAGE_ALIGN, MI_PAGE_MAX_OVERALLOC_ALIGN,
    MI_PAGE_MAX_START_BLOCK_ALIGN2, MI_PAGE_META_ALIGNED_FREE_SMALL, MI_PAGE_MIN_COMMIT_SIZE,
    MI_SMALL_SIZE_MAX, mi_arena_id_t, mi_arena_t, mi_block_visit_fun, mi_commit_fun_t,
    mi_heap_area_t, mi_heap_t, mi_msecs_t, mi_page_t, mi_subproc_t, mi_theap_t, mi_thread_local_t,
    mi_tld_t,
};

pub const MI_ARENA_SLICE_SIZE: usize = 8192 * size_of::<usize>();
pub const MI_ARENA_SLICE_ALIGN: usize = MI_ARENA_SLICE_SIZE;
pub const MI_ARENA_BCHUNK_BITS: usize = 64 * size_of::<usize>();
pub const MI_ARENA_MIN_SIZE: usize = MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE;
const MI_ARENA_MAX_CHUNK_OBJ_SIZE: usize = MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE;
pub const MI_META_PAGE_SIZE: usize = MI_ARENA_SLICE_SIZE;
pub const MI_PAGE_BLOCK_START_MAX_OFFSET: usize = 8 * MI_INTPTR_BITS;
// Retains upstream diagnostic/color constants and traversal markers used by
// compatibility paths outside the routine Rust test target.
#[allow(dead_code, non_upper_case_globals)]
pub const mi_forall_arenas_end: () = ();

const MI_MAX_ARENAS: usize = 160;
const MI_ARENA_BIN_COUNT: usize = MI_BIN_COUNT;
const MI_ARENA_MIN_OBJ_SIZE: usize = MI_ARENA_SLICE_SIZE;
const MI_ARENA_MAX_SIZE: usize = MI_BITMAP_MAX_BIT_COUNT * MI_ARENA_SLICE_SIZE;
const _: () = assert!(mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) > 0);
const _: () = assert!(mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) <= u32::MAX as usize);
const MI_SMALL_PAGE_SLICE_COUNT: crate::core_types::SliceCount = {
    match crate::core_types::SliceCount::from_usize_const(mi_slice_count_of_size(
        MI_SMALL_PAGE_SIZE,
    )) {
        Some(count) => count,
        None => unreachable!(),
    }
};
const _: () = assert!(mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) > 0);
const _: () = assert!(mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) <= u32::MAX as usize);
const MI_MEDIUM_PAGE_SLICE_COUNT: crate::core_types::SliceCount = {
    match crate::core_types::SliceCount::from_usize_const(mi_slice_count_of_size(
        MI_MEDIUM_PAGE_SIZE,
    )) {
        Some(count) => count,
        None => unreachable!(),
    }
};
const _: () = assert!(mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) > 0);
const _: () = assert!(mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) <= u32::MAX as usize);
const MI_LARGE_PAGE_SLICE_COUNT: crate::core_types::SliceCount = {
    match crate::core_types::SliceCount::from_usize_const(mi_slice_count_of_size(
        MI_LARGE_PAGE_SIZE,
    )) {
        Some(count) => count,
        None => unreachable!(),
    }
};
const ENOMEM: c_int = 12;

mod abandoned;
mod debug;
mod geometry;
mod heap_pages;
mod ids;
mod os_memory;
mod prefix;
mod purge;

use abandoned::{
    mi_abandoned_page_unown, mi_arena_try_claim_abandoned, mi_heap_ensure_arena_pages_from_raw,
    mi_page_is_full_local,
};
pub(crate) use geometry::_mi_arenas_contain;
use geometry::{
    mi_arena_info_slices, mi_arena_page_at_slice, mi_arena_range_is_after_info_slices,
    mi_arena_size, mi_arena_slice_start, mi_arena_start, mi_heap_arena_pages, mi_heap_has_page,
    mi_page_arena_pages,
};
pub(crate) use heap_pages::{_mi_heap_destroy_pages, _mi_heap_move_pages, _mi_heap_visit_blocks};
pub(crate) use ids::{_mi_arena_from_id, _mi_arena_id_none, _mi_arena_memid_is_suitable};
use ids::{
    mi_arena_from_memid, mi_arena_id_from_arena_typed, mi_arena_is_suitable, mi_memid_create_arena,
    mi_memid_is_os, mi_page_full_size, mi_page_memid_is_arena,
};
use os_memory::mi_arena_reserve;
#[allow(unused_imports)]
pub use os_memory::{
    mi_manage_memory, mi_manage_os_memory, mi_manage_os_memory_ex, mi_reserve_huge_os_pages,
    mi_reserve_huge_os_pages_at, mi_reserve_huge_os_pages_at_ex,
    mi_reserve_huge_os_pages_interleave, mi_reserve_os_memory, mi_reserve_os_memory_ex,
};
use prefix::{
    MiArenaPagesPrefix, MiArenaPrefix, MiHeapPrefix, MiSubprocPrefix, mi_arena_from_index_typed,
    mi_arena_index, mi_arena_pages_t, mi_heap_abandoned_count, mi_heap_arena_page_cell,
    mi_subproc_arena_cell,
};
pub(crate) use purge::{_mi_arenas_collect, _mi_arenas_free};

/// # Safety
/// `subproc` must point to a valid `mi_subproc_t`.
// SAFETY[AUDIT:arena-registry-count-index-boundary]: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_arenas_get_count(subproc: *mut mi_subproc_t) -> usize {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        crate::internal::mi_assert_internal!(false);
        return 0;
    };
    // SAFETY[AUDIT:arena-registry-count-index-boundary]: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
    unsafe { mi_arenas_get_count_typed(subproc) }
}

unsafe fn mi_arenas_get_count_typed(subproc: SubprocPtr) -> usize {
    unsafe {
        subproc
            .arena_count(crate::core_types::MemoryOrder::Relaxed)
            .get()
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn mi_set_errno(err: c_int) {
    // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    unsafe { *__errno_location() = err };
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    unsafe { *__error() = err };
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

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

unsafe fn mi_arena_commit(
    arena: ArenaPtr,
    range: crate::core_types::SliceRange,
    commit_size: crate::core_types::CommittedBytes,
    is_zero: *mut bool,
    already_committed: crate::core_types::CommittedBytes,
) -> bool {
    let arena_raw = arena.as_ptr();
    // SAFETY[AUDIT:arena-slice-commit-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    let start = unsafe {
        mi_arena_slice_start(arena, range.start())
            .as_ptr()
            .cast::<c_void>()
    };
    let size = commit_size.get();
    // SAFETY[AUDIT:arena-slice-commit-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    let commit_fun = unsafe { (*arena_raw.cast::<MiArenaPrefix>()).commit_fun };
    if let Some(commit_fun) = commit_fun {
        return unsafe {
            commit_fun(
                true,
                start,
                size,
                is_zero,
                (*arena_raw.cast::<MiArenaPrefix>()).commit_fun_arg,
            )
        };
    }
    if !already_committed.is_zero() {
        // SAFETY[AUDIT:arena-slice-commit-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        unsafe { _mi_os_commit_ex(start, size, is_zero, already_committed) }
    } else {
        unsafe { _mi_os_commit(start, size, is_zero) }
    }
}

unsafe fn mi_arena_try_alloc_at(
    arena: ArenaPtr,
    slice_count: crate::core_types::SliceCount,
    commit: bool,
    tseq: crate::core_types::ThreadSeq,
) -> Option<(*mut c_void, crate::core_types::ArenaAllocation)> {
    let arena_ptr = arena;
    let arena = arena.as_ptr();
    let slice_count = slice_count.get() as usize;
    let mut slice_index = 0usize;
    // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    if !unsafe {
        mi_bbitmap_try_find_and_clearN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            tseq.get(),
            slice_count,
            &mut slice_index,
        )
    } {
        return None;
    }

    // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    let p = unsafe {
        mi_arena_slice_start(arena_ptr, crate::core_types::SliceIndex::new(slice_index))
            .as_ptr()
            .cast::<c_void>()
    };
    let Some(range) = crate::core_types::SliceRange::new(slice_index, slice_count, arena_ptr).ok()
    else {
        crate::internal::mi_assert_internal!(false);
        // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        unsafe {
            mi_bbitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        };
        return None;
    };
    let mut allocation = crate::core_types::ArenaAllocation {
        arena: arena_ptr,
        range,
        // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        memid: unsafe { mi_memid_create_arena(arena, range) },
    };

    let mut touched_slices = slice_count;
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.initially_zero } {
        let mut already_dirty = 0usize;
        // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        allocation.memid.initially_zero = unsafe {
            mi_bitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index,
                slice_count,
                &mut already_dirty,
            )
        };
        crate::internal::mi_assert_internal!(already_dirty <= touched_slices);
        touched_slices -= already_dirty;
    }

    if commit {
        // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        let already_committed = unsafe {
            mi_bitmap_popcountN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
        if already_committed < slice_count {
            let mut commit_zero = false;
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            if !unsafe {
                mi_arena_commit(
                    arena_ptr,
                    range,
                    crate::core_types::CommittedBytes::new(mi_size_of_slices(slice_count)),
                    &mut commit_zero,
                    crate::internal::mi_committed_bytes_of_slices(slice_count - already_committed),
                )
            } {
                // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
                unsafe {
                    mi_bbitmap_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_free,
                        slice_index,
                        slice_count,
                    )
                };
                return None;
            }
            if commit_zero {
                allocation.memid.initially_zero = true;
            }
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                mi_bitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                    ptr::null_mut(),
                )
            };
            if MI_DEBUG > 1
                && allocation.memid.initially_zero
                // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
                && !unsafe { mi_mem_is_zero(p, mi_size_of_slices(slice_count)) }
            {
                crate::options::_mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!("internal error: arena allocation was not zero-initialized!\n"),
                );
                allocation.memid.initially_zero = false;
            }
        } else {
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe { _mi_os_reuse(p, mi_size_of_slices(slice_count)) };
            if unsafe { _mi_os_has_overcommit() }
                && touched_slices > 0
                && !unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned }
            {
                // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
                unsafe {
                    __mi_stat_increase_mt(
                        &mut (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed,
                        crate::internal::mi_committed_bytes_of_slices(touched_slices).get(),
                    )
                };
            }
        }

        // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        });
        allocation.memid.initially_committed = true;

        if allocation.memid.initially_zero {
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe { mi_track_mem_defined(p, slice_count * MI_ARENA_SLICE_SIZE) };
        } else {
            unsafe { mi_track_mem_undefined(p, slice_count * MI_ARENA_SLICE_SIZE) };
        }
    } else {
        // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        allocation.memid.initially_committed = unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
        if !allocation.memid.initially_committed {
            let mut already_committed_count = 0usize;
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                mi_bitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                    &mut already_committed_count,
                )
            };
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                mi_bitmap_clearN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            };
            // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                __mi_stat_adjust_decrease_mt(
                    ptr::addr_of_mut!((*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed),
                    crate::internal::mi_committed_bytes_of_slices(already_committed_count).get(),
                )
            };
        }
    }

    // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*allocation.arena.as_ptr().cast::<MiArenaPrefix>()).slices_free,
            allocation.range.start_usize(),
            allocation.range.count_usize(),
        )
    });
    if commit {
        // SAFETY[AUDIT:arena-slice-commit-accounting-boundary]: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*allocation.arena.as_ptr().cast::<MiArenaPrefix>()).slices_committed,
                allocation.range.start_usize(),
                allocation.range.count_usize(),
            )
        });
        crate::internal::mi_assert_internal!(allocation.memid.initially_committed);
    }
    // SAFETY[AUDIT:arena-slice-allocation-bitmaps-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bitmap_is_setN(
            (*allocation.arena.as_ptr().cast::<MiArenaPrefix>()).slices_dirty,
            allocation.range.start_usize(),
            allocation.range.count_usize(),
        )
    });

    Some((p, allocation))
}

/// # Safety
/// `subproc` must point to a valid `mi_subproc_t`, and `idx` must be in range.
// SAFETY[AUDIT:arena-registry-count-index-boundary]: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_arena_from_index(
    subproc: *mut mi_subproc_t,
    idx: usize,
) -> *mut mi_arena_t {
    let Some(subproc_ptr) = SubprocPtr::new(subproc) else {
        crate::internal::mi_assert_internal!(false);
        return ptr::null_mut();
    };
    let Some(arena_idx) = ArenaIndex::from_usize(idx) else {
        crate::internal::mi_assert_internal!(false);
        return ptr::null_mut();
    };
    // SAFETY[AUDIT:arena-registry-count-index-boundary]: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
    crate::internal::mi_assert_internal!(idx < unsafe { mi_arenas_get_count_typed(subproc_ptr) });
    unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) }
}

// SAFETY[AUDIT:arena-registry-count-index-boundary]: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_arena_t) -> mi_arena_id_t = ids::mi_arena_id_from_arena;

#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_subproc_t) -> usize = mi_arenas_get_count;

// SAFETY[AUDIT:arena-registry-count-index-boundary]: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_subproc_t, usize) -> *mut mi_arena_t = mi_arena_from_index;

unsafe fn mi_arena_start_idx(
    heap: BorrowedHeap,
    tseq: crate::core_types::ThreadSeq,
    arena_cycle: ArenaIndex,
) -> ArenaIndex {
    let arena_cycle = arena_cycle.get() as usize;
    let heap = heap.ptr().as_ptr().cast::<MiHeapPrefix>();
    // SAFETY[AUDIT:arena-registry-start-index-boundary]: arena iteration start indexes are derived from typed heap/thread sequence domains and bounded arena counts.
    let subproc = unsafe { (*heap).subproc.cast::<MiSubprocPrefix>() };
    let hseq = crate::core_types::HeapSeq::new(unsafe { (*heap).heap_seq });
    let hcount = unsafe { (*subproc).heap_count.load(Ordering::Relaxed) };
    if arena_cycle <= 1 {
        return unsafe { ArenaIndex::new_unchecked(0) };
    }
    if hseq.get() == 0 || hcount <= 1 || arena_cycle > 0x8FF {
        // SAFETY[AUDIT:arena-registry-start-index-boundary]: arena iteration start indexes are derived from typed heap/thread sequence domains and bounded arena counts.
        return unsafe { ArenaIndex::new_unchecked((tseq.get() % arena_cycle) as u16) };
    }

    crate::internal::mi_assert_internal!(arena_cycle <= 0x8FF);
    let frac = (arena_cycle * 256) / hcount;
    let start = if frac == 0 {
        hseq.get() % arena_cycle
    } else {
        let hspot = hseq.get() % hcount;
        let mut start = (frac * hspot) / 256;
        if frac >= 512 {
            start += tseq.get() % (frac / 256);
        }
        start
    };
    crate::internal::mi_assert_internal!(start < arena_cycle);
    // SAFETY[AUDIT:arena-registry-start-index-boundary]: arena iteration start indexes are derived from typed heap/thread sequence domains and bounded arena counts.
    unsafe { ArenaIndex::new_unchecked(start as u16) }
}

unsafe fn mi_arena_numa_node(arena: *const mi_arena_t) -> crate::core_types::NumaNode {
    crate::core_types::NumaNode::new(unsafe { (*arena.cast::<MiArenaPrefix>()).numa_node })
}

unsafe fn mi_arena_set_numa_node(arena: *mut mi_arena_t, numa_node: crate::core_types::NumaNode) {
    // SAFETY[AUDIT:arena-registry-suitability-boundary]: arena suitability checks keep pinned, requested-arena, and NUMA predicates separate from table mutation.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).numa_node = numa_node.get();
    }
}

unsafe fn mi_arena_is_suitable_ex(
    arena: ArenaPtr,
    req_arena: Option<ArenaPtr>,
    match_numa: crate::core_types::MatchNuma,
    numa_node: crate::core_types::NumaNode,
    allow_pinned: crate::core_types::AllowPinned,
) -> bool {
    let arena_raw = arena.as_ptr();
    // SAFETY[AUDIT:arena-registry-suitability-boundary]: arena suitability checks keep pinned, requested-arena, and NUMA predicates separate from table mutation.
    if !allow_pinned.get() && unsafe { (*arena_raw.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return false;
    }
    if unsafe { !mi_arena_is_suitable(arena, req_arena) } {
        return false;
    }
    if req_arena.is_none() {
        // SAFETY[AUDIT:arena-registry-suitability-boundary]: arena suitability checks keep pinned, requested-arena, and NUMA predicates separate from table mutation.
        let arena_numa = unsafe { mi_arena_numa_node(arena_raw) };
        let numa_suitable = arena_numa.is_suitable_for(numa_node);
        if match_numa.get() {
            if !numa_suitable {
                return false;
            }
        } else if numa_suitable {
            return false;
        }
    }
    true
}

unsafe fn mi_forall_arenas<F>(
    heap: BorrowedHeap,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    mut visit: F,
) where
    F: FnMut(ArenaPtr) -> bool,
{
    let heap = heap.ptr();
    // SAFETY[AUDIT:arena-registry-iteration-boundary]: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
    let Some(subproc) = (unsafe { heap.subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc) };
    let arena_cycle = arena_count.saturating_sub(1);
    let Some(arena_cycle_idx) = ArenaIndex::from_usize(arena_cycle) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:arena-registry-iteration-boundary]: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
    let start = unsafe {
        mi_arena_start_idx(BorrowedHeap::new_unchecked(heap), tseq, arena_cycle_idx).get() as usize
    };
    for i in 0..arena_count {
        let arena = if let Some(req_arena) = req_arena {
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
            let Some(arena_idx) = ArenaIndex::from_usize(idx) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY[AUDIT:arena-registry-iteration-boundary]: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
            let Some(arena) = (unsafe { subproc.arena(arena_idx) }) else {
                continue;
            };
            arena
        };
        if !visit(arena) {
            break;
        }
    }
}

unsafe fn mi_forall_suitable_arenas<F>(
    heap: BorrowedHeap,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    match_numa: bool,
    numa_node: crate::core_types::NumaNode,
    allow_large: bool,
    mut visit: F,
) where
    F: FnMut(ArenaPtr) -> bool,
{
    // SAFETY[AUDIT:arena-registry-iteration-boundary]: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
    unsafe {
        mi_forall_arenas(heap, req_arena, tseq, |arena| {
            !mi_arena_is_suitable_ex(
                arena,
                req_arena,
                crate::core_types::MatchNuma::new(match_numa),
                numa_node,
                crate::core_types::AllowPinned::new(allow_large),
            ) || visit(arena)
        })
    };
}

#[allow(clippy::too_many_arguments)]
unsafe fn mi_arenas_try_find_free(
    heap: BorrowedHeap,
    slice_count: crate::core_types::SliceCount,
    alignment: crate::core_types::Alignment,
    commit: bool,
    allow_large: bool,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    numa_node: crate::core_types::NumaNode,
    allocation: *mut crate::core_types::ArenaAllocation,
) -> *mut c_void {
    crate::internal::mi_assert!(alignment.get() <= MI_ARENA_SLICE_ALIGN);
    if alignment.get() > MI_ARENA_SLICE_ALIGN {
        return ptr::null_mut();
    }

    let mut found = ptr::null_mut();
    // SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    unsafe {
        mi_forall_suitable_arenas(
            heap,
            req_arena,
            tseq,
            true,
            numa_node,
            allow_large,
            |arena| {
                if let Some((p, arena_allocation)) =
                    mi_arena_try_alloc_at(arena, slice_count, commit, tseq)
                {
                    found = p;
                    *allocation = arena_allocation;
                    false
                } else {
                    true
                }
            },
        );
    }
    if !found.is_null() || numa_node.is_any() {
        return found;
    }

    // SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    unsafe {
        mi_forall_suitable_arenas(
            heap,
            req_arena,
            tseq,
            false,
            numa_node,
            allow_large,
            |arena| {
                if let Some((p, arena_allocation)) =
                    mi_arena_try_alloc_at(arena, slice_count, commit, tseq)
                {
                    found = p;
                    *allocation = arena_allocation;
                    false
                } else {
                    true
                }
            },
        );
    }
    found
}

unsafe fn mi_arena_pages_size(
    slice_count: crate::core_types::SliceCount,
) -> (
    crate::core_types::ReservedBytes,
    crate::core_types::ReservedBytes,
) {
    let slice_count = slice_count.get() as usize;
    crate::internal::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_pages_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 1 + MI_ARENA_BIN_COUNT;
    // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bitmaps_size = bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) };
    let size = base_size + bitmaps_size;
    (
        crate::core_types::ReservedBytes::new(size),
        crate::core_types::ReservedBytes::new(base_size),
    )
}

unsafe fn mi_arena_info_slices_needed(
    slice_count: crate::core_types::SliceCount,
    bitmap_base: *mut usize,
) -> usize {
    let slice_count = slice_count.get() as usize;
    crate::internal::mi_assert_internal!(slice_count.is_multiple_of(MI_ARENA_BCHUNK_BITS));
    let base_size = _mi_align_up(size_of::<mi_arena_t>(), MI_BCHUNK_SIZE);
    let bitmaps_count = 4 + MI_ARENA_BIN_COUNT;
    // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bitmaps_size = (bitmaps_count * unsafe { mi_bitmap_size(slice_count, ptr::null_mut()) })
        + unsafe { mi_bbitmap_size(slice_count, ptr::null_mut()) };
    let pages_size = if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        slice_count * size_of::<mi_page_t>()
    } else {
        0
    };
    let size = base_size + bitmaps_size + pages_size;
    // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let info_size = _mi_align_up(size, unsafe { crate::os::_mi_os_page_size_typed().get() })
        + unsafe { crate::os::_mi_os_secure_guard_page_size() };
    let info_slices = mi_slice_count_of_size(info_size);
    if !bitmap_base.is_null() {
        unsafe { *bitmap_base = base_size };
    }
    info_slices
}

unsafe fn mi_arena_bitmap_init(
    slice_count: crate::core_types::SliceCount,
    base: *mut *mut u8,
) -> *mut mi_bitmap_t {
    // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bitmap = unsafe { *base }.cast::<mi_bitmap_t>();
    let size = unsafe { mi_bitmap_init(bitmap, slice_count.get() as usize, true) };
    unsafe { *base = (*base).add(size) };
    bitmap
}

unsafe fn mi_arena_bbitmap_init(
    slice_count: crate::core_types::SliceCount,
    base: *mut *mut u8,
) -> *mut mi_bbitmap_t {
    // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let bbitmap = unsafe { *base }.cast::<mi_bbitmap_t>();
    let size = unsafe { mi_bbitmap_init(bbitmap, slice_count.get() as usize, true) };
    unsafe { *base = (*base).add(size) };
    bbitmap
}

unsafe fn mi_arena_pages_alloc(
    arena: ArenaPtr,
    range: crate::core_types::SliceRange,
    alloc_size: crate::core_types::ReservedBytes,
    bitmap_base: crate::core_types::ReservedBytes,
) -> *mut mi_arena_pages_t {
    crate::internal::mi_assert_internal!(range.start_usize() == 0);
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
        range.end_usize() <= unsafe { arena.slice_count() }.get() as usize
    );
    let slice_count = range.count();
    let arena_pages =
        unsafe { mi_heap_zalloc_aligned(mi_heap_main(), alloc_size.get(), MI_BCHUNK_SIZE) }
            .cast::<mi_arena_pages_t>();
    if arena_pages.is_null() {
        return ptr::null_mut();
    }
    // SAFETY[AUDIT:arena-slice-layout-boundary]: arena slice and range projection keeps slice indexes, committed ranges, and address containment within registered arena bounds.
    let mut base = unsafe { arena_pages.cast::<u8>().add(bitmap_base.get()) };
    crate::internal::mi_assert_internal!((base as usize).is_multiple_of(MI_BCHUNK_SIZE));
    unsafe { (*arena_pages).pages = mi_arena_bitmap_init(slice_count, &mut base) };
    for i in 0..MI_ARENA_BIN_COUNT {
        unsafe { (*arena_pages).pages_abandoned[i] = mi_arena_bitmap_init(slice_count, &mut base) };
    }
    arena_pages
}

unsafe fn mi_arenas_unsafe_destroy(subproc: SubprocPtr) {
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    // SAFETY[AUDIT:arena-registry-destroy-boundary]: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    for i in 0..arena_count {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY[AUDIT:arena-registry-destroy-boundary]: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
        let arena = unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) };
        if !arena.is_null() {
            unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .store(ptr::null_mut(), crate::core_types::MemoryOrder::Release);
            }
            // SAFETY[AUDIT:arena-registry-destroy-boundary]: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
            if mi_memid_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid }) {
                unsafe {
                    crate::internal::_mi_os_free_ex(
                        mi_arena_start(ArenaPtr::new_unchecked(arena))
                            .as_ptr()
                            .cast::<c_void>(),
                        mi_arena_size(ArenaPtr::new_unchecked(arena)).get(),
                        true,
                        (*arena.cast::<MiArenaPrefix>()).memid,
                        subproc,
                    );
                }
            }
        }
    }
    let mut expected = arena_count;
    // SAFETY[AUDIT:arena-registry-destroy-boundary]: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
    let _ = unsafe {
        crate::core_types::AtomicUsizeField(
            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
        )
        .compare_exchange_acq_rel(&mut expected, 0)
    };
}

/// # Safety
/// Caller must ensure `subproc` is valid and destruction is only used in the same narrow cases
/// as the original C implementation.
// SAFETY[AUDIT:arena-registry-destroy-boundary]: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_unsafe_destroy_all(subproc: *mut mi_subproc_t) {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:arena-registry-destroy-boundary]: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
    unsafe { mi_arenas_unsafe_destroy(subproc) };
}

unsafe fn mi_arenas_add(
    subproc: SubprocPtr,
    arena_ptr: ArenaPtr,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let subproc = subproc.as_ptr();
    let arena = arena_ptr.as_ptr();
    // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
    crate::internal::mi_assert_internal!(unsafe { arena_ptr.slice_count() }.get() > 0);
    if !arena_id.is_null() {
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
    let mut count = unsafe { mi_arenas_get_count_typed(SubprocPtr::new_unchecked(subproc)) };
    for i in 0..count {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
        if unsafe { mi_arena_from_index_typed(SubprocPtr::new_unchecked(subproc), arena_idx) }
            .is_null()
        {
            unsafe { arena_ptr.set_index(arena_idx) };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
            if unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .compare_exchange_release(&mut expected, arena)
            } {
                if !arena_id.is_null() {
                    // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
                    unsafe { *arena_id = mi_arena_id_from_arena_typed(arena_ptr) };
                }
                return true;
            }
        }
    }

    while count < MI_MAX_ARENAS {
        let next_count = count + 1;
        // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
        if unsafe {
            crate::core_types::AtomicUsizeField(
                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count).cast::<usize>(),
            )
            .compare_exchange_acq_rel(&mut count, next_count)
        } {
            let Some(arena_idx) = ArenaIndex::from_usize(count) else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
            unsafe { arena_ptr.set_index(arena_idx) };
            let mut expected = ptr::null_mut::<mi_arena_t>();
            if unsafe {
                mi_subproc_arena_cell(subproc, arena_idx)
                    .compare_exchange_release(&mut expected, arena)
            } {
                // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
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
                    // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
                    unsafe { *arena_id = mi_arena_id_from_arena_typed(arena_ptr) };
                }
                return true;
            }
        }
    }

    // SAFETY[AUDIT:arena-registry-add-boundary]: arena registration publishes typed arena indexes through atomic subproc cells and rolls back failed publication.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).arena_idx = 0;
        (*arena.cast::<MiArenaPrefix>()).subproc = ptr::null_mut();
    }
    false
}

#[allow(clippy::too_many_arguments)]
unsafe fn mi_arena_initialize(
    subproc: SubprocPtr,
    start: OsMemPtr,
    slice_count: crate::core_types::SliceCount,
    parent: *mut mi_arena_t,
    total_size: crate::core_types::ReservedBytes,
    numa_node: crate::core_types::NumaNode,
    exclusive: bool,
    memid: mi_memid_t,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> *mut mi_arena_t {
    let subproc_raw = subproc.as_ptr();
    let start = start.as_ptr();
    let slice_count_usize = slice_count.get() as usize;
    crate::internal::mi_assert_internal!((start as usize).is_multiple_of(MI_ARENA_SLICE_ALIGN));
    crate::internal::mi_assert_internal!(mi_size_of_slices(slice_count_usize) >= MI_ARENA_MIN_SIZE);

    if slice_count_usize > MI_BITMAP_MAX_BIT_COUNT {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is too large (size {} MiB, maximum is {} MiB)",
            mi_size_of_slices(slice_count_usize) / (1024 * 1024),
            mi_size_of_slices(MI_BITMAP_MAX_BIT_COUNT) / (1024 * 1024)
        ));
        return ptr::null_mut();
    }

    let mut bitmap_base = 0usize;
    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    let info_slices = unsafe { mi_arena_info_slices_needed(slice_count, &mut bitmap_base) };
    if slice_count_usize < info_slices + 1 {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is not large enough (size {} KiB, minimum required is {} KiB)",
            mi_size_of_slices(slice_count_usize) / 1024,
            mi_size_of_slices(info_slices + 1) / 1024
        ));
        return ptr::null_mut();
    } else if info_slices >= MI_ARENA_BCHUNK_BITS {
        crate::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is too large with respect to the maximum object size (size {} MiB, meta-info slices {}, maximum object slices are {})",
            mi_size_of_slices(slice_count_usize) / (1024 * 1024),
            info_slices,
            MI_ARENA_BCHUNK_BITS
        ));
        return ptr::null_mut();
    }

    let arena = start.cast::<mi_arena_t>();
    if !memid.initially_committed {
        let mut commit_size = mi_size_of_slices(info_slices);
        if !memid.is_pinned {
            // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
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
            // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
            unsafe { _mi_os_commit(arena.cast::<c_void>(), commit_size, ptr::null_mut()) }
        };
        if !ok {
            crate::options::_mi_warning_message_fmt(format_args!(
                "unable to commit meta-data for OS memory"
            ));
            return ptr::null_mut();
        }
    } else if !memid.is_pinned {
        // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
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
        // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
        unsafe {
            crate::internal::_mi_memzero(
                arena.cast::<c_void>(),
                mi_size_of_slices(info_slices) - crate::os::_mi_os_secure_guard_page_size(),
            )
        };
    }

    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).subproc = subproc_raw;
        (*arena.cast::<MiArenaPrefix>()).memid = memid;
        (*arena.cast::<MiArenaPrefix>()).is_exclusive = exclusive;
        (*arena.cast::<MiArenaPrefix>()).slice_count = slice_count_usize;
        (*arena.cast::<MiArenaPrefix>()).info_slices = info_slices;
        let numa_node = if numa_node.is_any()
            && mi_option_is_enabled(mi_option_t::mi_option_arena_is_numa_local)
        {
            crate::os::_mi_os_numa_node_typed()
        } else {
            numa_node
        };
        mi_arena_set_numa_node(arena, numa_node);
        (*arena.cast::<MiArenaPrefix>())
            .purge_expire
            .store(0, Ordering::Relaxed);
        (*arena.cast::<MiArenaPrefix>()).commit_fun = commit_fun;
        (*arena.cast::<MiArenaPrefix>()).commit_fun_arg = commit_fun_arg;
        (*arena.cast::<MiArenaPrefix>()).parent = parent;
        (*arena.cast::<MiArenaPrefix>()).total_size = total_size.get();
    }

    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    let mut base = unsafe {
        mi_arena_start(ArenaPtr::new_unchecked(arena))
            .as_ptr()
            .add(bitmap_base)
    };
    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_free = mi_arena_bbitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_committed =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_dirty = mi_arena_bitmap_init(slice_count, &mut base)
    };
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).slices_purge = mi_arena_bitmap_init(slice_count, &mut base)
    };
    // SAFETY[AUDIT:arena-initialization-page-table-boundary]: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
    unsafe {
        (*arena.cast::<MiArenaPrefix>()).pages_main.pages =
            mi_arena_bitmap_init(slice_count, &mut base)
    };
    for i in 0..MI_ARENA_BIN_COUNT {
        // SAFETY[AUDIT:arena-initialization-page-table-boundary]: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_main.pages_abandoned[i] =
                mi_arena_bitmap_init(slice_count, &mut base)
        };
    }
    if crate::internal::MI_PAGE_META_IS_SEPARATED != 0 {
        // SAFETY[AUDIT:arena-initialization-page-table-boundary]: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
        unsafe {
            (*arena.cast::<MiArenaPrefix>()).pages_meta = base.cast::<mi_page_t>();
            base = base.add(slice_count_usize * size_of::<mi_page_t>());
        }
    } else {
        // SAFETY[AUDIT:arena-initialization-page-table-boundary]: arena initialization lays out page table bitmaps and optional separated page metadata within the reserved slice range.
        unsafe { (*arena.cast::<MiArenaPrefix>()).pages_meta = ptr::null_mut() };
    }
    crate::internal::mi_assert_internal!(
        mi_size_of_slices(info_slices)
            >= unsafe {
                base.offset_from(
                    mi_arena_start(ArenaPtr::new_unchecked(arena))
                        .as_ptr()
                        .cast_const(),
                ) as usize
            }
    );

    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    unsafe {
        mi_bbitmap_unsafe_setN(
            (*arena.cast::<MiArenaPrefix>()).slices_free,
            info_slices,
            (*arena.cast::<MiArenaPrefix>()).slice_count - info_slices,
        )
    };
    if memid.initially_committed {
        // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }
    if !memid.initially_zero {
        // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
        unsafe {
            mi_bitmap_unsafe_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                0,
                (*arena.cast::<MiArenaPrefix>()).slice_count,
            )
        };
    }

    // SAFETY[AUDIT:arena-initialization-boundary]: arena initialization lays out metadata, bitmaps, page tables, guard pages, and registry state within the reserved slice range.
    if !unsafe { mi_arenas_add(subproc, ArenaPtr::new_unchecked(arena), arena_id) } {
        return ptr::null_mut();
    }
    arena
}

#[allow(clippy::too_many_arguments)]
unsafe fn mi_arenas_try_alloc(
    heap: BorrowedHeap,
    slice_count: crate::core_types::SliceCount,
    alignment: crate::core_types::Alignment,
    commit: bool,
    allow_large: bool,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    numa_node: crate::core_types::NumaNode,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    crate::internal::mi_assert!(alignment.get() <= MI_ARENA_SLICE_ALIGN);
    if (slice_count.get() as usize).saturating_mul(MI_ARENA_SLICE_SIZE) > MI_ARENA_MAX_SIZE {
        return ptr::null_mut();
    }
    let mut allocation = MaybeUninit::<crate::core_types::ArenaAllocation>::uninit();

    // SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
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
            allocation.as_mut_ptr(),
        )
    };
    if !p.is_null() {
        // SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        unsafe { *memid = allocation.assume_init().memid };
        return p;
    }
    if req_arena.is_some() || _mi_preloading() {
        return ptr::null_mut();
    }
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc) {
        return ptr::null_mut();
    }

    // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
    let Some(subproc) = (unsafe { heap.ptr().subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return ptr::null_mut();
    };
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc) };
    {
        // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
        let _lock = unsafe { MiLockGuard::acquire(subproc.arena_reserve_lock()) };
        if arena_count == unsafe { mi_arenas_get_count_typed(subproc) } {
            let mut arena_id = unsafe { _mi_arena_id_none() };
            let _ = unsafe {
                mi_arena_reserve(
                    subproc,
                    crate::core_types::ReservedBytes::new(mi_size_of_slices(
                        slice_count.get() as usize
                    )),
                    crate::core_types::NumaNode::any(),
                    allow_large,
                    &mut arena_id,
                )
            };
        }
    }

    // SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
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
            allocation.as_mut_ptr(),
        )
    };
    if !p.is_null() {
        // SAFETY[AUDIT:arena-slice-allocation-dispatch-boundary]: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        unsafe { *memid = allocation.assume_init().memid };
    }
    p
}

#[derive(Clone, Copy)]
struct ArenaOsAllocRequest {
    size: usize,
    alignment: usize,
    align_offset: usize,
}

unsafe fn mi_arena_os_alloc_aligned(
    request: ArenaOsAllocRequest,
    commit: bool,
    allow_large: bool,
    req_arena_id: mi_arena_id_t,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc)
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        || req_arena_id != unsafe { _mi_arena_id_none() }
    {
        unsafe { mi_set_errno(ENOMEM) };
        return ptr::null_mut();
    }

    if request.align_offset > 0 {
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        unsafe {
            _mi_os_alloc_aligned_at_offset(
                request.size,
                request.alignment,
                request.align_offset,
                commit,
                allow_large,
                memid,
            )
        }
    } else {
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        unsafe { _mi_os_alloc_aligned(request.size, request.alignment, commit, allow_large, memid) }
    }
}

struct ArenaAllocTypedArgs {
    heap: Option<HeapPtr>,
    layout: crate::core_types::LayoutRequest,
    align_offset: crate::core_types::AlignmentOffset,
    arena_slice_count: Option<crate::core_types::SliceCount>,
    commit: bool,
    allow_large: bool,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    numa_node: crate::core_types::NumaNode,
    memid: *mut mi_memid_t,
}

#[inline(always)]
fn mi_arena_slice_count_for_request(
    size: crate::core_types::RequestedSize,
) -> Option<crate::core_types::SliceCount> {
    crate::core_types::SliceCount::from_usize(mi_slice_count_of_size(size.get()))
}

unsafe fn mi_arenas_alloc_aligned_typed(args: ArenaAllocTypedArgs) -> *mut c_void {
    let ArenaAllocTypedArgs {
        heap,
        layout,
        align_offset,
        arena_slice_count,
        commit,
        allow_large,
        req_arena,
        tseq,
        numa_node,
        memid,
    } = args;
    let size = layout.requested();
    let alignment = layout.alignment();

    if !mi_option_is_enabled(mi_option_t::mi_option_disallow_arena_alloc)
        && size.get() >= MI_ARENA_MIN_OBJ_SIZE
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        && size.get() <= unsafe { mi_arena_max_object_size() }
        && alignment.get() <= MI_ARENA_SLICE_ALIGN
        && align_offset.get() == 0
        && let (Some(slice_count), Some(heap)) = (arena_slice_count, heap)
    {
        let arena_alignment = crate::core_types::Alignment::new(alignment.get())
            // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
            .unwrap_or_else(|| unsafe { crate::core_types::Alignment::new_unchecked(1) });
        let p = unsafe {
            mi_arenas_try_alloc(
                BorrowedHeap::new_unchecked(heap),
                slice_count,
                arena_alignment,
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

    // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
    unsafe {
        mi_arena_os_alloc_aligned(
            ArenaOsAllocRequest {
                size: size.get(),
                alignment: alignment.get(),
                align_offset: align_offset.get(),
            },
            commit,
            allow_large,
            req_arena
                .map(ArenaPtr::as_ptr)
                .unwrap_or(ptr::null_mut())
                .cast::<c_void>(),
            memid,
        )
    }
}

/// # Safety
/// Internal arena allocator wrapper following the C ABI.
// SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
    crate::internal::mi_assert_internal!(!memid.is_null());
    crate::internal::mi_assert_internal!(size > 0);

    match crate::core_types::LayoutRequest::aligned_at(
        size,
        alignment,
        crate::core_types::AlignmentOffset::new(align_offset),
        crate::core_types::Zeroed::No,
    ) {
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        Ok(layout) => unsafe {
            mi_arenas_alloc_aligned_typed(ArenaAllocTypedArgs {
                heap: HeapPtr::new(heap),
                layout,
                align_offset: crate::core_types::AlignmentOffset::new(align_offset),
                arena_slice_count: mi_arena_slice_count_for_request(layout.requested()),
                commit,
                allow_large,
                req_arena: ArenaPtr::new(req_arena),
                tseq: crate::core_types::ThreadSeq::new(tseq),
                numa_node: crate::core_types::NumaNode::new(numa_node),
                memid,
            })
        },
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        Err(_) => unsafe {
            mi_arena_os_alloc_aligned(
                ArenaOsAllocRequest {
                    size,
                    alignment,
                    align_offset,
                },
                commit,
                allow_large,
                req_arena.cast::<c_void>(),
                memid,
            )
        },
    }
}

struct ArenaAllocArgs {
    heap: Option<HeapPtr>,
    size: crate::core_types::RequestedSize,
    arena_slice_count: Option<crate::core_types::SliceCount>,
    commit: bool,
    allow_large: bool,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    numa_node: crate::core_types::NumaNode,
    memid: *mut mi_memid_t,
}

unsafe fn mi_arenas_alloc_typed(args: ArenaAllocArgs) -> *mut c_void {
    let ArenaAllocArgs {
        heap,
        size,
        arena_slice_count,
        commit,
        allow_large,
        req_arena,
        tseq,
        numa_node,
        memid,
    } = args;
    let alignment = MI_ARENA_SLICE_SIZE;

    match crate::core_types::LayoutRequest::aligned(size.get(), alignment) {
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        Ok(layout) => unsafe {
            mi_arenas_alloc_aligned_typed(ArenaAllocTypedArgs {
                heap,
                layout,
                align_offset: crate::core_types::AlignmentOffset::new(0),
                arena_slice_count,
                commit,
                allow_large,
                req_arena,
                tseq,
                numa_node,
                memid,
            })
        },
        // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        Err(_) => unsafe {
            mi_arena_os_alloc_aligned(
                ArenaOsAllocRequest {
                    size: size.get(),
                    alignment,
                    align_offset: 0,
                },
                commit,
                allow_large,
                req_arena
                    .map(ArenaPtr::as_ptr)
                    .unwrap_or(ptr::null_mut())
                    .cast::<c_void>(),
                memid,
            )
        },
    }
}

/// # Safety
/// Internal arena allocator wrapper following the C ABI.
// SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
    crate::internal::mi_assert_internal!(!memid.is_null());
    crate::internal::mi_assert_internal!(size > 0);
    let size = crate::core_types::RequestedSize::new(size);

    // SAFETY[AUDIT:arena-os-fallback-boundary]: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
    unsafe {
        mi_arenas_alloc_typed(ArenaAllocArgs {
            heap: HeapPtr::new(heap),
            size,
            arena_slice_count: mi_arena_slice_count_for_request(size),
            commit,
            allow_large,
            req_arena: ArenaPtr::new(req_arena),
            tseq: crate::core_types::ThreadSeq::new(tseq),
            numa_node: crate::core_types::NumaNode::new(numa_node),
            memid,
        })
    }
}

unsafe fn mi_arenas_page_try_find_abandoned(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
) -> *mut mi_page_t {
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    let heap = unsafe { _mi_theap_heap(theap) };
    let tseq = crate::core_types::ThreadSeq::new(unsafe { (*(*theap).tld).thread_seq });
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let slice_count_usize = slice_count.get() as usize;

    let bin = mi_bin_from_block_size(block_size);
    crate::internal::mi_assert_internal!(bin.as_usize() < MI_BIN_COUNT);
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    if unsafe { mi_heap_abandoned_count(heap, bin).load_relaxed() } == 0 {
        return ptr::null_mut();
    }

    let any_numa = crate::core_types::NumaNode::any();
    let mut result = ptr::null_mut();
    let claim_fun = ThreadLocalClaimCallback::new(mi_arena_try_claim_abandoned);
    // SAFETY[AUDIT:arena-abandoned-boundary]: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    unsafe {
        let Some(heap_ptr) = HeapPtr::new(heap) else {
            crate::internal::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        let borrowed_heap = BorrowedHeap::new_unchecked(heap_ptr);
        mi_forall_suitable_arenas(
            borrowed_heap,
            ArenaPtr::new(req_arena),
            tseq,
            true,
            any_numa,
            true,
            |arena| {
                let arena_raw = arena.as_ptr();
                let arena_pages = mi_heap_arena_pages(borrowed_heap, arena);
                if arena_pages.is_null() {
                    return true;
                }
                let mut slice_index = SliceIndex::new(0);
                let bitmap = (*arena_pages).pages_abandoned[bin.as_usize()];
                if mi_bitmap_try_find_and_claim(
                    bitmap,
                    tseq,
                    &mut slice_index,
                    claim_fun,
                    Some(arena),
                ) {
                    let page = mi_arena_page_at_slice(arena, slice_index);
                    let page_raw = page.as_ptr();
                    crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
                    crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
                    crate::internal::mi_assert_internal!(mi_heap_has_page(
                        BorrowedHeap::new_unchecked(HeapPtr::new_unchecked(heap)),
                        arena,
                        page,
                    ));
                    mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1);
                    (*theap).stats.pages_reclaim_on_alloc.total += 1;
                    crate::internal::__mi_stat_decrease(
                        ptr::addr_of_mut!((*theap).stats.pages_abandoned),
                        1,
                    );
                    _mi_page_free_collect(page_raw, false);
                    crate::internal::mi_assert_internal!(mi_bbitmap_is_clearN(
                        (*arena_raw.cast::<MiArenaPrefix>()).slices_free,
                        slice_index.get(),
                        slice_count_usize,
                    ));
                    let committed = crate::page::mi_page_slice_committed(page_raw);
                    crate::internal::mi_assert_internal!(
                        !committed.is_zero()
                            || mi_bitmap_is_setN(
                                (*arena_raw.cast::<MiArenaPrefix>()).slices_committed,
                                slice_index.get(),
                                slice_count_usize,
                            )
                    );
                    crate::internal::mi_assert_internal!(mi_bitmap_is_setN(
                        (*arena_raw.cast::<MiArenaPrefix>()).slices_dirty,
                        slice_index.get(),
                        slice_count_usize,
                    ));
                    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
                        mi_page_slice_start(page_raw).cast(),
                        MI_PAGE_ALIGN
                    ));
                    crate::internal::mi_assert_internal!(
                        crate::internal::_mi_ptr_page(mi_page_start(page_raw).cast()) == page_raw
                    );
                    crate::internal::mi_assert_internal!(
                        mi_page_block_size(page_raw) == block_size.get()
                    );
                    crate::internal::mi_assert_internal!(!mi_page_is_full_local(page_raw));
                    result = page_raw;
                    false
                } else {
                    true
                }
            },
        )
    };
    result
}

unsafe fn mi_arenas_page_alloc_fresh_area(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    _block_size: crate::core_types::BlockSize,
    block_alignment: crate::core_types::MaybeAlignment,
    os_align: bool,
    commit: bool,
    memid: *mut mi_memid_t,
) -> *mut u8 {
    let block_alignment = block_alignment.get();
    let allow_large = true;
    let page_alignment = MI_ARENA_SLICE_ALIGN;
    // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
    let heap = unsafe { _mi_theap_heap(theap) };
    let tld = unsafe { (*theap).tld };
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let heap_numa =
        crate::core_types::NumaNode::new(unsafe { (*heap.cast::<MiHeapPrefix>()).numa_node });
    // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
    let tld_numa = crate::core_types::NumaNode::new(unsafe { (*tld).numa_node });
    let numa_node = if !heap_numa.is_any() {
        heap_numa
    } else {
        tld_numa
    };

    let mut start = ptr::null_mut::<u8>();
    // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
    unsafe { *memid = _mi_memid_none() };
    let slice_count_usize = slice_count.get() as usize;
    let alloc_size = mi_size_of_slices(slice_count_usize);
    if !mi_option_is_enabled(mi_option_t::mi_option_disallow_arena_alloc)
        && !os_align
        // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
        && slice_count_usize <= unsafe { mi_arena_max_object_size() } / MI_ARENA_SLICE_SIZE
    {
        let Some(heap_ptr) = HeapPtr::new(heap) else {
            crate::internal::mi_assert_internal!(false);
            return ptr::null_mut();
        };
        // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
        let page_alignment = unsafe { crate::core_types::Alignment::new_unchecked(page_alignment) };
        start = unsafe {
            mi_arenas_try_alloc(
                BorrowedHeap::new_unchecked(heap_ptr),
                slice_count,
                page_alignment,
                commit,
                allow_large,
                ArenaPtr::new(req_arena),
                crate::core_types::ThreadSeq::new((*tld).thread_seq),
                numa_node,
                memid,
            )
            .cast::<u8>()
        };
        if !start.is_null() {
            // SAFETY[AUDIT:arena-heap-page-publication-boundary]: heap arena-page publication installs or updates per-heap arena page bitmaps before page ownership becomes visible.
            match crate::core_types::MemId::view(unsafe { *memid }) {
                crate::core_types::MemIdView::Arena(a) => {
                    let arena = a.arena.as_ptr();
                    let arena_pages = unsafe { mi_heap_ensure_arena_pages_from_raw(heap, arena) };
                    if arena_pages.is_null() {
                        // SAFETY[AUDIT:arena-heap-page-publication-boundary]: heap arena-page publication installs or updates per-heap arena page bitmaps before page ownership becomes visible.
                        unsafe { _mi_arenas_free(start.cast(), alloc_size, *memid) };
                        start = ptr::null_mut();
                    } else {
                        let slice_idx = a.range.start().0;
                        crate::internal::mi_assert_internal!(unsafe {
                            !mi_bitmap_is_setN((*arena_pages).pages, slice_idx, 1)
                        });
                        // SAFETY[AUDIT:arena-heap-page-publication-boundary]: heap arena-page publication installs or updates per-heap arena page bitmaps before page ownership becomes visible.
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
            // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
            unsafe {
                mi_arena_os_alloc_aligned(
                    ArenaOsAllocRequest {
                        size: alloc_size,
                        alignment: block_alignment,
                        align_offset: page_alignment,
                    },
                    commit,
                    allow_large,
                    req_arena.cast(),
                    memid,
                )
                .cast::<u8>()
            }
        } else {
            // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
            unsafe {
                mi_arena_os_alloc_aligned(
                    ArenaOsAllocRequest {
                        size: alloc_size,
                        alignment: page_alignment,
                        align_offset: 0,
                    },
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
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        start.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(
        !os_align
            || crate::internal::_mi_is_aligned(
                // SAFETY[AUDIT:arena-page-fresh-boundary]: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
                unsafe { start.add(page_alignment) }.cast(),
                block_alignment
            )
    );
    start
}

unsafe fn mi_page_block_start(block_size: crate::core_types::BlockSize, os_align: bool) -> usize {
    let block_size = block_size.get();
    if os_align {
        MI_PAGE_ALIGN
    } else if crate::internal::_mi_is_power_of_two(block_size)
        && block_size <= MI_PAGE_MAX_START_BLOCK_ALIGN2
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
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
    block_alignment: crate::core_types::MaybeAlignment,
    commit: bool,
) -> *mut mi_page_t {
    let block_size_raw = block_size.get();
    let block_alignment_raw = block_alignment.get();
    let os_align = block_alignment_raw > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let slice_count_usize = slice_count.get() as usize;
    let alloc_size = mi_size_of_slices(slice_count_usize);
    // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
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
        // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
        if unsafe { !(*arena.cast::<MiArenaPrefix>()).pages_meta.is_null() } {
            let page_meta = unsafe {
                (*arena.cast::<MiArenaPrefix>())
                    .pages_meta
                    .add(a.range.start().0)
            };
            // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
            crate::internal::mi_assert_internal!(unsafe { (*page_meta).block_size == 0 });
            if block_size_raw > MI_SMALL_SIZE_MAX || MI_PAGE_META_ALIGNED_FREE_SMALL == 0 {
                page = page_meta;
                page_meta_is_separate = true;
                block_start = 0;
                if block_size_raw >= size_of::<usize>()
                    && block_size_raw <= MI_PAGE_BLOCK_START_MAX_OFFSET
                    && crate::internal::_mi_is_power_of_two(block_size_raw)
                {
                    block_start += block_size_raw;
                }
                // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
                unsafe {
                    crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>())
                };
            }
        }
    }
    if page.is_null() {
        page = slice_start.cast::<mi_page_t>();
        // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
        block_start = unsafe { mi_page_block_start(block_size, os_align) };
    }

    let mut commit_size = crate::core_types::CommittedBytes::new(0);
    if !memid.initially_committed {
        let mut commit_size_raw =
            _mi_align_up(block_start + block_size_raw, MI_PAGE_MIN_COMMIT_SIZE);
        if commit_size_raw > page_noguard_size {
            commit_size_raw = page_noguard_size;
        }
        commit_size = crate::core_types::CommittedBytes::new(commit_size_raw);
        let mut is_zero = false;
        // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
        if !unsafe {
            let (arena_ptr, range) = match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => (a.arena, a.range),
                _ => {
                    crate::internal::mi_assert_internal!(false);
                    _mi_arenas_free(slice_start.cast(), alloc_size, memid);
                    return ptr::null_mut();
                }
            };
            mi_arena_commit(
                arena_ptr,
                range,
                commit_size,
                &mut is_zero,
                crate::core_types::CommittedBytes::new(0),
            )
        } {
            // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
            unsafe { _mi_arenas_free(slice_start.cast(), alloc_size, memid) };
            return ptr::null_mut();
        }
    }
    if !memid.initially_zero && !page_meta_is_separate {
        // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
        unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
    }

    if memid.initially_committed {
        if memid.initially_zero {
            // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
            unsafe {
                mi_track_mem_defined(slice_start.cast(), slice_count_usize * MI_ARENA_SLICE_SIZE)
            };
        } else {
            unsafe {
                mi_track_mem_undefined(slice_start.cast(), slice_count_usize * MI_ARENA_SLICE_SIZE)
            };
        }
    }
    if MI_DEBUG > 1
        && memid.initially_zero
        && memid.initially_committed
        // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
        && !unsafe { mi_mem_is_zero(slice_start.cast(), page_noguard_size) }
    {
        crate::options::_mi_error_message_fmt(
            Errno::EFAULT,
            format_args!("internal error: page memory was not zero initialized.\n"),
        );
        memid.initially_zero = false;
        if block_start > 0 {
            // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
            unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
        }
    }
    let reserved = if os_align {
        1
    } else {
        (page_noguard_size - block_start) / block_size_raw
    };
    crate::internal::mi_assert_internal!(reserved > 0 && u16::try_from(reserved).is_ok());

    // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
    unsafe {
        crate::page::mi_page_set_reserved_count(
            page,
            crate::core_types::ReservedBlockCount::new(reserved as u16),
        )
    };
    // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
    unsafe { (*page).page_start = slice_start.add(block_start) };
    unsafe { (*page).block_size = block_size_raw };
    unsafe { crate::page::mi_page_set_slice_committed(page, commit_size) };
    unsafe { (*page).memid = memid };
    unsafe { (*page).free_is_zero = memid.initially_zero };
    // SAFETY[AUDIT:arena-page-fresh-metadata-boundary]: fresh arena page setup initializes metadata, commit state, and layout counters.
    crate::internal::mi_assert_internal!(unsafe { (*page).free.is_null() });
    crate::internal::mi_assert_internal!(
        page_meta_is_separate == unsafe { mi_page_meta_is_separated(page) }
    );
    crate::internal::mi_assert_internal!(unsafe { mi_page_slice_start(page) } == slice_start);

    // SAFETY[AUDIT:arena-page-ownership-transition-boundary]: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    unsafe {
        (*page).xthread_free = crate::core_types::ThreadFreeWord::from_parts(
            None,
            crate::core_types::ThreadFreeOwner::Unowned,
        )
        .raw()
    };
    // SAFETY[AUDIT:arena-page-ownership-transition-boundary]: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    let claimed = unsafe { page_ptr.thread_free_cell().claim_acq_rel() };
    crate::internal::mi_assert_internal!(claimed);
    if !unsafe { _mi_page_map_register(page) } {
        unsafe { _mi_arenas_free(slice_start.cast(), alloc_size, memid) };
        return ptr::null_mut();
    }

    // SAFETY[AUDIT:arena-page-ownership-transition-boundary]: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    unsafe { __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.pages), 1) };
    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    unsafe { __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.page_bins[stats_bin]), 1) };

    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY[AUDIT:arena-page-ownership-transition-boundary]: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY[AUDIT:arena-page-ownership-transition-boundary]: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    crate::internal::mi_assert_internal!(unsafe { mi_page_block_size(page) } == block_size_raw);
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    page
}

unsafe fn mi_arenas_page_regular_alloc(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
) -> *mut mi_page_t {
    // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    let mut page = unsafe { mi_arenas_page_try_find_abandoned(theap, slice_count, block_size) };
    if !page.is_null() {
        return page;
    }

    let commit_on_demand =
        crate::options::mi_option_get(mi_option_t::mi_option_page_commit_on_demand);
    let slice_count_usize = slice_count.get() as usize;
    let commit = slice_count_usize <= mi_slice_count_of_size(MI_PAGE_MIN_COMMIT_SIZE)
        // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        || (commit_on_demand == 2 && unsafe { _mi_os_has_overcommit() })
        || commit_on_demand == 0;
    page = unsafe {
        mi_arenas_page_alloc_fresh(
            theap,
            slice_count,
            block_size,
            crate::core_types::MaybeAlignment::new_unchecked(0),
            commit,
        )
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    crate::internal::mi_assert_internal!(match crate::core_types::MemId::view(unsafe {
        (*page).memid
    }) {
        crate::core_types::MemIdView::Arena(a) => a.range.count() == slice_count,
        _ => true, // Non-arena allocations trivially pass this slice_count assert
    });
    // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    if !unsafe { crate::page::_mi_page_init(theap, page) } {
        unsafe {
            _mi_arenas_free(
                page.cast(),
                mi_page_full_size(PagePtr::new_unchecked(page)).get(),
                (*page).memid,
            )
        };
        return ptr::null_mut();
    }
    page
}

#[inline(never)]
unsafe fn mi_arenas_page_singleton_alloc(
    theap: *mut mi_theap_t,
    block_size: crate::core_types::BlockSize,
    block_alignment: crate::core_types::MaybeAlignment,
) -> *mut mi_page_t {
    let block_size_raw = block_size.get();
    let block_alignment_raw = block_alignment.get();
    let os_align = block_alignment_raw > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let info_size = if os_align {
        MI_PAGE_ALIGN
    } else {
        crate::internal::mi_page_info_size()
    };
    let slice_count = mi_slice_count_of_size(info_size + block_size_raw);
    let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
        // SAFETY[AUDIT:arena-page-singleton-boundary]: singleton arena page setup handles over-aligned or large objects while preserving one-block page invariants.
        unsafe { mi_set_errno(ENOMEM) };
        return ptr::null_mut();
    };
    let page = unsafe {
        mi_arenas_page_alloc_fresh(theap, slice_count, block_size, block_alignment, true)
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    // SAFETY[AUDIT:arena-page-singleton-boundary]: singleton arena page setup handles over-aligned or large objects while preserving one-block page invariants.
    crate::internal::mi_assert_internal!(unsafe {
        crate::page::mi_page_reserved_count(page).get() == 1
    });
    if !unsafe { crate::page::_mi_page_init(theap, page) } {
        unsafe {
            _mi_arenas_free(
                page.cast(),
                mi_page_full_size(PagePtr::new_unchecked(page)).get(),
                (*page).memid,
            )
        };
        return ptr::null_mut();
    }
    page
}

/// # Safety
/// Internal arena page allocator matching the C ABI.
// SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_page_alloc(
    theap: *mut mi_theap_t,
    block_size: usize,
    block_alignment: usize,
) -> *mut mi_page_t {
    // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    let block_size_typed = unsafe { crate::core_types::BlockSize::new_unchecked(block_size) };
    let block_alignment_typed =
        unsafe { crate::core_types::MaybeAlignment::new_unchecked(block_alignment) };
    let page = if block_alignment > MI_PAGE_MAX_OVERALLOC_ALIGN {
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_power_of_two(block_alignment));
        // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        unsafe { mi_arenas_page_singleton_alloc(theap, block_size_typed, block_alignment_typed) }
    } else if block_size <= MI_SMALL_MAX_OBJ_SIZE {
        unsafe { mi_arenas_page_regular_alloc(theap, MI_SMALL_PAGE_SLICE_COUNT, block_size_typed) }
    } else if block_size <= MI_MEDIUM_MAX_OBJ_SIZE {
        unsafe { mi_arenas_page_regular_alloc(theap, MI_MEDIUM_PAGE_SLICE_COUNT, block_size_typed) }
    } else if MI_ENABLE_LARGE_PAGES != 0 && block_size <= MI_LARGE_MAX_OBJ_SIZE {
        // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        unsafe { mi_arenas_page_regular_alloc(theap, MI_LARGE_PAGE_SLICE_COUNT, block_size_typed) }
    } else {
        unsafe { mi_arenas_page_singleton_alloc(theap, block_size_typed, block_alignment_typed) }
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::internal::mi_assert_internal!(
        block_alignment <= MI_PAGE_MAX_OVERALLOC_ALIGN
            || crate::internal::_mi_is_aligned(
                // SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
                unsafe { mi_page_start(page) }.cast(),
                block_alignment
            )
    );
    page
}

/// # Safety
/// Internal arena page free path matching the C ABI.
// SAFETY[AUDIT:arena-page-free-entry-boundary]: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
pub(crate) unsafe extern "C" fn _mi_arenas_page_free(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY[AUDIT:arena-page-free-entry-boundary]: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
    let abandoned = unsafe { crate::core_types::AbandonedPage::new_unchecked(page_ptr) };
    let all_free = unsafe { crate::core_types::AllFreePage::abandoned_unchecked(abandoned) };
    let current_theap = if current_theapx.is_null() {
        None
    } else {
        // SAFETY[AUDIT:arena-page-free-entry-boundary]: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(current_theapx),
            )
        })
    };
    // SAFETY[AUDIT:arena-page-free-entry-boundary]: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
    unsafe { _mi_arenas_page_free_internal(&all_free, current_theap) };
}

unsafe fn _mi_arenas_page_free_internal<S>(
    page_wrapper: &crate::core_types::AllFreePage<S>,
    current_theap: Option<crate::core_types::ThreadHeapLocal>,
) {
    let page = page_wrapper.page().as_ptr();
    let current_theapx = current_theap
        .map(|theap| theap.ptr().as_ptr())
        .unwrap_or(ptr::null_mut());
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_all_free(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe {
        (*page).next.is_null() && (*page).prev.is_null()
    });

    // SAFETY[AUDIT:arena-page-free-stats-boundary]: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    if !current_theapx.is_null() {
        unsafe {
            crate::internal::__mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.page_bins[stats_bin]),
                1,
            )
        };
        // SAFETY[AUDIT:arena-page-free-stats-boundary]: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
        unsafe {
            crate::internal::__mi_stat_decrease(ptr::addr_of_mut!((*current_theapx).stats.pages), 1)
        };
    } else {
        let heap = unsafe { mi_page_heap(page) };
        // SAFETY[AUDIT:arena-page-free-stats-boundary]: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
        unsafe {
            crate::internal::__mi_stat_decrease_mt(&mut (*heap).stats.page_bins[stats_bin], 1)
        };
        unsafe { crate::internal::__mi_stat_decrease_mt(&mut (*heap).stats.pages, 1) };
    }

    if MI_DEBUG > 1
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        && unsafe { mi_page_memid_is_arena(page) }
        && !unsafe { mi_page_is_full_local(page) }
    {
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        let arena = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        let committed = unsafe { crate::page::mi_page_slice_committed(page) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        crate::internal::mi_assert_internal!(!unsafe {
            mi_bitmap_is_setN(
                (*arena_pages).pages_abandoned[bin.as_usize()],
                slice_index,
                1,
            )
        });
        // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN((*arena_pages).pages, slice_index, 1)
        });
    }

    // SAFETY[AUDIT:arena-page-free-validation-boundary]: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
    unsafe { _mi_page_map_unregister(page) };
    if unsafe { mi_page_memid_is_arena(page) } {
        let mut arena_pages = ptr::null_mut();
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        // SAFETY[AUDIT:arena-page-free-release-boundary]: free release clears arena metadata and returns backing slices after ownership is proven.
        let arena = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        crate::internal::mi_assert_internal!(!arena_pages.is_null());
        // SAFETY[AUDIT:arena-page-free-release-boundary]: free release clears arena metadata and returns backing slices after ownership is proven.
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        let committed = unsafe { crate::page::mi_page_slice_committed(page) };
        if !committed.is_zero() {
            let total_slices = committed.get() / MI_ARENA_SLICE_SIZE;
            crate::internal::mi_assert_internal!(slice_count >= total_slices);
            if total_slices > 0 {
                // SAFETY[AUDIT:arena-page-free-release-boundary]: free release clears arena metadata and returns backing slices after ownership is proven.
                unsafe {
                    mi_bitmap_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        total_slices,
                        ptr::null_mut(),
                    )
                };
            }
            let extra = committed.get() % MI_ARENA_SLICE_SIZE;
            if extra > 0 {
                // SAFETY[AUDIT:arena-page-free-stats-boundary]: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
                unsafe {
                    crate::internal::__mi_stat_decrease_mt(
                        &mut (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed,
                        crate::core_types::CommittedBytes::new(extra).get(),
                    )
                };
            }
        } else {
            // SAFETY[AUDIT:arena-page-free-release-boundary]: free release clears arena metadata and returns backing slices after ownership is proven.
            crate::internal::mi_assert_internal!(unsafe {
                mi_bitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            });
        }
    }
    // SAFETY[AUDIT:arena-page-free-release-boundary]: free release clears arena metadata and returns backing slices after ownership is proven.
    if unsafe { mi_page_meta_is_separated(page) } {
        unsafe { (*page).block_size = 0 };
    }
    unsafe {
        _mi_arenas_free(
            mi_page_slice_start(page).cast(),
            mi_page_full_size(PagePtr::new_unchecked(page)).get(),
            (*page).memid,
        )
    };
}

/// # Safety
/// `page` must be an all-free page that has been removed from its page queue.
/// `owner` must be the thread heap that owned the page before release.
pub(crate) unsafe fn _mi_arenas_page_free_typed<S>(
    page: crate::core_types::AllFreePage<S>,
    owner: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::DestroyedPage {
    // SAFETY[AUDIT:arena-page-free-release-boundary]: free release clears arena metadata and returns backing slices after ownership is proven.
    unsafe { _mi_arenas_page_free(page.page().as_ptr(), owner.ptr().as_ptr()) };
    page.free_to_arena(owner)
}

/// # Safety
/// Internal arena abandon path matching the C ABI.
// SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
pub(crate) unsafe extern "C" fn _mi_arenas_page_abandon(
    page: *mut mi_page_t,
    current_theap: *mut mi_theap_t,
) {
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    let page_wrapper = if unsafe { mi_page_is_full_local(page) } {
        unsafe { crate::core_types::FullPage::new_unchecked(page_ptr).for_abandon() }
    } else {
        unsafe { crate::core_types::ActivePage::new_unchecked(page_ptr).for_abandon() }
    };
    // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    let current_theap = unsafe {
        crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(current_theap),
        )
    };
    // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    unsafe { _mi_arenas_page_abandon_internal(page_wrapper, current_theap) };
}

unsafe fn _mi_arenas_page_abandon_internal(
    page_wrapper: crate::core_types::PageForAbandon,
    current_theap: crate::core_types::ThreadHeapLocal,
) {
    let page = match page_wrapper {
        crate::core_types::PageForAbandon::Active(page) => page.page().as_ptr(),
        crate::core_types::PageForAbandon::Full(page) => page.page().as_ptr(),
    };
    let current_theap = current_theap.ptr().as_ptr();
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_all_free(page) });
    crate::internal::mi_assert_internal!(unsafe {
        (*page).next.is_null() && (*page).prev.is_null()
    });

    // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    let heap = unsafe { mi_page_heap(page) };
    crate::internal::mi_assert_internal!(heap == unsafe { _mi_theap_heap(current_theap) });
    if unsafe { mi_page_memid_is_arena(page) } && !unsafe { mi_page_is_full_local(page) } {
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        let arena = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        let _ = arena;
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        crate::internal::mi_assert_internal!(!unsafe { mi_page_is_singleton(page) });
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        let committed = unsafe { crate::page::mi_page_slice_committed(page) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index,
                slice_count,
            )
        });

        let atomic_tid =
            // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
            crate::core_types::AtomicPageThreadId(crate::core_types::AtomicUsizeField(unsafe {
                ptr::addr_of_mut!((*page).xthread_id)
            }));
        unsafe { atomic_tid.store_owner(crate::core_types::PageOwnerId::AbandonedMapped) };
        let was_clear =
            // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
            unsafe { mi_bitmap_set((*arena_pages).pages_abandoned[bin.as_usize()], slice_index) };
        crate::internal::MI_UNUSED(&was_clear);
        crate::internal::mi_assert_internal!(was_clear);
        unsafe { mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1) };
        unsafe { __mi_stat_increase(ptr::addr_of_mut!((*current_theap).stats.pages_abandoned), 1) };
    } else {
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        if !unsafe { mi_page_memid_is_arena(page) }
            && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned)
        {
            unsafe {
                mi_lock_acquire(ptr::addr_of_mut!(
                    (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
                ))
            };
            // SAFETY[AUDIT:arena-page-abandon-os-list-boundary]: arena page abandon links non-arena pages into the owning heap OS-abandoned list under its lock.
            unsafe {
                (*page).prev = ptr::null_mut();
                (*page).next = (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages;
                if !(*page).next.is_null() {
                    (*(*page).next).prev = page;
                }
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages = page;
            }
            // SAFETY[AUDIT:arena-page-abandon-os-list-boundary]: arena page abandon links non-arena pages into the owning heap OS-abandoned list under its lock.
            unsafe {
                mi_lock_release(ptr::addr_of_mut!(
                    (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
                ))
            };
        }
        // SAFETY[AUDIT:arena-page-abandon-boundary]: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
        unsafe { __mi_stat_increase(ptr::addr_of_mut!((*current_theap).stats.pages_abandoned), 1) };
    }
    unsafe {
        mi_abandoned_page_unown(
            crate::core_types::AbandonedPage::new_unchecked(PagePtr::new_unchecked(page)),
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(current_theap),
            ),
        )
    };
}

/// # Safety
/// Called from the free path on pages already associated with an arena.
// SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_page_try_reabandon_to_mapped(
    page: *mut mi_page_t,
) -> bool {
    let page_raw = page;
    // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    let page = unsafe {
        crate::core_types::OwnedAbandonedPage::new_unchecked(PagePtr::new_unchecked(page))
    };
    let arena_memid = match crate::core_types::MemId::view(unsafe { (*page_raw).memid }) {
        crate::core_types::MemIdView::Arena(arena_memid) => arena_memid,
        _ => return false,
    };
    // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    unsafe { _mi_arenas_page_try_reabandon_to_mapped_arena(page, arena_memid) }
}

#[inline(never)]
pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(
    page: crate::core_types::OwnedAbandonedPage,
    _arena_memid: crate::core_types::ArenaMemId,
) -> bool {
    let page = page.page().as_ptr();
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_is_abandoned_mapped(page) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_is_full_local(page) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_all_free(page) });
    // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    crate::internal::mi_assert_internal!(!unsafe { mi_page_is_singleton(page) });

    if unsafe { mi_page_is_full_local(page) } || unsafe { mi_page_is_abandoned_mapped(page) } {
        return false;
    }

    // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    let Some(theap) = (unsafe {
        _mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page))
    }) else {
        return false;
    };
    let theap = theap.as_ptr();

    // SAFETY[AUDIT:arena-page-reabandon-boundary]: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    unsafe {
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reabandon_full), 1)
    };
    unsafe { __mi_stat_adjust_decrease(ptr::addr_of_mut!((*theap).stats.pages_abandoned), 1) };
    unsafe { _mi_arenas_page_abandon(page, theap) };
    true
}

/// # Safety
/// Called from `free` paths while racing with abandoned-page readers.
// SAFETY[AUDIT:arena-page-unabandon-entry-boundary]: unabandon entry validates raw page and thread-heap witnesses before cleanup.
pub(crate) unsafe extern "C" fn _mi_arenas_page_unabandon(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    let page = unsafe {
        crate::core_types::OwnedAbandonedPage::new_unchecked(PagePtr::new_unchecked(page))
    };
    let current_theap = if current_theapx.is_null() {
        None
    } else {
        // SAFETY[AUDIT:arena-page-unabandon-entry-boundary]: unabandon entry validates raw page and thread-heap witnesses before cleanup.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(current_theapx),
            )
        })
    };
    // SAFETY[AUDIT:arena-page-unabandon-entry-boundary]: unabandon entry validates raw page and thread-heap witnesses before cleanup.
    unsafe { _mi_arenas_page_unabandon_internal(page, current_theap) };
}

unsafe fn _mi_arenas_page_unabandon_internal(
    page: crate::core_types::OwnedAbandonedPage,
    current_theap: Option<crate::core_types::ThreadHeapLocal>,
) {
    let page = page.page().as_ptr();
    let current_theapx = current_theap
        .map(|theap| theap.ptr().as_ptr())
        .unwrap_or(ptr::null_mut());

    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY[AUDIT:arena-page-unabandon-entry-boundary]: unabandon entry validates raw page and thread-heap witnesses before cleanup.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY[AUDIT:arena-page-unabandon-entry-boundary]: unabandon entry validates raw page and thread-heap witnesses before cleanup.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(
        current_theapx.is_null()
            || unsafe { _mi_thread_id() == (*(*current_theapx).tld).thread_id }
    );

    // SAFETY[AUDIT:arena-page-unabandon-entry-boundary]: unabandon entry validates raw page and thread-heap witnesses before cleanup.
    let heap = unsafe { mi_page_heap(page) };
    if unsafe { mi_page_is_abandoned_mapped(page) } {
        crate::internal::mi_assert_internal!(unsafe { mi_page_memid_is_arena(page) });
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        // SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]: mapped unabandon clears arena abandoned bitmaps and heap counters.
        let arena = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        let _ = arena;
        // SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]: mapped unabandon clears arena abandoned bitmaps and heap counters.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]: mapped unabandon clears arena abandoned bitmaps and heap counters.
        let committed = unsafe { crate::page::mi_page_slice_committed(page) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                || unsafe {
                    mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
                }
        );
        // SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]: mapped unabandon clears arena abandoned bitmaps and heap counters.
        unsafe {
            mi_bitmap_clear_once_set((*arena_pages).pages_abandoned[bin.as_usize()], slice_index)
        };
        unsafe { mi_page_clear_abandoned_mapped(PagePtr::new_unchecked(page)) };
        unsafe { mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1) };
    // SAFETY[AUDIT:arena-page-unabandon-mapped-boundary]: mapped unabandon clears arena abandoned bitmaps and heap counters.
    } else if !unsafe { mi_page_memid_is_arena(page) }
        && mi_option_is_enabled(mi_option_t::mi_option_visit_abandoned)
    {
        unsafe {
            mi_lock_acquire(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
            ))
        };
        // SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
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
        // SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
        unsafe {
            mi_lock_release(ptr::addr_of_mut!(
                (*heap.cast::<MiHeapPrefix>()).os_abandoned_pages_lock
            ))
        };
    }

    if !current_theapx.is_null() {
        // SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
        unsafe {
            __mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.pages_abandoned),
                1,
            )
        };
    } else {
        // SAFETY[AUDIT:arena-page-unabandon-os-list-boundary]: OS unabandon unlinks heap abandoned-list records and reconciles page stats.
        unsafe { __mi_stat_decrease_mt(&mut (*heap).stats.pages_abandoned, 1) };
    }
}

#[used]
static MI_ARENAS_PAGE_SINGLETON_ALLOC_KEEP: unsafe fn(
    *mut mi_theap_t,
    crate::core_types::BlockSize,
    crate::core_types::MaybeAlignment,
) -> *mut mi_page_t = mi_arenas_page_singleton_alloc;
// SAFETY[AUDIT:arena-page-regular-boundary]: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
#[used]
static MI_ARENAS_PAGE_ALLOC_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    usize,
    usize,
) -> *mut mi_page_t = _mi_arenas_page_alloc;
