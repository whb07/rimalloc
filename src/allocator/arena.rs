//! Arena management

use core::ffi::{c_int, c_long, c_void};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use core::mem::offset_of;
use core::mem::{MaybeUninit, size_of};
use core::ptr;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use crate::allocator::options::{
    mi_option_get, mi_option_get_size, mi_option_is_enabled, mi_option_t,
};
use crate::allocator::page::{
    _mi_page_stats_bin, MI_LARGE_MAX_OBJ_SIZE, MI_LARGE_PAGE_SIZE, MI_MEDIUM_MAX_OBJ_SIZE,
    MI_MEDIUM_PAGE_SIZE, MI_PAGE_OSPAGE_BLOCK_ALIGN2, MI_SMALL_MAX_OBJ_SIZE, MI_SMALL_PAGE_SIZE,
    mi_bin_from_block_size, mi_page_set_theap,
};
use crate::allocator::page_map::{
    _mi_page_map_register, _mi_page_map_unregister, _mi_safe_ptr_page,
};
use crate::allocator::stats::{
    MI_CBIN_HUGE, MI_CBIN_LARGE, MI_CBIN_MEDIUM, MI_CBIN_OTHER, MI_CBIN_SMALL,
};
use crate::allocator::thread_heap::{heap_area_for_page, visit_page_blocks};
use crate::allocator::tracking::{mi_track_mem_defined, mi_track_mem_undefined};
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
use crate::internal::memid::{_mi_memid_none, mi_memid_t};
use crate::internal::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt,
    __mi_stat_counter_increase, __mi_stat_counter_increase_mt, __mi_stat_decrease,
    __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt, _mi_align_up, _mi_clamp,
    _mi_divide_up, _mi_is_heap_main, _mi_meta_free, _mi_meta_is_meta_page, _mi_os_commit,
    _mi_os_commit_ex, _mi_os_free, _mi_os_free_ex, _mi_os_has_overcommit,
    _mi_os_has_virtual_reserve, _mi_os_reuse, _mi_preloading, _mi_subproc, _mi_theap_heap,
    _mi_thread_id, _mi_toupper, MI_DEBUG, mi_mem_is_zero, mi_mul_overflow, mi_page_all_free,
    mi_page_block_size, mi_page_block_size_typed, mi_page_claim_ownership,
    mi_page_clear_abandoned_mapped, mi_page_committed_typed, mi_page_heap, mi_page_is_abandoned,
    mi_page_is_abandoned_mapped, mi_page_is_full, mi_page_is_owned, mi_page_is_singleton,
    mi_page_meta_is_separated, mi_page_slice_start, mi_page_start, mi_size_of_slices,
    mi_slice_count_of_size,
};
use crate::mimalloc::{mi_heap_main, mi_heap_theap, mi_heap_zalloc_aligned};
use crate::os::{
    _mi_os_alloc_aligned, _mi_os_alloc_aligned_at_offset, _mi_os_alloc_huge_os_pages,
    _mi_os_minimal_purge_size, _mi_os_numa_node_count,
};
use crate::prim::_mi_page_associated_theap_peek;
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
mod metadata;
mod os_abandoned;
mod os_memory;
mod prefix;
mod purge;

use abandoned::{
    _mi_arenas_page_abandon_internal, _mi_arenas_page_unabandon_internal,
    mi_arena_try_claim_abandoned, mi_heap_ensure_arena_pages_from_raw, mi_page_is_full_local,
};
use geometry::{
    mi_arena_info_slices, mi_arena_numa_node, mi_arena_page_at_slice,
    mi_arena_range_is_after_info_slices, mi_arena_size, mi_arena_slice_start, mi_arena_start,
    mi_heap_arena_pages, mi_heap_has_page, mi_page_arena_pages,
};
pub(crate) use heap_pages::{_mi_heap_destroy_pages, _mi_heap_move_pages};
pub(crate) use ids::{_mi_arena_from_id, _mi_arena_id_none, _mi_arena_memid_is_suitable};
use ids::{
    mi_arena_from_memid, mi_arena_id_from_arena_typed, mi_arena_is_suitable, mi_memid_create_arena,
    mi_memid_is_os, mi_page_full_size, mi_page_memid_is_arena,
};
use metadata::mi_arenas_unsafe_destroy;
use os_memory::{
    ArenaOsAllocRequest, mi_arena_commit, mi_arena_os_alloc_aligned, mi_arena_reserve,
};
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
// SAFETY: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_arenas_get_count(subproc: *mut mi_subproc_t) -> usize {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        crate::internal::mi_assert_internal!(false);
        return 0;
    };
    // SAFETY: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
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
    // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    unsafe { *__errno_location() = err };
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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

/// Exclusive claim on a range cleared from an arena's free-slice bitmap.
///
/// Dropping an unpublished claim atomically returns the range to the free
/// bitmap. Dirty and committed state are deliberately not rolled back, matching
/// the allocator's existing failed-commit semantics.
struct ClaimedArenaSlices {
    allocation: crate::core_types::ArenaAllocation,
}

impl ClaimedArenaSlices {
    /// # Safety
    ///
    /// `arena` and its free bitmap must remain initialized and live until this
    /// claim is published or dropped, with concurrent arena teardown excluded.
    unsafe fn try_claim(
        arena: ArenaPtr,
        slice_count: SliceCount,
        tseq: crate::core_types::ThreadSeq,
    ) -> Option<Self> {
        let arena_raw = arena.as_ptr();
        let count = slice_count.get() as usize;
        let mut start = 0usize;
        // SAFETY: the caller guarantees live arena metadata and free bitmap;
        // the bitmap operation atomically transfers one matching range.
        if !unsafe {
            mi_bbitmap_try_find_and_clearN(
                (*arena_raw.cast::<MiArenaPrefix>()).slices_free,
                tseq.get(),
                count,
                &mut start,
            )
        } {
            return None;
        }

        let Ok(range) = crate::core_types::SliceRange::new(start, count, arena) else {
            crate::internal::mi_assert_internal!(false);
            // SAFETY: the successful bitmap claim above owns this exact range,
            // so failure to represent it must return those bits before exit.
            unsafe {
                mi_bbitmap_setN(
                    (*arena_raw.cast::<MiArenaPrefix>()).slices_free,
                    start,
                    count,
                )
            };
            return None;
        };
        // SAFETY: `range` is a checked range in this initialized arena.
        let memid = unsafe { mi_memid_create_arena(arena_raw, range) };
        Some(Self {
            allocation: crate::core_types::ArenaAllocation::new(arena, range, memid),
        })
    }

