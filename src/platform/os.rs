//! Primitive OS / platform layer

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use crate::allocator::arena::MI_ARENA_SLICE_SIZE;
use crate::allocator::options::{
    _mi_verbose_message, _mi_warning_message_fmt, mi_option_get, mi_option_get_size,
    mi_option_is_enabled, mi_option_t,
};
use crate::allocator::stats::{_mi_clock_end, _mi_clock_start, mi_stats_t};
use crate::atomic::mi_lock_t;
use crate::bits::{MI_GiB, MI_INTPTR_SIZE, MI_KiB, MI_MAX_VABITS, MI_MiB};
use crate::core_types::{
    BorrowedSubproc, CommittedBytes, InitiallyZero, MemId, MemIdView, OsMem, OsMemId, OsMemKind,
    OsMemPtr, OsPageSize, Pinned, ReservedBytes, SubprocPtr, ptr_exposed_addr,
};
use crate::internal::memid::{_mi_memid_none, mi_memid_t};
use crate::internal::{
    __mi_stat_counter_increase_mt, __mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_align_up,
    _mi_align_up_ptr, _mi_is_power_of_two, _mi_memzero_aligned, _mi_preloading, _mi_subproc,
    MI_DEBUG, MI_SECURE, MI_TRACK_ENABLED, mi_theap_is_initialized,
};
use crate::prim::mi_os_mem_config_t;
use crate::prim::{
    _mi_prim_alloc, _mi_prim_alloc_huge_os_pages, _mi_prim_commit, _mi_prim_decommit,
    _mi_prim_free, _mi_prim_mem_init, _mi_prim_numa_node, _mi_prim_numa_node_count,
    _mi_prim_protect, _mi_prim_reset, _mi_prim_reuse, _mi_theap_default,
};
use crate::types::{mi_arena_t, mi_commit_fun_t, mi_heap_t, mi_page_t, mi_subproc_t};

pub const MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_4_GIB: usize = 4 * MI_MiB;
pub const MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_32_GIB: usize = 32 * MI_MiB;
pub const MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB: usize = if MI_INTPTR_SIZE < 8 {
    MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_4_GIB
} else {
    MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB_32_GIB
};

pub const MI_HINT_ALIGN: usize = 4usize << 20;
pub const MI_HINT_BASE: usize = 2usize << 40;
pub const MI_HINT_AREA: usize = 4usize << 40;
pub const MI_HINT_MAX: usize = 30usize << 40;
pub const MI_HUGE_OS_PAGE_SIZE: usize = MI_GiB;

const MI_MAX_ARENAS: usize = 160;

static ALIGNED_BASE: AtomicUsize = AtomicUsize::new(0);
static MI_HUGE_START: AtomicUsize = AtomicUsize::new(0);
static MI_NUMA_NODE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// A successful primitive OS allocation result.
///
/// This records the returned base and flags only; it does not own the mapping
/// or provide automatic release.
struct PrimitiveOsAllocation {
    base: OsMemPtr,
    initially_zero: InitiallyZero,
    pinned: Pinned,
}

/// A successful plain OS allocation that is already committed.
///
/// This records mapping status only; it does not own the mapping or release it
/// automatically.
struct CommittedOsAllocation {
    base: OsMemPtr,
    size: ReservedBytes,
    initially_zero: InitiallyZero,
    pinned: Pinned,
}

/// Result of an OS commit attempt, including whether the committed bytes were
/// reported as initially zero. This is a status value, not mapping ownership.
pub(crate) struct OsCommitResult {
    success: bool,
    initially_zero: InitiallyZero,
}

impl OsCommitResult {
    #[inline]
    pub(crate) fn success(&self) -> bool {
        self.success
    }

    #[inline]
    pub(crate) fn initially_zero(self) -> InitiallyZero {
        self.initially_zero
    }
}

/// Result of an effective primitive decommit attempt.
///
/// `None` from the helper means page containment produced no effective range;
/// callers retain their distinct legacy no-op defaults in that case.
struct OsDecommitResult {
    success: bool,
    needs_recommit: bool,
}

static mut mi_os_mem_config: mi_os_mem_config_t = mi_os_mem_config_t {
    page_size: 4096,
    large_page_size: 0,
    alloc_granularity: 4096,
    physical_memory_in_kib: MI_DEFAULT_PHYSICAL_MEMORY_IN_KIB,
    virtual_address_bits: MI_MAX_VABITS,
    has_overcommit: true,
    has_partial_free: false,
    has_virtual_reserve: true,
    has_transparent_huge_pages: false,
};

/// A checked power-of-two OS allocation alignment normalized to whole pages.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OsAlignment(usize);

impl OsAlignment {
    #[inline]
    fn normalized(requested: usize, page_size: OsPageSize) -> Option<Self> {
        let page_size = page_size.get();
        let remainder = requested % page_size;
        let normalized = if remainder == 0 {
            requested
        } else {
            requested.checked_add(page_size - remainder)?
        };
        if normalized < page_size || !normalized.is_power_of_two() {
            return None;
        }
        Some(Self(normalized))
    }

    #[inline]
    fn from_page_multiple(requested: usize, page_size: OsPageSize) -> Option<Self> {
        if !requested.is_multiple_of(page_size.get()) {
            return None;
        }
        Self::normalized(requested, page_size)
    }

    #[inline]
    fn get(self) -> usize {
        self.0
    }

    #[inline]
    fn align_up(self, value: usize) -> Option<usize> {
        let mask = self.0 - 1;
        value.checked_add(mask).map(|sum| sum & !mask)
    }

    #[inline]
    fn overreserve(self, size: usize) -> Option<usize> {
        let total = size.checked_add(self.0)?;
        (total < usize::MAX).then_some(total)
    }
}

/// A nonempty, page-aligned exposed-address interval.
///
/// This is geometry only: it does not prove backing allocation coverage, provenance,
/// ownership, liveness, or that the interval may be accessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OsPageRange {
    start: NonNull<c_void>,
    size: ReservedBytes,
}

impl OsPageRange {
    #[inline]
    fn covering(addr: *mut c_void, size: ReservedBytes, page_size: OsPageSize) -> Option<Self> {
        if size.get() == 0 {
            return None;
        }
        let start = NonNull::new(addr)?;
        let address = ptr_exposed_addr(addr);
        let end = address.checked_add(size.get())?;
        let alignment = page_size.get();
        let aligned_start = address - (address % alignment);
        let remainder = end % alignment;
        let aligned_end = if remainder == 0 {
            end
        } else {
            end.checked_add(alignment - remainder)?
        };
        Self::from_bounds(start, aligned_start, aligned_end)
    }

    #[inline]
    fn contained(addr: *mut c_void, size: ReservedBytes, page_size: OsPageSize) -> Option<Self> {
        if size.get() == 0 {
            return None;
        }
        let start = NonNull::new(addr)?;
        let address = ptr_exposed_addr(addr);
        let end = address.checked_add(size.get())?;
        let alignment = page_size.get();
        let remainder = address % alignment;
        let aligned_start = if remainder == 0 {
            address
        } else {
            address.checked_add(alignment - remainder)?
        };
        let aligned_end = end - (end % alignment);
        Self::from_bounds(start, aligned_start, aligned_end)
    }

    #[inline]
    fn from_bounds(
        start: NonNull<c_void>,
        aligned_start: usize,
        aligned_end: usize,
    ) -> Option<Self> {
        let size = aligned_end.checked_sub(aligned_start)?;
        if size == 0 {
            return None;
        }
        let aligned = start.as_ptr().map_addr(|_| aligned_start);
        Some(Self {
            start: NonNull::new(aligned)?,
            size: ReservedBytes::new(size),
        })
    }

    #[inline]
    fn start(self) -> *mut c_void {
        self.start.as_ptr()
    }

    #[inline]
    fn size(self) -> ReservedBytes {
        self.size
    }
}

/// Return the address of process-global OS configuration storage.
///
/// This does not read the mutable static or create a reference; callers must
/// separately justify every field access through the returned raw pointer.
#[inline]
fn mi_os_mem_config_ptr() -> *const mi_os_mem_config_t {
    core::ptr::addr_of!(mi_os_mem_config)
}

#[repr(C)]
struct MiSubprocStatsPrefix {
    subproc_seq: usize,
    next: *mut mi_subproc_t,
    prev: *mut mi_subproc_t,
    arena_count: AtomicUsize,
    arenas: [*mut c_void; MI_MAX_ARENAS],
    arena_reserve_lock: mi_lock_t,
    purge_expire: AtomicI64,
    heap_main: *mut mi_heap_t,
    heaps: *mut mi_heap_t,
    heaps_lock: mi_lock_t,
    thread_count: AtomicUsize,
    thread_total_count: AtomicUsize,
    heap_count: AtomicUsize,
    heap_total_count: AtomicUsize,
    memid: mi_memid_t,
    stats: mi_stats_t,
}

#[repr(C)]
struct MiArenaCommitPrefix {
    memid: mi_memid_t,
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
    slices_free: *mut c_void,
    slices_committed: *mut c_void,
    slices_dirty: *mut c_void,
    slices_purge: *mut c_void,
    pages_meta: *mut mi_page_t,
}

/// # Safety
/// The process OS configuration must not be concurrently rewritten by `_mi_os_init`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_has_overcommit() -> bool {
    // SAFETY: the caller excludes configuration mutation while this initialized
    // boolean field is read through the raw static address.
    unsafe { (*mi_os_mem_config_ptr()).has_overcommit }
}

/// # Safety
/// The process OS configuration must not be concurrently rewritten by `_mi_os_init`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_has_virtual_reserve() -> bool {
    // SAFETY: the caller excludes configuration mutation while this initialized
    // boolean field is read through the raw static address.
    unsafe { (*mi_os_mem_config_ptr()).has_virtual_reserve }
}

/// # Safety
/// The process OS configuration must not be concurrently rewritten by `_mi_os_init`.
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_page_size() -> usize {
    // SAFETY: the caller excludes configuration mutation while the initialized
    // page-size field is read through the raw static address.
    unsafe { (*mi_os_mem_config_ptr()).page_size }
}

/// # Safety
/// The OS memory configuration must contain a nonzero page size and must not be
/// concurrently rewritten by `_mi_os_init`.
#[inline]
pub(crate) unsafe fn _mi_os_page_size_typed() -> crate::core_types::OsPageSize {
    // SAFETY: this function's contract keeps initialized configuration stable.
    let page_size = unsafe { _mi_os_page_size() };
    crate::internal::mi_assert_internal!(page_size > 0);
    // SAFETY: primitive initialization and the assertion establish nonzero size.
    unsafe { crate::core_types::OsPageSize::new_unchecked(page_size) }
}

/// # Safety
/// The OS memory configuration must be initialized and must not be concurrently
/// rewritten by `_mi_os_init`.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_large_page_size() -> usize {
    // SAFETY: the caller keeps initialized OS configuration stable for this read.
    let large = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    if large != 0 {
        large
    } else {
        // SAFETY: the caller keeps initialized OS configuration stable.
        unsafe { _mi_os_page_size_typed().get() }
    }
}

