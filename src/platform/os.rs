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
    BorrowedSubproc, CommittedBytes, MemId, MemIdView, OsMemId, OsMemKind, OsMemPtr, OsPageSize,
    ReservedBytes, SubprocPtr, ptr_exposed_addr,
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

// SAFETY: OS memory config readers preserve process-global memory configuration invariants.
#[inline]
unsafe fn mi_os_mem_config_ptr() -> *const mi_os_mem_config_t {
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

// SAFETY: OS subproc stats projection uses live borrowed subproc metadata.
unsafe fn mi_os_current_subproc() -> Option<BorrowedSubproc> {
    SubprocPtr::new(unsafe { _mi_subproc() })
        .map(|subproc| unsafe { BorrowedSubproc::new(subproc) })
}

unsafe fn mi_os_subproc_stats_mut(subproc: BorrowedSubproc) -> *mut mi_stats_t {
    // SAFETY: OS subproc stats projection uses live borrowed subproc metadata.
    unsafe { ptr::addr_of_mut!((*subproc.ptr().as_ptr().cast::<MiSubprocStatsPrefix>()).stats) }
}

// SAFETY: OS memory config readers preserve process-global memory configuration invariants.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_has_overcommit() -> bool {
    unsafe { (*mi_os_mem_config_ptr()).has_overcommit }
}

// SAFETY: OS memory config readers preserve process-global memory configuration invariants.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_has_virtual_reserve() -> bool {
    unsafe { (*mi_os_mem_config_ptr()).has_virtual_reserve }
}

// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_page_size() -> usize {
    unsafe { (*mi_os_mem_config_ptr()).page_size }
}

/// # Safety
/// The OS memory configuration must have been initialized and contain a nonzero page size.
// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[inline]
pub(crate) unsafe fn _mi_os_page_size_typed() -> crate::core_types::OsPageSize {
    let page_size = unsafe { _mi_os_page_size() };
    crate::internal::mi_assert_internal!(page_size > 0);
    unsafe { crate::core_types::OsPageSize::new_unchecked(page_size) }
}

/// # Safety
/// The OS memory configuration must have been initialized before this is called concurrently.
// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_large_page_size() -> usize {
    let large = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    if large != 0 {
        large
    } else {
        // SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
        unsafe { _mi_os_page_size_typed().get() }
    }
}

/// # Safety
/// The OS memory configuration and options state must be initialized.
// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_minimal_purge_size() -> usize {
    let minsize = mi_option_get_size(mi_option_t::mi_option_minimal_purge_size);
    if minsize != 0 {
        _mi_align_up(minsize, unsafe { _mi_os_page_size_typed().get() })
    } else {
        // SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
        let has_transparent_huge_pages =
            unsafe { (*mi_os_mem_config_ptr()).has_transparent_huge_pages };
        if has_transparent_huge_pages && mi_option_get(mi_option_t::mi_option_allow_thp) == 2 {
            unsafe { _mi_os_large_page_size() }
        } else {
            // SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
            unsafe { _mi_os_page_size_typed().get() }
        }
    }
}

/// # Safety
/// The OS memory configuration must have been initialized.
// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_guard_page_size() -> usize {
    let gsize = unsafe { _mi_os_page_size_typed().get() };
    crate::internal::mi_debug_assert!(gsize <= (MI_ARENA_SLICE_SIZE / 4));
    gsize
}

/// # Safety
/// The OS memory configuration must have been initialized.
// SAFETY: OS memory config readers preserve process-global memory configuration invariants.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_virtual_address_bits() -> usize {
    let vbits = unsafe { (*mi_os_mem_config_ptr()).virtual_address_bits };
    crate::internal::mi_debug_assert!(vbits <= MI_MAX_VABITS);
    vbits
}

/// # Safety
/// The OS memory configuration must have been initialized.
// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_canuse_large_page(size: usize, alignment: usize) -> bool {
    let large_page_size = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    large_page_size != 0
        && size.is_multiple_of(large_page_size)
        && alignment.is_multiple_of(large_page_size)
}