    #[inline(always)]
    fn arena(&self) -> ArenaPtr {
        self.allocation.arena()
    }

    #[inline(always)]
    fn range(&self) -> crate::core_types::SliceRange {
        self.allocation.range()
    }

    #[inline(always)]
    fn memid(&self) -> mi_memid_t {
        self.allocation.memid()
    }

    #[inline(always)]
    fn memid_mut(&mut self) -> &mut mi_memid_t {
        self.allocation.memid_mut()
    }

    #[inline(always)]
    fn publish(self) -> crate::core_types::ArenaAllocation {
        let claim = core::mem::ManuallyDrop::new(self);
        claim.allocation
    }
}

impl Drop for ClaimedArenaSlices {
    fn drop(&mut self) {
        let arena = self.arena().as_ptr();
        let range = self.range();
        // SAFETY: an unpublished value exclusively owns this exact cleared
        // free-bitmap range, and its construction contract keeps the bitmap
        // live through this drop.
        unsafe {
            mi_bbitmap_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                range.start_usize(),
                range.count_usize(),
            )
        };
    }
}

struct PendingFreshPage {
    start: core::ptr::NonNull<u8>,
    alloc_size: usize,
    memid: mi_memid_t,
    heap_page_bit: Option<(core::ptr::NonNull<mi_arena_pages_t>, SliceIndex)>,
    separate_page_meta: Option<PagePtr>,
}

impl PendingFreshPage {
    /// # Safety
    ///
    /// `start` must be a live allocation described by `alloc_size` and `memid`.
    /// Its arena/heap metadata must outlive this pending value, with concurrent
    /// teardown excluded.
    unsafe fn new(start: *mut u8, alloc_size: usize, memid: mi_memid_t) -> Self {
        // SAFETY: required by this constructor's contract.
        let start = unsafe { core::ptr::NonNull::new_unchecked(start) };
        Self {
            start,
            alloc_size,
            memid,
            heap_page_bit: None,
            separate_page_meta: None,
        }
    }

    #[inline(always)]
    fn start(&self) -> *mut u8 {
        self.start.as_ptr()
    }

    #[inline(always)]
    fn memid(&self) -> mi_memid_t {
        self.memid
    }

    #[inline(always)]
    fn memid_mut(&mut self) -> &mut mi_memid_t {
        &mut self.memid
    }

    /// # Safety
    /// `arena_pages` must remain live until publication or rollback, and this
    /// pending allocation must exclusively own its currently clear page bit.
    unsafe fn install_heap_page_bit(
        &mut self,
        arena_pages: *mut mi_arena_pages_t,
        index: SliceIndex,
    ) -> bool {
        let Some(arena_pages) = core::ptr::NonNull::new(arena_pages) else {
            return false;
        };
        // SAFETY: the caller provides a live heap arena-pages bitmap and the
        // allocation's checked arena slice index. A true result proves this
        // transaction changed the bit from clear to set.
        if !unsafe { mi_bitmap_set((*arena_pages.as_ptr()).pages, index.get()) } {
            crate::internal::mi_assert_internal!(false);
            return false;
        }
        self.heap_page_bit = Some((arena_pages, index));
        true
    }

    #[inline(always)]
    fn set_separate_page_meta(&mut self, page: PagePtr) {
        self.separate_page_meta = Some(page);
    }

    /// # Safety
    /// The pending page metadata and ownership word must be fully initialized,
    /// and the page-map lifecycle must remain initialized through registration.
    unsafe fn register(self, page: PagePtr) -> Option<RegisteredFreshPage> {
        // SAFETY: the caller guarantees initialized live page metadata.
        if unsafe { _mi_page_map_register(page.as_ptr()) } {
            Some(RegisteredFreshPage {
                _pending: self,
                page,
            })
        } else {
            None
        }
    }

    #[cold]
    #[inline(never)]
    /// # Safety
    /// This may be called exactly once, and only while this value is being
    /// dropped. It releases every resource still owned by the transaction
    /// without disarming the fields.
    unsafe fn rollback(&mut self) {
        if let Some((arena_pages, index)) = self.heap_page_bit {
            // SAFETY: the transaction installed and still owns this exact bit.
            unsafe { mi_bitmap_clear((*arena_pages.as_ptr()).pages, index.get()) };
        }
        if let Some(page) = self.separate_page_meta {
            // SAFETY: the pending allocation exclusively initialized this
            // arena-owned metadata slot, which outlives the transaction.
            unsafe {
                crate::internal::_mi_memzero_aligned(page.as_ptr().cast(), size_of::<mi_page_t>())
            };
        }
        // SAFETY: the transaction still owns this allocation and its memid.
        unsafe { _mi_arenas_free(self.start().cast(), self.alloc_size, self.memid()) };
    }
}

impl Drop for PendingFreshPage {
    fn drop(&mut self) {
        // SAFETY: Drop runs exactly once and does not access the transaction
        // after rollback releases its remaining resources.
        unsafe { self.rollback() };
    }
}

struct RegisteredFreshPage {
    _pending: PendingFreshPage,
    page: PagePtr,
}

impl RegisteredFreshPage {
    #[inline(always)]
    fn publish(self) -> *mut mi_page_t {
        let registered = core::mem::ManuallyDrop::new(self);
        registered.page.as_ptr()
    }
}

