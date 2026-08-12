//! Arena management

use core::ffi::{c_int, c_long, c_void};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use core::mem::offset_of;
use core::mem::size_of;
use core::ptr::{self, NonNull};
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
    mi_bbitmap_try_find_and_clearN, mi_bbitmap_unsafe_setN, mi_bchunkmap_t, mi_bitmap_clear,
    mi_bitmap_clear_once_set, mi_bitmap_clearN, mi_bitmap_forall_set_typed, mi_bitmap_init,
    mi_bitmap_is_all_clear, mi_bitmap_is_clearN, mi_bitmap_is_set, mi_bitmap_is_setN,
    mi_bitmap_popcountN, mi_bitmap_set, mi_bitmap_setN, mi_bitmap_size, mi_bitmap_t,
    mi_bitmap_try_find_and_claim, mi_bitmap_unsafe_setN,
};
use crate::bits::{MI_INTPTR_BITS, MI_MiB};
use crate::core_types::{
    ArenaIndex, ArenaPtr, BlockVisitCallback, BlockVisitCallbackArg, BorrowedHeap, Errno, HeapPtr,
    InteriorUserPtr, MiLockGuard, OsMemPtr, OwnedHeap, PagePtr, ReservedBytes, SliceCount,
    SliceIndex, SubprocPtr, ThreadLocalClaimCallback,
};
use crate::internal::memid::mi_memid_t;
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

pub(crate) use abandoned::{_mi_arenas_page_abandon_internal, _mi_arenas_page_unabandon_internal};
use abandoned::{
    mi_arena_try_claim_abandoned, mi_heap_ensure_arena_pages_from_raw, mi_page_is_full_local,
};
use geometry::{
    PageArenaMembership, mi_arena_info_slices, mi_arena_page_at_slice,
    mi_arena_range_is_after_info_slices, mi_arena_size, mi_arena_slice_start, mi_arena_start,
    mi_heap_arena_pages, mi_heap_has_page,
};
pub(crate) use heap_pages::{_mi_heap_destroy_pages, _mi_heap_move_pages};
pub(crate) use ids::{_mi_arena_from_id, _mi_arena_id_none, _mi_arena_memid_is_suitable};
use ids::{
    mi_arena_id_from_arena_typed, mi_arena_is_suitable, mi_memid_create_arena, mi_memid_is_os,
    mi_page_full_size, mi_page_memid_is_arena,
};
use metadata::mi_arenas_unsafe_destroy;
use os_memory::{
    ArenaOsAllocRequest, mi_arena_commit, mi_arena_os_alloc_aligned, mi_arena_reserve,
    mi_write_arena_os_allocation_result,
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
/// `subproc` must remain live and initialized with valid aligned arena-count
/// atomic storage for this load. Concurrent teardown is excluded; concurrent
/// registry growth may proceed through its publication protocol.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_arenas_get_count(subproc: *mut mi_subproc_t) -> usize {
    let Some(subproc) = SubprocPtr::new(subproc) else {
        crate::internal::mi_assert_internal!(false);
        return 0;
    };
    // SAFETY: the raw contract keeps the live subproc's arena-count atomic
    // storage valid for this relaxed snapshot.
    unsafe { mi_arenas_get_count_typed(subproc) }
}

