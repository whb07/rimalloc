//! Arena management

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use crate::atomic::mi_atomic_load_ptr_acquire;
use crate::bits::MI_INTPTR_BITS;
use crate::types::{mi_arena_id_t, mi_arena_t, mi_commit_fun_t, mi_heap_t, mi_subproc_t};

pub const MI_ARENA_SLICE_SIZE: usize = 8192 * size_of::<usize>();
pub const MI_ARENA_SLICE_ALIGN: usize = MI_ARENA_SLICE_SIZE;
pub const MI_ARENA_BCHUNK_BITS: usize = 64 * size_of::<usize>();
pub const MI_ARENA_MIN_SIZE: usize = MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE;
pub const MI_PAGE_BLOCK_START_MAX_OFFSET: usize = 8 * MI_INTPTR_BITS;
#[allow(non_upper_case_globals)]
pub const mi_forall_arenas_end: () = ();

const MI_MAX_ARENAS: usize = 160;

#[repr(C)]
struct MiHeapPrefix {
    subproc: *mut mi_subproc_t,
    heap_seq: usize,
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
    commit_fun: mi_commit_fun_t,
    commit_fun_arg: *mut c_void,
    total_size: usize,
    parent: *mut mi_arena_t,
}

pub extern "C" fn mi_arena_min_alignment() -> usize {
    MI_ARENA_SLICE_ALIGN
}

pub extern "C" fn mi_arena_min_size() -> usize {
    MI_ARENA_MIN_SIZE
}

/// # Safety
/// Matches the C internal API and returns the null arena-id sentinel.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_arena_id_none() -> mi_arena_id_t {
    ptr::null_mut()
}

/// # Safety
/// `id` must be either null or a valid top-level `mi_arena_t*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_arena_from_id(id: mi_arena_id_t) -> *mut mi_arena_t {
    let arena = id.cast::<mi_arena_t>();
    crate::mi_assert_internal!(
        arena.is_null() || unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() }
    );
    arena
}

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