impl Drop for RegisteredFreshPage {
    fn drop(&mut self) {
        // SAFETY: construction proves this exact page is registered; the
        // pending allocation remains live until its field drops afterward.
        unsafe { _mi_page_map_unregister(self.page.as_ptr()) };
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
    // SAFETY: arena allocation excludes concurrent teardown and keeps the
    // arena bitmap live until the claim is published or rolled back.
    let mut claim = unsafe { ClaimedArenaSlices::try_claim(arena_ptr, slice_count, tseq)? };
    let slice_count = slice_count.get() as usize;
    let range = claim.range();
    let slice_index = range.start_usize();

    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    let p = unsafe {
        mi_arena_slice_start(arena_ptr, crate::core_types::SliceIndex::new(slice_index))
            .as_ptr()
            .cast::<c_void>()
    };
    let mut touched_slices = slice_count;
    if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.initially_zero } {
        let mut already_dirty = 0usize;
        // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        claim.memid_mut().initially_zero = unsafe {
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
        // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        let already_committed = unsafe {
            mi_bitmap_popcountN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
        if already_committed < slice_count {
            let mut commit_zero = false;
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            if !unsafe {
                mi_arena_commit(
                    arena_ptr,
                    range,
                    crate::core_types::CommittedBytes::new(mi_size_of_slices(slice_count)),
                    &mut commit_zero,
                    crate::internal::mi_committed_bytes_of_slices(slice_count - already_committed),
                )
            } {
                return None;
            }
            if commit_zero {
                claim.memid_mut().initially_zero = true;
            }
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                mi_bitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                    ptr::null_mut(),
                )
            };
            if MI_DEBUG > 1
                && claim.memid().initially_zero
                // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
                && !unsafe { mi_mem_is_zero(p, mi_size_of_slices(slice_count)) }
            {
                crate::allocator::options::_mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!("internal error: arena allocation was not zero-initialized!\n"),
                );
                claim.memid_mut().initially_zero = false;
            }
        } else {
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe { _mi_os_reuse(p, mi_size_of_slices(slice_count)) };
            if unsafe { _mi_os_has_overcommit() }
                && touched_slices > 0
                && !unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned }
            {
                // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
                unsafe {
                    __mi_stat_increase_mt(
                        ptr::addr_of_mut!(
                            (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed
                        ),
                        crate::internal::mi_committed_bytes_of_slices(touched_slices).get(),
                    )
                };
            }
        }

        // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        });
        claim.memid_mut().initially_committed = true;

        if claim.memid().initially_zero {
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe { mi_track_mem_defined(p, slice_count * MI_ARENA_SLICE_SIZE) };
        } else {
            unsafe { mi_track_mem_undefined(p, slice_count * MI_ARENA_SLICE_SIZE) };
        }
    } else {
        // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        claim.memid_mut().initially_committed = unsafe {
            mi_bitmap_is_setN(
                (*arena.cast::<MiArenaPrefix>()).slices_committed,
                slice_index,
                slice_count,
            )
        };
        if !claim.memid().initially_committed {
            let mut already_committed_count = 0usize;
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                mi_bitmap_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                    &mut already_committed_count,
                )
            };
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                mi_bitmap_clearN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            };
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe {
                __mi_stat_adjust_decrease_mt(
                    ptr::addr_of_mut!((*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed),
                    crate::internal::mi_committed_bytes_of_slices(already_committed_count).get(),
                )
            };
        }
    }

    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bbitmap_is_clearN(
            (*claim.arena().as_ptr().cast::<MiArenaPrefix>()).slices_free,
            claim.range().start_usize(),
            claim.range().count_usize(),
        )
    });
    if commit {
        // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*claim.arena().as_ptr().cast::<MiArenaPrefix>()).slices_committed,
                claim.range().start_usize(),
                claim.range().count_usize(),
            )
        });
        crate::internal::mi_assert_internal!(claim.memid().initially_committed);
    }
    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
    crate::internal::mi_assert_internal!(unsafe {
        mi_bitmap_is_setN(
            (*claim.arena().as_ptr().cast::<MiArenaPrefix>()).slices_dirty,
            claim.range().start_usize(),
            claim.range().count_usize(),
        )
    });

    Some((p, claim.publish()))
}

/// # Safety
/// `subproc` must point to a valid `mi_subproc_t`, and `idx` must be in range.
// SAFETY: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
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
    // SAFETY: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
    crate::internal::mi_assert_internal!(idx < unsafe { mi_arenas_get_count_typed(subproc_ptr) });
    unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) }
}

// SAFETY: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_arena_t) -> mi_arena_id_t = ids::mi_arena_id_from_arena;

#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_subproc_t) -> usize = mi_arenas_get_count;

// SAFETY: arena count/index lookup validates subproc pointers and typed arena indexes before C ABI exposure.
#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_subproc_t, usize) -> *mut mi_arena_t = mi_arena_from_index;