/// Read the current published arena count without stabilizing later registry
/// entries.
///
/// # Safety
/// `subproc` must remain live and initialized with its arena-count atomic
/// storage valid for this load; concurrent teardown is excluded. Concurrent
/// registry growth may proceed through its publication protocol.
unsafe fn mi_arenas_get_count_typed(subproc: SubprocPtr) -> usize {
    // SAFETY: the caller keeps the live subproc's aligned arena-count atomic
    // storage valid for this relaxed snapshot.
    unsafe {
        subproc
            .arena_count(crate::core_types::MemoryOrder::Relaxed)
            .get()
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn mi_set_errno(err: c_int) {
    // SAFETY: this declaration matches the Linux/Android libc ABI.
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    // SAFETY: `__errno_location` returns this thread's writable errno slot.
    unsafe { *__errno_location() = err };
}

#[cfg(target_vendor = "apple")]
fn mi_set_errno(err: c_int) {
    // SAFETY: this declaration matches the Apple libc ABI.
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    // SAFETY: `__error` returns this thread's writable errno slot.
    unsafe { *__error() = err };
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn mi_set_errno(_err: c_int) {}

fn mi_arena_max_object_size() -> usize {
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
    fn set_initially_zero(&mut self, initially_zero: bool) {
        self.memid.initially_zero = initially_zero;
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

    /// # Safety
    /// `page` must identify the live separate metadata slot for this pending
    /// allocation. The transaction must exclusively own its initialization,
    /// and the slot must remain writable through publication or rollback.
    #[inline(always)]
    unsafe fn set_separate_page_meta(&mut self, page: PagePtr) {
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
    fn publish(self) -> crate::core_types::Page<crate::core_types::Uninit> {
        let registered = core::mem::ManuallyDrop::new(self);
        // SAFETY: registration publishes fully prepared page metadata, while
        // this consuming boundary retains exclusive initialization ownership.
        unsafe { crate::core_types::assume_uninit_page(registered.page) }
    }
}

impl Drop for RegisteredFreshPage {
    fn drop(&mut self) {
        // SAFETY: construction proves this exact page is registered; the
        // pending allocation remains live until its field drops afterward.
        unsafe { _mi_page_map_unregister(self.page.as_ptr()) };
    }
}

/// # Safety
/// `arena` must identify live initialized registered arena metadata whose free, dirty, committed,
/// and accounting bitmap backing remains live through the claim. Its reserved slice backing and
/// subprocess stats must remain valid, and teardown must not race. Concurrent allocation attempts
/// are permitted through the bitmap claim protocol. When `commit` is true, the claimed OS range must
/// remain suitable for the arena commit/reuse and tracking operations until publication completes;
/// any stored commit callback and its opaque argument must remain valid for synchronous invocation
/// and the callback must not unwind.
unsafe fn mi_arena_try_alloc_at(
    arena: ArenaPtr,
    slice_count: crate::core_types::SliceCount,
    commit: bool,
    tseq: crate::core_types::ThreadSeq,
) -> Option<(NonNull<c_void>, crate::core_types::ArenaAllocation)> {
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
    // SAFETY: a typed arena slice start is non-null; changing only its pointee type preserves that
    // invariant without dereferencing it.
    let p = unsafe { NonNull::new_unchecked(p) };
    let mut touched_slices = slice_count;
    // SAFETY: the function contract keeps the arena prefix initialized and stable while the claim's
    // initial-zero state is derived from its backing MemId.
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
                && !unsafe { mi_mem_is_zero(p.as_ptr(), mi_size_of_slices(slice_count)) }
            {
                crate::allocator::options::_mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!("internal error: arena allocation was not zero-initialized!\n"),
                );
                claim.memid_mut().initially_zero = false;
            }
        } else {
            // SAFETY: arena slice allocation updates committed bitmaps, commit accounting, reuse state, and tracking consistently.
            unsafe { _mi_os_reuse(p.as_ptr(), mi_size_of_slices(slice_count)) };
            // SAFETY: allocator OS configuration remains initialized for the allocation epoch.
            if unsafe { _mi_os_has_overcommit() }
                && touched_slices > 0
                // SAFETY: the live stable arena prefix keeps its backing MemId readable; pinned
                // arenas are excluded from overcommit accounting adjustment.
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
            unsafe { mi_track_mem_defined(p.as_ptr(), slice_count * MI_ARENA_SLICE_SIZE) };
        } else {
            // SAFETY: the published claim owns this live slice range, which tracking transitions to
            // undefined before the allocation becomes visible.
            unsafe { mi_track_mem_undefined(p.as_ptr(), slice_count * MI_ARENA_SLICE_SIZE) };
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
/// `subproc` must remain live and initialized with stable registry storage, and
/// `idx` must be less than its current published arena count. Concurrent
/// registry growth may use the publication protocol, but teardown or relinking
/// existing entries must not race this lookup.
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
    // SAFETY: the raw contract keeps the live subproc count atomic valid; this
    // relaxed snapshot is used only to verify the caller-provided index bound.
    crate::internal::mi_assert_internal!(idx < unsafe { mi_arenas_get_count_typed(subproc_ptr) });
    // SAFETY: `arena_idx` is table-bounded and caller-proven below the published
    // count; the live registry cell is loaded with acquire ordering.
    unsafe { mi_arena_from_index_typed(subproc_ptr, arena_idx) }
}

// SAFETY: this assertion checks the translated C ABI signature of the arena-id export.
#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_arena_t) -> mi_arena_id_t = ids::mi_arena_id_from_arena;

#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_subproc_t) -> usize = mi_arenas_get_count;

// SAFETY: this assertion checks the translated C ABI signature of the arena lookup export.
#[cfg(feature = "c-abi")]
const _: unsafe extern "C" fn(*mut mi_subproc_t, usize) -> *mut mi_arena_t = mi_arena_from_index;

fn mi_arena_start_idx(
    hseq: crate::core_types::HeapSeq,
    hcount: usize,
    tseq: crate::core_types::ThreadSeq,
    arena_cycle: ArenaIndex,
) -> ArenaIndex {
    let arena_cycle = arena_cycle.get() as usize;
    if arena_cycle <= 1 {
        // SAFETY: zero is in the arena-index domain.
        return unsafe { ArenaIndex::new_unchecked(0) };
    }
    if hseq.get() == 0 || hcount <= 1 {
        // SAFETY: modulo by the nonzero bounded arena cycle produces an index
        // below `arena_cycle`, which is already an ArenaIndex value.
        return unsafe { ArenaIndex::new_unchecked((tseq.get() % arena_cycle) as u16) };
    }

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
    // SAFETY: the calculation above proves `start < arena_cycle`; the input
    // ArenaIndex bounds the cycle to the arena table domain.
    unsafe { ArenaIndex::new_unchecked(start as u16) }
}

/// # Safety
/// `arena` and an optional `req_arena` must remain live registered arena
/// metadata with stable role, parent, MemId, pinned, and NUMA fields during the
/// check. Registry/backing teardown or relinking must not race; allocator
/// protocol bitmap updates are permitted.
unsafe fn mi_arena_is_suitable_ex(
    arena: ArenaPtr,
    req_arena: Option<ArenaPtr>,
    match_numa: crate::core_types::MatchNuma,
    numa_node: crate::core_types::NumaNode,
    allow_pinned: crate::core_types::AllowPinned,
) -> bool {
    let arena_raw = arena.as_ptr();
    // SAFETY: the function contract keeps the registered arena prefix and its
    // initialized MemId pinned flag readable for this snapshot.
    if !allow_pinned.get() && unsafe { (*arena_raw.cast::<MiArenaPrefix>()).memid.is_pinned } {
        return false;
    }
    // SAFETY: both optional arena identities and their stable role/parent
    // metadata satisfy the lower-level suitability contract.
    if unsafe { !mi_arena_is_suitable(arena, req_arena) } {
        return false;
    }
    if req_arena.is_none() {
        // SAFETY: suitability runs while this registered live arena prefix is
        // stable; its initialized NUMA field is copied without mutation.
        let arena_numa =
            crate::types::NumaNode::new(unsafe { (*arena_raw.cast::<MiArenaPrefix>()).numa_node });
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

/// Visit the current arena registry snapshot in allocation preference order.
///
/// # Safety
/// `heap` must remain live and initialized with a stable subprocess association.
/// The subprocess registry, its published cells, every yielded arena, and arena
/// backing must remain live; teardown or relinking existing entries must not
/// race, while concurrent growth and allocator state mutation may use their
/// publication protocols. An optional `req_arena` must be compatible and live.
/// `visit` runs synchronously, must not retain an `ArenaPtr` or mutate registry
/// structure, and must not unwind after publishing allocator state.
unsafe fn mi_forall_arenas<F>(
    heap: BorrowedHeap,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    mut visit: F,
) where
    F: FnMut(ArenaPtr) -> bool,
{
    let heap_ptr = heap.ptr();
    // SAFETY: the function contract keeps the live initialized heap and its
    // stable subprocess field readable for the traversal snapshot.
    let Some(subproc) = (unsafe { heap_ptr.subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY: the same traversal epoch keeps the heap sequence field and the
    // associated subproc heap-count atomic valid for these scalar snapshots.
    let (hseq, hcount) = unsafe {
        let heap_raw = heap_ptr.as_ptr().cast::<MiHeapPrefix>();
        let subproc_raw = subproc.as_ptr().cast::<MiSubprocPrefix>();
        (
            crate::core_types::HeapSeq::new((*heap_raw).heap_seq),
            (*subproc_raw).heap_count.load(Ordering::Relaxed),
        )
    };
    // SAFETY: the live subproc count atomic remains valid, and concurrent growth
    // publishes table cells independently with acquire loads below.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc) };
    let arena_cycle = arena_count.saturating_sub(1);
    let Some(arena_cycle_idx) = ArenaIndex::from_usize(arena_cycle) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    let start = mi_arena_start_idx(hseq, hcount, tseq, arena_cycle_idx).get() as usize;
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
            // SAFETY: `arena_idx` is below the relaxed count snapshot; the live
            // registry cell is loaded with acquire ordering to observe publication.
            let Some(arena) =
                ArenaPtr::new(unsafe { mi_arena_from_index_typed(subproc, arena_idx) })
            else {
                continue;
            };
            arena
        };
        if !visit(arena) {
            break;
        }
    }
}

/// Visit arenas matching the requested pinned, requested-arena, and NUMA policy.
///
/// # Safety
/// `heap`, its subprocess registry, every yielded arena, and their backing must
/// remain live and stable against teardown or registry relinking. Each arena's
/// pinned, exclusive-role, parent, and NUMA fields must remain stable during
/// filtering. Concurrent registry growth and allocator state updates may use
/// their publication protocols. An optional `req_arena` must be compatible and
/// live. `visit` runs synchronously, must not retain an `ArenaPtr` or mutate
/// registry structure, and must not unwind after publishing allocator state.
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
    let mut visit_suitable = |arena| {
        // SAFETY: the enclosing function contract keeps each synchronously
        // yielded arena and optional requested arena metadata stable.
        let suitable = unsafe {
            mi_arena_is_suitable_ex(
                arena,
                req_arena,
                crate::core_types::MatchNuma::new(match_numa),
                numa_node,
                crate::core_types::AllowPinned::new(allow_large),
            )
        };
        !suitable || visit(arena)
    };
    // SAFETY: the function contract provides the complete live registry epoch;
    // this filtering adapter is synchronous and non-retaining and passes the
    // caller's post-publication unwind obligation through to `visit`.
    unsafe { mi_forall_arenas(heap, req_arena, tseq, &mut visit_suitable) };
}

#[allow(clippy::too_many_arguments)]
/// # Safety
/// `heap` must remain live and initialized with a stable subprocess association. The heap/subprocess,
/// arena registry, every visited arena, and its bitmap/backing storage must remain live; teardown or
/// registry relinking must not race. If supplied, `req_arena` must be a live registered arena
/// compatible with that subprocess. Concurrent slice claims and registry growth are permitted
/// through their allocator protocols. When `commit` is true, any stored commit callback and opaque
/// argument must remain valid for synchronous invocation and the callback must not unwind.
unsafe fn mi_arenas_try_find_free(
    heap: BorrowedHeap,
    slice_count: crate::core_types::SliceCount,
    alignment: crate::core_types::Alignment,
    commit: bool,
    allow_large: bool,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    numa_node: crate::core_types::NumaNode,
) -> Option<(NonNull<c_void>, crate::core_types::ArenaAllocation)> {
    crate::internal::mi_assert!(alignment.get() <= MI_ARENA_SLICE_ALIGN);
    if alignment.get() > MI_ARENA_SLICE_ALIGN {
        return None;
    }

    let mut found = None;
    // SAFETY: the function contract keeps the heap registry and each visited arena live. The
    // internal visitor synchronously claims at most one range and stores its complete allocation
    // value before stopping traversal.
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
                    found = Some((p, arena_allocation));
                    false
                } else {
                    true
                }
            },
        );
    }
    if found.is_some() || numa_node.is_any() {
        return found;
    }

    // SAFETY: the same live registry/arena epoch covers the fallback traversal; NUMA suitability is
    // relaxed, but the allocation claim and synchronous stop protocol are unchanged.
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
                    found = Some((p, arena_allocation));
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
/// # Safety
/// `heap` must remain live and initialized with a stable subprocess association. The heap/subprocess,
/// arena registry, and visited arena metadata/backing must remain live; teardown and registry
/// relinking must not race. If supplied, `req_arena` must be a compatible live registered arena.
/// Concurrent arena claims and registry growth remain permitted through their allocator protocols.
/// When `commit` is true, any stored commit callback and opaque argument must remain valid for
/// synchronous invocation and the callback must not unwind.
unsafe fn mi_arenas_try_alloc(
    heap: BorrowedHeap,
    slice_count: crate::core_types::SliceCount,
    alignment: crate::core_types::Alignment,
    commit: bool,
    allow_large: bool,
    req_arena: Option<ArenaPtr>,
    tseq: crate::core_types::ThreadSeq,
    numa_node: crate::core_types::NumaNode,
) -> Option<(NonNull<c_void>, crate::core_types::ArenaAllocation)> {
    crate::internal::mi_assert!(alignment.get() <= MI_ARENA_SLICE_ALIGN);
    if (slice_count.get() as usize).saturating_mul(MI_ARENA_SLICE_SIZE) > MI_ARENA_MAX_SIZE {
        return None;
    }
    // SAFETY: the allocator call contract keeps the borrowed heap, its subprocess registry, and
    // visited arenas live; the returned allocation value is initialized only after a successful
    // bitmap claim.
    if let Some((p, allocation)) = unsafe {
        mi_arenas_try_find_free(
            heap,
            slice_count,
            alignment,
            commit,
            allow_large,
            req_arena,
            tseq,
            numa_node,
        )
    } {
        return Some((p, allocation));
    }
    if req_arena.is_some() || _mi_preloading() {
        return None;
    }
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc) {
        return None;
    }

    // SAFETY: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
    let Some(subproc) = (unsafe { heap.ptr().subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return None;
    };
    // SAFETY: the function contract keeps the subprocess and initialized arena-count atomic live.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc) };
    {
        // SAFETY: the live subprocess owns this initialized reserve lock; the guard serializes
        // registry growth and releases the same lock at scope exit.
        let _lock = unsafe { MiLockGuard::acquire(subproc.arena_reserve_lock()) };
        // SAFETY: the held reserve lock stabilizes registry growth while the live subprocess count
        // is compared with the pre-lock snapshot.
        if arena_count == unsafe { mi_arenas_get_count_typed(subproc) } {
            // SAFETY: the reserve lock and function contract keep the subprocess registry/stats live;
            // reserve callbacks remain valid and non-unwinding for the synchronous growth attempt.
            let _ = unsafe {
                mi_arena_reserve(
                    subproc,
                    crate::core_types::ReservedBytes::new(mi_size_of_slices(
                        slice_count.get() as usize
                    )),
                    crate::core_types::NumaNode::any(),
                    allow_large,
                )
            };
        }
    }

    // SAFETY: after reserve growth, the allocator contract still keeps the heap registry and arenas
    // live for a second synchronous claim attempt.
    unsafe {
        mi_arenas_try_find_free(
            heap,
            slice_count,
            alignment,
            commit,
            allow_large,
            req_arena,
            tseq,
            numa_node,
        )
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

/// # Safety
/// If present, `heap` must identify a live initialized borrowed heap with a stable subprocess
/// association; the heap/subprocess must not be torn down during the call. If present, `req_arena`
/// must be a compatible live registered arena. `memid` must be non-null, aligned, and uniquely
/// writable on every successful arena or OS allocation. Allocator OS configuration and any stored
/// arena commit or primitive allocation callback, including opaque arguments, must remain valid for
/// synchronous invocation and must not unwind. Arena registry or backing teardown must not race;
/// concurrent claims and growth may use their allocator protocols.
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
        && size.get() <= mi_arena_max_object_size()
        && alignment.get() <= MI_ARENA_SLICE_ALIGN
        && align_offset.get() == 0
        && let (Some(slice_count), Some(heap)) = (arena_slice_count, heap)
    {
        let arena_alignment = crate::core_types::Alignment::new(alignment.get())
            // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
            .unwrap_or_else(|| unsafe { crate::core_types::Alignment::new_unchecked(1) });
        // SAFETY: the function contract keeps this non-null borrowed heap initialized with a stable
        // subprocess association for the synchronous allocation attempt.
        let heap = unsafe { BorrowedHeap::new_unchecked(heap) };
        // SAFETY: the function contract keeps the heap association, registry, requested arena, and
        // visited arena backing live while the allocation result is produced.
        let arena_allocation = unsafe {
            mi_arenas_try_alloc(
                heap,
                slice_count,
                arena_alignment,
                commit,
                allow_large,
                req_arena,
                tseq,
                numa_node,
            )
        };
        if let Some((p, allocation)) = arena_allocation {
            // SAFETY: the typed allocator boundary requires a uniquely writable memid output; the
            // owned allocation result supplies its fully initialized value.
            unsafe { *memid = allocation.into_memid() };
            return p.as_ptr();
        }
    }

    // SAFETY: arena OS fallback allocation preserves alignment, requested layout, memid initialization, errno, and disallow-options behavior.
    let result = unsafe {
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
        )
    };
    // SAFETY: the raw boundary keeps `memid` aligned and uniquely writable while
    // the helper writes the MemId matching this fallback pointer result.
    unsafe { mi_write_arena_os_allocation_result(result, &mut *memid) }
}

/// # Safety
/// `size` must be nonzero. If non-null, `heap` must be a live initialized
/// borrowed heap with a stable subprocess association, and `req_arena` must be
/// a compatible live registered arena. `memid` must be non-null, aligned, and
/// uniquely writable for successful result publication. Allocator OS state and
/// every stored arena commit or primitive allocation callback and opaque argument
/// must remain valid for synchronous invocation and must not unwind. Heap,
/// subprocess, arena registry, and backing teardown must not race; concurrent
/// arena claims and growth may proceed through their atomic protocols.
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
        Err(_) => {
            // SAFETY: the raw contract keeps OS configuration and the requested
            // arena live for this synchronous fallback attempt.
            let result = unsafe {
                mi_arena_os_alloc_aligned(
                    ArenaOsAllocRequest {
                        size,
                        alignment,
                        align_offset,
                    },
                    commit,
                    allow_large,
                    req_arena.cast::<c_void>(),
                )
            };
            // SAFETY: the raw boundary keeps `memid` aligned and uniquely
            // writable while the helper writes the matching fallback MemId.
            unsafe { mi_write_arena_os_allocation_result(result, &mut *memid) }
        }
    }
}