// SAFETY: OS page-size helpers read initialized memory config and preserve nonzero page-size domains.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_good_alloc_size(size: usize) -> usize {
    let align_size = if size < 512 * MI_KiB {
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
/// Must be called during process initialization before concurrent OS allocation use.
// SAFETY: OS memory config initialization updates the process-global memory configuration before concurrent OS allocation use.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_init() {
    unsafe { _mi_prim_mem_init(core::ptr::addr_of_mut!(mi_os_mem_config)) };
}

/// # Safety
/// The OS memory configuration must be initialized; secure/release randomized hints also require
/// an initialized default theap.
// SAFETY: aligned hint generation reads initialized memory config and the initialized default thread heap.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_get_aligned_hint(try_alignment: usize, size: usize) -> *mut c_void {
    if MI_INTPTR_SIZE < 8 || try_alignment <= unsafe { (*mi_os_mem_config_ptr()).alloc_granularity }
    {
        return core::ptr::null_mut();
    }
    // SAFETY: aligned hint generation reads initialized memory config and the initialized default thread heap.
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
            // SAFETY: aligned hint generation reads initialized memory config and the initialized default thread heap.
            let theap = unsafe { _mi_theap_default() };
            if unsafe { !mi_theap_is_initialized(theap) } {
                return core::ptr::null_mut();
            }
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
/// The OS memory configuration must have been initialized.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
#[allow(clippy::absurd_extreme_comparisons)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_size() -> usize {
    if MI_SECURE > 0 {
        unsafe { _mi_os_guard_page_size() }
    } else {
        0
    }
}

/// # Safety
/// `addr`, when non-null, must designate the guard page associated with `memid`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
            let commit_fun = unsafe { (*arena.cast::<MiArenaCommitPrefix>()).commit_fun };
            if let Some(commit_fun) = commit_fun {
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
/// `addr` must be at least one secure guard page past the page to protect, and `memid` must
/// describe that allocation.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_set_before(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
/// `addr`, when non-null, must designate the guard page associated with `memid`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
            let commit_fun = unsafe { (*arena.cast::<MiArenaCommitPrefix>()).commit_fun };
            if let Some(commit_fun) = commit_fun {
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
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        return unsafe {
            _mi_os_commit(addr, _mi_os_secure_guard_page_size(), core::ptr::null_mut())
        };
    }
    true
}

/// # Safety
/// `addr` must be at least one secure guard page past the page to reset, and `memid` must
/// describe that allocation.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_reset_before(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
pub(crate) unsafe extern "C" fn _mi_os_commit_ex(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
    stat_size: CommittedBytes,
) -> bool {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe {
        if !is_zero.is_null() {
            *is_zero = false;
        }
        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!(stats.commit_calls), 1);
        }

        let Some(range) =
            OsPageRange::covering(addr, ReservedBytes::new(size), _mi_os_page_size_typed())
        else {
            return true;
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
            return false;
        }
        if os_is_zero && !is_zero.is_null() {
            *is_zero = true;
        }
        if !subproc.is_null() {
            let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
            __mi_stat_increase_mt(ptr::addr_of_mut!(stats.committed), stat_size.get());
        }
        true
    }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> bool {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe { _mi_os_commit_ex(addr, size, is_zero, CommittedBytes::new(size)) }
}