unsafe fn mi_arena_start_idx(
    heap: BorrowedHeap,
    tseq: crate::core_types::ThreadSeq,
    arena_cycle: ArenaIndex,
) -> ArenaIndex {
    let arena_cycle = arena_cycle.get() as usize;
    let heap = heap.ptr().as_ptr().cast::<MiHeapPrefix>();
    // SAFETY: arena iteration start indexes are derived from typed heap/thread sequence domains and bounded arena counts.
    let subproc = unsafe { (*heap).subproc.cast::<MiSubprocPrefix>() };
    let hseq = crate::core_types::HeapSeq::new(unsafe { (*heap).heap_seq });
    let hcount = unsafe { (*subproc).heap_count.load(Ordering::Relaxed) };
    if arena_cycle <= 1 {
        return unsafe { ArenaIndex::new_unchecked(0) };
    }
    if hseq.get() == 0 || hcount <= 1 || arena_cycle > 0x8FF {
        // SAFETY: arena iteration start indexes are derived from typed heap/thread sequence domains and bounded arena counts.
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
    // SAFETY: arena iteration start indexes are derived from typed heap/thread sequence domains and bounded arena counts.
    unsafe { ArenaIndex::new_unchecked(start as u16) }
}

unsafe fn mi_arena_is_suitable_ex(
    arena: ArenaPtr,
    req_arena: Option<ArenaPtr>,
    match_numa: crate::core_types::MatchNuma,
    numa_node: crate::core_types::NumaNode,
    allow_pinned: crate::core_types::AllowPinned,
) -> bool {
    let arena_raw = arena.as_ptr();
    // SAFETY: arena suitability checks keep pinned, requested-arena, and NUMA predicates separate from table mutation.
    if !allow_pinned.get() && unsafe { (*arena_raw.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return false;
    }
    if unsafe { !mi_arena_is_suitable(arena, req_arena) } {
        return false;
    }
    if req_arena.is_none() {
        // SAFETY: arena suitability checks keep pinned, requested-arena, and NUMA predicates separate from table mutation.
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
    // SAFETY: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
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
    // SAFETY: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
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
            // SAFETY: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
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
    // SAFETY: arena iteration loads typed subproc table cells and stops on visitor decisions without mutating registry state.
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
    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
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

    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
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

/// # Safety
/// Caller must ensure `subproc` is valid and destruction is only used in the same narrow cases
/// as the original C implementation.
// SAFETY: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_unsafe_destroy_all(subproc: *mut mi_subproc_t) {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: unsafe arena teardown clears subproc table cells and arena counts while preserving OS-owned arena release rules.
    unsafe { mi_arenas_unsafe_destroy(subproc) };
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

    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
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
        // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        unsafe { *memid = allocation.assume_init().into_memid() };
        return p;
    }
    if req_arena.is_some() || _mi_preloading() {
        return ptr::null_mut();
    }
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc) {
        return ptr::null_mut();
    }

    // SAFETY: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
    let Some(subproc) = (unsafe { heap.ptr().subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return ptr::null_mut();
    };
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc) };
    {
        // SAFETY: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
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

    // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
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
        // SAFETY: arena slice allocation updates free/dirty/committed bitmaps, memid state, commit accounting, and allocation metadata consistently.
        unsafe { *memid = allocation.assume_init().into_memid() };
    }
    p
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
        // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
        && size.get() <= unsafe { mi_arena_max_object_size() }
        && alignment.get() <= MI_ARENA_SLICE_ALIGN
        && align_offset.get() == 0
        && let (Some(slice_count), Some(heap)) = (arena_slice_count, heap)
    {
        let arena_alignment = crate::core_types::Alignment::new(alignment.get())
            // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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

    // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
// SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
        // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
        // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
        // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
        // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
// SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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

    // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
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
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    let heap = unsafe { _mi_theap_heap(theap) };
    let tseq = crate::core_types::ThreadSeq::new(unsafe { (*(*theap).tld).thread_seq });
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let slice_count_usize = slice_count.get() as usize;

    let bin = mi_bin_from_block_size(block_size);
    crate::internal::mi_assert_internal!(bin.as_usize() < MI_BIN_COUNT);
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
    if unsafe { mi_heap_abandoned_count(heap, bin).load_relaxed() } == 0 {
        return ptr::null_mut();
    }

    let any_numa = crate::core_types::NumaNode::any();
    let mut result = ptr::null_mut();
    let claim_fun = ThreadLocalClaimCallback::new(mi_arena_try_claim_abandoned);
    // SAFETY: abandoned-page claim and reclaim walks heap arena-page tables and remote-free state before transferring page ownership.
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
                    let owned_page = crate::core_types::assume_owned_abandoned_page(page);
                    crate::allocator::page::collect_page_free_lists_abandoned(&owned_page, false);
                    crate::internal::mi_assert_internal!(mi_bbitmap_is_clearN(
                        (*arena_raw.cast::<MiArenaPrefix>()).slices_free,
                        slice_index.get(),
                        slice_count_usize,
                    ));
                    let committed = crate::allocator::page::mi_page_slice_committed(page_raw);
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
) -> Option<PendingFreshPage> {
    let block_alignment = block_alignment.get();
    let allow_large = true;
    let page_alignment = MI_ARENA_SLICE_ALIGN;
    // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
    let heap = unsafe { _mi_theap_heap(theap) };
    let tld = unsafe { (*theap).tld };
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let heap_numa =
        crate::core_types::NumaNode::new(unsafe { (*heap.cast::<MiHeapPrefix>()).numa_node });
    // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
    let tld_numa = crate::core_types::NumaNode::new(unsafe { (*tld).numa_node });
    let numa_node = if !heap_numa.is_any() {
        heap_numa
    } else {
        tld_numa
    };

    let mut start = ptr::null_mut::<u8>();
    let mut arena_pending = None;
    let mut memid = unsafe { _mi_memid_none() };
    // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
    let slice_count_usize = slice_count.get() as usize;
    let alloc_size = mi_size_of_slices(slice_count_usize);
    if !mi_option_is_enabled(mi_option_t::mi_option_disallow_arena_alloc)
        && !os_align
        // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
        && slice_count_usize <= unsafe { mi_arena_max_object_size() } / MI_ARENA_SLICE_SIZE
    {
        let Some(heap_ptr) = HeapPtr::new(heap) else {
            crate::internal::mi_assert_internal!(false);
            return None;
        };
        // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
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
                &mut memid,
            )
            .cast::<u8>()
        };
        if !start.is_null() {
            // SAFETY: successful arena allocation returned live backing and a
            // matching memid owned by this pending transaction.
            let mut pending = unsafe { PendingFreshPage::new(start, alloc_size, memid) };
            // SAFETY: heap arena-page publication installs or updates per-heap arena page bitmaps before page ownership becomes visible.
            match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => {
                    let arena = a.arena().as_ptr();
                    let arena_pages = unsafe { mi_heap_ensure_arena_pages_from_raw(heap, arena) };
                    let slice_index = a.range().start();
                    // SAFETY: the ensured bitmap belongs to this live heap and
                    // arena; the pending allocation owns its clear start bit.
                    if !unsafe { pending.install_heap_page_bit(arena_pages, slice_index) } {
                        drop(pending);
                        start = ptr::null_mut();
                    } else {
                        arena_pending = Some(pending);
                    }
                }
                _ => {
                    // Fallback for non-arena allocations that bypassed the arena path above.
                    arena_pending = Some(pending);
                }
            }
        }
    }

    if start.is_null() {
        start = if os_align {
            // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
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
                    &mut memid,
                )
                .cast::<u8>()
            }
        } else {
            // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
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
                    &mut memid,
                )
                .cast::<u8>()
            }
        };
    }

    if start.is_null() {
        return None;
    }
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        start.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(
        !os_align
            || crate::internal::_mi_is_aligned(
                // SAFETY: fresh arena page setup reserves slices, commits initial memory, initializes metadata, and preserves page alignment/layout invariants.
                unsafe { start.add(page_alignment) }.cast(),
                block_alignment
            )
    );
    if let Some(pending) = arena_pending {
        Some(pending)
    } else {
        // SAFETY: successful OS allocation returned live backing and matching
        // memid, now owned by the pending transaction.
        Some(unsafe { PendingFreshPage::new(start, alloc_size, memid) })
    }
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
    let Some(mut pending) = (unsafe {
        mi_arenas_page_alloc_fresh_area(
            theap,
            slice_count,
            block_size,
            block_alignment,
            os_align,
            commit,
        )
    }) else {
        return ptr::null_mut();
    };
    let slice_start = pending.start();
    let mut memid = pending.memid();

    let page_noguard_size = alloc_size;
    let mut page = ptr::null_mut::<mi_page_t>();
    let mut page_meta_is_separate = false;
    let mut block_start = 0usize;

    if let crate::core_types::MemIdView::Arena(a) = crate::core_types::MemId::view(memid) {
        let arena = a.arena().as_ptr();
        // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
        if unsafe { !(*arena.cast::<MiArenaPrefix>()).pages_meta.is_null() } {
            let page_meta = unsafe {
                (*arena.cast::<MiArenaPrefix>())
                    .pages_meta
                    .add(a.range().start_usize())
            };
            // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
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
                // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
                unsafe {
                    crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>())
                };
            }
        }
    }
    if page.is_null() {
        page = slice_start.cast::<mi_page_t>();
        // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
        block_start = unsafe { mi_page_block_start(block_size, os_align) };
    }
    if page_meta_is_separate {
        // SAFETY: the selected separate arena metadata slot is non-null and
        // remains live through publication or rollback.
        pending.set_separate_page_meta(unsafe { PagePtr::new_unchecked(page) });
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
        // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
        if !unsafe {
            let (arena_ptr, range) = match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => (a.arena(), a.range()),
                _ => {
                    crate::internal::mi_assert_internal!(false);
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
            return ptr::null_mut();
        }
    }
    if !memid.initially_zero && !page_meta_is_separate {
        // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
        unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
    }

    if memid.initially_committed {
        if memid.initially_zero {
            // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
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
        // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
        && !unsafe { mi_mem_is_zero(slice_start.cast(), page_noguard_size) }
    {
        crate::allocator::options::_mi_error_message_fmt(
            Errno::EFAULT,
            format_args!("internal error: page memory was not zero initialized.\n"),
        );
        memid.initially_zero = false;
        if block_start > 0 {
            // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
            unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
        }
    }
    let reserved = if os_align {
        1
    } else {
        (page_noguard_size - block_start) / block_size_raw
    };
    crate::internal::mi_assert_internal!(reserved > 0 && u16::try_from(reserved).is_ok());

    // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
    unsafe {
        crate::allocator::page::mi_page_set_reserved_count(
            page,
            crate::core_types::ReservedBlockCount::new(reserved as u16),
        )
    };
    // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
    unsafe { (*page).page_start = slice_start.add(block_start) };
    unsafe { (*page).block_size = block_size_raw };
    unsafe { crate::allocator::page::mi_page_set_slice_committed(page, commit_size) };
    unsafe { (*page).memid = memid };
    unsafe { (*page).free_is_zero = memid.initially_zero };
    // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: fresh arena page setup initializes metadata, commit state, and layout counters.
    crate::internal::mi_assert_internal!(unsafe { page_ptr.free().is_empty() });
    crate::internal::mi_assert_internal!(
        page_meta_is_separate == unsafe { mi_page_meta_is_separated(page) }
    );
    crate::internal::mi_assert_internal!(unsafe { mi_page_slice_start(page) } == slice_start);

    // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    unsafe {
        (*page).xthread_free = crate::core_types::ThreadFreeWord::from_parts(
            None,
            crate::core_types::ThreadFreeOwner::Unowned,
        )
        .raw()
    };
    // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    let claimed = unsafe { page_ptr.thread_free_cell().claim_acq_rel() };
    crate::internal::mi_assert_internal!(claimed);
    // Keep memid diagnostics and page metadata synchronized before the pending
    // transaction enters registered state.
    *pending.memid_mut() = memid;
    let Some(registered) = (unsafe { pending.register(page_ptr) }) else {
        return ptr::null_mut();
    };

    // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    unsafe { __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.pages), 1) };
    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    unsafe { __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.page_bins[stats_bin]), 1) };

    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY: arena page ownership transitions update owner words, page maps, and visibility only after page metadata invariants hold.
    crate::internal::mi_assert_internal!(unsafe { mi_page_block_size(page) } == block_size_raw);
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    registered.publish()
}