/// # Safety
/// The OS memory configuration and options state must be initialized; OS
/// configuration must not be concurrently rewritten by `_mi_os_init`.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_minimal_purge_size() -> usize {
    let minsize = mi_option_get_size(mi_option_t::mi_option_minimal_purge_size);
    if minsize != 0 {
        // SAFETY: the caller keeps initialized OS configuration stable.
        _mi_align_up(minsize, unsafe { _mi_os_page_size_typed().get() })
    } else {
        // SAFETY: the caller keeps initialized OS configuration stable while
        // the transparent-huge-page policy field is read.
        let has_transparent_huge_pages =
            unsafe { (*mi_os_mem_config_ptr()).has_transparent_huge_pages };
        if has_transparent_huge_pages && mi_option_get(mi_option_t::mi_option_allow_thp) == 2 {
            // SAFETY: the caller keeps initialized OS configuration stable.
            unsafe { _mi_os_large_page_size() }
        } else {
            // SAFETY: the caller keeps initialized OS configuration stable.
            unsafe { _mi_os_page_size_typed().get() }
        }
    }
}

/// # Safety
/// The OS memory configuration must be initialized and must not be concurrently
/// rewritten by `_mi_os_init`.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_guard_page_size() -> usize {
    // SAFETY: the caller keeps initialized OS configuration stable.
    let gsize = unsafe { _mi_os_page_size_typed().get() };
    crate::internal::mi_debug_assert!(gsize <= (MI_ARENA_SLICE_SIZE / 4));
    gsize
}

/// # Safety
/// The OS memory configuration must be initialized and must not be concurrently
/// rewritten by `_mi_os_init`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_virtual_address_bits() -> usize {
    // SAFETY: the caller excludes configuration mutation while this initialized
    // scalar field is read through the raw static address.
    let vbits = unsafe { (*mi_os_mem_config_ptr()).virtual_address_bits };
    crate::internal::mi_debug_assert!(vbits <= MI_MAX_VABITS);
    vbits
}

/// # Safety
/// The OS memory configuration must be initialized and must not be concurrently
/// rewritten by `_mi_os_init`.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_canuse_large_page(size: usize, alignment: usize) -> bool {
    // SAFETY: the caller keeps initialized OS configuration stable for this read.
    let large_page_size = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    large_page_size != 0
        && size.is_multiple_of(large_page_size)
        && alignment.is_multiple_of(large_page_size)
}

/// # Safety
/// The OS memory configuration must contain a nonzero page size and must not be
/// concurrently rewritten by `_mi_os_init`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_good_alloc_size(size: usize) -> usize {
    let align_size = if size < 512 * MI_KiB {
        // SAFETY: the caller keeps initialized OS configuration stable.
        unsafe { _mi_os_page_size_typed().get() }
    } else if size < 2 * MI_MiB {
        64 * MI_KiB
    } else if size < 8 * MI_MiB {
        256 * MI_KiB
    } else if size < 32 * MI_MiB {
        MI_MiB
    } else {
        4 * MI_MiB
    };
    if size >= usize::MAX - align_size {
        size
    } else {
        _mi_align_up(size, align_size)
    }
}

/// # Safety
/// Must be called during process initialization while the global OS
/// configuration and primitive dynamic-dispatch statics are uniquely writable.
/// Initialization must complete and those statics must be frozen before any
/// concurrent OS allocation or primitive reader becomes reachable.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_init() {
    // SAFETY: the caller provides the unique epoch in which the primitive may
    // initialize both its dynamic-dispatch statics (including Windows slots)
    // and this writable configuration output; all are frozen before readers.
    unsafe { _mi_prim_mem_init(core::ptr::addr_of_mut!(mi_os_mem_config)) };
}

/// # Safety
/// OS configuration must be initialized and stable. In secure or release
/// builds, the current thread's default heap must remain live and initialized
/// while its random state is advanced.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_get_aligned_hint(try_alignment: usize, size: usize) -> *mut c_void {
    // SAFETY: the caller keeps initialized configuration stable while this
    // scalar field is read.
    if MI_INTPTR_SIZE < 8 || try_alignment <= unsafe { (*mi_os_mem_config_ptr()).alloc_granularity }
    {
        return core::ptr::null_mut();
    }
    // SAFETY: the caller keeps the initialized virtual-address width stable.
    let virtual_address_bits = unsafe { (*mi_os_mem_config_ptr()).virtual_address_bits };
    if try_alignment > MI_HINT_ALIGN || virtual_address_bits < 46 {
        return core::ptr::null_mut();
    }
    let size = _mi_align_up(size, MI_HINT_ALIGN);
    if size > 16 * MI_GiB {
        return core::ptr::null_mut();
    }
    let size = size.saturating_add(MI_HINT_ALIGN);

    let mut hint = ALIGNED_BASE.fetch_add(size, Ordering::AcqRel);
    if hint == 0 || hint > MI_HINT_MAX {
        let mut init = MI_HINT_BASE;
        if cfg!(feature = "secure") || cfg!(not(debug_assertions)) {
            // SAFETY: the caller keeps the current thread's default heap live
            // for this non-initializing lookup.
            let theap = unsafe { _mi_theap_default() };
            // SAFETY: the lookup returned the current default-heap slot, whose
            // initialized-state field remains readable during this operation.
            if unsafe { !mi_theap_is_initialized(theap) } {
                return core::ptr::null_mut();
            }
            // SAFETY: the initialized default heap is current-thread owned and
            // remains live while its random state is advanced.
            let r = unsafe { crate::allocator::thread_heap::_mi_theap_random_next(theap) };
            init += (MI_HINT_ALIGN * ((r >> 17) & 0xFFFFF)) % MI_HINT_AREA;
        }
        let _ = ALIGNED_BASE.compare_exchange(
            hint.saturating_add(size),
            init,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        hint = ALIGNED_BASE.fetch_add(size, Ordering::AcqRel);
    }
    if !hint.is_multiple_of(MI_HINT_ALIGN) || !hint.is_multiple_of(try_alignment) {
        return core::ptr::null_mut();
    }
    hint as *mut c_void
}

/// # Safety
/// OS page configuration must be initialized and stable for this query.
#[inline(never)]
#[allow(clippy::absurd_extreme_comparisons)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_size() -> usize {
    if MI_SECURE > 0 {
        // SAFETY: the caller keeps initialized page configuration stable for
        // this guard-size query.
        unsafe { _mi_os_guard_page_size() }
    } else {
        0
    }
}

/// # Safety
/// `addr`, when non-null, must designate the live guard page associated with
/// `memid`. For arena memory, the arena prefix and stored callback/argument must
/// remain live and stable, tolerate concurrent arena commit operations, and be
/// valid for a synchronous decommit request. The managed-arena lifecycle must
/// keep the range and argument valid for any later use retained by the callback,
/// and the callback must not unwind.
#[inline(never)]
#[allow(clippy::absurd_extreme_comparisons)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_set_at(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
    if addr.is_null() {
        return true;
    }
    if MI_SECURE > 0 && !memid.is_pinned {
        let arena = match crate::core_types::MemId::view(memid) {
            crate::core_types::MemIdView::Arena(a) => a.arena().as_ptr(),
            _ => core::ptr::null_mut(),
        };
        if !arena.is_null() {
            // SAFETY: the decoded arena MemId and caller contract keep the live
            // prefix and its callback field readable for this operation.
            let commit_fun = unsafe { (*arena.cast::<MiArenaCommitPrefix>()).commit_fun };
            if let Some(commit_fun) = commit_fun {
                // SAFETY: the caller keeps the guard range and stored opaque
                // argument valid for this synchronous decommit callback and
                // for any later retained use under the managed-arena lifecycle;
                // the callback does not unwind.
                return unsafe {
                    commit_fun(
                        false,
                        addr,
                        _mi_os_secure_guard_page_size(),
                        core::ptr::null_mut(),
                        (*arena.cast::<MiArenaCommitPrefix>()).commit_fun_arg,
                    )
                };
            }
        }
        return false;
    }
    true
}

/// # Safety
/// `addr` must be at least one secure guard page past the live guard range, and
/// `memid` must describe that allocation. OS page configuration must remain
/// initialized and stable. For arena memory, the arena prefix and stored
/// callback/argument must remain live and stable, tolerate concurrent arena
/// commits, and be valid for the synchronous decommit request. The managed-arena
/// lifecycle must cover any later retained use, and the callback must not unwind.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_set_before(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
    // SAFETY: the caller keeps the preceding guard range and OS page size valid;
    // the downstream decommit callback, arena prefix, and opaque argument meet
    // the propagated lifecycle, concurrency, retention, and unwind contract.
    unsafe {
        _mi_os_secure_guard_page_set_at(
            addr.cast::<u8>()
                .sub(_mi_os_secure_guard_page_size())
                .cast::<c_void>(),
            memid,
        )
    }
}

/// # Safety
/// `addr`, when non-null, must designate the live guard page associated with
/// `memid`. For arena memory, the arena prefix and stored callback/argument must
/// remain live and stable, tolerate concurrent arena commit operations, and be
/// valid for a synchronous commit request. The managed-arena lifecycle must keep
/// the range and argument valid for any later use retained by the callback, and
/// the callback must not unwind. Without a callback, the page-covering range
/// must be suitable for the active primitive commit operation.
#[inline(never)]
#[allow(clippy::absurd_extreme_comparisons)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_reset_at(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
    if addr.is_null() {
        return true;
    }
    if MI_SECURE > 0 && !memid.is_pinned {
        let arena = match crate::core_types::MemId::view(memid) {
            crate::core_types::MemIdView::Arena(a) => a.arena().as_ptr(),
            _ => core::ptr::null_mut(),
        };
        if !arena.is_null() {
            // SAFETY: the decoded arena MemId and caller contract keep the live
            // prefix and its callback field readable for this operation.
            let commit_fun = unsafe { (*arena.cast::<MiArenaCommitPrefix>()).commit_fun };
            if let Some(commit_fun) = commit_fun {
                // SAFETY: the caller keeps the guard range and stored opaque
                // argument valid for this synchronous commit callback and for
                // any later retained use under the managed-arena lifecycle;
                // the callback does not unwind.
                return unsafe {
                    commit_fun(
                        true,
                        addr,
                        _mi_os_secure_guard_page_size(),
                        core::ptr::null_mut(),
                        (*arena.cast::<MiArenaCommitPrefix>()).commit_fun_arg,
                    )
                };
            }
        }
        // SAFETY: without a managed callback, the caller keeps this live guard
        // range suitable for primitive commitment and accounting.
        return unsafe { _mi_os_commit(addr, _mi_os_secure_guard_page_size()) }.success();
    }
    true
}

/// # Safety
/// `addr` must be at least one secure guard page past the live guard range, and
/// `memid` must describe that allocation. OS page configuration must remain
/// initialized and stable. For arena memory, the arena prefix and stored
/// callback/argument must remain live and stable, tolerate concurrent arena
/// commits, and be valid for the synchronous commit request. The managed-arena
/// lifecycle must cover any later retained use, and the callback must not unwind.
/// Without a callback, the page-covering guard range must remain suitable for
/// primitive commitment and accounting.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_reset_before(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
    // SAFETY: the caller keeps the preceding guard range and OS page size valid;
    // the downstream callback or primitive commit path meets the propagated
    // arena-lifecycle, concurrency, accounting, retention, and unwind contract.
    unsafe {
        _mi_os_secure_guard_page_reset_at(
            addr.cast::<u8>()
                .sub(_mi_os_secure_guard_page_size())
                .cast::<c_void>(),
            memid,
        )
    }
}