#[inline]
unsafe fn mi_os_decommit_ex(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
    stat_size: CommittedBytes,
) -> bool {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe {
        crate::internal::mi_assert_internal!(!needs_recommit.is_null());

        let Some(range) =
            OsPageRange::contained(addr, ReservedBytes::new(size), _mi_os_page_size_typed())
        else {
            return true;
        };
        let start = range.start();
        let csize = range.size().get();

        *needs_recommit = true;
        let err = _mi_prim_decommit(start, csize, needs_recommit);
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "cannot decommit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
                err, err, start, csize
            ));
        } else if *needs_recommit {
            let subproc = _mi_subproc();
            if !subproc.is_null() {
                let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
                __mi_stat_decrease_mt(ptr::addr_of_mut!(stats.committed), stat_size.get());
            }
        }
        crate::internal::mi_assert_internal!(err == 0);
        err == 0
    }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_decommit(addr: *mut c_void, size: usize) -> bool {
    let mut needs_recommit = false;
    unsafe { mi_os_decommit_ex(addr, size, &mut needs_recommit, CommittedBytes::new(size)) }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
pub unsafe extern "C" fn _mi_os_reset(addr: *mut c_void, size: usize) -> bool {
    unsafe {
        let Some(range) =
            OsPageRange::contained(addr, ReservedBytes::new(size), _mi_os_page_size_typed())
        else {
            return true;
        };
        let start = range.start();
        let csize = range.size().get();

        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!(stats.reset), csize);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!(stats.reset_calls), 1);
        }

        if MI_DEBUG > 1 && MI_SECURE == 0 && MI_TRACK_ENABLED == 0 {
            core::ptr::write_bytes(start, 0, csize);
        }

        let err = _mi_prim_reset(start, csize);
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "cannot reset OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
                err, err, start, csize
            ));
        }
        err == 0
    }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_reuse(addr: *mut c_void, size: usize) {
    let Some(range) = OsPageRange::contained(addr, ReservedBytes::new(size), unsafe {
        _mi_os_page_size_typed()
    }) else {
        return;
    };
    let start = range.start();
    let csize = range.size().get();
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let err = unsafe { _mi_prim_reuse(start, csize) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "cannot reuse OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            err, err, start, csize
        ));
    }
}

/// # Safety
/// `p..p+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
pub(crate) unsafe extern "C" fn _mi_os_purge_ex(
    p: *mut c_void,
    size: usize,
    allow_reset: bool,
    stat_size: usize,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
) -> bool {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe {
        if mi_option_get(mi_option_t::mi_option_purge_delay) < 0 {
            return false;
        }
        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!(stats.purge_calls), 1);
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!(stats.purged), size);
        }

        if let Some(commit_fun) = commit_fun {
            commit_fun(false, p, size, core::ptr::null_mut(), commit_fun_arg)
        } else if mi_option_is_enabled(mi_option_t::mi_option_purge_decommits) && !_mi_preloading()
        {
            let mut needs_recommit = true;
            mi_os_decommit_ex(p, size, &mut needs_recommit, CommittedBytes::new(stat_size));
            needs_recommit
        } else {
            if allow_reset {
                _mi_os_reset(p, size);
            }
            false
        }
    }
}