unsafe fn mi_arenas_page_regular_alloc(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
) -> *mut mi_page_t {
    // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    let mut page = unsafe { mi_arenas_page_try_find_abandoned(theap, slice_count, block_size) };
    if !page.is_null() {
        return page;
    }

    let commit_on_demand =
        crate::allocator::options::mi_option_get(mi_option_t::mi_option_page_commit_on_demand);
    let slice_count_usize = slice_count.get() as usize;
    let commit = slice_count_usize <= mi_slice_count_of_size(MI_PAGE_MIN_COMMIT_SIZE)
        // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
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
    // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    crate::internal::mi_assert_internal!(match crate::core_types::MemId::view(unsafe {
        (*page).memid
    }) {
        crate::core_types::MemIdView::Arena(a) => a.range().count() == slice_count,
        _ => true, // Non-arena allocations trivially pass this slice_count assert
    });
    // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    if !unsafe { crate::allocator::page::_mi_page_init(theap, page) } {
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
        // SAFETY: singleton arena page setup handles over-aligned or large objects while preserving one-block page invariants.
        unsafe { mi_set_errno(ENOMEM) };
        return ptr::null_mut();
    };
    let page = unsafe {
        mi_arenas_page_alloc_fresh(theap, slice_count, block_size, block_alignment, true)
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: singleton arena page setup handles over-aligned or large objects while preserving one-block page invariants.
    crate::internal::mi_assert_internal!(unsafe {
        crate::allocator::page::mi_page_reserved_count(page).get() == 1
    });
    if !unsafe { crate::allocator::page::_mi_page_init(theap, page) } {
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
// SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_page_alloc(
    theap: *mut mi_theap_t,
    block_size: usize,
    block_alignment: usize,
) -> *mut mi_page_t {
    // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
    let block_size_typed = unsafe { crate::core_types::BlockSize::new_unchecked(block_size) };
    let block_alignment_typed =
        unsafe { crate::core_types::MaybeAlignment::new_unchecked(block_alignment) };
    let page = if block_alignment > MI_PAGE_MAX_OVERALLOC_ALIGN {
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_power_of_two(block_alignment));
        // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        unsafe { mi_arenas_page_singleton_alloc(theap, block_size_typed, block_alignment_typed) }
    } else if block_size <= MI_SMALL_MAX_OBJ_SIZE {
        unsafe { mi_arenas_page_regular_alloc(theap, MI_SMALL_PAGE_SLICE_COUNT, block_size_typed) }
    } else if block_size <= MI_MEDIUM_MAX_OBJ_SIZE {
        unsafe { mi_arenas_page_regular_alloc(theap, MI_MEDIUM_PAGE_SLICE_COUNT, block_size_typed) }
    } else if MI_ENABLE_LARGE_PAGES != 0 && block_size <= MI_LARGE_MAX_OBJ_SIZE {
        // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        unsafe { mi_arenas_page_regular_alloc(theap, MI_LARGE_PAGE_SLICE_COUNT, block_size_typed) }
    } else {
        unsafe { mi_arenas_page_singleton_alloc(theap, block_size_typed, block_alignment_typed) }
    };
    if page.is_null() {
        return ptr::null_mut();
    }
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::internal::mi_assert_internal!(
        block_alignment <= MI_PAGE_MAX_OVERALLOC_ALIGN
            || crate::internal::_mi_is_aligned(
                // SAFETY: regular arena page allocation reuses abandoned pages or initializes fresh size-class pages with matching slice counts.
                unsafe { mi_page_start(page) }.cast(),
                block_alignment
            )
    );
    page
}