/// # Safety
/// A nonempty `addr..addr+size` must remain a live caller-controlled range
/// suitable for the active primitive commit operation. OS configuration and
/// current subprocess statistics must remain live and initialized, and
/// `stat_size` must match the committed bytes charged on success. Null or empty
/// input is accepted as a successful no-op.
pub(crate) unsafe fn _mi_os_commit_ex(
    addr: *mut c_void,
    size: usize,
    stat_size: CommittedBytes,
) -> OsCommitResult {
    // SAFETY: the caller keeps the configuration, subprocess statistics, and
    // nonempty primitive range valid for this coherent commit/accounting epoch.
    unsafe {
        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*stats).commit_calls), 1);
        }

        let Some(range) =
            OsPageRange::covering(addr, ReservedBytes::new(size), _mi_os_page_size_typed())
        else {
            return OsCommitResult {
                success: true,
                initially_zero: InitiallyZero(false),
            };
        };
        let start = range.start();
        let csize = range.size().get();

        let mut os_is_zero = false;
        let err = _mi_prim_commit(start, csize, &mut os_is_zero);
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "cannot commit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
                err, err, start, csize
            ));
            return OsCommitResult {
                success: false,
                initially_zero: InitiallyZero(false),
            };
        }
        if !subproc.is_null() {
            let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
            __mi_stat_increase_mt(ptr::addr_of_mut!((*stats).committed), stat_size.get());
        }
        OsCommitResult {
            success: true,
            initially_zero: InitiallyZero(os_is_zero),
        }
    }
}

/// # Safety
/// A nonempty `addr..addr+size` must remain a live caller-controlled range
/// suitable for the active primitive commit operation. OS configuration and
/// current subprocess statistics must remain live and initialized. Null or
/// empty input is accepted as a successful no-op.
#[inline(never)]
pub(crate) unsafe fn _mi_os_commit(addr: *mut c_void, size: usize) -> OsCommitResult {
    // SAFETY: the full range size is also its committed-byte accounting, and
    // the remaining lifetime and primitive requirements are inherited.
    unsafe { _mi_os_commit_ex(addr, size, CommittedBytes::new(size)) }
}

/// # Safety
///
/// `addr..addr+size` must remain a live caller-controlled range suitable for
/// the active primitive decommit operation. OS configuration and current
/// subprocess statistics must remain live, and `stat_size` must match the
/// committed bytes charged if the primitive reports that recommit is needed.
#[inline]
unsafe fn mi_os_decommit_ex(
    addr: *mut c_void,
    size: usize,
    stat_size: CommittedBytes,
) -> Option<OsDecommitResult> {
    // SAFETY: the caller supplies the live primitive range and exact accounting;
    // the primitive's in/out flag remains local to this decommit epoch.
    unsafe {
        let range =
            OsPageRange::contained(addr, ReservedBytes::new(size), _mi_os_page_size_typed())?;
        let start = range.start();
        let csize = range.size().get();

        let mut needs_recommit = true;
        let err = _mi_prim_decommit(start, csize, &mut needs_recommit);
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "cannot decommit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
                err, err, start, csize
            ));
        } else if needs_recommit {
            let subproc = _mi_subproc();
            if !subproc.is_null() {
                let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
                __mi_stat_decrease_mt(ptr::addr_of_mut!((*stats).committed), stat_size.get());
            }
        }
        crate::internal::mi_assert_internal!(err == 0);
        Some(OsDecommitResult {
            success: err == 0,
            needs_recommit,
        })
    }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_decommit(addr: *mut c_void, size: usize) -> bool {
    // SAFETY: the caller keeps the primitive range live and its full committed
    // size supplies the accounting for an effective decommit.
    unsafe { mi_os_decommit_ex(addr, size, CommittedBytes::new(size)) }
        .is_none_or(|result| result.success)
}

/// # Safety
/// A nonempty contained portion of `addr..addr+size` must remain a live
/// caller-controlled range suitable for primitive reset. In high-debug builds
/// it must also be writable. OS configuration, current subprocess statistics,
/// and primitive dispatch must remain initialized and stable. Null, empty, or
/// subpage input is accepted as a successful no-op.
pub unsafe extern "C" fn _mi_os_reset(addr: *mut c_void, size: usize) -> bool {
    // SAFETY: the caller keeps initialized OS page configuration stable while
    // the contained reset geometry is derived.
    let page_size = unsafe { _mi_os_page_size_typed() };
    let Some(range) = OsPageRange::contained(addr, ReservedBytes::new(size), page_size) else {
        return true;
    };
    let start = range.start();
    let csize = range.size().get();

    // The noninitializing lookup is safe; the operation contract keeps the
    // returned subprocess statistics live for this synchronous update.
    let subproc = _mi_subproc();
    if !subproc.is_null() {
        // SAFETY: the live subprocess prefix contains aligned atomic statistics;
        // all concurrent accesses to these fields use the atomic MT protocol.
        unsafe {
            let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*stats).reset), csize);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*stats).reset_calls), 1);
        }
    }

    if MI_DEBUG > 1 && MI_SECURE == 0 && MI_TRACK_ENABLED == 0 {
        // SAFETY: the caller keeps the contained reset range writable in the
        // configuration where debug reset poisoning is enabled.
        unsafe { core::ptr::write_bytes(start, 0, csize) };
    }

    // SAFETY: the validated contained range remains live and suitable for the
    // initialized primitive reset operation.
    let err = unsafe { _mi_prim_reset(start, csize) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "cannot reset OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            err, err, start, csize
        ));
    }
    err == 0
}

/// # Safety
/// A nonempty contained portion of `addr..addr+size` must remain a live
/// caller-controlled range suitable for primitive reuse. OS configuration and
/// primitive dispatch must remain initialized and stable. Null, empty, or
/// subpage input is accepted as a no-op.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_reuse(addr: *mut c_void, size: usize) {
    // SAFETY: the caller keeps initialized page configuration stable while the
    // contained reuse geometry is derived.
    let Some(range) = OsPageRange::contained(addr, ReservedBytes::new(size), unsafe {
        _mi_os_page_size_typed()
    }) else {
        return;
    };
    let start = range.start();
    let csize = range.size().get();
    // SAFETY: the validated contained range remains live and suitable for the
    // initialized primitive reuse operation.
    let err = unsafe { _mi_prim_reuse(start, csize) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "cannot reuse OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            err, err, start, csize
        ));
    }
}

/// # Safety
/// `p..p+size` must designate a live caller-controlled range that remains valid
/// for the purge operation. When no callback is supplied and an OS decommit or
/// reset can be selected, the range must be suitable for the active OS
/// primitive layer and its configuration must be initialized. When the
/// decommit branch is selected, `stat_size` must accurately describe the
/// committed bytes charged to that operation.
/// If `commit_fun` is present, it must accept a decommit request for this exact
/// range on the calling thread; `commit_fun_arg` must remain valid for every
/// access the callback performs and satisfy its managed-arena lifetime,
/// threading, and reentrancy contract. The callback must not unwind.
pub(crate) unsafe extern "C" fn _mi_os_purge_ex(
    p: *mut c_void,
    size: usize,
    allow_reset: bool,
    stat_size: usize,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
) -> bool {
    if mi_option_get(mi_option_t::mi_option_purge_delay) < 0 {
        return false;
    }

    // SAFETY: the current subproc remains live while its atomic statistics are
    // updated, and all concurrent access to these fields is atomic.
    unsafe {
        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*stats).purge_calls), 1);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*stats).purged), size);
        }
    }

    if let Some(commit_fun) = commit_fun {
        // SAFETY: the caller keeps the managed range and opaque argument valid,
        // permits invocation on this thread, and guarantees the callback does
        // not unwind across its C ABI boundary.
        return unsafe { commit_fun(false, p, size, core::ptr::null_mut(), commit_fun_arg) };
    }

    if mi_option_is_enabled(mi_option_t::mi_option_purge_decommits) && !_mi_preloading() {
        // SAFETY: the caller exclusively controls the live reserved range and
        // supplies its exact committed-byte accounting for OS decommit.
        return unsafe { mi_os_decommit_ex(p, size, CommittedBytes::new(stat_size)) }
            .is_none_or(|result| result.needs_recommit);
    }

    if allow_reset {
        // SAFETY: the caller exclusively controls the live reserved range for
        // this OS reset operation.
        unsafe { _mi_os_reset(p, size) };
    }
    false
}

/// # Safety
/// `p..p+size` must designate a live reserved virtual-memory range that the
/// caller exclusively controls for purge and keeps valid for the call. The OS
/// memory configuration must be initialized.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_purge(p: *mut c_void, size: usize) -> bool {
    // SAFETY: the caller keeps this exclusively controlled range live for the
    // purge; the committed-byte accounting matches its full size, and this
    // wrapper supplies no callback or opaque argument.
    unsafe { _mi_os_purge_ex(p, size, true, size, None, core::ptr::null_mut()) }
}

/// Apply or remove primitive protection on the contained whole-page range.
///
/// # Safety
/// A nonempty contained portion of `addr..addr+size` must remain a live
/// caller-controlled mapping suitable for the requested primitive protection
/// transition. OS configuration and primitive dispatch must remain initialized
/// and stable, and incompatible concurrent access to the range must be excluded.
#[inline]
unsafe fn mi_os_protectx(addr: *mut c_void, size: usize, protect: bool) -> bool {
    // SAFETY: the caller keeps initialized page configuration stable while the
    // contained protection geometry is derived.
    let Some(range) = OsPageRange::contained(addr, ReservedBytes::new(size), unsafe {
        _mi_os_page_size_typed()
    }) else {
        return false;
    };
    let start = range.start();
    let csize = range.size().get();
    // SAFETY: the validated contained mapping remains live and exclusively
    // controlled for this initialized primitive protection transition.
    let err = unsafe { _mi_prim_protect(start, csize, protect) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "cannot {} OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            if protect { "protect" } else { "unprotect" },
            err,
            err,
            start,
            csize
        ));
    }
    err == 0
}

/// # Safety
/// A nonempty contained portion of `addr..addr+size` must remain a live mapping
/// suitable for primitive protection. OS configuration and dispatch must remain
/// stable, and incompatible concurrent access must be excluded.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_protect(addr: *mut c_void, size: usize) -> bool {
    // SAFETY: the caller's live-range and concurrency contract permits the
    // requested protect transition.
    unsafe { mi_os_protectx(addr, size, true) }
}

/// # Safety
/// A nonempty contained portion of `addr..addr+size` must remain a live mapping
/// suitable for removing primitive protection. OS configuration and dispatch
/// must remain stable, and incompatible concurrent access must be excluded.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_unprotect(addr: *mut c_void, size: usize) -> bool {
    // SAFETY: the caller's live-range and concurrency contract permits the
    // requested unprotect transition.
    unsafe { mi_os_protectx(addr, size, false) }
}