/// # Safety
/// `size` must be nonzero. If non-null, `heap` must be a live initialized
/// borrowed heap with a stable subprocess association, and `req_arena` must be
/// a compatible live registered arena. `memid` must be non-null, aligned, and
/// uniquely writable for successful result publication. Allocator OS state and
/// every stored arena commit or primitive allocation callback and opaque argument
/// must remain valid for synchronous invocation and must not unwind. Heap,
/// subprocess, arena registry, and backing teardown must not race; concurrent
/// arena claims and growth may proceed through their atomic protocols.
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
    let heap = HeapPtr::new(heap);
    let arena_slice_count = mi_arena_slice_count_for_request(size);
    let req_arena = ArenaPtr::new(req_arena);
    let tseq = crate::core_types::ThreadSeq::new(tseq);
    let numa_node = crate::core_types::NumaNode::new(numa_node);
    let alignment = MI_ARENA_SLICE_SIZE;

    match crate::core_types::LayoutRequest::aligned(size.get(), alignment) {
        // SAFETY: the raw contract keeps the heap, requested arena, callbacks, and output live;
        // checked layout and slice-count construction satisfy the typed allocation boundary.
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
        // SAFETY: the raw contract keeps OS configuration, requested arena, callbacks, and output
        // live; this preserves the prior fallback inputs after checked layout rejection.
        Err(_) => {
            // SAFETY: the raw contract keeps OS configuration and the requested
            // arena live for this synchronous fallback attempt.
            let result = unsafe {
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
                )
            };
            // SAFETY: the raw boundary keeps `memid` aligned and uniquely
            // writable while the helper writes the matching fallback MemId.
            unsafe { mi_write_arena_os_allocation_result(result, &mut *memid) }
        }
    }
}