/// # Safety
/// Internal arena page free path matching the C ABI.
// SAFETY: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
pub(crate) unsafe extern "C" fn _mi_arenas_page_free(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
    let abandoned = unsafe { crate::core_types::assume_abandoned_page(page_ptr) };
    let all_free = unsafe { crate::core_types::assume_all_free_abandoned(abandoned) };
    let current_theap = if current_theapx.is_null() {
        None
    } else {
        // SAFETY: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(current_theapx),
            )
        })
    };
    // SAFETY: free entry converts raw page and thread-heap inputs into typed all-free witnesses.
    unsafe { _mi_arenas_page_free_internal(&all_free, current_theap) };
}

unsafe fn _mi_arenas_page_free_internal<S>(
    page_wrapper: &crate::core_types::AllFreePage<S>,
    current_theap: Option<crate::core_types::ThreadHeapLocal>,
) {
    let page_ptr = page_wrapper.page();
    let page = page_ptr.as_ptr();
    let current_theapx = current_theap
        .map(|theap| theap.ptr().as_ptr())
        .unwrap_or(ptr::null_mut());
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_all_free(page) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    crate::internal::mi_assert_internal!(unsafe {
        page_ptr.queue_next().is_none() && page_ptr.queue_prev().is_none()
    });

    // SAFETY: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    if !current_theapx.is_null() {
        unsafe {
            crate::internal::__mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.page_bins[stats_bin]),
                1,
            )
        };
        // SAFETY: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
        unsafe {
            crate::internal::__mi_stat_decrease(ptr::addr_of_mut!((*current_theapx).stats.pages), 1)
        };
    } else {
        let heap = unsafe { mi_page_heap(page) };
        // SAFETY: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
        unsafe {
            crate::internal::__mi_stat_decrease_mt(
                ptr::addr_of_mut!((*heap).stats.page_bins[stats_bin]),
                1,
            )
        };
        unsafe {
            crate::internal::__mi_stat_decrease_mt(ptr::addr_of_mut!((*heap).stats.pages), 1)
        };
    }

    if MI_DEBUG > 1
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        && unsafe { mi_page_memid_is_arena(page) }
        && !unsafe { mi_page_is_full_local(page) }
    {
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        let mut arena_pages = ptr::null_mut();
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        let arena = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            )
        });
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page) };
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
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        crate::internal::mi_assert_internal!(!unsafe {
            mi_bitmap_is_setN(
                (*arena_pages).pages_abandoned[bin.as_usize()],
                slice_index,
                1,
            )
        });
        // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN((*arena_pages).pages, slice_index, 1)
        });
    }

    // SAFETY: free validation checks page alignment, ownership, memid, and arena bitmaps before release.
    unsafe { _mi_page_map_unregister(page) };
    if unsafe { mi_page_memid_is_arena(page) } {
        let mut arena_pages = ptr::null_mut();
        let mut slice_index = 0usize;
        let mut slice_count = 0usize;
        // SAFETY: free release clears arena metadata and returns backing slices after ownership is proven.
        let arena = unsafe {
            mi_page_arena_pages(
                PagePtr::new_unchecked(page),
                &mut slice_index,
                &mut slice_count,
                &mut arena_pages,
            )
        };
        crate::internal::mi_assert_internal!(!arena_pages.is_null());
        // SAFETY: free release clears arena metadata and returns backing slices after ownership is proven.
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page) };
        if !committed.is_zero() {
            let total_slices = committed.get() / MI_ARENA_SLICE_SIZE;
            crate::internal::mi_assert_internal!(slice_count >= total_slices);
            if total_slices > 0 {
                // SAFETY: free release clears arena metadata and returns backing slices after ownership is proven.
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
                // SAFETY: free accounting updates current-thread, heap, or subproc stats from ownership-derived metadata.
                unsafe {
                    crate::internal::__mi_stat_decrease_mt(
                        ptr::addr_of_mut!(
                            (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed
                        ),
                        crate::core_types::CommittedBytes::new(extra).get(),
                    )
                };
            }
        } else {
            // SAFETY: free release clears arena metadata and returns backing slices after ownership is proven.
            crate::internal::mi_assert_internal!(unsafe {
                mi_bitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            });
        }
    }
    // SAFETY: free release clears arena metadata and returns backing slices after ownership is proven.
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
    // SAFETY: free release clears arena metadata and returns backing slices after ownership is proven.
    unsafe { _mi_arenas_page_free(page.page().as_ptr(), owner.ptr().as_ptr()) };
    page.free_to_arena(owner)
}