/// # Safety
/// When present, `addr..addr+size` must be a primitive OS range that may be
/// released exactly once by the active primitive. The OS configuration must be
/// initialized, `size` must be a nonzero whole number of OS pages, and `commit_size`
/// must match the committed bytes currently charged to this release. A supplied
/// `subproc` must remain live and own that accounting; otherwise the current
/// subprocess lookup and registry must remain stable for the operation. Atomic
/// statistics updates may race, but range reuse or teardown may not. `None` is
/// accepted as a no-op and performs no accounting.
#[inline]
unsafe fn mi_os_prim_free_range(
    addr: Option<NonNull<c_void>>,
    size: ReservedBytes,
    commit_size: CommittedBytes,
    subproc: Option<BorrowedSubproc>,
) {
    // SAFETY: the caller keeps the primitive range, configuration, subprocess
    // registry, and accounting live for this ordered release/update epoch.
    unsafe {
        crate::internal::mi_assert_internal!(
            size.get().is_multiple_of(_mi_os_page_size_typed().get())
        );
        let Some(addr) = addr else {
            return;
        };
        let addr = addr.as_ptr();
        let err = _mi_prim_free(addr, size.get());
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "unable to free OS memory (error: {} (0x{:x}), size: 0x{:x} bytes, address: {:p})\n",
                err,
                err,
                size.get(),
                addr
            ));
        }
        let subproc = if subproc.is_some() {
            subproc
        } else {
            SubprocPtr::new(_mi_subproc()).map(|subproc| BorrowedSubproc::new(subproc))
        };
        if let Some(subproc) = subproc {
            let stats =
                ptr::addr_of_mut!((*subproc.ptr().as_ptr().cast::<MiSubprocStatsPrefix>()).stats);
            if !commit_size.is_zero() {
                __mi_stat_decrease_mt(ptr::addr_of_mut!((*stats).committed), commit_size.get());
            }
            __mi_stat_decrease_mt(ptr::addr_of_mut!((*stats).reserved), size.get());
        }
    }
}

/// Release an OS allocation using its recorded backing metadata.
///
/// # Safety
/// If `memid` decodes as OS memory, it must describe the live backing allocation
/// containing `addr..addr+size`; other MemId kinds are accepted as a no-op. For
/// OS memory, the range must not be concurrently used, reused, or freed, and
/// `still_committed` must describe the usable range's current committed charge.
/// A nonnull `subproc` must remain live and own the allocation's statistics;
/// otherwise current-subprocess lookup must remain valid. OS configuration and
/// primitive dispatch must remain initialized and stable for the release.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_free_ex(
    addr: *mut c_void,
    size: usize,
    still_committed: bool,
    memid: mi_memid_t,
    subproc: *mut mi_subproc_t,
) {
    let MemIdView::Os(memid) = MemId::view(memid) else {
        return;
    };
    let subproc = SubprocPtr::new(subproc);
    let addr = NonNull::new(addr);
    let size = ReservedBytes::new(size);
    // SAFETY: the raw contract couples the decoded OS MemId, usable range,
    // commitment state, and optional live accounting subprocess for release.
    unsafe {
        let subproc = subproc.map(|subproc| BorrowedSubproc::new(subproc));
        _mi_os_free_ex_typed(addr, size, still_committed, memid, subproc);
    }
}

/// A validated OS allocation backing range and its primitive-free dispatch plan.
///
/// This is geometry only and does not prove allocation ownership or liveness.
enum OsFreeBackingRange {
    Huge {
        base: OsMemPtr,
        size: ReservedBytes,
    },
    Primitive {
        memid: OsMemId,
        committed: CommittedBytes,
    },
}

impl OsFreeBackingRange {
    /// `legacy_normalized_usable_size` must equal `_mi_os_good_alloc_size(usable_size)`
    /// when `os.size` is zero; it is ignored otherwise.
    #[inline]
    fn from_usable(
        usable: Option<NonNull<c_void>>,
        usable_size: ReservedBytes,
        os: OsMemId,
        legacy_normalized_usable_size: ReservedBytes,
        still_committed: bool,
        page_size: OsPageSize,
    ) -> Option<Self> {
        let usable = usable?;
        let base = os.base?;
        let base_address = ptr_exposed_addr(base.as_ptr());
        let usable_address = ptr_exposed_addr(usable.as_ptr());
        let offset = usable_address.checked_sub(base_address)?;

        if !base_address.is_multiple_of(page_size.get()) {
            return None;
        }

        let declared_size = os.size.get();
        let initial_size = if declared_size == 0 {
            legacy_normalized_usable_size.get()
        } else {
            declared_size
        };
        let backing_size = if declared_size == 0 {
            initial_size.checked_add(offset)?
        } else {
            initial_size
        };
        if !backing_size.is_multiple_of(page_size.get()) {
            return None;
        }
        let usable_end = offset.checked_add(usable_size.get())?;
        if usable_end > backing_size {
            return None;
        }
        let committed = if still_committed {
            initial_size.checked_sub(offset)?
        } else {
            0
        };

        let size = ReservedBytes::new(backing_size);
        if matches!(&os.kind, OsMemKind::Huge) {
            if !os.pinned.0 {
                return None;
            }
            Some(Self::Huge { base, size })
        } else {
            Some(Self::Primitive {
                memid: OsMemId {
                    base: Some(base),
                    size,
                    initially_committed: os.initially_committed,
                    initially_zero: os.initially_zero,
                    pinned: os.pinned,
                    kind: OsMemKind::Normal,
                },
                committed: CommittedBytes::new(committed),
            })
        }
    }
}

/// Validate and dispatch one OS backing release.
///
/// # Safety
/// `os` must describe the live backing allocation containing `addr` and `size`.
/// The allocation must not be concurrently used, reused, or freed, and
/// `still_committed` must describe the usable range's committed charge. A
/// supplied `subproc` must remain live and own the accounting; otherwise current
/// subprocess lookup must remain valid. OS configuration and primitive dispatch
/// must remain initialized and stable throughout validation and release.
unsafe fn _mi_os_free_ex_typed(
    addr: Option<NonNull<c_void>>,
    size: ReservedBytes,
    still_committed: bool,
    os: OsMemId,
    subproc: Option<BorrowedSubproc>,
) {
    // SAFETY: the caller keeps the backing, configuration, primitive dispatch,
    // and accounting subprocess valid for this complete plan/dispatch epoch.
    unsafe {
        let normalized_size = if os.size.get() == 0 {
            ReservedBytes::new(_mi_os_good_alloc_size(size.get()))
        } else {
            os.size
        };
        let range = OsFreeBackingRange::from_usable(
            addr,
            size,
            os,
            normalized_size,
            still_committed,
            _mi_os_page_size_typed(),
        );
        crate::internal::mi_assert_internal!(range.is_some());
        let Some(range) = range else { return };
        match range {
            OsFreeBackingRange::Huge { base, size } => {
                mi_os_free_huge_os_pages(Some(base), size, subproc);
            }
            OsFreeBackingRange::Primitive { memid, committed } => {
                let addr = memid.base.and_then(|base| NonNull::new(base.as_ptr()));
                mi_os_prim_free_range(addr, memid.size, committed, subproc);
            }
        }
    }
}

/// # Safety
/// `memid` must decode to the live OS backing allocation containing `p..p+size`.
/// The range must remain unused and unreleased during this committed release;
/// OS configuration, primitive dispatch, and current-subprocess lookup must be
/// initialized and stable.
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    // SAFETY: this wrapper supplies full committed accounting and delegates
    // current-subprocess selection while preserving the raw backing contract.
    unsafe { _mi_os_free_ex(p, size, true, memid, core::ptr::null_mut()) };
}

/// # Safety
///
/// The OS configuration must be initialized. `size` must be nonzero and a
/// whole number of OS pages. The primitive allocator and current subprocess
/// statistics must remain live for this synchronous allocation attempt. A
/// non-null primitive result is returned with its reported large/zero flags
/// even when the primitive also reports a diagnostic error code.
#[inline]
unsafe fn mi_os_prim_alloc(
    size: usize,
    mut try_alignment: usize,
    commit: bool,
    mut allow_large: bool,
) -> Option<PrimitiveOsAllocation> {
    // SAFETY: the caller supplies initialized OS configuration and page-sized
    // geometry; raw primitive outputs remain local to this allocation epoch.
    unsafe {
        crate::internal::mi_assert_internal!(
            size > 0 && size.is_multiple_of(_mi_os_page_size_typed().get())
        );
        if size == 0 {
            return None;
        }
        if !commit {
            allow_large = false;
        }
        if try_alignment == 0 {
            try_alignment = 1;
        }

        let large_page_size = (*mi_os_mem_config_ptr()).large_page_size;
        if large_page_size > 0
            && size >= 8 * large_page_size
            && _mi_is_power_of_two(try_alignment)
            && try_alignment < large_page_size
        {
            try_alignment = large_page_size;
        }

        let mut is_large = false;
        let mut is_zero = false;
        let mut p = core::ptr::null_mut();
        let err = _mi_prim_alloc(
            core::ptr::null_mut(),
            size,
            try_alignment,
            commit,
            allow_large,
            &mut is_large,
            &mut is_zero,
            &mut p,
        );
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "unable to allocate OS memory (error: {} (0x{:x}), addr: {:p}, size: 0x{:x} bytes, align: 0x{:x}, commit: {}, allow large: {})\n",
                err,
                err,
                core::ptr::null_mut::<c_void>(),
                size,
                try_alignment,
                commit as i32,
                allow_large as i32
            ));
        }

        let p = NonNull::new(p);
        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!((*stats).mmap_calls), 1);
            if p.is_some() {
                let reserved = ReservedBytes::new(size);
                __mi_stat_increase_mt(ptr::addr_of_mut!((*stats).reserved), reserved.get());
                if commit {
                    let committed = CommittedBytes::new(size);
                    __mi_stat_increase_mt(ptr::addr_of_mut!((*stats).committed), committed.get());
                }
            }
        }
        let p = p?;
        Some(PrimitiveOsAllocation {
            base: OsMemPtr::new(p.as_ptr())?,
            initially_zero: InitiallyZero(is_zero),
            pinned: Pinned(is_large),
        })
    }
}