/// Search the current heap's suitable arenas for a matching abandoned page and
/// return its claimed ownership capability.
///
/// # Safety
/// `theap` must identify the current thread's live initialized thread heap, with
/// its heap/TLD association and heap arena-page table stable for this operation.
/// The heap, subprocess arena registry, matching arena metadata/backing, page
/// metadata, encoding keys, abandoned counters, and local stats must remain
/// live; the current thread must exclusively own local `theap` and stats
/// mutation. Teardown or non-protocol relinking must not race. Every page
/// selectable from the chosen abandoned bitmap must have this `block_size` and
/// a live arena MemId range of exactly `slice_count`; the abandoned, free,
/// committed, and dirty bitmaps must be initialized and cover that range.
/// Competing abandoned claims and remote frees may proceed only through their
/// atomic protocols. Callbacks and diagnostics reached while collecting the
/// claimed page must remain valid, non-reentrant for these objects, and
/// non-unwinding. A successful return is the sole ownership capability for the
/// claimed page; `None` retains no claim.
unsafe fn mi_arenas_page_try_find_abandoned(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
) -> Option<crate::core_types::OwnedAbandonedPage> {
    // SAFETY: the raw contract keeps the current thread heap and its associated
    // borrowed heap live for this synchronous arena search.
    let heap = unsafe { _mi_theap_heap(theap) };
    // SAFETY: the caller keeps the thread heap's initialized TLD association
    // live and stable while its thread sequence is read.
    let tseq = crate::core_types::ThreadSeq::new(unsafe { (*(*theap).tld).thread_seq });
    // SAFETY: `heap` is the live borrowed heap resolved above, whose exclusive
    // arena selection remains stable for the search.
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    let slice_count_usize = slice_count.get() as usize;

    let bin = mi_bin_from_block_size(block_size);
    crate::internal::mi_assert_internal!(bin.as_usize() < MI_BIN_COUNT);
    // SAFETY: the live heap prefix contains this bin's initialized atomic
    // abandoned count; concurrent updates use the same atomic protocol.
    if unsafe { mi_heap_abandoned_count(heap, bin).load_relaxed() } == 0 {
        return None;
    }

    let any_numa = crate::core_types::NumaNode::any();
    let mut result = None;
    let claim_fun = ThreadLocalClaimCallback::new(mi_arena_try_claim_abandoned);
    let Some(heap_ptr) = HeapPtr::new(heap) else {
        crate::internal::mi_assert_internal!(false);
        return None;
    };
    // SAFETY: the raw contract keeps this heap live for the synchronous arena
    // enumeration and establishes its stable borrowed-heap association.
    let borrowed_heap = unsafe { BorrowedHeap::new_unchecked(heap_ptr) };
    let mut visit_arena = |arena| {
        // SAFETY: the visitor keeps the registered arena and borrowed
        // heap stable while loading this arena's published page table.
        let arena_pages = unsafe { mi_heap_arena_pages(borrowed_heap, arena) };
        if arena_pages.is_null() {
            return true;
        }
        let mut slice_index = SliceIndex::new(0);
        // SAFETY: the non-null published table contains the initialized
        // abandoned bitmap for the checked block-size bin.
        let bitmap = unsafe { (*arena_pages).pages_abandoned[bin.as_usize()] };
        // SAFETY: the live bitmap's atomic claim transfers ownership of
        // at most one matching abandoned page; a successful claim is
        // carried through page projection and witness construction in
        // this same epoch before traversal can continue.
        let owned_page = unsafe {
            if !mi_bitmap_try_find_and_claim(bitmap, tseq, &mut slice_index, claim_fun, Some(arena))
            {
                None
            } else {
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
                Some(crate::core_types::assume_owned_abandoned_page(page))
            }
        };
        let Some(owned_page) = owned_page else {
            return true;
        };
        let page_raw = owned_page.page().as_ptr();
        // SAFETY: the owned claim keeps this abandoned page's local and
        // remote free-list metadata live through collection.
        unsafe { crate::allocator::page::collect_page_free_lists_abandoned(&owned_page, false) };
        let arena_raw = arena.as_ptr();
        // SAFETY: the claimed page and arena membership keep this exact
        // range's free bitmap live for the assertion-only check.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bbitmap_is_clearN(
                (*arena_raw.cast::<MiArenaPrefix>()).slices_free,
                slice_index.get(),
                slice_count_usize,
            )
        });
        // SAFETY: claimed ownership stabilizes the initialized page
        // commitment field for this assertion-only snapshot.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page_raw) };
        crate::internal::mi_assert_internal!(
            !committed.is_zero()
                        // SAFETY: the live arena membership covers this exact
                        // committed-bitmap range during the claim.
                        || unsafe {
                            mi_bitmap_is_setN(
                                (*arena_raw.cast::<MiArenaPrefix>()).slices_committed,
                                slice_index.get(),
                                slice_count_usize,
                            )
                        }
        );
        // SAFETY: the live arena membership covers this exact dirty
        // bitmap range during the claim.
        crate::internal::mi_assert_internal!(unsafe {
            mi_bitmap_is_setN(
                (*arena_raw.cast::<MiArenaPrefix>()).slices_dirty,
                slice_index.get(),
                slice_count_usize,
            )
        });
        // SAFETY: the owned abandoned page preserves geometry and
        // backing through these assertion-only metadata reads.
        unsafe {
            crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
                mi_page_slice_start(page_raw).cast(),
                MI_PAGE_ALIGN
            ));
            crate::internal::mi_assert_internal!(
                crate::internal::_mi_ptr_page(mi_page_start(page_raw).cast()) == page_raw
            );
            crate::internal::mi_assert_internal!(mi_page_block_size(page_raw) == block_size.get());
            crate::internal::mi_assert_internal!(!mi_page_is_full_local(page_raw));
        }
        result = Some(owned_page);
        false
    };
    // SAFETY: the caller keeps the borrowed heap, arena registry/backing, page
    // tables, and abandoned-page metadata live. The synchronous visitor does
    // not retain arena witnesses, and successful bitmap callbacks transfer the
    // page's atomic owner claim into `result` before traversal stops.
    unsafe {
        mi_forall_suitable_arenas(
            borrowed_heap,
            ArenaPtr::new(req_arena),
            tseq,
            true,
            any_numa,
            true,
            &mut visit_arena,
        )
    };
    result
}