/// # Safety
/// Internal arena abandon path matching the C ABI.
// SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
pub(crate) unsafe extern "C" fn _mi_arenas_page_abandon(
    page: *mut mi_page_t,
    current_theap: *mut mi_theap_t,
) {
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    let page_wrapper = if unsafe { mi_page_is_full_local(page) } {
        unsafe { crate::core_types::assume_full_page(page_ptr).into_abandon_candidate() }
    } else {
        unsafe { crate::core_types::assume_active_page(page_ptr).into_abandon_candidate() }
    };
    // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    let current_theap = unsafe {
        crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(current_theap),
        )
    };
    // SAFETY: arena page abandon paths publish abandoned state, update heap/arena bitmaps, stats, and owner words before releasing thread ownership.
    unsafe { _mi_arenas_page_abandon_internal(page_wrapper, current_theap) };
}

/// # Safety
/// Called from the free path on pages already associated with an arena.
// SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_arenas_page_try_reabandon_to_mapped(
    page: *mut mi_page_t,
) -> bool {
    let page_raw = page;
    // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    let page =
        unsafe { crate::core_types::assume_owned_abandoned_page(PagePtr::new_unchecked(page)) };
    let arena_memid = match crate::core_types::MemId::view(unsafe { (*page_raw).memid }) {
        crate::core_types::MemIdView::Arena(arena_memid) => arena_memid,
        _ => return false,
    };
    // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    unsafe { _mi_arenas_page_try_reabandon_to_mapped_arena(page, arena_memid).is_ok() }
}

#[inline(never)]
pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(
    page: crate::core_types::OwnedAbandonedPage,
    _arena_memid: crate::core_types::ArenaMemId,
) -> Result<crate::core_types::AbandonedMappedPageType, crate::core_types::OwnedAbandonedPage> {
    let page_raw = page.page().as_ptr();
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
        unsafe { mi_page_slice_start(page_raw) }.cast(),
        MI_PAGE_ALIGN
    ));
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page_raw).cast()) == page_raw
    });
    // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page_raw) });
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page_raw) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_is_abandoned_mapped(page_raw) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_is_full_local(page_raw) });
    crate::internal::mi_assert_internal!(!unsafe { mi_page_all_free(page_raw) });
    // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    crate::internal::mi_assert_internal!(!unsafe { mi_page_is_singleton(page_raw) });

    if unsafe { mi_page_is_full_local(page_raw) }
        || unsafe { mi_page_is_abandoned_mapped(page_raw) }
    {
        return Err(page);
    }

    // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    let Some(theap) = (unsafe {
        _mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page_raw))
    }) else {
        return Err(page);
    };
    let theap = theap.as_ptr();

    // SAFETY: reabandon republishes owned abandoned pages through arena ownership and stat reconciliation.
    unsafe {
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reabandon_full), 1)
    };
    unsafe { __mi_stat_adjust_decrease(ptr::addr_of_mut!((*theap).stats.pages_abandoned), 1) };
    unsafe { _mi_arenas_page_abandon(page_raw, theap) };
    Ok(page.into_mapped_abandoned())
}

/// # Safety
/// Called from `free` paths while racing with abandoned-page readers.
// SAFETY: unabandon entry validates raw page and thread-heap witnesses before cleanup.
pub(crate) unsafe extern "C" fn _mi_arenas_page_unabandon(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    // SAFETY: the raw compatibility
    // entry requires a claimed owned-abandoned page.
    let page =
        unsafe { crate::core_types::assume_owned_abandoned_page(PagePtr::new_unchecked(page)) };
    let current_theap = if current_theapx.is_null() {
        None
    } else {
        // SAFETY: unabandon entry validates raw page and thread-heap witnesses before cleanup.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(current_theapx),
            )
        })
    };
    // SAFETY: unabandon entry validates raw page and thread-heap witnesses before cleanup.
    unsafe { _mi_arenas_page_unabandon_internal(page, current_theap) };
}

#[cfg(test)]
mod claim_tests {
    use super::*;
    use std::alloc::{Layout, alloc_zeroed, dealloc};

    fn with_claim_fixture(test: impl FnOnce(ArenaPtr, *mut mi_bbitmap_t)) {
        let layout = Layout::new::<mi_bbitmap_t>();
        // SAFETY: the allocated bitmap and stack arena remain live throughout
        // `test`; all initialized free bits are restored before deallocation.
        unsafe {
            let bitmap = alloc_zeroed(layout).cast::<mi_bbitmap_t>();
            assert!(!bitmap.is_null());
            mi_bbitmap_init(bitmap, 16, true);
            assert!(mi_bbitmap_setN(bitmap, 5, 3));

            let mut arena = MaybeUninit::<mi_arena_t>::zeroed();
            let arena_raw = arena.as_mut_ptr();
            ptr::addr_of_mut!((*arena_raw).slice_count).write(16);
            ptr::addr_of_mut!((*arena_raw).slices_free).write(bitmap);
            let arena = ArenaPtr::new_unchecked(arena_raw);

            test(arena, bitmap);
            dealloc(bitmap.cast(), layout);
        }
    }