/// Allocate a normalized aligned OS range and return its matching MemId.
///
/// # Safety
/// OS configuration, primitive callbacks, and current subprocess statistics
/// must remain live and initialized for the allocation and any fallback trim or
/// rollback. A successful pair records the mapping and its release metadata but
/// does not provide exclusive ownership or automatic cleanup. The tuple's
/// usable aligned address may be interior to or after the MemId's OS backing
/// base; the MemId base and size govern backing release and accounting.
unsafe fn mi_os_alloc_aligned_memid(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
) -> Option<(NonNull<c_void>, MemId<OsMem>)> {
    if size == 0 {
        return None;
    }

    // SAFETY: initialized stable OS configuration supplies the page size used
    // by allocation-size normalization.
    let size = unsafe { _mi_os_good_alloc_size(size) };
    // SAFETY: the same stable configuration contains a nonzero page size.
    let os_page_size = unsafe { _mi_os_page_size_typed() };
    let alignment = OsAlignment::normalized(alignment, os_page_size)?;

    // SAFETY: allocator initialization populated alloc_granularity and the
    // caller excludes concurrent configuration mutation.
    let try_direct_alloc = unsafe {
        alignment.get() <= (*mi_os_mem_config_ptr()).alloc_granularity
            || alignment.get() <= size / 4
    };

    let mut os_is_large = false;
    let mut os_is_zero = false;
    let os_base: *mut c_void;
    let mut os_size = size;
    let mut p: *mut c_void = core::ptr::null_mut();

    if try_direct_alloc {
        // SAFETY: normalized `size` is a nonzero page multiple; OS configuration
        // and current subprocess statistics remain live for the attempt.
        let allocation = unsafe { mi_os_prim_alloc(size, alignment.get(), commit, allow_large) };
        if let Some(PrimitiveOsAllocation {
            base,
            initially_zero,
            pinned,
        }) = allocation
        {
            p = base.as_ptr();
            os_is_large = pinned.0;
            os_is_zero = initially_zero.0;
        }
    }

    if !p.is_null() && (p as usize & (alignment.get() - 1)) == 0 {
        os_base = p;
    } else {
        if !p.is_null() {
            // SAFETY: this rejected direct mapping is exactly `size` bytes;
            // committed accounting matches the allocation request.
            unsafe {
                mi_os_prim_free_range(
                    NonNull::new(p),
                    ReservedBytes::new(size),
                    CommittedBytes::new(if commit { size } else { 0 }),
                    None,
                )
            };
        }
        let over_size = alignment.overreserve(size)?;
        // SAFETY: initialized stable OS configuration supplies the primitive's
        // partial-free capability used to select the fallback transaction.
        if unsafe { !(*mi_os_mem_config_ptr()).has_partial_free } {
            // SAFETY: checked `over_size` remains a nonzero page multiple; OS
            // configuration and subprocess statistics are live for the attempt.
            let allocation = unsafe { mi_os_prim_alloc(over_size, 1, false, false) };
            let PrimitiveOsAllocation {
                base,
                initially_zero,
                pinned,
            } = allocation?;
            p = base.as_ptr();
            os_is_large = pinned.0;
            os_is_zero = initially_zero.0;

            os_base = p;
            os_size = over_size;
            p = _mi_align_up_ptr(p, alignment.get());

            // SAFETY: `p..p+size` lies within the live uncommitted over-reserve;
            // successful commitment charges exactly `size` bytes.
            if commit && !unsafe { _mi_os_commit(p, size) }.success() {
                // SAFETY: commit failure leaves the full over-reserve releasable
                // with zero committed-byte accounting.
                unsafe {
                    mi_os_prim_free_range(
                        NonNull::new(os_base),
                        ReservedBytes::new(over_size),
                        CommittedBytes::new(0),
                        None,
                    )
                };
                return None;
            }
        } else {
            // SAFETY: checked `over_size` remains a nonzero page multiple; OS
            // configuration and subprocess statistics are live for the attempt.
            let allocation = unsafe { mi_os_prim_alloc(over_size, 1, commit, false) };
            let PrimitiveOsAllocation {
                base,
                initially_zero,
                pinned,
            } = allocation?;
            p = base.as_ptr();
            os_is_large = pinned.0;
            os_is_zero = initially_zero.0;

            let aligned_p = _mi_align_up_ptr(p, alignment.get());
            let pre_size = aligned_p as usize - p as usize;
            // SAFETY: stable initialized page size rounds the retained middle;
            // checked over-reservation bounds the result within `over_size`.
            let mid_size = unsafe { _mi_align_up(size, _mi_os_page_size_typed().get()) };
            let post_size = over_size - pre_size - mid_size;

            if pre_size > 0 {
                // SAFETY: the nonempty prefix is the first `pre_size` bytes of
                // the live over-reserve and carries matching commit accounting.
                unsafe {
                    mi_os_prim_free_range(
                        NonNull::new(p),
                        ReservedBytes::new(pre_size),
                        CommittedBytes::new(if commit { pre_size } else { 0 }),
                        None,
                    )
                };
            }
            if post_size > 0 {
                // SAFETY: checked middle and prefix sizes keep this projection
                // within the live over-reserved allocation.
                let post = unsafe { aligned_p.cast::<u8>().add(mid_size) }.cast::<c_void>();
                // SAFETY: the nonempty suffix spans the remaining `post_size`
                // bytes and carries matching commit accounting.
                unsafe {
                    mi_os_prim_free_range(
                        NonNull::new(post),
                        ReservedBytes::new(post_size),
                        CommittedBytes::new(if commit { post_size } else { 0 }),
                        None,
                    )
                };
            }

            p = aligned_p;
            os_base = aligned_p;
            os_size = mid_size;
        }
    }

    let p = NonNull::new(p)?;
    let os_base = OsMemPtr::new(os_base)?;
    let memid = MemId::<OsMem>::os(
        Some(os_base),
        crate::core_types::ReservedBytes::new(os_size),
        crate::core_types::InitiallyCommitted(commit),
        crate::core_types::InitiallyZero(os_is_zero),
        crate::core_types::Pinned(os_is_large),
    );
    Some((p, memid))
}

/// Allocate committed OS backing for a plain allocation request.
///
/// # Safety
/// OS configuration, primitive dispatch, and current subprocess statistics
/// must remain live and initialized for this synchronous allocation attempt.
/// A successful result records its mapping status but provides no automatic
/// release or exclusive ownership.
#[inline]
unsafe fn mi_os_alloc_committed(size: usize) -> Option<CommittedOsAllocation> {
    if size == 0 {
        return None;
    }
    // SAFETY: stable initialized OS configuration supplies the page size used
    // to normalize this nonzero allocation request.
    let size = unsafe { _mi_os_good_alloc_size(size) };
    // SAFETY: normalized `size` is a nonzero page multiple, and the caller keeps
    // primitive configuration and subprocess statistics live for the attempt.
    let allocation = unsafe { mi_os_prim_alloc(size, 0, true, false) };
    let PrimitiveOsAllocation {
        base,
        initially_zero,
        pinned,
    } = allocation?;
    Some(CommittedOsAllocation {
        base,
        size: ReservedBytes::new(size),
        initially_zero,
        pinned,
    })
}

/// # Safety
/// `memid` must be null or point to aligned uniquely writable MemId storage. OS
/// configuration, primitive dispatch, and current subprocess statistics must
/// remain live and initialized. On success, the caller must eventually release
/// the returned mapping with the published MemId. A nonnull output is set to
/// NONE before validation and remains NONE on failure.
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_alloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    if !memid.is_null() {
        // SAFETY: the boundary contract makes the nonnull output uniquely writable.
        unsafe { *memid = _mi_memid_none() };
    }
    if size == 0 || memid.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: the boundary keeps OS configuration, primitive dispatch, and
    // subprocess statistics live for this committed allocation attempt.
    let allocation = unsafe { mi_os_alloc_committed(size) };
    let Some(CommittedOsAllocation {
        base,
        size,
        initially_zero,
        pinned,
    }) = allocation
    else {
        return core::ptr::null_mut();
    };
    let p = base.as_ptr();

    // SAFETY: the output remains uniquely writable, and the committed status
    // supplies metadata for exactly the mapping returned to the caller.
    unsafe {
        *memid = MemId::<OsMem>::os(
            Some(base),
            size,
            crate::core_types::InitiallyCommitted(true),
            initially_zero,
            pinned,
        )
        .into_raw();
    }
    p
}

/// # Safety
/// `memid` must be null or point to aligned uniquely writable MemId storage. OS
/// configuration, primitive dispatch, and current subprocess statistics must
/// remain live and initialized. On success, the caller must eventually release
/// the returned mapping with the published MemId. A nonnull output is set to
/// NONE before validation and remains NONE on failure.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_zalloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    if !memid.is_null() {
        // SAFETY: the boundary contract makes the nonnull output uniquely writable.
        unsafe { *memid = _mi_memid_none() };
    }
    if size == 0 || memid.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: the boundary keeps OS configuration, primitive dispatch, and
    // subprocess statistics live for this committed allocation attempt.
    let allocation = unsafe { mi_os_alloc_committed(size) };
    let Some(CommittedOsAllocation {
        base,
        size: backing_size,
        initially_zero,
        pinned,
    }) = allocation
    else {
        return core::ptr::null_mut();
    };
    let p = base.as_ptr();
    if !initially_zero.0 {
        // SAFETY: the successful committed mapping is writable for at least the
        // original requested `size`, matching the legacy zalloc zeroing extent.
        unsafe { _mi_memzero_aligned(p, size) };
    }

    // SAFETY: the output remains uniquely writable, and zalloc has established
    // zeroed requested bytes before publishing the matching committed MemId.
    unsafe {
        *memid = MemId::<OsMem>::os(
            Some(base),
            backing_size,
            crate::core_types::InitiallyCommitted(true),
            InitiallyZero(true),
            pinned,
        )
        .into_raw();
    }
    p
}

/// # Safety
/// `memid` must point to writable memory-id storage.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if !memid.is_null() {
        unsafe {
            *memid = _mi_memid_none();
        }
    }
    if size == 0 || memid.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: the raw boundary keeps OS configuration and subprocess state
    // live while the helper produces a matching pointer and MemId pair.
    let Some((p, new_memid)) =
        (unsafe { mi_os_alloc_aligned_memid(size, alignment, commit, allow_large) })
    else {
        return core::ptr::null_mut();
    };
    // SAFETY: the boundary contract makes `memid` uniquely writable, and the
    // typed result guarantees this raw MemId describes the returned pointer.
    unsafe { *memid = new_memid.into_raw() };
    p.as_ptr()
}

/// # Safety
/// `memid` must point to writable memory-id storage. `offset` must be within `size`.
pub unsafe extern "C" fn _mi_os_alloc_aligned_at_offset(
    size: usize,
    alignment: usize,
    offset: usize,
    commit: bool,
    allow_large: bool,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if !memid.is_null() {
        unsafe {
            *memid = _mi_memid_none();
        }
    }
    let os_page_size = unsafe { _mi_os_page_size_typed() };
    if memid.is_null() || offset > size {
        return core::ptr::null_mut();
    }
    let Some(alignment) = OsAlignment::from_page_multiple(alignment, os_page_size) else {
        return core::ptr::null_mut();
    };
    if offset == 0 {
        return unsafe { _mi_os_alloc_aligned(size, alignment.get(), commit, allow_large, memid) };
    }

    let Some(extra) = alignment
        .align_up(offset)
        .and_then(|aligned| aligned.checked_sub(offset))
    else {
        return core::ptr::null_mut();
    };
    let Some(oversize) = size.checked_add(extra).filter(|total| *total < usize::MAX) else {
        return core::ptr::null_mut();
    };
    let start =
        unsafe { _mi_os_alloc_aligned(oversize, alignment.get(), commit, allow_large, memid) };
    if start.is_null() {
        return core::ptr::null_mut();
    }

    let p = unsafe { start.cast::<u8>().add(extra) }.cast::<c_void>();
    crate::internal::mi_assert!(unsafe {
        crate::internal::_mi_is_aligned(p.cast::<u8>().add(offset).cast(), alignment.get())
    });
    if commit
        && extra >= os_page_size.get()
        && let Some(range) = OsPageRange::covering(start, ReservedBytes::new(extra), os_page_size)
    {
        let dstart = range.start();
        let dsize = range.size().get();
        let mut needs_recommit = false;
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        let err = unsafe { _mi_prim_decommit(dstart, dsize, &mut needs_recommit) };
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "unable to decommit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
                err, err, dstart, dsize
            ));
        }
    }
    p
}

#[inline]
fn mi_os_huge_reservation_size(pages: usize) -> Option<ReservedBytes> {
    pages
        .checked_mul(MI_HUGE_OS_PAGE_SIZE)
        .map(ReservedBytes::new)
}

#[inline]
fn mi_os_huge_reservation_end(start: usize, size: ReservedBytes) -> Option<usize> {
    start.checked_add(size.get())
}

