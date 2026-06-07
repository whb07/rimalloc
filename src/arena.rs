//! Arena management

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use crate::atomic::mi_atomic_load_ptr_acquire;
use crate::bitmap::{MI_BCHUNK_SIZE, MI_BITMAP_MAX_BIT_COUNT};
use crate::bits::MI_INTPTR_BITS;
use crate::internal::{
    _mi_align_up, _mi_memid_create, _mi_os_commit, _mi_os_commit_ex, mi_memid_t, mi_memkind_t,
    mi_size_of_slices,
};
use crate::options::{mi_option_get_size, mi_option_t};
use crate::types::{
    mi_arena_id_t, mi_arena_t, mi_commit_fun_t, mi_heap_t, mi_page_t, mi_subproc_t,
};

pub const MI_ARENA_SLICE_SIZE: usize = 8192 * size_of::<usize>();
pub const MI_ARENA_SLICE_ALIGN: usize = MI_ARENA_SLICE_SIZE;
pub const MI_ARENA_BCHUNK_BITS: usize = 64 * size_of::<usize>();
pub const MI_ARENA_MIN_SIZE: usize = MI_ARENA_BCHUNK_BITS * MI_ARENA_SLICE_SIZE;
pub const MI_PAGE_BLOCK_START_MAX_OFFSET: usize = 8 * MI_INTPTR_BITS;
#[allow(non_upper_case_globals)]
pub const mi_forall_arenas_end: () = ();

const MI_MAX_ARENAS: usize = 160;
const MI_ARENA_MIN_OBJ_SIZE: usize = MI_ARENA_SLICE_SIZE;
const MI_ARENA_MAX_SIZE: usize = MI_BITMAP_MAX_BIT_COUNT * MI_ARENA_SLICE_SIZE;

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
    slices_free: *mut crate::bitmap::mi_bbitmap_t,
    slices_committed: *mut crate::bitmap::mi_bitmap_t,
    slices_dirty: *mut crate::bitmap::mi_bitmap_t,
    slices_purge: *mut crate::bitmap::mi_bitmap_t,
    pages_meta: *mut mi_page_t,
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_arena_memid_is_suitable(
    memid: mi_memid_t,
    request_arena: *mut mi_arena_t,
) -> bool {
    if memid.memkind == mi_memkind_t::MI_MEM_ARENA {
        unsafe { mi_arena_is_suitable(memid.mem.arena.arena, request_arena) }
    } else {
        unsafe { mi_arena_is_suitable(ptr::null_mut(), request_arena) }
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
    if !arena.is_null() && unsafe { ((*arena.cast::<MiArenaPrefix>()).commit_fun as usize) != 0 } {
        unsafe {
            ((*arena.cast::<MiArenaPrefix>()).commit_fun)(
                true,
                start,
                size,
                is_zero,
                (*arena.cast::<MiArenaPrefix>()).commit_fun_arg,
            )
        }
    } else if already_committed > 0 {
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
unsafe fn mi_arena_slice_start(arena: *mut mi_arena_t, slice_index: usize) -> *mut u8 {
    crate::mi_assert_internal!(
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    unsafe { mi_arena_start(arena).add(mi_size_of_slices(slice_index)) }
}

#[allow(dead_code)]
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
unsafe fn mi_memid_create_arena(
    arena: *mut mi_arena_t,
    slice_index: usize,
    slice_count: usize,
) -> mi_memid_t {
    crate::mi_assert_internal!(u32::try_from(slice_index).is_ok());
    crate::mi_assert_internal!(u32::try_from(slice_count).is_ok());
    crate::mi_assert_internal!(slice_count > 0);
    crate::mi_assert_internal!(
        slice_index < unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }
    );
    let mut memid = unsafe { _mi_memid_create(mi_memkind_t::MI_MEM_ARENA) };
    memid.mem.arena.arena = arena;
    memid.mem.arena.slice_index = slice_index as u32;
    memid.mem.arena.slice_count = slice_count as u32;
    memid
}

#[allow(dead_code)]
unsafe fn mi_arena_from_memid(
    memid: mi_memid_t,
    slice_index: *mut usize,
    slice_count: *mut usize,
) -> *mut mi_arena_t {
    crate::mi_assert_internal!(memid.memkind == mi_memkind_t::MI_MEM_ARENA);
    let arena = unsafe { memid.mem.arena.arena };
    if !slice_index.is_null() {
        unsafe { *slice_index = memid.mem.arena.slice_index as usize };
    }
    if !slice_count.is_null() {
        unsafe { *slice_count = memid.mem.arena.slice_count as usize };
    }
    arena
}

#[allow(dead_code)]
const fn mi_memkind_is_os(memkind: mi_memkind_t) -> bool {
    matches!(
        memkind,
        mi_memkind_t::MI_MEM_OS | mi_memkind_t::MI_MEM_OS_HUGE | mi_memkind_t::MI_MEM_OS_REMAP
    )
}

#[allow(dead_code)]
unsafe fn mi_memid_is_os(memid: mi_memid_t) -> bool {
    mi_memkind_is_os(memid.memkind)
}

#[allow(dead_code)]
unsafe fn mi_page_full_size(page: *mut mi_page_t) -> usize {
    let memid = unsafe { (*page).memid };
    if memid.memkind == mi_memkind_t::MI_MEM_ARENA {
        unsafe { memid.mem.arena.slice_count as usize * MI_ARENA_SLICE_SIZE }
    } else if unsafe { mi_memid_is_os(memid) } || memid.memkind == mi_memkind_t::MI_MEM_EXTERNAL {
        let base = unsafe { memid.mem.os.base.cast::<u8>() };
        let page_ptr = page.cast::<u8>();
        crate::mi_assert_internal!(base <= page_ptr);
        let presize = unsafe { page_ptr.offset_from(base) };
        crate::mi_assert_internal!(unsafe { memid.mem.os.size as isize >= presize });
        if presize > unsafe { memid.mem.os.size as isize } {
            0
        } else {
            unsafe { memid.mem.os.size - presize as usize }
        }
    } else {
        0
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