    #[test]
    fn claimed_arena_slices_drop_restores_exact_free_range() {
        with_claim_fixture(|arena, bitmap| {
            // SAFETY: the fixture owns initialized arena and bitmap storage for
            // the complete claim lifetime and excludes concurrent teardown.
            let claim = unsafe {
                ClaimedArenaSlices::try_claim(
                    arena,
                    SliceCount::from_usize(3).unwrap(),
                    crate::core_types::ThreadSeq::new(0),
                )
                .unwrap()
            };
            assert_eq!(claim.range().start_usize(), 5);
            assert_eq!(claim.range().count_usize(), 3);
            // SAFETY: the fixture bitmap has 16 initialized bits.
            assert!(unsafe { mi_bbitmap_is_clearN(bitmap, 5, 3) });

            drop(claim);
            // SAFETY: dropping the unpublished claim restores its exact range.
            assert!(unsafe { mi_bbitmap_is_setN(bitmap, 5, 3) });
        });
    }

    #[test]
    fn published_arena_slices_keep_the_range_claimed() {
        with_claim_fixture(|arena, bitmap| {
            // SAFETY: the fixture owns initialized arena and bitmap storage for
            // the complete claim lifetime and excludes concurrent teardown.
            let claim = unsafe {
                ClaimedArenaSlices::try_claim(
                    arena,
                    SliceCount::from_usize(3).unwrap(),
                    crate::core_types::ThreadSeq::new(0),
                )
                .unwrap()
            };
            let allocation = claim.publish();
            assert_eq!(allocation.range().start_usize(), 5);
            assert_eq!(allocation.range().count_usize(), 3);
            // SAFETY: publication transfers the allocation while intentionally
            // leaving its exact free-bitmap range cleared.
            assert!(unsafe { mi_bbitmap_is_clearN(bitmap, 5, 3) });

            // Restore the fixture's free bits before releasing bitmap storage.
            assert!(unsafe { mi_bbitmap_setN(bitmap, 5, 3) });
        });
    }

    #[cfg(not(miri))]
    #[test]
    fn pending_fresh_page_rollback_clears_heap_bit_and_separate_metadata() {
        let bitmap_layout = Layout::new::<mi_bitmap_t>();
        // SAFETY: all raw fixtures are initialized and remain live through the
        // pending transaction; the None memid makes backing release a no-op.
        unsafe {
            let bitmap = alloc_zeroed(bitmap_layout).cast::<mi_bitmap_t>();
            assert!(!bitmap.is_null());
            mi_bitmap_init(bitmap, 16, true);

            let mut arena_pages = MaybeUninit::<mi_arena_pages_t>::zeroed();
            ptr::addr_of_mut!((*arena_pages.as_mut_ptr()).pages).write(bitmap);
            let mut backing = MaybeUninit::<u8>::uninit();
            let mut page_meta = MaybeUninit::<mi_page_t>::zeroed();
            ptr::addr_of_mut!((*page_meta.as_mut_ptr()).block_size).write(128);

            let mut pending = PendingFreshPage::new(
                backing.as_mut_ptr(),
                1,
                crate::internal::memid::_mi_memid_none(),
            );
            assert!(pending.install_heap_page_bit(arena_pages.as_mut_ptr(), SliceIndex::new(5)));
            pending.set_separate_page_meta(PagePtr::new_unchecked(page_meta.as_mut_ptr()));
            assert!(mi_bitmap_is_setN(bitmap, 5, 1));

            drop(pending);
            assert!(mi_bitmap_is_clearN(bitmap, 5, 1));
            assert_eq!((*page_meta.as_ptr()).block_size, 0);

            ptr::addr_of_mut!((*page_meta.as_mut_ptr()).block_size).write(256);
            let mut reused = PendingFreshPage::new(
                backing.as_mut_ptr(),
                1,
                crate::internal::memid::_mi_memid_none(),
            );
            assert!(reused.install_heap_page_bit(arena_pages.as_mut_ptr(), SliceIndex::new(5)));
            reused.set_separate_page_meta(PagePtr::new_unchecked(page_meta.as_mut_ptr()));
            drop(reused);
            assert!(mi_bitmap_is_clearN(bitmap, 5, 1));
            assert_eq!((*page_meta.as_ptr()).block_size, 0);
            dealloc(bitmap.cast(), bitmap_layout);
        }
    }

    #[test]
    fn pending_fresh_page_drop_restores_heap_bit_for_reuse() {
        let bitmap_layout = Layout::new::<mi_bitmap_t>();
        // SAFETY: raw bitmap fixtures remain initialized and live through both
        // pending transactions; the None memid makes backing release a no-op.
        unsafe {
            let bitmap = alloc_zeroed(bitmap_layout).cast::<mi_bitmap_t>();
            assert!(!bitmap.is_null());
            mi_bitmap_init(bitmap, 16, true);
            let mut arena_pages = MaybeUninit::<mi_arena_pages_t>::zeroed();
            ptr::addr_of_mut!((*arena_pages.as_mut_ptr()).pages).write(bitmap);
            let mut backing = MaybeUninit::<u8>::uninit();

            for _ in 0..2 {
                let mut pending = PendingFreshPage::new(
                    backing.as_mut_ptr(),
                    1,
                    crate::internal::memid::_mi_memid_none(),
                );
                assert!(
                    pending.install_heap_page_bit(arena_pages.as_mut_ptr(), SliceIndex::new(5))
                );
                assert!(mi_bitmap_is_setN(bitmap, 5, 1));
                drop(pending);
                assert!(mi_bitmap_is_clearN(bitmap, 5, 1));
            }

            dealloc(bitmap.cast(), bitmap_layout);
        }
    }
}