#[inline]
fn mi_os_claim_huge_pages(pages: usize) -> Option<(usize, ReservedBytes)> {
    #[cfg(target_pointer_width = "32")]
    let _ = pages;
    #[cfg(target_pointer_width = "32")]
    {
        return None;
    }

    #[cfg(not(target_pointer_width = "32"))]
    {
        let size = mi_os_huge_reservation_size(pages)?;
        let mut huge_start = MI_HUGE_START.load(Ordering::Relaxed);
        loop {
            let mut start = huge_start;
            if start == 0 {
                start = 8usize << 40;
                if MI_SECURE != 0 || MI_DEBUG == 0 {
                    // SAFETY: process initialization publishes the default
                    // thread-heap pointer before huge reservations can run.
                    let theap = unsafe { _mi_theap_default() };
                    // SAFETY: `theap` is the live process-default metadata;
                    // this predicate only reads its initialized-state marker.
                    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
                        // SAFETY: the preceding predicate established initialized
                        // default-thread-heap random state for this synchronous use.
                        let r = unsafe { crate::internal::_mi_theap_random_next(theap) };
                        let random_offset = MI_HUGE_OS_PAGE_SIZE * ((r >> 17) & 0x0FFF);
                        start = start.checked_add(random_offset)?;
                    } else {
                        _mi_warning_message_fmt(format_args!(
                            "failed to randomize the start address of huge pages allocation ({} bytes at {:p})",
                            size.get(),
                            ptr::without_provenance_mut::<c_void>(start)
                        ));
                    }
                }
            }
            let end = mi_os_huge_reservation_end(start, size)?;
            match MI_HUGE_START.compare_exchange_weak(
                huge_start,
                end,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some((start, size)),
                Err(current) => huge_start = current,
            }
        }
    }
}

/// Release a contiguous huge-page backing one primitive page at a time.
///
/// # Safety
/// When present, `p` and `size` must describe a live contiguous backing made of
/// whole `MI_HUGE_OS_PAGE_SIZE` primitive mappings, each releasable exactly once.
/// The range must not be concurrently used, reused, or freed. A supplied
/// `subproc` must remain live and own all reserved/committed accounting;
/// otherwise current-subprocess lookup must remain valid. OS configuration and
/// primitive dispatch must remain initialized and stable. `None` or zero size is
/// accepted as a no-op.
#[inline]
unsafe fn mi_os_free_huge_os_pages(
    p: Option<OsMemPtr>,
    size: ReservedBytes,
    subproc: Option<BorrowedSubproc>,
) {
    let mut size = size.get();
    let Some(p) = p else {
        return;
    };
    if size == 0 {
        return;
    }
    // SAFETY: the caller keeps each separately mapped huge page, primitive
    // dispatch, and accounting subprocess valid through ordered page release.
    // Huge-page allocation used these same scalar addresses as primitive hints;
    // no provenance from one released mapping is reused for the next mapping.
    unsafe {
        let mut address = ptr_exposed_addr(p.as_ptr());
        while size >= MI_HUGE_OS_PAGE_SIZE {
            mi_os_prim_free_range(
                NonNull::new(ptr::without_provenance_mut::<c_void>(address)),
                ReservedBytes::new(MI_HUGE_OS_PAGE_SIZE),
                CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE),
                subproc,
            );
            size -= MI_HUGE_OS_PAGE_SIZE;
            if size != 0 {
                let Some(next) = address.checked_add(MI_HUGE_OS_PAGE_SIZE) else {
                    crate::internal::mi_assert_internal!(false);
                    return;
                };
                address = next;
            }
        }
    }
}

/// Return the process-wide NUMA node count, initializing its atomic cache when needed.
///
/// The allocator lifecycle completes `_mi_os_init` and primitive dispatch
/// initialization before NUMA queries become reachable, and never mutates the
/// platform NUMA dispatch statics afterward.
#[inline(never)]
pub extern "C" fn _mi_os_numa_node_count() -> i32 {
    let mut count = MI_NUMA_NODE_COUNT.load(Ordering::Acquire);
    if count == 0 {
        let ncount = mi_option_get(mi_option_t::mi_option_use_numa_nodes);
        count = if ncount > 0 && ncount < i32::MAX as _ {
            ncount as usize
        } else {
            // SAFETY: allocator lifecycle completed `_mi_os_init` and primitive
            // initialization, and excludes concurrent or subsequent mutation of
            // Windows dynamic-dispatch statics. The primitive synchronously owns
            // every syscall/path/output buffer used by this query.
            let n = unsafe { _mi_prim_numa_node_count() };
            if n == 0 || n > i32::MAX as usize {
                1
            } else {
                n
            }
        };
        MI_NUMA_NODE_COUNT.store(count, Ordering::Release);
        if count > 1 {
            _mi_verbose_message(format_args!("using {} numa regions\n", count));
        }
    }
    crate::internal::mi_assert_internal!(count > 0 && count <= i32::MAX as usize);
    count as i32
}

#[inline]
/// Query the current node after allocator lifecycle has frozen primitive NUMA dispatch.
fn mi_os_numa_node_get() -> i32 {
    let numa_count = _mi_os_numa_node_count();
    if numa_count <= 1 {
        return 0;
    }
    // SAFETY: allocator lifecycle completed `_mi_os_init` and primitive
    // initialization, and excludes concurrent or subsequent mutation of Windows
    // dynamic-dispatch statics. The primitive keeps its processor/node syscall
    // outputs in valid local storage for the synchronous query.
    let n = unsafe { _mi_prim_numa_node() };
    let mut numa_node = if n < i32::MAX as usize { n as i32 } else { 0 };
    if numa_node >= numa_count {
        numa_node %= numa_count;
    }
    numa_node
}

/// Return the current process NUMA node normalized to the cached node count.
/// Allocator lifecycle guarantees primitive initialization and freezes the
/// platform NUMA dispatch statics before this API becomes reachable.
#[inline(never)]
pub extern "C" fn _mi_os_numa_node() -> i32 {
    if MI_NUMA_NODE_COUNT.load(Ordering::Relaxed) == 1 {
        0
    } else {
        mi_os_numa_node_get()
    }
}

/// Return the current process NUMA node in its scalar domain type. Allocator
/// lifecycle guarantees primitive initialization before this API is used.
#[inline]
pub(crate) fn _mi_os_numa_node_typed() -> crate::core_types::NumaNode {
    crate::core_types::NumaNode::new(_mi_os_numa_node())
}

/// # Safety
/// `memid` must be non-null, aligned, and uniquely writable for one memory ID.
/// Any non-null `pages_reserved` and `psize` outputs must likewise be aligned
/// and uniquely writable for one `usize`. The returned memory must be released
/// through the matching OS huge-page free path.
// SAFETY: this internal ABI boundary initializes each caller-provided output,
// then publishes only successfully mapped contiguous huge pages and their MemId.
pub unsafe extern "C" fn _mi_os_alloc_huge_os_pages(
    pages: usize,
    numa_node: i32,
    max_msecs: i64,
    pages_reserved: *mut usize,
    psize: *mut usize,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    // SAFETY: the caller keeps every non-null output uniquely writable for the
    // transaction; primitive mappings are accessed only through their returned
    // ranges, and successful pages remain live through MemId publication.
    unsafe {
        if !memid.is_null() {
            *memid = _mi_memid_none();
        }
        if !psize.is_null() {
            *psize = 0;
        }
        if !pages_reserved.is_null() {
            *pages_reserved = 0;
        }
        if memid.is_null() {
            return ptr::null_mut();
        }
        let Some((start, size)) = mi_os_claim_huge_pages(pages) else {
            return ptr::null_mut();
        };

        let start_t = _mi_clock_start();
        let mut page = 0usize;
        let mut all_zero = true;
        let mut mapped_start = None;
        let mut mapped_start_address = None;
        while page < pages {
            let offset = page * MI_HUGE_OS_PAGE_SIZE;
            let addr = ptr::without_provenance_mut::<c_void>(start + offset);
            let mut is_zero = false;
            let mut p: *mut c_void = ptr::null_mut();
            let err = _mi_prim_alloc_huge_os_pages(
                addr,
                MI_HUGE_OS_PAGE_SIZE,
                numa_node,
                &mut is_zero,
                &mut p,
            );
            if !is_zero {
                all_zero = false;
            }
            if err != 0 {
                _mi_warning_message_fmt(format_args!(
                    "unable to allocate huge OS page (error: {} (0x{:x}), address: {:p}, size: {:x} bytes)\n",
                    err, err, addr, MI_HUGE_OS_PAGE_SIZE
                ));
                break;
            }
            if p != addr {
                if !p.is_null() {
                    _mi_warning_message_fmt(format_args!(
                        "could not allocate contiguous huge OS page {} at {:p}\n",
                        page, addr
                    ));
                    // SAFETY: the primitive returned this committed mapping, but
                    // allocator statistics have not yet recorded it; release it
                    // directly so no uncharged counters are decremented.
                    let free_err = _mi_prim_free(p, MI_HUGE_OS_PAGE_SIZE);
                    if free_err != 0 {
                        _mi_warning_message_fmt(format_args!(
                            "unable to free OS memory (error: {} (0x{:x}), size: 0x{:x} bytes, address: {:p})\n",
                            free_err, free_err, MI_HUGE_OS_PAGE_SIZE, p
                        ));
                    }
                }
                break;
            }

            let mapped = NonNull::new(p);
            crate::internal::mi_assert_internal!(mapped.is_some());
            let Some(mapped) = mapped else { break };
            if page == 0 {
                mapped_start = Some(mapped);
                mapped_start_address = Some(ptr_exposed_addr(mapped.as_ptr()));
            }
            crate::internal::mi_assert_internal!(mapped_start.is_some());
            crate::internal::mi_assert_internal!(
                ptr_exposed_addr(mapped.as_ptr()) == mapped_start_address.unwrap_or(start) + offset
            );

            page += 1;
            let subproc = _mi_subproc();
            if !subproc.is_null() {
                let stats = ptr::addr_of_mut!((*subproc.cast::<MiSubprocStatsPrefix>()).stats);
                let committed = CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE);
                let reserved = ReservedBytes::new(MI_HUGE_OS_PAGE_SIZE);
                __mi_stat_increase_mt(ptr::addr_of_mut!((*stats).committed), committed.get());
                __mi_stat_increase_mt(ptr::addr_of_mut!((*stats).reserved), reserved.get());
            }

            if max_msecs > 0 {
                let mut elapsed = _mi_clock_end(start_t);
                if page >= 1 {
                    let estimate = (elapsed / (page as i64 + 1)) * pages as i64;
                    if estimate > 2 * max_msecs {
                        elapsed = max_msecs + 1;
                    }
                }
                if elapsed > max_msecs {
                    _mi_warning_message_fmt(format_args!(
                        "huge OS page allocation timed out (after allocating {} page(s))\n",
                        page
                    ));
                    break;
                }
            }
        }

        let allocated = page * MI_HUGE_OS_PAGE_SIZE;
        crate::internal::mi_assert_internal!(allocated <= size.get());
        if !pages_reserved.is_null() {
            *pages_reserved = page;
        }
        if !psize.is_null() {
            *psize = allocated;
        }
        let Some(mapped_start) = mapped_start else {
            return ptr::null_mut();
        };
        crate::internal::mi_assert_internal!(page != 0);
        *memid = crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(
            crate::core_types::OsMemPtr::new(mapped_start.as_ptr()),
            crate::core_types::ReservedBytes::new(allocated),
            crate::core_types::InitiallyCommitted(true),
            crate::core_types::InitiallyZero(all_zero),
            crate::core_types::Pinned(true),
        )
        .into_raw();
        crate::internal::mi_assert!((*memid).is_pinned);
        mapped_start.as_ptr()
    }
}