/// # Safety
/// `theap` must identify the current thread's live initialized thread heap, with exclusive local
/// access to its heap/TLD association and arena-page table. Its heap, TLD, subprocess arena registry,
/// page map, and any allocated arena or OS backing must remain live and stable through construction;
/// teardown or non-protocol relinking must not race. Stored arena commit and primitive OS callbacks,
/// including their opaque arguments, must remain valid for synchronous invocation and must not unwind.
/// `slice_count` must describe a nonzero page allocation, and `block_alignment` must be zero or a
/// valid allocator alignment; its value alone determines arena versus OS-aligned policy.
unsafe fn mi_arenas_page_alloc_fresh_area(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    block_alignment: crate::core_types::MaybeAlignment,
    commit: bool,
) -> Option<PendingFreshPage> {
    let block_alignment = block_alignment.get();
    let os_align = block_alignment > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let allow_large = true;
    let page_alignment = MI_ARENA_SLICE_ALIGN;
    // SAFETY: the fresh-area contract keeps the current thread heap and its
    // borrowed heap association live for this synchronous allocation attempt.
    let heap = unsafe { _mi_theap_heap(theap) };
    // SAFETY: the caller keeps the current thread heap's initialized TLD
    // association live and stable throughout the allocation attempt.
    let tld = unsafe { (*theap).tld };
    // SAFETY: `heap` is the live borrowed heap resolved above, whose requested
    // exclusive arena remains stable for this operation.
    let req_arena = unsafe { (*heap.cast::<MiHeapPrefix>()).exclusive_arena };
    // SAFETY: the live heap prefix keeps its initialized NUMA policy readable.
    let heap_numa =
        crate::core_types::NumaNode::new(unsafe { (*heap.cast::<MiHeapPrefix>()).numa_node });
    // SAFETY: the live TLD keeps its initialized NUMA policy readable.
    let tld_numa = crate::core_types::NumaNode::new(unsafe { (*tld).numa_node });
    let numa_node = if !heap_numa.is_any() {
        heap_numa
    } else {
        tld_numa
    };

    let slice_count_usize = slice_count.get() as usize;
    let alloc_size = mi_size_of_slices(slice_count_usize);
    let arena_pending = if !mi_option_is_enabled(mi_option_t::mi_option_disallow_arena_alloc)
        && !os_align
        && slice_count_usize <= mi_arena_max_object_size() / MI_ARENA_SLICE_SIZE
    {
        let Some(heap_ptr) = HeapPtr::new(heap) else {
            crate::internal::mi_assert_internal!(false);
            return None;
        };
        // SAFETY: the fixed arena page alignment is nonzero and power-of-two.
        let page_alignment = unsafe { crate::core_types::Alignment::new_unchecked(page_alignment) };
        // SAFETY: the function contract keeps this non-null heap locally owned, initialized, and
        // live for the synchronous arena attempt.
        let heap_borrowed = unsafe { BorrowedHeap::new_unchecked(heap_ptr) };
        // SAFETY: the live initialized TLD remains stable under current-thread ownership.
        let thread_seq = crate::core_types::ThreadSeq::new(unsafe { (*tld).thread_seq });
        // SAFETY: the fresh-page contract keeps the heap/TLD association, arena registry, and
        // requested arena/backing live while the owned allocation result is produced.
        let arena_allocation = unsafe {
            mi_arenas_try_alloc(
                heap_borrowed,
                slice_count,
                page_alignment,
                commit,
                allow_large,
                ArenaPtr::new(req_arena),
                thread_seq,
                numa_node,
            )
        };
        if let Some((arena_start, allocation)) = arena_allocation {
            let arena = allocation.arena();
            let range = allocation.range();
            let memid = allocation.into_memid();
            let start = arena_start.as_ptr().cast::<u8>();
            // SAFETY: successful arena allocation returned live backing and a
            // matching memid owned by this pending transaction.
            let mut pending = unsafe { PendingFreshPage::new(start, alloc_size, memid) };
            // SAFETY: the owned arena allocation preserves its registered arena
            // identity and exact range; the live heap table is ensured before
            // the page start bit becomes visible.
            let arena_pages = unsafe { mi_heap_ensure_arena_pages_from_raw(heap, arena.as_ptr()) };
            // SAFETY: the ensured bitmap belongs to this live heap and exact
            // arena; the pending allocation owns its clear range start bit.
            if unsafe { pending.install_heap_page_bit(arena_pages, range.start()) } {
                Some(pending)
            } else {
                drop(pending);
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let pending = if let Some(pending) = arena_pending {
        pending
    } else {
        let request = if os_align {
            ArenaOsAllocRequest {
                size: alloc_size,
                alignment: block_alignment,
                align_offset: page_alignment,
            }
        } else {
            ArenaOsAllocRequest {
                size: alloc_size,
                alignment: page_alignment,
                align_offset: 0,
            }
        };
        // SAFETY: the fresh-area contract keeps OS state, requested arena, and
        // callbacks live for the synchronous allocation result.
        let (start, memid) =
            unsafe { mi_arena_os_alloc_aligned(request, commit, allow_large, req_arena.cast()) }
                .ok()?;
        let start = start.cast::<u8>();
        // SAFETY: the OS allocator returned this live allocation and matching
        // memid; both are now exclusively owned by the pending transaction.
        unsafe { PendingFreshPage::new(start.as_ptr(), alloc_size, memid) }
    };
    let start = pending.start();
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
    Some(pending)
}

fn mi_page_block_start(block_size: crate::core_types::BlockSize, os_align: bool) -> usize {
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

/// Allocate, initialize, register, and publish one fresh uninitialized page.
///
/// # Safety
/// `theap` must identify the current thread's live initialized thread heap, with
/// exclusive local access to page metadata initialization and stats publication.
/// `slice_count` must describe a nonzero allocation large enough for the requested
/// nonzero `block_size`; `block_alignment` must be zero or a valid allocator
/// alignment. The heap/TLD association, arena registry and page tables, page map,
/// allocator OS state, backing memory, and stored callbacks/arguments must remain
/// live and stable through rollback or publication. Synchronous callbacks must
/// not unwind; teardown and non-protocol metadata mutation must not race.
unsafe fn mi_arenas_page_alloc_fresh(
    theap: *mut mi_theap_t,
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
    block_alignment: crate::core_types::MaybeAlignment,
    commit: bool,
) -> Option<crate::core_types::Page<crate::core_types::Uninit>> {
    let block_size_raw = block_size.get();
    let block_alignment_raw = block_alignment.get();
    let os_align = block_alignment_raw > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let slice_count_usize = slice_count.get() as usize;
    let alloc_size = mi_size_of_slices(slice_count_usize);
    // SAFETY: the fresh-page contract is forwarded unchanged to the backing
    // transaction, which returns sole rollback ownership on success.
    let mut pending =
        (unsafe { mi_arenas_page_alloc_fresh_area(theap, slice_count, block_alignment, commit) })?;
    let slice_start = pending.start();
    let mut memid = pending.memid();

    let page_noguard_size = alloc_size;
    let mut page = ptr::null_mut::<mi_page_t>();
    let mut page_meta_is_separate = false;
    let mut block_start = 0usize;

    if let crate::core_types::MemIdView::Arena(a) = crate::core_types::MemId::view(memid) {
        let arena = a.arena().as_ptr();
        // SAFETY: the pending allocation's decoded arena and range keep this
        // registered arena prefix and its pages_meta field live and stable.
        if unsafe { !(*arena.cast::<MiArenaPrefix>()).pages_meta.is_null() } {
            // SAFETY: the typed arena range supplies an in-bounds start index
            // for the live separate metadata array selected above.
            let page_meta = unsafe {
                (*arena.cast::<MiArenaPrefix>())
                    .pages_meta
                    .add(a.range().start_usize())
            };
            // SAFETY: this transaction exclusively owns the selected fresh
            // metadata slot, whose block_size field is initialized/readable.
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
                // SAFETY: this transaction exclusively owns the aligned live
                // separate metadata slot and may initialize its full extent.
                unsafe {
                    crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>())
                };
            }
        }
    }
    if page.is_null() {
        page = slice_start.cast::<mi_page_t>();
        block_start = mi_page_block_start(block_size, os_align);
    }
    if page_meta_is_separate {
        // SAFETY: the decoded arena range selected this live separate metadata
        // slot, which this pending transaction exclusively initializes and can
        // zero again during rollback.
        unsafe { pending.set_separate_page_meta(PagePtr::new_unchecked(page)) };
    }

    let mut commit_size = crate::core_types::CommittedBytes::new(0);
    if !memid.initially_committed {
        let mut commit_size_raw =
            _mi_align_up(block_start + block_size_raw, MI_PAGE_MIN_COMMIT_SIZE);
        if commit_size_raw > page_noguard_size {
            commit_size_raw = page_noguard_size;
        }
        commit_size = crate::core_types::CommittedBytes::new(commit_size_raw);
        // SAFETY: an uncommitted pending backing necessarily carries its exact
        // arena/range MemId; the backing and callback remain live, and is_zero
        // is uniquely writable for this synchronous commit.
        let mut is_zero = false;
        // SAFETY: memid is the arena/range MemId from the pending backing; mi_arena_commit is called with exclusive backing ownership, live arena, and writable is_zero slot.
        if !unsafe {
            let (arena_ptr, range) = match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => (a.arena(), a.range()),
                _ => {
                    crate::internal::mi_assert_internal!(false);
                    return None;
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
            return None;
        }
    }
    if !memid.initially_zero && !page_meta_is_separate {
        // SAFETY: colocated page metadata lies within this transaction's live
        // writable backing and must be initialized before field publication.
        unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
    }

    if memid.initially_committed {
        if memid.initially_zero {
            // SAFETY: the committed initially-zero backing belongs exclusively
            // to this transaction and tracking covers its exact slice extent.
            unsafe {
                mi_track_mem_defined(slice_start.cast(), slice_count_usize * MI_ARENA_SLICE_SIZE)
            };
        } else {
            // SAFETY: the committed nonzero backing belongs exclusively to this
            // transaction and tracking covers its exact slice extent.
            unsafe {
                mi_track_mem_undefined(slice_start.cast(), slice_count_usize * MI_ARENA_SLICE_SIZE)
            };
        }
    }
    if MI_DEBUG > 1
        && memid.initially_zero
        && memid.initially_committed
        // SAFETY: the committed live backing covers the complete pending page
        // extent and remains readable during this diagnostic scan.
        && !unsafe { mi_mem_is_zero(slice_start.cast(), page_noguard_size) }
    {
        crate::allocator::options::_mi_error_message_fmt(
            Errno::EFAULT,
            format_args!("internal error: page memory was not zero initialized.\n"),
        );
        memid.initially_zero = false;
        if block_start > 0 {
            // SAFETY: this transaction exclusively owns the live page metadata
            // extent and may restore it after the zero-state diagnostic fails.
            unsafe { crate::internal::_mi_memzero_aligned(page.cast(), size_of::<mi_page_t>()) };
        }
    }
    let reserved = if os_align {
        1
    } else {
        (page_noguard_size - block_start) / block_size_raw
    };
    crate::internal::mi_assert_internal!(reserved > 0 && u16::try_from(reserved).is_ok());

    // SAFETY: the pending transaction exclusively owns this live page metadata
    // location. The checked layout values initialize its coupled reserved,
    // start, block-size, commit, MemId, and zero-state fields before registration.
    unsafe {
        crate::allocator::page::mi_page_set_reserved_count(
            page,
            crate::core_types::ReservedBlockCount::new(reserved as u16),
        );
        (*page).page_start = slice_start.add(block_start);
        (*page).block_size = block_size_raw;
        crate::allocator::page::mi_page_set_slice_committed(page, commit_size);
        (*page).memid = memid;
        (*page).free_is_zero = memid.initially_zero;
    };
    // SAFETY: the selected metadata address is non-null and aligned within the
    // transaction's live allocation.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: the freshly initialized page's available-list head is readable
    // and remains transaction-exclusive before registration.
    crate::internal::mi_assert_internal!(unsafe { page_ptr.free().is_empty() });
    // SAFETY: the initialized metadata location and MemId remain stable, so its
    // separate/colocated classification is readable.
    crate::internal::mi_assert_internal!(
        // SAFETY: page is a freshly initialized live page with stable free-list head and metadata classification before registration.
        page_meta_is_separate == unsafe { mi_page_meta_is_separated(page) }
    );
    // SAFETY: the freshly initialized page geometry remains stable and its
    // recorded slice start must match the transaction backing.
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
    pending.set_initially_zero(memid.initially_zero);
    // SAFETY: all page metadata and the owner word are initialized above; the
    // pending backing and page-map lifecycle remain live through registration.
    let registered = (unsafe { pending.register(page_ptr) })?;

    // SAFETY: the caller keeps the current thread heap's exclusive stats live;
    // registration now stabilizes the initialized page metadata used to select
    // and increment its exact page-bin counters.
    unsafe {
        __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.pages), 1);
        let stats_bin = _mi_page_stats_bin(page);
        __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.page_bins[stats_bin]), 1);
    };

    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: registration keeps the initialized page geometry live and
        // stable through this alignment validation.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    // SAFETY: registration keeps the initialized page start and page-map entry
    // live and stable through this identity validation.
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY: registration keeps the initialized block-size field readable.
    crate::internal::mi_assert_internal!(unsafe { mi_page_block_size(page) } == block_size_raw);
    // SAFETY: the initialized owner word remains claimed and the page has not
    // yet been attached to a thread heap, so abandoned state is readable.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    // SAFETY: the initialized atomic owner word remains live after the
    // successful claim and before the uninitialized-page witness is published.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });

    Some(registered.publish())
}

#[must_use = "the page-state capability must be consumed by the page allocator"]
pub(crate) enum ArenaPageAllocation {
    Claimed(crate::core_types::OwnedAbandonedPage),
    Active(crate::core_types::ActivePage),
}

impl ArenaPageAllocation {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        match self {
            Self::Claimed(page) => page.page(),
            Self::Active(page) => page.page(),
        }
    }
}