/// # Safety
/// `p..p+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_purge(p: *mut c_void, size: usize) -> bool {
    unsafe { _mi_os_purge_ex(p, size, true, size, None, core::ptr::null_mut()) }
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline]
unsafe fn mi_os_protectx(addr: *mut c_void, size: usize, protect: bool) -> bool {
    let Some(range) = OsPageRange::contained(addr, ReservedBytes::new(size), unsafe {
        _mi_os_page_size_typed()
    }) else {
        return false;
    };
    let start = range.start();
    let csize = range.size().get();
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_protect(addr: *mut c_void, size: usize) -> bool {
    unsafe { mi_os_protectx(addr, size, true) }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_unprotect(addr: *mut c_void, size: usize) -> bool {
    unsafe { mi_os_protectx(addr, size, false) }
}

/// # Safety
/// When present, `addr..addr+size` must be a primitive OS range that may be
/// released. The OS configuration must be initialized, `size` must be page
/// aligned, `commit_size` must match the committed accounting for the range,
/// and `subproc` must remain live when supplied.
#[inline]
unsafe fn mi_os_prim_free_range(
    addr: Option<NonNull<c_void>>,
    size: ReservedBytes,
    commit_size: CommittedBytes,
    subproc: Option<BorrowedSubproc>,
) {
    // SAFETY: the caller preserves the primitive range and accounting contracts above.
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
        let subproc = subproc.or_else(|| mi_os_current_subproc());
        if let Some(subproc) = subproc {
            let stats = mi_os_subproc_stats_mut(subproc);
            if !commit_size.is_zero() {
                __mi_stat_decrease_mt(ptr::addr_of_mut!((*stats).committed), commit_size.get());
            }
            __mi_stat_decrease_mt(ptr::addr_of_mut!((*stats).reserved), size.get());
        }
    }
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let subproc = SubprocPtr::new(subproc).map(|subproc| unsafe { BorrowedSubproc::new(subproc) });
    let addr = NonNull::new(addr);
    let size = ReservedBytes::new(size);
    unsafe { _mi_os_free_ex_typed(addr, size, still_committed, memid, subproc) };
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

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
unsafe fn _mi_os_free_ex_typed(
    addr: Option<NonNull<c_void>>,
    size: ReservedBytes,
    still_committed: bool,
    os: OsMemId,
    subproc: Option<BorrowedSubproc>,
) {
    let normalized_size = if os.size.get() == 0 {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        ReservedBytes::new(unsafe { _mi_os_good_alloc_size(size.get()) })
    } else {
        os.size
    };
    let range =
        OsFreeBackingRange::from_usable(addr, size, os, normalized_size, still_committed, unsafe {
            _mi_os_page_size_typed()
        });
    crate::internal::mi_assert_internal!(range.is_some());
    let Some(range) = range else { return };
    match range {
        OsFreeBackingRange::Huge { base, size } => {
            unsafe { mi_os_free_huge_os_pages(Some(base), size, subproc) };
        }
        OsFreeBackingRange::Primitive { memid, committed } => {
            let addr = memid.base.and_then(|base| NonNull::new(base.as_ptr()));
            unsafe { mi_os_prim_free_range(addr, memid.size, committed, subproc) };
        }
    }
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    unsafe { _mi_os_free_ex(p, size, true, memid, core::ptr::null_mut()) };
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline]
unsafe fn mi_os_prim_alloc_at(
    hint_addr: *mut c_void,
    size: usize,
    mut try_alignment: usize,
    commit: bool,
    mut allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
) -> *mut c_void {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe {
        crate::internal::mi_assert_internal!(
            size > 0 && size.is_multiple_of(_mi_os_page_size_typed().get())
        );
        crate::internal::mi_assert_internal!(!is_zero.is_null());
        crate::internal::mi_assert_internal!(!is_large.is_null());
        if size == 0 {
            return core::ptr::null_mut();
        }
        if !commit {
            allow_large = false;
        }
        if try_alignment == 0 {
            try_alignment = 1;
        }

        let large_page_size = (*mi_os_mem_config_ptr()).large_page_size;
        if large_page_size > 0
            && hint_addr.is_null()
            && size >= 8 * large_page_size
            && _mi_is_power_of_two(try_alignment)
            && try_alignment < large_page_size
        {
            try_alignment = large_page_size;
        }

        *is_zero = false;
        let mut p = core::ptr::null_mut();
        let err = _mi_prim_alloc(
            hint_addr,
            size,
            try_alignment,
            commit,
            allow_large,
            is_large,
            is_zero,
            &mut p,
        );
        if err != 0 {
            _mi_warning_message_fmt(format_args!(
                "unable to allocate OS memory (error: {} (0x{:x}), addr: {:p}, size: 0x{:x} bytes, align: 0x{:x}, commit: {}, allow large: {})\n",
                err, err, hint_addr, size, try_alignment, commit as i32, allow_large as i32
            ));
        }

        let subproc = _mi_subproc();
        if !subproc.is_null() {
            let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
            __mi_stat_counter_increase_mt(ptr::addr_of_mut!(stats.mmap_calls), 1);
            if !p.is_null() {
                let reserved = ReservedBytes::new(size);
                __mi_stat_increase_mt(ptr::addr_of_mut!(stats.reserved), reserved.get());
                if commit {
                    let committed = CommittedBytes::new(size);
                    __mi_stat_increase_mt(ptr::addr_of_mut!(stats.committed), committed.get());
                }
            }
        }
        p
    }
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline]
unsafe fn mi_os_prim_alloc(
    size: usize,
    try_alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
) -> *mut c_void {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe {
        mi_os_prim_alloc_at(
            core::ptr::null_mut(),
            size,
            try_alignment,
            commit,
            allow_large,
            is_large,
            is_zero,
        )
    }
}

// SAFETY: offset-aligned allocation preserves caller offset, page alignment, and memid out-pointer contracts.
unsafe fn mi_os_alloc_aligned_memid(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
) -> (*mut c_void, mi_memid_t) {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let mut memid = unsafe { _mi_memid_none() };
    if size == 0 {
        return (core::ptr::null_mut(), memid);
    }

    // SAFETY: offset-aligned allocation preserves caller offset, page alignment, and memid out-pointer contracts.
    let size = unsafe { _mi_os_good_alloc_size(size) };
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let os_page_size = unsafe { _mi_os_page_size_typed() };
    let alignment = _mi_align_up(alignment, os_page_size.get());
    if alignment < os_page_size.get() || (alignment & (alignment - 1)) != 0 {
        return (core::ptr::null_mut(), memid);
    }

    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let try_direct_alloc = unsafe {
        alignment <= (*mi_os_mem_config_ptr()).alloc_granularity || alignment <= size / 4
    };

    let mut os_is_large = false;
    let mut os_is_zero = false;
    let os_base: *mut c_void;
    let mut os_size = size;
    let mut p: *mut c_void = core::ptr::null_mut();

    if try_direct_alloc {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        p = unsafe {
            mi_os_prim_alloc(
                size,
                alignment,
                commit,
                allow_large,
                &mut os_is_large,
                &mut os_is_zero,
            )
        };
    }

    if !p.is_null() && (p as usize & (alignment - 1)) == 0 {
        os_base = p;
    } else {
        if !p.is_null() {
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
            unsafe {
                mi_os_prim_free_range(
                    NonNull::new(p),
                    ReservedBytes::new(size),
                    CommittedBytes::new(if commit { size } else { 0 }),
                    None,
                )
            };
        }
        if size >= usize::MAX - alignment {
            return (core::ptr::null_mut(), memid);
        }

        let over_size = size + alignment;
        // SAFETY: over-reserve fallback preserves partial-free, commit, and trimmed range contracts.
        if unsafe { !(*mi_os_mem_config_ptr()).has_partial_free } {
            p = unsafe {
                mi_os_prim_alloc(
                    over_size,
                    1,
                    false,
                    false,
                    &mut os_is_large,
                    &mut os_is_zero,
                )
            };
            if p.is_null() {
                return (core::ptr::null_mut(), memid);
            }

            os_base = p;
            os_size = over_size;
            p = _mi_align_up_ptr(p, alignment);

            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
            if commit && !unsafe { _mi_os_commit(p, size, core::ptr::null_mut()) } {
                unsafe {
                    mi_os_prim_free_range(
                        NonNull::new(os_base),
                        ReservedBytes::new(over_size),
                        CommittedBytes::new(0),
                        None,
                    )
                };
                return (core::ptr::null_mut(), memid);
            }
        } else {
            // SAFETY: over-reserve fallback preserves partial-free, commit, and trimmed range contracts.
            p = unsafe {
                mi_os_prim_alloc(
                    over_size,
                    1,
                    commit,
                    false,
                    &mut os_is_large,
                    &mut os_is_zero,
                )
            };
            if p.is_null() {
                return (core::ptr::null_mut(), memid);
            }

            let aligned_p = _mi_align_up_ptr(p, alignment);
            let pre_size = aligned_p as usize - p as usize;
            // SAFETY: over-reserve fallback preserves partial-free, commit, and trimmed range contracts.
            let mid_size = unsafe { _mi_align_up(size, _mi_os_page_size_typed().get()) };
            let post_size = over_size - pre_size - mid_size;

            if pre_size > 0 {
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
                // SAFETY: over-reserve fallback preserves partial-free, commit, and trimmed range contracts.
                let post = unsafe { aligned_p.cast::<u8>().add(mid_size) }.cast::<c_void>();
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

    if p.is_null() || os_base.is_null() {
        return (core::ptr::null_mut(), memid);
    }

    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    memid = crate::core_types::MemId::<crate::core_types::OsMem>::os(
        crate::core_types::OsMemPtr::new(os_base),
        crate::core_types::ReservedBytes::new(os_size),
        crate::core_types::InitiallyCommitted(commit),
        crate::core_types::InitiallyZero(os_is_zero),
        crate::core_types::Pinned(os_is_large),
    )
    .into_raw();
    (p, memid)
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline]
unsafe fn mi_os_ensure_zero(p: *mut c_void, size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    if p.is_null() || size == 0 {
        return p;
    }
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    if !unsafe { (*memid).initially_committed } {
        let mut is_zero = false;
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        if !unsafe { _mi_os_commit(p, size, &mut is_zero) } {
            unsafe { _mi_os_free(p, size, *memid) };
            return core::ptr::null_mut();
        }
        unsafe { (*memid).initially_committed = true };
    }
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    if unsafe { (*memid).initially_zero } {
        return p;
    }
    unsafe { _mi_memzero_aligned(p, size) };
    unsafe { (*memid).initially_zero = true };
    p
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_alloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    if !memid.is_null() {
        unsafe {
            *memid = _mi_memid_none();
        }
    }
    if size == 0 || memid.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let size = unsafe { _mi_os_good_alloc_size(size) };
    let mut os_is_large = false;
    let mut os_is_zero = false;
    let p = unsafe { mi_os_prim_alloc(size, 0, true, false, &mut os_is_large, &mut os_is_zero) };
    if p.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe {
        *memid = crate::core_types::MemId::<crate::core_types::OsMem>::os(
            crate::core_types::OsMemPtr::new(p),
            crate::core_types::ReservedBytes::new(size),
            crate::core_types::InitiallyCommitted(true),
            crate::core_types::InitiallyZero(os_is_zero),
            crate::core_types::Pinned(os_is_large),
        )
        .into_raw();
    }
    p
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_zalloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    let p = unsafe { _mi_os_alloc(size, memid) };
    unsafe { mi_os_ensure_zero(p, size, memid) }
}

/// # Safety
/// `memid` must point to writable memory-id storage.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if !memid.is_null() {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe {
            *memid = _mi_memid_none();
        }
    }
    if size == 0 || memid.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let (p, new_memid) = unsafe { mi_os_alloc_aligned_memid(size, alignment, commit, allow_large) };
    if p.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        *memid = new_memid;
    }
    p
}

/// # Safety
/// `memid` must point to writable memory-id storage. `offset` must be within `size`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
pub unsafe extern "C" fn _mi_os_alloc_aligned_at_offset(
    size: usize,
    alignment: usize,
    offset: usize,
    commit: bool,
    allow_large: bool,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if !memid.is_null() {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe {
            *memid = _mi_memid_none();
        }
    }
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let os_page_size = unsafe { _mi_os_page_size_typed() };
    if memid.is_null() || offset > size || !alignment.is_multiple_of(os_page_size.get()) {
        return core::ptr::null_mut();
    }
    if offset == 0 {
        // SAFETY: offset-aligned allocation preserves caller offset, page alignment, and memid out-pointer contracts.
        return unsafe { _mi_os_alloc_aligned(size, alignment, commit, allow_large, memid) };
    }

    let extra = _mi_align_up(offset, alignment) - offset;
    if size >= usize::MAX - extra {
        return core::ptr::null_mut();
    }
    let oversize = size + extra;
    // SAFETY: offset-aligned allocation preserves caller offset, page alignment, and memid out-pointer contracts.
    let start = unsafe { _mi_os_alloc_aligned(oversize, alignment, commit, allow_large, memid) };
    if start.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: offset-aligned allocation preserves caller offset, page alignment, and memid out-pointer contracts.
    let p = unsafe { start.cast::<u8>().add(extra) }.cast::<c_void>();
    // SAFETY: offset-aligned allocation preserves caller offset, page alignment, and memid out-pointer contracts.
    crate::internal::mi_assert!(unsafe {
        crate::internal::_mi_is_aligned(p.cast::<u8>().add(offset).cast(), alignment)
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

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline]
unsafe fn mi_os_claim_huge_pages(pages: usize, total_size: *mut usize) -> *mut u8 {
    #[cfg(target_pointer_width = "32")]
    let _ = pages;
    if !total_size.is_null() {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe { *total_size = 0 };
    }
    #[cfg(target_pointer_width = "32")]
    {
        return core::ptr::null_mut();
    }

    #[cfg(not(target_pointer_width = "32"))]
    {
        let size = pages * MI_HUGE_OS_PAGE_SIZE;
        let mut huge_start = MI_HUGE_START.load(Ordering::Relaxed);
        loop {
            let mut start = huge_start;
            if start == 0 {
                start = 8usize << 40;
                if MI_SECURE != 0 || MI_DEBUG == 0 {
                    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
                    let theap = unsafe { _mi_theap_default() };
                    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
                        let r = unsafe { crate::internal::_mi_theap_random_next(theap) };
                        start += MI_HUGE_OS_PAGE_SIZE * ((r >> 17) & 0x0FFF);
                    } else {
                        _mi_warning_message_fmt(format_args!(
                            "failed to randomize the start address of huge pages allocation ({} bytes at {:p})",
                            size, start as *mut c_void
                        ));
                    }
                }
            }
            let end = start + size;
            match MI_HUGE_START.compare_exchange_weak(
                huge_start,
                end,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    if !total_size.is_null() {
                        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
                        unsafe { *total_size = size };
                    }
                    return start as *mut u8;
                }
                Err(current) => huge_start = current,
            }
        }
    }
}

// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
    let mut base = p.as_ptr().cast::<u8>();
    while size >= MI_HUGE_OS_PAGE_SIZE {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe {
            mi_os_prim_free_range(
                NonNull::new(base.cast::<c_void>()),
                ReservedBytes::new(MI_HUGE_OS_PAGE_SIZE),
                CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE),
                subproc,
            )
        };
        size -= MI_HUGE_OS_PAGE_SIZE;
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        base = unsafe { base.add(MI_HUGE_OS_PAGE_SIZE) };
    }
}