/// # Safety
/// OS configuration, primitive callbacks, and current subprocess statistics
/// must remain live and initialized. Each optional output must be null or point
/// to aligned writable storage for its type. A successful returned mapping must
/// remain live until the caller releases it through `mi_os_free`. On success,
/// `base` and `full_size` receive the backing release range, which may differ
/// from the returned usable address; the caller must release that backing
/// accordingly. On failure, all optional outputs remain untouched.
#[inline]
#[allow(clippy::too_many_arguments)]
unsafe fn mi_os_alloc_aligned_ex(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
    is_committed: *mut bool,
    is_pinned: *mut bool,
    base: *mut *mut c_void,
    full_size: *mut usize,
) -> *mut c_void {
    // SAFETY: the exported boundary keeps primitive configuration and
    // subprocess state live while producing matching mapping metadata.
    let Some((p, memid)) =
        (unsafe { mi_os_alloc_aligned_memid(size, alignment, commit, allow_large) })
    else {
        return core::ptr::null_mut();
    };
    let memid = memid.into_raw();
    if !is_committed.is_null() {
        // SAFETY: the contract makes this optional bool output writable.
        unsafe { *is_committed = memid.initially_committed };
    }
    if !is_pinned.is_null() {
        // SAFETY: the contract makes this optional bool output writable.
        unsafe { *is_pinned = memid.is_pinned };
    }
    if !base.is_null()
        || !full_size.is_null()
        || (memid.initially_committed && !memid.initially_zero)
    {
        let (os_base, os_size) = match crate::core_types::MemId::view(memid) {
            crate::core_types::MemIdView::Os(os) => (
                os.base.map(|p| p.as_ptr()).unwrap_or(core::ptr::null_mut()),
                os.size.get(),
            ),
            crate::core_types::MemIdView::External(external) => (
                external
                    .base
                    .map(|p| p.as_ptr())
                    .unwrap_or(core::ptr::null_mut()),
                external.size.get(),
            ),
            _ => (core::ptr::null_mut(), 0),
        };
        if !base.is_null() {
            // SAFETY: the contract makes this optional pointer output writable.
            unsafe { *base = os_base };
        }
        if !full_size.is_null() {
            // SAFETY: the contract makes this optional size output writable.
            unsafe { *full_size = os_size };
        }
        if memid.initially_committed && !memid.initially_zero {
            // SAFETY: the successful MemId identifies the full live writable
            // backing range, which requires explicit zeroing for this API.
            unsafe { _mi_memzero_aligned(os_base, os_size) };
        }
    }
    p.as_ptr()
}