/// Complete fresh-page initialization or release the still-uninitialized page
/// through its matching allocation identity.
///
/// # Safety
/// `owner` must be the live current-thread owner of the registered `page`, with
/// local ownership of its metadata and stats. The page must be detached, have
/// no used blocks, and retain its claimed thread-free owner bit. Its backing,
/// initialized MemId and geometry, page map, heap/TLD association, and allocator
/// teardown state must remain stable through initialization or cleanup.
unsafe fn mi_arenas_page_finish_fresh(
    owner: crate::core_types::ThreadHeapLocal,
    page: crate::core_types::Page<crate::core_types::Uninit>,
) -> Option<crate::core_types::ActivePage> {
    // SAFETY: the caller proves `owner` is the live current-thread owner and
    // `page` is its registered uninitialized-page capability.
    match unsafe { crate::allocator::page::mi_page_init_typed(owner, page) } {
        Ok(active) => Some(active),
        Err(page) => {
            let page_ptr = page.ptr();
            // SAFETY: failed initialization returns the same live, registered,
            // detached page with no used blocks; returning its owner to the
            // abandoned state preserves the claimed thread-free owner bit.
            unsafe { mi_page_set_theap(page_ptr.as_ptr(), ptr::null_mut()) };
            // SAFETY: the failed-init page remains registered with stable MemId
            // and geometry, has no used blocks or queue links, and the owner
            // transition above established abandoned claimed ownership.
            let all_free = unsafe { page.into_all_free_after_failed_init() };
            // SAFETY: `owner` is the matching live stats owner; the all-free
            // witness carries the exact detached registered allocation through
            // page-map, heap-bit, accounting, and backing release.
            let destroyed = unsafe { _mi_arenas_page_free_typed(all_free, owner) };
            let _ = destroyed.page();
            None
        }
    }
}

/// # Safety
/// `owner` must identify the current thread's live initialized thread heap with
/// local access to its heap/TLD association and page initialization.
/// Its heap, TLD, subprocess arena registry, page map, arena metadata/backing,
/// and allocation callbacks must remain live and stable; callbacks invoked
/// synchronously must not unwind. Teardown and non-protocol relinking must not
/// race, while abandoned claims and remote frees may use their atomic protocols.
unsafe fn mi_arenas_page_regular_alloc(
    owner: crate::core_types::ThreadHeapLocal,
    slice_count: crate::core_types::SliceCount,
    block_size: crate::core_types::BlockSize,
) -> Option<ArenaPageAllocation> {
    let theap = owner.ptr().as_ptr();
    // SAFETY: the regular-allocation contract supplies the live thread heap,
    // registry, arena backing, and atomic abandoned-page claim epoch.
    if let Some(page) = unsafe { mi_arenas_page_try_find_abandoned(theap, slice_count, block_size) }
    {
        return Some(ArenaPageAllocation::Claimed(page));
    }

    let commit_on_demand =
        crate::allocator::options::mi_option_get(mi_option_t::mi_option_page_commit_on_demand);
    let slice_count_usize = slice_count.get() as usize;
    let commit = slice_count_usize <= mi_slice_count_of_size(MI_PAGE_MIN_COMMIT_SIZE)
        // SAFETY: allocator OS configuration remains initialized and stable for
        // this policy read during fresh-page selection.
        || (commit_on_demand == 2 && unsafe { _mi_os_has_overcommit() })
        || commit_on_demand == 0;
    // SAFETY: the regular-allocation contract keeps the current thread heap,
    // backing transaction, page map, and callbacks live; zero alignment selects
    // ordinary page geometry for this checked size-class slice count.
    let page_uninit = (unsafe {
        mi_arenas_page_alloc_fresh(
            theap,
            slice_count,
            block_size,
            crate::core_types::MaybeAlignment::new_unchecked(0),
            commit,
        )
    })?;
    let page = page_uninit.ptr().as_ptr();
    // SAFETY: the registered uninitialized page keeps its initialized memid
    // readable until the consuming initialization or cleanup below.
    crate::internal::mi_assert_internal!(match crate::core_types::MemId::view(unsafe {
        (*page).memid
    }) {
        crate::core_types::MemIdView::Arena(a) => a.range().count() == slice_count,
        _ => true, // Non-arena allocations trivially pass this slice_count assert
    });
    // SAFETY: the regular-allocation contract keeps the current thread heap
    // and returned registered page stable through consuming init-or-cleanup.
    unsafe { mi_arenas_page_finish_fresh(owner, page_uninit) }.map(ArenaPageAllocation::Active)
}

/// # Safety
/// `owner` must identify the current thread's live initialized thread heap with
/// local access to fresh-page initialization. Its heap/TLD, arena
/// registry and backing, page map, allocator OS state, and stored callbacks must
/// remain live and stable; synchronously invoked callbacks must not unwind and
/// teardown must not race.
#[inline(never)]
unsafe fn mi_arenas_page_singleton_alloc(
    owner: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
    block_alignment: crate::core_types::MaybeAlignment,
) -> Option<ArenaPageAllocation> {
    let theap = owner.ptr().as_ptr();
    let block_size_raw = block_size.get();
    let block_alignment_raw = block_alignment.get();
    let os_align = block_alignment_raw > MI_PAGE_MAX_OVERALLOC_ALIGN;
    let info_size = if os_align {
        MI_PAGE_ALIGN
    } else {
        crate::internal::mi_page_info_size()
    };
    let Some(full_size) = info_size.checked_add(block_size_raw) else {
        mi_set_errno(ENOMEM);
        return None;
    };
    let slice_count = mi_slice_count_of_size(full_size);
    let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
        mi_set_errno(ENOMEM);
        return None;
    };
    // SAFETY: the singleton-allocation contract keeps the current thread heap,
    // backing transaction, page map, and callbacks live for this derived slice
    // count and caller-validated optional alignment.
    let page_uninit = (unsafe {
        mi_arenas_page_alloc_fresh(theap, slice_count, block_size, block_alignment, true)
    })?;
    let page = page_uninit.ptr().as_ptr();
    // SAFETY: the registered uninitialized singleton keeps its reserved-count
    // metadata live and readable until consuming initialization or cleanup.
    crate::internal::mi_assert_internal!(unsafe {
        crate::allocator::page::mi_page_reserved_count(page).get() == 1
    });
    // SAFETY: the singleton-allocation contract keeps the current thread heap
    // and returned registered page stable through consuming init-or-cleanup.
    unsafe { mi_arenas_page_finish_fresh(owner, page_uninit) }.map(ArenaPageAllocation::Active)
}

/// # Safety
/// `owner` must identify the current thread's live initialized thread heap with
/// local access to page allocation and initialization. Its heap/TLD, arena
/// registry and backing, page map, allocator OS state, and stored callbacks must
/// remain live and stable; synchronously invoked callbacks must not unwind and
/// teardown or non-protocol relinking must not race.
#[inline(never)]
pub(crate) unsafe fn _mi_arenas_page_alloc(
    owner: crate::core_types::ThreadHeapLocal,
    block_size: crate::core_types::BlockSize,
    block_alignment: crate::core_types::MaybeAlignment,
) -> Option<ArenaPageAllocation> {
    let block_size_raw = block_size.get();
    let block_alignment_raw = block_alignment.get();
    let allocation = if block_alignment_raw > MI_PAGE_MAX_OVERALLOC_ALIGN {
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_power_of_two(
            block_alignment_raw
        ));
        // SAFETY: the outer contract keeps the thread heap, registry/backing,
        // page map, OS state, and callbacks live for singleton initialization.
        unsafe { mi_arenas_page_singleton_alloc(owner, block_size, block_alignment) }
    } else if block_size_raw <= MI_SMALL_MAX_OBJ_SIZE {
        // SAFETY: the outer contract establishes the regular allocation epoch.
        unsafe { mi_arenas_page_regular_alloc(owner, MI_SMALL_PAGE_SLICE_COUNT, block_size) }
    } else if block_size_raw <= MI_MEDIUM_MAX_OBJ_SIZE {
        // SAFETY: the outer contract establishes the regular allocation epoch.
        unsafe { mi_arenas_page_regular_alloc(owner, MI_MEDIUM_PAGE_SLICE_COUNT, block_size) }
    } else if MI_ENABLE_LARGE_PAGES != 0 && block_size_raw <= MI_LARGE_MAX_OBJ_SIZE {
        // SAFETY: the outer contract establishes the regular allocation epoch.
        unsafe { mi_arenas_page_regular_alloc(owner, MI_LARGE_PAGE_SLICE_COUNT, block_size) }
    } else {
        // SAFETY: the outer contract keeps the thread heap, registry/backing,
        // page map, OS state, and callbacks live for singleton initialization.
        unsafe { mi_arenas_page_singleton_alloc(owner, block_size, block_alignment) }
    };
    let allocation = allocation?;
    let page = allocation.page().as_ptr();
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        // SAFETY: either returned capability keeps this live page's initialized
        // arena geometry readable through validation.
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    // SAFETY: either returned capability keeps its initialized page start and
    // page-map association live for validation.
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    crate::internal::mi_assert_internal!(
        block_alignment_raw <= MI_PAGE_MAX_OVERALLOC_ALIGN
            || crate::internal::_mi_is_aligned(
                // SAFETY: either returned capability keeps this initialized
                // page start readable for the requested alignment check.
                unsafe { mi_page_start(page) }.cast(),
                block_alignment_raw
            )
    );
    Some(allocation)
}