/// # Safety
/// Reads process-wide NUMA configuration and may initialize its cached value.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_numa_node_count() -> i32 {
    let mut count = MI_NUMA_NODE_COUNT.load(Ordering::Acquire);
    if count == 0 {
        let ncount = mi_option_get(mi_option_t::mi_option_use_numa_nodes);
        count = if ncount > 0 && ncount < i32::MAX as _ {
            ncount as usize
        } else {
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
unsafe fn mi_os_numa_node_get() -> i32 {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let numa_count = unsafe { _mi_os_numa_node_count() };
    if numa_count <= 1 {
        return 0;
    }
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let n = unsafe { _mi_prim_numa_node() };
    let mut numa_node = if n < i32::MAX as usize { n as i32 } else { 0 };
    if numa_node >= numa_count {
        numa_node %= numa_count;
    }
    numa_node
}

/// # Safety
/// Reads process-wide NUMA configuration and may query the OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_numa_node() -> i32 {
    if MI_NUMA_NODE_COUNT.load(Ordering::Relaxed) == 1 {
        0
    } else {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe { mi_os_numa_node_get() }
    }
}

/// # Safety
/// Reads process-wide NUMA configuration and may query the OS primitive layer.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[inline]
pub(crate) unsafe fn _mi_os_numa_node_typed() -> crate::core_types::NumaNode {
    unsafe { crate::core_types::NumaNode::new(_mi_os_numa_node()) }
}

/// # Safety
/// Any non-null output pointers must be writable. The returned memory must be released through
/// the matching OS huge-page free path.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
pub unsafe extern "C" fn _mi_os_alloc_huge_os_pages(
    pages: usize,
    numa_node: i32,
    max_msecs: i64,
    pages_reserved: *mut usize,
    psize: *mut usize,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
        let mut size = 0usize;
        let start = mi_os_claim_huge_pages(pages, &mut size);
        if start.is_null() {
            return ptr::null_mut();
        }

        let start_t = _mi_clock_start();
        let mut page = 0usize;
        let mut all_zero = true;
        while page < pages {
            let addr = start.add(page * MI_HUGE_OS_PAGE_SIZE).cast::<c_void>();
            let mut is_zero = false;
            let mut p: *mut c_void = ptr::null_mut();
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
                    mi_os_prim_free_range(
                        NonNull::new(p),
                        ReservedBytes::new(MI_HUGE_OS_PAGE_SIZE),
                        CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE),
                        None,
                    );
                }
                break;
            }

            page += 1;
            let subproc = _mi_subproc();
            if !subproc.is_null() {
                let stats = &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats;
                let committed = CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE);
                let reserved = ReservedBytes::new(MI_HUGE_OS_PAGE_SIZE);
                __mi_stat_increase_mt(ptr::addr_of_mut!(stats.committed), committed.get());
                __mi_stat_increase_mt(ptr::addr_of_mut!(stats.reserved), reserved.get());
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
        crate::internal::mi_assert_internal!(allocated <= size);
        if !pages_reserved.is_null() {
            *pages_reserved = page;
        }
        if !psize.is_null() {
            *psize = allocated;
        }
        if page != 0 {
            crate::internal::mi_assert_internal!(!start.is_null());
            if !memid.is_null() {
                *memid = crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(
                    crate::core_types::OsMemPtr::new(start.cast::<c_void>()),
                    crate::core_types::ReservedBytes::new(allocated),
                    crate::core_types::InitiallyCommitted(true),
                    crate::core_types::InitiallyZero(all_zero),
                    crate::core_types::Pinned(true),
                )
                .into_raw();
                crate::internal::mi_assert!((*memid).is_pinned);
            }
        }
        if page == 0 {
            ptr::null_mut()
        } else {
            start.cast::<c_void>()
        }
    }
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    let (p, memid) = unsafe { mi_os_alloc_aligned_memid(size, alignment, commit, allow_large) };
    if p.is_null() {
        return p;
    }
    if !is_committed.is_null() {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe { *is_committed = memid.initially_committed };
    }
    if !is_pinned.is_null() {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
        unsafe { *is_pinned = memid.is_pinned };
    }
    if !base.is_null()
        || !full_size.is_null()
        || (memid.initially_committed && !memid.initially_zero)
    {
        // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
            unsafe { *base = os_base };
        }
        if !full_size.is_null() {
            unsafe { *full_size = os_size };
        }
        if memid.initially_committed && !memid.initially_zero {
            // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
            unsafe { _mi_memzero_aligned(os_base, os_size) };
        }
    }
    p
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_commit(p: *mut c_void, size: usize) {
    let _ = unsafe { _mi_os_commit(p, size, core::ptr::null_mut()) };
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    base: *mut *mut c_void,
    full_size: *mut usize,
) -> *mut c_void {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_alloc(
    size: usize,
    commit: bool,
    full_size: *mut usize,
) -> *mut c_void {
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
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
    // SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
    unsafe { _mi_os_free(p, size, memid) };
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
// SAFETY: OS memory callers preserve memid, page-size, alignment, and out-pointer contracts.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_os_decommit(p: *mut c_void, size: usize) {
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