/// # Safety
/// For nonempty input, the whole page-covering primitive interval obtained by
/// aligning around `p..p+size` must remain a live caller-controlled mapping
/// suitable for commitment. OS configuration, current subprocess statistics,
/// and primitive dispatch must remain initialized and stable. Null or empty
/// input is accepted as a no-op.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_commit(p: *mut c_void, size: usize) {
    // SAFETY: the caller keeps the complete page-covering mapping and allocator
    // OS/accounting state valid for this full-size committed-byte transaction.
    let _ = unsafe { _mi_os_commit(p, size) };
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    base: *mut *mut c_void,
    full_size: *mut usize,
) -> *mut c_void {
    unsafe {
        mi_os_alloc_aligned_ex(
            size,
            alignment,
            commit,
            false,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            base,
            full_size,
        )
    }
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_alloc(
    size: usize,
    commit: bool,
    full_size: *mut usize,
) -> *mut c_void {
    unsafe {
        mi_os_alloc_aligned(
            size,
            (*mi_os_mem_config_ptr()).alloc_granularity,
            commit,
            core::ptr::null_mut(),
            full_size,
        )
    }
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_alloc_aligned_allow_large(
    size: usize,
    alignment: usize,
    commit: bool,
    is_committed: *mut bool,
    is_pinned: *mut bool,
    base: *mut *mut c_void,
    full_size: *mut usize,
) -> *mut c_void {
    unsafe {
        mi_os_alloc_aligned_ex(
            size,
            alignment,
            commit,
            true,
            is_committed,
            is_pinned,
            base,
            full_size,
        )
    }
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_free(p: *mut c_void, size: usize) {
    if p.is_null() || size == 0 {
        return;
    }
    let memid = crate::core_types::MemId::<crate::core_types::OsMem>::os(
        crate::core_types::OsMemPtr::new(p),
        crate::core_types::ReservedBytes::new(size),
        crate::core_types::InitiallyCommitted(true),
        crate::core_types::InitiallyZero(false),
        crate::core_types::Pinned(false),
    )
    .into_raw();
    unsafe { _mi_os_free(p, size, memid) };
}

/// # Safety
/// A nonempty contained portion of `p..p+size` must remain a live
/// caller-controlled mapping suitable for primitive decommit. OS configuration,
/// current subprocess statistics, and primitive dispatch must remain initialized
/// and stable; the full size must match committed accounting. Null, empty, or
/// subpage input is accepted as a no-op.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_decommit(p: *mut c_void, size: usize) {
    // SAFETY: the caller keeps the mapping live and supplies its full committed
    // accounting to the primitive decommit boundary.
    let _ = unsafe { _mi_os_decommit(p, size) };
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::*;
    use crate::core_types::{InitiallyCommitted, InitiallyZero, Pinned};

    fn page_size(size: usize) -> OsPageSize {
        // SAFETY: every test page size is explicitly nonzero.
        unsafe { OsPageSize::new_unchecked(size) }
    }

    fn aligned_ptr(storage: &mut [u8], alignment: usize) -> *mut c_void {
        let base = storage.as_mut_ptr();
        let address = ptr_exposed_addr(base);
        let remainder = address % alignment;
        let aligned = if remainder == 0 {
            address
        } else {
            address + alignment - remainder
        };
        base.map_addr(|_| aligned).cast()
    }

    #[test]
    fn huge_page_reservation_geometry_rejects_overflow() {
        assert_eq!(mi_os_huge_reservation_size(0), Some(ReservedBytes::new(0)));
        let one = mi_os_huge_reservation_size(1).unwrap();
        assert_eq!(one, ReservedBytes::new(MI_HUGE_OS_PAGE_SIZE));
        assert!(mi_os_huge_reservation_size(usize::MAX).is_none());

        assert_eq!(
            mi_os_huge_reservation_end(usize::MAX - one.get(), one),
            Some(usize::MAX)
        );
        assert!(mi_os_huge_reservation_end(usize::MAX - one.get() + 1, one).is_none());
    }

    #[test]
    fn huge_page_allocation_rejects_null_memid_before_primitive_work() {
        let mut pages_reserved = usize::MAX;
        let mut size = usize::MAX;
        // SAFETY: the optional outputs are uniquely writable; a null mandatory
        // MemId is intentionally supplied to exercise preflight rejection.
        let result = unsafe {
            _mi_os_alloc_huge_os_pages(1, 0, 0, &mut pages_reserved, &mut size, ptr::null_mut())
        };
        assert!(result.is_null());
        assert_eq!(pages_reserved, 0);
        assert_eq!(size, 0);
    }

    #[test]
    fn os_commit_no_effective_range_reports_success_without_zeroing() {
        // SAFETY: null and empty input is the documented no-op case; default OS
        // configuration and subprocess statistics remain live for the call.
        let result = unsafe { _mi_os_commit(ptr::null_mut(), 0) };
        assert!(result.success());
        assert!(!result.initially_zero().0);
    }

    #[test]
    fn aligned_allocation_rejects_zero_without_inert_memid() {
        // SAFETY: zero size returns before consulting OS configuration or
        // attempting any primitive allocation.
        let result = unsafe { mi_os_alloc_aligned_memid(0, 1, true, false) };
        assert!(result.is_none());
    }

    #[test]
    fn plain_allocators_publish_none_for_zero_size() {
        let os_memid = || {
            MemId::<OsMem>::os(
                None,
                ReservedBytes::new(16),
                InitiallyCommitted(true),
                InitiallyZero(false),
                Pinned(false),
            )
            .into_raw()
        };

        let mut alloc_memid = os_memid();
        // SAFETY: `alloc_memid` is uniquely writable; zero size rejects before
        // consulting OS configuration or invoking a primitive allocation.
        let alloc = unsafe { _mi_os_alloc(0, &mut alloc_memid) };
        assert!(alloc.is_null());
        assert!(matches!(MemId::view(alloc_memid), MemIdView::None));

        let mut zalloc_memid = os_memid();
        // SAFETY: `zalloc_memid` is uniquely writable; zero size rejects before
        // consulting OS configuration or invoking a primitive allocation.
        let zalloc = unsafe { _mi_os_zalloc(0, &mut zalloc_memid) };
        assert!(zalloc.is_null());
        assert!(matches!(MemId::view(zalloc_memid), MemIdView::None));
    }

    fn assert_range(range: OsPageRange, start: usize, size: usize) {
        assert_eq!(ptr_exposed_addr(range.start()), start);
        assert_eq!(range.size(), ReservedBytes::new(size));
    }

    fn os_memid(base: *mut c_void, size: usize) -> OsMemId {
        OsMemId {
            base: OsMemPtr::new(base),
            size: ReservedBytes::new(size),
            initially_committed: InitiallyCommitted(true),
            initially_zero: InitiallyZero(false),
            pinned: Pinned(false),
            kind: OsMemKind::Normal,
        }
    }

    fn primitive_plan(range: OsFreeBackingRange) -> (OsMemPtr, ReservedBytes, CommittedBytes) {
        let OsFreeBackingRange::Primitive { memid, committed } = range else {
            panic!("expected primitive OS free plan")
        };
        (memid.base.unwrap(), memid.size, committed)
    }

    #[test]
    fn os_alignment_normalizes_checked_page_geometry() {
        let page = page_size(16);
        assert!(OsAlignment::normalized(0, page).is_none());
        assert_eq!(OsAlignment::normalized(1, page).unwrap().get(), 16);
        assert_eq!(OsAlignment::normalized(16, page).unwrap().get(), 16);
        assert_eq!(OsAlignment::normalized(17, page).unwrap().get(), 32);
        assert!(OsAlignment::normalized(48, page).is_none());
        assert!(OsAlignment::normalized(usize::MAX, page).is_none());
    }

    #[test]
    fn os_alignment_preserves_offset_page_multiple_boundary() {
        let page = page_size(16);
        assert!(OsAlignment::from_page_multiple(0, page).is_none());
        assert!(OsAlignment::from_page_multiple(1, page).is_none());
        assert_eq!(OsAlignment::from_page_multiple(16, page).unwrap().get(), 16);
        assert!(OsAlignment::from_page_multiple(17, page).is_none());
        assert!(OsAlignment::from_page_multiple(48, page).is_none());
        assert!(OsAlignment::from_page_multiple(usize::MAX, page).is_none());
    }

    #[test]
    fn os_alignment_checks_align_up_and_overreserve_overflow() {
        let alignment = OsAlignment::normalized(32, page_size(16)).unwrap();
        assert_eq!(alignment.align_up(0), Some(0));
        assert_eq!(alignment.align_up(1), Some(32));
        assert_eq!(alignment.align_up(32), Some(32));
        assert_eq!(alignment.align_up(33), Some(64));
        assert_eq!(alignment.align_up(usize::MAX), None);
        assert_eq!(alignment.overreserve(usize::MAX - 33), Some(usize::MAX - 1));
        assert_eq!(alignment.overreserve(usize::MAX - 32), None);
        assert_eq!(alignment.overreserve(usize::MAX - 31), None);
    }

    #[test]
    fn os_alignment_has_scalar_layout() {
        assert_eq!(size_of::<OsAlignment>(), size_of::<usize>());
    }

    #[test]
    fn os_page_range_rejects_null_and_zero() {
        let page = page_size(16);
        assert!(OsPageRange::covering(ptr::null_mut(), ReservedBytes::new(16), page).is_none());
        assert!(OsPageRange::contained(ptr::null_mut(), ReservedBytes::new(16), page).is_none());

        let mut storage = [0u8; 64];
        let aligned = aligned_ptr(&mut storage, page.get());
        let misaligned = aligned.map_addr(|address| address + 1);
        assert!(OsPageRange::covering(aligned, ReservedBytes::new(0), page).is_none());
        assert!(OsPageRange::contained(aligned, ReservedBytes::new(0), page).is_none());
        assert!(OsPageRange::covering(misaligned, ReservedBytes::new(0), page).is_none());
        assert!(OsPageRange::contained(misaligned, ReservedBytes::new(0), page).is_none());
    }

    #[test]
    fn os_page_range_handles_exact_and_straddled_intervals() {
        let page = page_size(16);
        let mut storage = [0u8; 128];
        let aligned = aligned_ptr(&mut storage, page.get());
        let start = ptr_exposed_addr(aligned);

        assert_range(
            OsPageRange::covering(aligned, ReservedBytes::new(16), page).unwrap(),
            start,
            16,
        );
        assert_range(
            OsPageRange::contained(aligned, ReservedBytes::new(16), page).unwrap(),
            start,
            16,
        );

        let misaligned = aligned.map_addr(|address| address + 3);
        assert_range(
            OsPageRange::covering(misaligned, ReservedBytes::new(30), page).unwrap(),
            start,
            48,
        );
        assert_range(
            OsPageRange::contained(misaligned, ReservedBytes::new(30), page).unwrap(),
            start + 16,
            16,
        );
    }

    #[test]
    fn os_page_range_rejects_interior_subpage_and_tracks_one_past() {
        let page = page_size(16);
        let mut storage = [0u8; 64];
        let aligned = aligned_ptr(&mut storage, page.get());
        let start = ptr_exposed_addr(aligned);
        let interior = aligned.map_addr(|address| address + 1);

        assert!(OsPageRange::contained(interior, ReservedBytes::new(14), page).is_none());
        let range = OsPageRange::covering(interior, ReservedBytes::new(14), page).unwrap();
        assert_eq!(
            ptr_exposed_addr(range.start()) + range.size().get(),
            start + 16
        );
    }

    #[test]
    fn os_page_range_supports_non_power_of_two_pages() {
        let page = page_size(6);
        let mut storage = [0u8; 64];
        let aligned = aligned_ptr(&mut storage, page.get());
        let start = ptr_exposed_addr(aligned);
        let interior = aligned.map_addr(|address| address + 1);

        assert_range(
            OsPageRange::covering(interior, ReservedBytes::new(11), page).unwrap(),
            start,
            12,
        );
        assert_range(
            OsPageRange::contained(interior, ReservedBytes::new(11), page).unwrap(),
            start + 6,
            6,
        );
    }

    #[test]
    fn os_page_range_rejects_end_and_alignment_overflow() {
        let page = page_size(16);
        let mut storage = [0u8; 64];
        let aligned = aligned_ptr(&mut storage, page.get());
        let address = ptr_exposed_addr(aligned);

        assert!(OsPageRange::covering(aligned, ReservedBytes::new(usize::MAX), page).is_none());
        assert!(OsPageRange::contained(aligned, ReservedBytes::new(usize::MAX), page).is_none());
        let end_at_max = ReservedBytes::new(usize::MAX - address);
        assert!(OsPageRange::covering(aligned, end_at_max, page).is_none());

        let near_zero = ptr::without_provenance_mut::<c_void>(1);
        assert!(OsPageRange::covering(near_zero, ReservedBytes::new(1), page).is_none());

        let near_max = ptr::without_provenance_mut::<c_void>(usize::MAX - 1);
        assert!(OsPageRange::contained(near_max, ReservedBytes::new(1), page).is_none());
    }

    #[test]
    fn os_page_range_has_compact_geometry_layout() {
        assert_eq!(size_of::<OsPageRange>(), 2 * size_of::<usize>());
    }

    #[test]
    fn os_free_backing_range_validates_interior_usable_geometry() {
        let page = page_size(16);
        let mut storage = [0u8; 128];
        let base = aligned_ptr(&mut storage, page.get());
        let usable = NonNull::new(base.map_addr(|address| address + 16));
        let os = os_memid(base, 64);

        let range = OsFreeBackingRange::from_usable(
            usable,
            ReservedBytes::new(32),
            os,
            ReservedBytes::new(64),
            true,
            page,
        )
        .unwrap();
        let (range_base, range_size, committed) = primitive_plan(range);
        assert_eq!(range_base.as_ptr(), base);
        assert_eq!(range_size, ReservedBytes::new(64));
        assert_eq!(committed.get(), 48);

        let os = os_memid(base, 64);
        let uncommitted = OsFreeBackingRange::from_usable(
            usable,
            ReservedBytes::new(32),
            os,
            ReservedBytes::new(64),
            false,
            page,
        )
        .unwrap();
        assert_eq!(primitive_plan(uncommitted).2.get(), 0);
    }

    #[test]
    fn os_free_backing_range_preserves_legacy_unsized_memid_expansion() {
        let page = page_size(16);
        let mut storage = [0u8; 128];
        let base = aligned_ptr(&mut storage, page.get());
        let usable = NonNull::new(base.map_addr(|address| address + 16));
        let os = os_memid(base, 0);

        let range = OsFreeBackingRange::from_usable(
            usable,
            ReservedBytes::new(16),
            os,
            ReservedBytes::new(32),
            true,
            page,
        )
        .unwrap();
        let (range_base, range_size, committed) = primitive_plan(range);
        assert_eq!(range_base.as_ptr(), base);
        assert_eq!(range_size, ReservedBytes::new(48));
        assert_eq!(committed.get(), 16);

        let farther_usable = NonNull::new(base.map_addr(|address| address + 32));
        let os = os_memid(base, 0);
        assert!(
            OsFreeBackingRange::from_usable(
                farther_usable,
                ReservedBytes::new(16),
                os,
                ReservedBytes::new(16),
                true,
                page,
            )
            .is_none()
        );
        let os = os_memid(base, 0);
        let uncommitted = OsFreeBackingRange::from_usable(
            farther_usable,
            ReservedBytes::new(16),
            os,
            ReservedBytes::new(16),
            false,
            page,
        )
        .unwrap();
        let (_, size, committed) = primitive_plan(uncommitted);
        assert_eq!(size, ReservedBytes::new(48));
        assert_eq!(committed.get(), 0);
    }

    #[test]
    fn os_free_backing_range_rejects_invalid_backing_geometry() {
        let page = page_size(16);
        let mut storage = [0u8; 128];
        let base = aligned_ptr(&mut storage, page.get());
        let usable = NonNull::new(base.map_addr(|address| address + 16));

        let missing_base = os_memid(ptr::null_mut(), 64);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(16),
                missing_base,
                ReservedBytes::new(64),
                true,
                page,
            )
            .is_none()
        );
        let valid = os_memid(base, 64);
        assert!(
            OsFreeBackingRange::from_usable(
                None,
                ReservedBytes::new(16),
                valid,
                ReservedBytes::new(64),
                true,
                page,
            )
            .is_none()
        );

        let non_page_multiple = os_memid(base, 24);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(8),
                non_page_multiple,
                ReservedBytes::new(24),
                true,
                page,
            )
            .is_none()
        );
        let misaligned_base = os_memid(base.map_addr(|address| address + 1), 64);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(8),
                misaligned_base,
                ReservedBytes::new(64),
                true,
                page,
            )
            .is_none()
        );
        let base_after_usable = os_memid(base.map_addr(|address| address + 32), 64);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(8),
                base_after_usable,
                ReservedBytes::new(64),
                true,
                page,
            )
            .is_none()
        );
        let too_small = os_memid(base, 32);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(17),
                too_small,
                ReservedBytes::new(32),
                true,
                page,
            )
            .is_none()
        );
    }

    #[test]
    fn os_free_backing_range_validates_primitive_dispatch_kind() {
        let page = page_size(16);
        let mut storage = [0u8; 128];
        let base = aligned_ptr(&mut storage, page.get());
        let usable = NonNull::new(base);

        let mut huge = os_memid(base, 64);
        huge.kind = OsMemKind::Huge;
        huge.pinned = Pinned(true);
        let plan = OsFreeBackingRange::from_usable(
            usable,
            ReservedBytes::new(16),
            huge,
            ReservedBytes::new(64),
            true,
            page,
        )
        .unwrap();
        let OsFreeBackingRange::Huge {
            base: huge_base,
            size: huge_size,
        } = plan
        else {
            panic!("expected huge OS free plan")
        };
        assert_eq!(huge_base.as_ptr(), base);
        assert_eq!(huge_size, ReservedBytes::new(64));

        let mut unpinned_huge = os_memid(base, 64);
        unpinned_huge.kind = OsMemKind::Huge;
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(16),
                unpinned_huge,
                ReservedBytes::new(64),
                true,
                page,
            )
            .is_none()
        );

        let mut pinned_normal = os_memid(base, 64);
        pinned_normal.pinned = Pinned(true);
        let plan = OsFreeBackingRange::from_usable(
            usable,
            ReservedBytes::new(16),
            pinned_normal,
            ReservedBytes::new(64),
            true,
            page,
        )
        .unwrap();
        let OsFreeBackingRange::Primitive { memid, .. } = plan else {
            panic!("expected primitive OS free plan")
        };
        assert!(memid.pinned.0);
        assert!(matches!(memid.kind, OsMemKind::Normal));
    }

    #[test]
    fn os_free_backing_range_rejects_checked_arithmetic_overflow() {
        let page = page_size(16);
        let base = ptr::without_provenance_mut::<c_void>(16);
        let usable = NonNull::new(ptr::without_provenance_mut::<c_void>(usize::MAX - 8));
        let bounded = os_memid(base, usize::MAX - 15);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(32),
                bounded,
                ReservedBytes::new(usize::MAX - 15),
                true,
                page,
            )
            .is_none()
        );

        let unknown_size = os_memid(base, 0);
        assert!(
            OsFreeBackingRange::from_usable(
                usable,
                ReservedBytes::new(0),
                unknown_size,
                ReservedBytes::new(32),
                true,
                page,
            )
            .is_none()
        );
    }
}