/// # Safety
/// `page` must be a non-null registered, detached, abandoned, all-free page
/// retaining its claimed thread-free owner bit and empty queue links. Its MemId,
/// page-map registration, arena table/start bit, metadata, and backing must remain
/// live and stable through release. If non-null, `current_theapx` must be the
/// live current-thread stats owner for this page; otherwise its owning heap's
/// atomic stats must remain live. Protocol remote-head publication is permitted,
/// but relink, claim, non-protocol metadata mutation, and teardown are excluded.
pub(crate) unsafe extern "C" fn _mi_arenas_page_free(
    page: *mut mi_page_t,
    current_theapx: *mut mi_theap_t,
) {
    // SAFETY: the raw contract supplies non-null live page metadata for the
    // complete synchronous free transaction.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: the raw contract establishes abandoned state for this registered
    // page while its thread-free ownership claim remains held.
    let abandoned = unsafe { crate::core_types::assume_abandoned_page(page_ptr) };
    // SAFETY: the raw contract establishes zero used blocks and detached queue
    // state for this claimed abandoned page.
    let all_free = unsafe { crate::core_types::assume_all_free_abandoned(abandoned) };
    let current_theap = if current_theapx.is_null() {
        None
    } else {
        // SAFETY: the raw contract identifies the matching live current-thread
        // stats owner and excludes its teardown through release.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(current_theapx),
            )
        })
    };
    // SAFETY: the typed all-free capability and optional stats owner carry the
    // complete unregister, accounting, bitmap, and backing-release epoch.
    let destroyed = unsafe { _mi_arenas_page_free_internal(all_free, current_theap) };
    let _ = destroyed.page();
}

/// Consume one all-free page through page-map removal, accounting, arena-bit
/// release, and backing release.
///
/// # Safety
/// `page_wrapper` must retain the claimed thread-free owner bit and represent a
/// live registered detached abandoned page with stable MemId, geometry, page-map
/// registration, heap arena bit, and backing. `current_theap`, when present,
/// must be the matching live current-thread stats owner; otherwise the owning
/// heap's atomic stats must remain live. For arena pages, the decoded range must
/// remain allocated and clear in `slices_free`, and its `(slice start, full
/// size, MemId)` must identify the same live, not-yet-freed backing allocation.
/// The arena table, free/committed/page bitmaps, subprocess statistics, tracking
/// state, and stored arena, purge, release, OS, diagnostic callbacks and opaque
/// arguments must remain live and valid for synchronous use, must not
/// incompatibly reenter this page, heap, or arena, and must not unwind. Relink,
/// competing claim, non-protocol metadata mutation, and teardown are excluded
/// through the terminal release; protocol remote-head publication remains
/// permitted. This call consumes the page and backing exactly once. The returned
/// `DestroyedPage` retains only terminal identity and must not be used to access
/// the released page.
pub(crate) unsafe fn _mi_arenas_page_free_internal<S>(
    page_wrapper: crate::core_types::AllFreePage<S>,
    current_theap: Option<crate::core_types::ThreadHeapLocal>,
) -> crate::core_types::DestroyedPage {
    let page_ptr = page_wrapper.page();
    let page = page_ptr.as_ptr();
    let current_theapx = current_theap
        .map(|theap| theap.ptr().as_ptr())
        .unwrap_or(ptr::null_mut());

    // SAFETY: the claimed page keeps its initialized slice geometry live for
    // this assertion-only alignment check.
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
        unsafe { mi_page_slice_start(page) }.cast(),
        MI_PAGE_ALIGN
    ));
    // SAFETY: the same stable geometry permits the original page-start lookup
    // and page-map round trip, evaluated only with internal assertions enabled.
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::_mi_ptr_page(mi_page_start(page).cast()) == page
    });
    // SAFETY: the retained thread-free claim keeps the initialized ownership
    // word stable through this assertion-only predicate.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_owned(page) });
    // SAFETY: the all-free capability and excluded non-protocol mutation keep
    // the initialized used count stable through this assertion-only predicate.
    crate::internal::mi_assert_internal!(unsafe { mi_page_all_free(page) });
    // SAFETY: the detached all-free page remains in its caller-proven abandoned
    // state through this assertion-only predicate.
    crate::internal::mi_assert_internal!(unsafe { mi_page_is_abandoned(page) });
    // SAFETY: the caller excludes relinking; preserve the original next read
    // followed by the short-circuited previous read inside the assertion.
    crate::internal::mi_assert_internal!(unsafe {
        page_ptr.queue_next().is_none() && page_ptr.queue_prev().is_none()
    });
    // SAFETY: the page's initialized MemId remains stable through this arena
    // classification before any publication is removed.
    let arena_membership = if unsafe { mi_page_memid_is_arena(page) } {
        // SAFETY: the caller-proven arena page retains its live heap, MemId,
        // arena table, and published start bit through the operation snapshot.
        let membership = unsafe { PageArenaMembership::from_page(page_ptr).unwrap_unchecked() };
        Some(membership)
    } else {
        None
    };

    // SAFETY: the claimed initialized page keeps its block-size classification
    // stable while selecting the accounting bin before publication removal.
    let stats_bin = unsafe { _mi_page_stats_bin(page) };
    if !current_theapx.is_null() {
        // SAFETY: the supplied current-thread stats owner remains live and
        // exclusively mutable; preserve bin-before-pages decrement order.
        unsafe {
            crate::internal::__mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.page_bins[stats_bin]),
                1,
            );
            crate::internal::__mi_stat_decrease(
                ptr::addr_of_mut!((*current_theapx).stats.pages),
                1,
            );
        }
    } else {
        // SAFETY: the page's initialized heap association remains live and
        // stable while selecting the atomic statistics owner.
        let heap = unsafe { mi_page_heap(page) };
        // SAFETY: the owning heap's atomic statistics remain live; preserve
        // bin-before-pages decrement order under the MT protocol.
        unsafe {
            crate::internal::__mi_stat_decrease_mt(
                ptr::addr_of_mut!((*heap).stats.page_bins[stats_bin]),
                1,
            );
            crate::internal::__mi_stat_decrease_mt(ptr::addr_of_mut!((*heap).stats.pages), 1);
        }
    }

    // SAFETY: the page's initialized full flag remains stable; short circuiting
    // preserves the original debug-gate-before-full-read order.
    if MI_DEBUG > 1
        && !unsafe { mi_page_is_full_local(page) }
        && let Some(membership) = arena_membership.as_ref()
    {
        // SAFETY: the live page's initialized block size remains stable while
        // selecting the debug abandoned-bitmap bin.
        let bin = mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) });
        let arena = membership.arena().as_ptr();
        let range = membership.range();
        let slice_index = range.start_usize();
        let slice_count = range.count_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: this operation-scoped membership snapshot keeps the arena,
        // page table, and checked bitmap range live for the ordered debug-only
        // verification; no publication has been removed yet.
        unsafe {
            crate::internal::mi_assert_internal!(mi_bbitmap_is_clearN(
                (*arena.cast::<MiArenaPrefix>()).slices_free,
                slice_index,
                slice_count,
            ));
            let committed = crate::allocator::page::mi_page_slice_committed(page);
            crate::internal::mi_assert_internal!(
                !committed.is_zero()
                    || mi_bitmap_is_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        slice_count,
                    )
            );
            crate::internal::mi_assert_internal!(!mi_bitmap_is_setN(
                (*arena_pages).pages_abandoned[bin.as_usize()],
                slice_index,
                1,
            ));
            crate::internal::mi_assert_internal!(mi_bitmap_is_setN(
                (*arena_pages).pages,
                slice_index,
                1
            ));
        }
    }

    // SAFETY: the page is still registered and the caller owns this terminal
    // transition; unregister it before clearing arena publication or backing.
    unsafe { _mi_page_map_unregister(page) };
    if let Some(membership) = arena_membership {
        let arena = membership.arena().as_ptr();
        let range = membership.range();
        let slice_index = range.start_usize();
        let slice_count = range.count_usize();
        let arena_pages = membership.arena_pages().as_ptr();
        // SAFETY: the validated membership identifies this live heap table and
        // published start bit; this is its single terminal clear.
        unsafe { mi_bitmap_clear((*arena_pages).pages, slice_index) };
        // SAFETY: the page metadata remains live after unregister and before
        // backing release; preserve the committed-size read after page-bit clear.
        let committed = unsafe { crate::allocator::page::mi_page_slice_committed(page) };
        if !committed.is_zero() {
            let total_slices = committed.get() / MI_ARENA_SLICE_SIZE;
            crate::internal::mi_assert_internal!(slice_count >= total_slices);
            if total_slices > 0 {
                // SAFETY: the decoded arena range bounds `total_slices`; publish
                // committed coverage only after clearing the heap page bit.
                unsafe {
                    mi_bitmap_setN(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index,
                        total_slices,
                        ptr::null_mut(),
                    );
                }
            }
            let extra = committed.get() % MI_ARENA_SLICE_SIZE;
            if extra > 0 {
                // SAFETY: the live arena names its live subprocess statistics;
                // atomically remove the partial committed tail after bitmap publication.
                unsafe {
                    crate::internal::__mi_stat_decrease_mt(
                        ptr::addr_of_mut!(
                            (*(*arena.cast::<MiArenaPrefix>()).subproc).stats.committed
                        ),
                        crate::core_types::CommittedBytes::new(extra).get(),
                    );
                }
            }
        } else {
            // SAFETY: the stable membership range covers initialized committed
            // bitmap storage for this assertion-only zero-commit check.
            crate::internal::mi_assert_internal!(unsafe {
                mi_bitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_committed,
                    slice_index,
                    slice_count,
                )
            });
        }
    }
    // SAFETY: page metadata remains live until backing release; preserve the
    // separated-metadata predicate before the conditional block-size clear.
    if unsafe { mi_page_meta_is_separated(page) } {
        // SAFETY: this terminal path exclusively owns the separate page metadata
        // and clears its block size immediately before releasing the backing.
        unsafe { (*page).block_size = 0 };
    }
    // SAFETY: all registration and arena publication has been removed in the
    // required order. Materialize slice start, full size, then MemId in the
    // original call expression, consume the backing once, and terminalize the
    // all-free capability without any subsequent page access.
    unsafe {
        _mi_arenas_free(
            mi_page_slice_start(page).cast(),
            mi_page_full_size(PagePtr::new_unchecked(page)).get(),
            (*page).memid,
        );
        page_wrapper.into_destroyed_after_arena_free()
    }
}

/// # Safety
/// `page` must be an all-free page removed from its page queue, or a registered
/// failed-init page with no used blocks and no queue links after its owner was
/// returned to the abandoned state. It must retain claimed thread-free ownership
/// and stable MemId, geometry, page-map, and heap arena-bit state. `owner` must
/// be the live thread heap responsible for the page's allocation statistics;
/// concurrent relink, claim, mutation, and teardown are excluded through release.
pub(crate) unsafe fn _mi_arenas_page_free_typed<S>(
    page: crate::core_types::AllFreePage<S>,
    owner: crate::core_types::ThreadHeapLocal,
) -> crate::core_types::DestroyedPage {
    // SAFETY: the consumed all-free witness proves the detached page state, and
    // `owner` is the live local heap responsible for its release accounting.
    // Both remain valid while the internal path unregisters and frees backing.
    unsafe { _mi_arenas_page_free_internal(page, Some(owner)) }
}

/// # Safety
/// `page` must be non-null live page metadata already detached from its queue,
/// marked abandoned, and retaining the claimed thread-free owner bit.
/// `current_theap` must be its live current-thread local heap association and
/// stats owner. Page geometry, lists, MemId, heap, and arena membership must
/// remain stable; concurrent claim, relink, or teardown is excluded, while
/// protocol remote-free publication remains allowed.
pub(crate) unsafe fn _mi_arenas_page_abandon(
    page: *mut mi_page_t,
    current_theap: *mut mi_theap_t,
) -> crate::core_types::PageAbandonOutcome {
    // SAFETY: the caller has detached this live page, cleared its thread heap,
    // marked it abandoned, and retains the claimed thread-free owner bit.
    let page_ptr = unsafe { PagePtr::new_unchecked(page) };
    // SAFETY: the raw abandon boundary above establishes live abandoned page
    // metadata with the remote-free ownership claim still held.
    let page = unsafe { crate::core_types::assume_owned_abandoned_page(page_ptr) };
    // SAFETY: the caller supplies the non-null live current-thread local heap
    // matching this page's retained association and abandoned stats owner.
    let current_theap = unsafe {
        crate::core_types::ThreadHeapLocal::new_unchecked(
            crate::core_types::ThreadHeapPtr::new_unchecked(current_theap),
        )
    };
    // SAFETY: the typed witnesses retain active claimed ownership and the matching local heap; the
    // raw contract excludes competing mutation while publication and claim release run synchronously.
    unsafe { _mi_arenas_page_abandon_internal(page, current_theap) }
}

/// Re-publish a claimed abandoned arena page through mapped-abandoned state.
///
/// # Safety
/// `page` must be a live initialized claimed abandoned page that is neither full,
/// all-free, singleton, nor already mapped-abandoned. `arena_memid` must be the
/// arena MemId decoded from that same page. Its associated thread heap, stats,
/// arena membership, backing, and page metadata must remain live and stable for
/// the transition. Concurrent claim, relink, reassignment, or teardown is
/// excluded; protocol remote-free publication remains allowed.
#[inline(never)]
pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(
    page: crate::core_types::OwnedAbandonedPage,
    arena_memid: crate::core_types::ArenaMemId,
) -> Result<crate::core_types::AbandonedDisposition, crate::core_types::OwnedAbandonedPage> {
    // SAFETY: the contract keeps the claimed page, decoded arena identity,
    // associated heap, stats, page map, metadata, and backing stable for this
    // synchronous transaction. The block consumes the ownership witness once
    // and returns the exact resulting disposition.
    unsafe {
        // Validate eligibility and the caller's decoded arena identity.
        let page_ptr = page.page();
        let page_raw = page_ptr.as_ptr();
        crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
            mi_page_slice_start(page_raw).cast(),
            MI_PAGE_ALIGN
        ));
        crate::internal::mi_assert_internal!(
            crate::internal::_mi_ptr_page(mi_page_start(page_raw).cast()) == page_raw
        );
        crate::internal::mi_assert_internal!(mi_page_is_owned(page_raw));
        crate::internal::mi_assert_internal!(mi_page_is_abandoned(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_is_abandoned_mapped(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_is_full_local(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_all_free(page_raw));
        crate::internal::mi_assert_internal!(!mi_page_is_singleton(page_raw));

        let memid_matches = match crate::core_types::MemId::view((*page_raw).memid) {
            crate::core_types::MemIdView::Arena(current) => {
                current.arena().as_ptr() == arena_memid.arena().as_ptr()
                    && current.range().start_usize() == arena_memid.range().start_usize()
                    && current.range().count_usize() == arena_memid.range().count_usize()
            }
            _ => false,
        };
        if !memid_matches
            || mi_page_is_full_local(page_raw)
            || mi_page_is_abandoned_mapped(page_raw)
        {
            return Err(page);
        }

        // Reconcile source-heap stats before republishing the page.
        let Some(theap) = _mi_page_associated_theap_peek(page_ptr) else {
            return Err(page);
        };
        let theap = theap.as_ptr();

        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_reabandon_full), 1);
        __mi_stat_adjust_decrease(ptr::addr_of_mut!((*theap).stats.pages_abandoned), 1);
        // Consume claimed ownership through publication or terminal release.
        let outcome = _mi_arenas_page_abandon_internal(
            page,
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            ),
        );
        match outcome {
            crate::core_types::PageAbandonOutcome::Abandoned => {
                Ok(crate::core_types::AbandonedDisposition::Mapped)
            }
            crate::core_types::PageAbandonOutcome::Destroyed => {
                Ok(crate::core_types::AbandonedDisposition::Destroyed(
                    crate::core_types::assume_destroyed_after_arena_free(page_ptr),
                ))
            }
        }
    }
}

#[cfg(test)]
mod claim_tests {
    use super::*;
    use core::mem::MaybeUninit;
    use std::alloc::{Layout, alloc_zeroed, dealloc};

    #[test]
    fn arena_start_index_stays_inside_the_typed_cycle_domain() {
        let index = |value| ArenaIndex::from_usize(value).expect("test cycle is arena-bounded");
        assert_eq!(
            mi_arena_start_idx(
                crate::core_types::HeapSeq::new(7),
                4,
                crate::core_types::ThreadSeq::new(11),
                index(0),
            )
            .get(),
            0
        );
        assert_eq!(
            mi_arena_start_idx(
                crate::core_types::HeapSeq::new(7),
                4,
                crate::core_types::ThreadSeq::new(11),
                index(1),
            )
            .get(),
            0
        );
        assert_eq!(
            mi_arena_start_idx(
                crate::core_types::HeapSeq::new(0),
                4,
                crate::core_types::ThreadSeq::new(3),
                index(2),
            )
            .get(),
            1
        );
        assert_eq!(
            mi_arena_start_idx(
                crate::core_types::HeapSeq::new(7),
                1,
                crate::core_types::ThreadSeq::new(4),
                index(3),
            )
            .get(),
            1
        );
        assert_eq!(
            mi_arena_start_idx(
                crate::core_types::HeapSeq::new(7),
                0,
                crate::core_types::ThreadSeq::new(7),
                index(5),
            )
            .get(),
            2
        );
        assert_eq!(
            mi_arena_start_idx(
                crate::core_types::HeapSeq::new(321),
                50_000,
                crate::core_types::ThreadSeq::new(9),
                index(crate::types::MI_MAX_ARENAS - 1),
            )
            .get() as usize,
            321 % (crate::types::MI_MAX_ARENAS - 1)
        );

        for cycle in 2..crate::types::MI_MAX_ARENAS {
            let start = mi_arena_start_idx(
                crate::core_types::HeapSeq::new(cycle * 3),
                cycle / 2 + 2,
                crate::core_types::ThreadSeq::new(cycle * 5),
                index(cycle),
            );
            assert!((start.get() as usize) < cycle);
        }
    }

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
            // SAFETY: the fixture still owns its live bitmap allocation and
            // restores the same in-range bits before deallocation.
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
