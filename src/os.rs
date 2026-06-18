//! Primitive OS / platform layer

#![allow(non_camel_case_types, non_upper_case_globals, unsafe_op_in_unsafe_fn)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::atomic::mi_lock_t;
use crate::bits::{MI_GiB, MI_INTPTR_SIZE, MI_KiB, MI_MAX_VABITS, MI_MiB};
use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_decrease, __mi_stat_increase, _mi_align_down_ptr,
    _mi_align_up, _mi_align_up_ptr, _mi_is_power_of_two, _mi_memid_create_os, _mi_memid_none,
    _mi_memzero_aligned, _mi_preloading, _mi_subproc, MI_DEBUG, MI_SECURE, MI_TRACK_ENABLED,
    mi_commit_fun_t, mi_memid_t, mi_theap_is_initialized,
};
use crate::options::{
    _mi_verbose_message, _mi_warning_message_fmt, mi_option_get, mi_option_get_size,
    mi_option_is_enabled, mi_option_t,
};
use crate::prim::mi_os_mem_config_t;
use crate::prim::{
    _mi_prim_alloc, _mi_prim_alloc_huge_os_pages, _mi_prim_commit, _mi_prim_decommit,
    _mi_prim_free, _mi_prim_mem_init, _mi_prim_numa_node, _mi_prim_numa_node_count,
    _mi_prim_protect, _mi_prim_reset, _mi_prim_reuse, _mi_theap_default,
};
use crate::stats::{_mi_clock_end, _mi_clock_start, mi_stats_t};
use crate::types::{mi_arena_t, mi_heap_t, mi_page_t, mi_subproc_t};

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

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_has_overcommit() -> bool {
    unsafe { (*mi_os_mem_config_ptr()).has_overcommit }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_has_virtual_reserve() -> bool {
    unsafe { (*mi_os_mem_config_ptr()).has_virtual_reserve }
}

#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_page_size() -> usize {
    unsafe { (*mi_os_mem_config_ptr()).page_size }
}

/// # Safety
/// The OS memory configuration must have been initialized before this is called concurrently.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_large_page_size() -> usize {
    let large = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    if large != 0 {
        large
    } else {
        unsafe { _mi_os_page_size() }
    }
}

/// # Safety
/// The OS memory configuration and options state must be initialized.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_minimal_purge_size() -> usize {
    let minsize = mi_option_get_size(mi_option_t::mi_option_minimal_purge_size);
    if minsize != 0 {
        _mi_align_up(minsize, unsafe { _mi_os_page_size() })
    } else if unsafe { (*mi_os_mem_config_ptr()).has_transparent_huge_pages }
        && mi_option_get(mi_option_t::mi_option_allow_thp) == 2
    {
        unsafe { _mi_os_large_page_size() }
    } else {
        unsafe { _mi_os_page_size() }
    }
}

/// # Safety
/// The OS memory configuration must have been initialized.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_guard_page_size() -> usize {
    let gsize = unsafe { _mi_os_page_size() };
    crate::mi_debug_assert!(gsize <= (MI_ARENA_SLICE_SIZE / 4));
    gsize
}

/// # Safety
/// The OS memory configuration must have been initialized.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_virtual_address_bits() -> usize {
    let vbits = unsafe { (*mi_os_mem_config_ptr()).virtual_address_bits };
    crate::mi_debug_assert!(vbits <= MI_MAX_VABITS);
    vbits
}

/// # Safety
/// The OS memory configuration must have been initialized.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_canuse_large_page(size: usize, alignment: usize) -> bool {
    let large_page_size = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    large_page_size != 0
        && size.is_multiple_of(large_page_size)
        && alignment.is_multiple_of(large_page_size)
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_good_alloc_size(size: usize) -> usize {
    let align_size = if size < 512 * MI_KiB {
        unsafe { _mi_os_page_size() }
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
#[inline(never)]
pub unsafe extern "C" fn _mi_os_init() {
    unsafe { _mi_prim_mem_init(core::ptr::addr_of_mut!(mi_os_mem_config)) };
}

/// # Safety
/// The OS memory configuration must be initialized; secure/release randomized hints also require
/// an initialized default theap.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_get_aligned_hint(try_alignment: usize, size: usize) -> *mut c_void {
    if MI_INTPTR_SIZE < 8 || try_alignment <= unsafe { (*mi_os_mem_config_ptr()).alloc_granularity }
    {
        return core::ptr::null_mut();
    }
    if try_alignment > MI_HINT_ALIGN
        || unsafe { (*mi_os_mem_config_ptr()).virtual_address_bits } < 46
    {
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
            let theap = unsafe { _mi_theap_default() };
            if unsafe { !mi_theap_is_initialized(theap) } {
                return core::ptr::null_mut();
            }
            let r = unsafe { crate::theap::_mi_theap_random_next(theap) };
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

#[inline]
fn mi_os_page_align_areax(
    conservative: bool,
    addr: *mut c_void,
    size: usize,
    newsize: *mut usize,
) -> *mut c_void {
    crate::mi_assert!(!addr.is_null() && size > 0);
    if !newsize.is_null() {
        unsafe { *newsize = 0 };
    }
    if addr.is_null() || size == 0 {
        return core::ptr::null_mut();
    }

    let page_size = unsafe { _mi_os_page_size() };
    let start = if conservative {
        _mi_align_up_ptr(addr.cast_const(), page_size)
    } else {
        _mi_align_down_ptr(addr.cast_const(), page_size)
    };
    let end_addr = unsafe { addr.cast::<u8>().add(size) };
    let end = if conservative {
        _mi_align_down_ptr(end_addr.cast::<c_void>().cast_const(), page_size)
    } else {
        _mi_align_up_ptr(end_addr.cast::<c_void>().cast_const(), page_size)
    };

    let diff = (end as usize).wrapping_sub(start as usize);
    if end as usize <= start as usize {
        return core::ptr::null_mut();
    }

    crate::mi_assert_internal!((conservative && diff <= size) || (!conservative && diff >= size));
    if !newsize.is_null() {
        unsafe { *newsize = diff };
    }
    start
}

#[inline]
fn mi_os_page_align_area_conservative(
    addr: *mut c_void,
    size: usize,
    newsize: *mut usize,
) -> *mut c_void {
    mi_os_page_align_areax(true, addr, size, newsize)
}

/// # Safety
/// The OS memory configuration must have been initialized.
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
            crate::core_types::MemIdView::Arena(a) => a.arena.as_ptr(),
            _ => core::ptr::null_mut(),
        };
        if !arena.is_null() {
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
#[inline(never)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_set_before(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
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
            crate::core_types::MemIdView::Arena(a) => a.arena.as_ptr(),
            _ => core::ptr::null_mut(),
        };
        if !arena.is_null() {
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
        return unsafe {
            _mi_os_commit(addr, _mi_os_secure_guard_page_size(), core::ptr::null_mut())
        };
    }
    true
}

/// # Safety
/// `addr` must be at least one secure guard page past the page to reset, and `memid` must
/// describe that allocation.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_secure_guard_page_reset_before(
    addr: *mut c_void,
    memid: mi_memid_t,
) -> bool {
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
pub(crate) unsafe extern "C" fn _mi_os_commit_ex(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
    stat_size: usize,
) -> bool {
    if !is_zero.is_null() {
        unsafe { *is_zero = false };
    }
    let subproc = unsafe { _mi_subproc() };
    if !subproc.is_null() {
        let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
        __mi_stat_counter_increase(ptr::addr_of_mut!(stats.commit_calls), 1);
    }

    let mut csize = 0;
    let start = mi_os_page_align_areax(false, addr, size, &mut csize);
    if csize == 0 {
        return true;
    }

    let mut os_is_zero = false;
    let err = unsafe { _mi_prim_commit(start, csize, &mut os_is_zero) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "cannot commit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            err, err, start, csize
        ));
        return false;
    }
    if os_is_zero && !is_zero.is_null() {
        unsafe { *is_zero = true };
    }
    if !subproc.is_null() {
        let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
        __mi_stat_increase(ptr::addr_of_mut!(stats.committed), stat_size);
    }
    true
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> bool {
    unsafe { _mi_os_commit_ex(addr, size, is_zero, size) }
}

#[inline]
unsafe fn mi_os_decommit_ex(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
    stat_size: usize,
) -> bool {
    crate::mi_assert_internal!(!needs_recommit.is_null());

    let mut csize = 0;
    let start = mi_os_page_align_area_conservative(addr, size, &mut csize);
    if csize == 0 {
        return true;
    }

    *needs_recommit = true;
    let err = unsafe { _mi_prim_decommit(start, csize, needs_recommit) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "cannot decommit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
            err, err, start, csize
        ));
    } else if *needs_recommit {
        let subproc = unsafe { _mi_subproc() };
        if !subproc.is_null() {
            let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
            __mi_stat_decrease(ptr::addr_of_mut!(stats.committed), stat_size);
        }
    }
    crate::mi_assert_internal!(err == 0);
    err == 0
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_decommit(addr: *mut c_void, size: usize) -> bool {
    let mut needs_recommit = false;
    unsafe { mi_os_decommit_ex(addr, size, &mut needs_recommit, size) }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
pub unsafe extern "C" fn _mi_os_reset(addr: *mut c_void, size: usize) -> bool {
    let mut csize = 0;
    let start = mi_os_page_align_area_conservative(addr, size, &mut csize);
    if csize == 0 {
        return true;
    }

    let subproc = unsafe { _mi_subproc() };
    if !subproc.is_null() {
        let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
        __mi_stat_counter_increase(ptr::addr_of_mut!(stats.reset), csize);
        __mi_stat_counter_increase(ptr::addr_of_mut!(stats.reset_calls), 1);
    }

    if MI_DEBUG > 1 && MI_SECURE == 0 && MI_TRACK_ENABLED == 0 {
        unsafe { core::ptr::write_bytes(start, 0, csize) };
    }

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
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_reuse(addr: *mut c_void, size: usize) {
    let mut csize = 0;
    let start = mi_os_page_align_area_conservative(addr, size, &mut csize);
    if csize == 0 {
        return;
    }
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
    let subproc = unsafe { _mi_subproc() };
    if !subproc.is_null() {
        let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
        __mi_stat_counter_increase(ptr::addr_of_mut!(stats.purge_calls), 1);
        __mi_stat_counter_increase(ptr::addr_of_mut!(stats.purged), size);
    }

    if let Some(commit_fun) = commit_fun {
        unsafe { commit_fun(false, p, size, core::ptr::null_mut(), commit_fun_arg) }
    } else if mi_option_is_enabled(mi_option_t::mi_option_purge_decommits) && !_mi_preloading() {
        let mut needs_recommit = true;
        unsafe { mi_os_decommit_ex(p, size, &mut needs_recommit, stat_size) };
        needs_recommit
    } else {
        if allow_reset {
            unsafe { _mi_os_reset(p, size) };
        }
        false
    }
}

/// # Safety
/// `p..p+size` must designate a valid virtual memory range for the active OS primitive layer.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_purge(p: *mut c_void, size: usize) -> bool {
    unsafe { _mi_os_purge_ex(p, size, true, size, None, core::ptr::null_mut()) }
}

#[inline]
unsafe fn mi_os_protectx(addr: *mut c_void, size: usize, protect: bool) -> bool {
    let mut csize = 0;
    let start = mi_os_page_align_area_conservative(addr, size, &mut csize);
    if csize == 0 {
        return false;
    }
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
#[inline(never)]
pub unsafe extern "C" fn _mi_os_protect(addr: *mut c_void, size: usize) -> bool {
    unsafe { mi_os_protectx(addr, size, true) }
}

/// # Safety
/// `addr..addr+size` must designate a valid virtual memory range for the active OS primitive layer.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_unprotect(addr: *mut c_void, size: usize) -> bool {
    unsafe { mi_os_protectx(addr, size, false) }
}

#[inline]
unsafe fn mi_os_prim_free(
    addr: *mut c_void,
    size: usize,
    commit_size: usize,
    subproc: *mut mi_subproc_t,
) {
    crate::mi_assert_internal!(size.is_multiple_of(unsafe { _mi_os_page_size() }));
    if addr.is_null() {
        return;
    }
    let err = unsafe { _mi_prim_free(addr, size) };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "unable to free OS memory (error: {} (0x{:x}), size: 0x{:x} bytes, address: {:p})\n",
            err, err, size, addr
        ));
    }
    let subproc = if subproc.is_null() {
        unsafe { crate::internal::_mi_subproc() }
    } else {
        subproc
    };
    if !subproc.is_null() {
        let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
        if commit_size > 0 {
            __mi_stat_decrease(ptr::addr_of_mut!(stats.committed), commit_size);
        }
        __mi_stat_decrease(ptr::addr_of_mut!(stats.reserved), size);
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_free_ex(
    addr: *mut c_void,
    size: usize,
    still_committed: bool,
    memid: mi_memid_t,
    subproc: *mut mi_subproc_t,
) {
    let crate::core_types::MemIdView::Os(os) = crate::core_types::MemId::view(memid) else {
        return;
    };
    let (mut csize, base) = (
        os.size.0,
        os.base.map(|p| p.as_ptr()).unwrap_or(core::ptr::null_mut()),
    );
    if csize == 0 {
        csize = unsafe { _mi_os_good_alloc_size(size) };
    }
    crate::mi_assert_internal!(csize >= size);

    let commit_size = if still_committed { csize } else { 0 };
    if base != addr {
        crate::mi_assert_internal!(base <= addr);
        let diff = addr as usize - base as usize;
        if csize == 0 {
            csize += diff;
        }
        if still_committed {
            // commit_size -= diff; // Handled below by subtracting from final argument instead to match original code structure correctly
        }
    }
    unsafe {
        mi_os_prim_free(
            base,
            csize,
            if still_committed {
                commit_size - (addr as usize - base as usize)
            } else {
                0
            },
            subproc,
        )
    };
}

#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_free(p: *mut c_void, size: usize, memid: mi_memid_t) {
    unsafe { _mi_os_free_ex(p, size, true, memid, core::ptr::null_mut()) };
}

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
    crate::mi_assert_internal!(size > 0 && size.is_multiple_of(unsafe { _mi_os_page_size() }));
    crate::mi_assert_internal!(!is_zero.is_null());
    crate::mi_assert_internal!(!is_large.is_null());
    if size == 0 {
        return core::ptr::null_mut();
    }
    if !commit {
        allow_large = false;
    }
    if try_alignment == 0 {
        try_alignment = 1;
    }

    let large_page_size = unsafe { (*mi_os_mem_config_ptr()).large_page_size };
    if large_page_size > 0
        && hint_addr.is_null()
        && size >= 8 * large_page_size
        && _mi_is_power_of_two(try_alignment)
        && try_alignment < large_page_size
    {
        try_alignment = large_page_size;
    }

    unsafe {
        *is_zero = false;
    }
    let mut p = core::ptr::null_mut();
    let err = unsafe {
        _mi_prim_alloc(
            hint_addr,
            size,
            try_alignment,
            commit,
            allow_large,
            is_large,
            is_zero,
            &mut p,
        )
    };
    if err != 0 {
        _mi_warning_message_fmt(format_args!(
            "unable to allocate OS memory (error: {} (0x{:x}), addr: {:p}, size: 0x{:x} bytes, align: 0x{:x}, commit: {}, allow large: {})\n",
            err, err, hint_addr, size, try_alignment, commit as i32, allow_large as i32
        ));
    }

    let subproc = unsafe { _mi_subproc() };
    if !subproc.is_null() {
        let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
        __mi_stat_counter_increase(ptr::addr_of_mut!(stats.mmap_calls), 1);
        if !p.is_null() {
            __mi_stat_increase(ptr::addr_of_mut!(stats.reserved), size);
            if commit {
                __mi_stat_increase(ptr::addr_of_mut!(stats.committed), size);
            }
        }
    }
    p
}

#[inline]
unsafe fn mi_os_prim_alloc(
    size: usize,
    try_alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
) -> *mut c_void {
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

unsafe fn mi_os_alloc_aligned_memid(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
) -> (*mut c_void, mi_memid_t) {
    let mut memid = unsafe { _mi_memid_none() };
    if size == 0 {
        return (core::ptr::null_mut(), memid);
    }

    let size = unsafe { _mi_os_good_alloc_size(size) };
    let alignment = unsafe { _mi_align_up(alignment, _mi_os_page_size()) };
    if alignment < unsafe { _mi_os_page_size() } || (alignment & (alignment - 1)) != 0 {
        return (core::ptr::null_mut(), memid);
    }

    let try_direct_alloc = unsafe {
        alignment <= (*mi_os_mem_config_ptr()).alloc_granularity || alignment <= size / 4
    };

    let mut os_is_large = false;
    let mut os_is_zero = false;
    let os_base: *mut c_void;
    let mut os_size = size;
    let mut p: *mut c_void = core::ptr::null_mut();

    if try_direct_alloc {
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
            let _ = unsafe { _mi_prim_free(p, size) };
        }
        if size >= usize::MAX - alignment {
            return (core::ptr::null_mut(), memid);
        }

        let over_size = size + alignment;
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

            if commit && !unsafe { _mi_os_commit(p, size, core::ptr::null_mut()) } {
                let _ = unsafe { _mi_prim_free(os_base, over_size) };
                return (core::ptr::null_mut(), memid);
            }
        } else {
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
            let mid_size = unsafe { _mi_align_up(size, _mi_os_page_size()) };
            let post_size = over_size - pre_size - mid_size;

            if pre_size > 0 {
                let _ = unsafe { _mi_prim_free(p, pre_size) };
            }
            if post_size > 0 {
                let post = unsafe { aligned_p.cast::<u8>().add(mid_size) }.cast::<c_void>();
                let _ = unsafe { _mi_prim_free(post, post_size) };
            }

            p = aligned_p;
            os_base = aligned_p;
            os_size = mid_size;
        }
    }

    if p.is_null() || os_base.is_null() {
        return (core::ptr::null_mut(), memid);
    }

    memid = unsafe { _mi_memid_create_os(os_base, os_size, commit, os_is_zero, os_is_large) };
    (p, memid)
}

#[inline]
unsafe fn mi_os_ensure_zero(p: *mut c_void, size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    if p.is_null() || size == 0 {
        return p;
    }
    if !unsafe { (*memid).initially_committed } {
        let mut is_zero = false;
        if !unsafe { _mi_os_commit(p, size, &mut is_zero) } {
            unsafe { _mi_os_free(p, size, *memid) };
            return core::ptr::null_mut();
        }
        unsafe { (*memid).initially_committed = true };
    }
    if unsafe { (*memid).initially_zero } {
        return p;
    }
    unsafe { _mi_memzero_aligned(p, size) };
    unsafe { (*memid).initially_zero = true };
    p
}

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

    let size = unsafe { _mi_os_good_alloc_size(size) };
    let mut os_is_large = false;
    let mut os_is_zero = false;
    let p = unsafe { mi_os_prim_alloc(size, 0, true, false, &mut os_is_large, &mut os_is_zero) };
    if p.is_null() {
        return core::ptr::null_mut();
    }

    unsafe {
        *memid = _mi_memid_create_os(p, size, true, os_is_zero, os_is_large);
    }
    p
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_os_zalloc(size: usize, memid: *mut mi_memid_t) -> *mut c_void {
    let p = unsafe { _mi_os_alloc(size, memid) };
    unsafe { mi_os_ensure_zero(p, size, memid) }
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

    let (p, new_memid) = unsafe { mi_os_alloc_aligned_memid(size, alignment, commit, allow_large) };
    if p.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        *memid = new_memid;
    }
    p
}

#[used]
static MI_OS_PAGE_SIZE_KEEP: unsafe extern "C" fn() -> usize = _mi_os_page_size;
#[used]
static MI_OS_GUARD_PAGE_SIZE_KEEP: unsafe extern "C" fn() -> usize = _mi_os_guard_page_size;
#[used]
static MI_OS_SECURE_GUARD_PAGE_SIZE_KEEP: unsafe extern "C" fn() -> usize =
    _mi_os_secure_guard_page_size;
#[used]
static MI_OS_ALLOC_KEEP: unsafe extern "C" fn(usize, *mut mi_memid_t) -> *mut c_void = _mi_os_alloc;
#[used]
static MI_OS_FREE_KEEP: unsafe extern "C" fn(*mut c_void, usize, mi_memid_t) = _mi_os_free;
#[used]
static MI_OS_REUSE_KEEP: unsafe extern "C" fn(*mut c_void, usize) = _mi_os_reuse;
#[used]
static MI_OS_PURGE_KEEP: unsafe extern "C" fn(*mut c_void, usize) -> bool = _mi_os_purge;
#[used]
static MI_OS_PROTECT_KEEP: unsafe extern "C" fn(*mut c_void, usize) -> bool = _mi_os_protect;
#[used]
static MI_OS_UNPROTECT_KEEP: unsafe extern "C" fn(*mut c_void, usize) -> bool = _mi_os_unprotect;
#[used]
static MI_OS_SECURE_GUARD_PAGE_SET_AT_KEEP: unsafe extern "C" fn(*mut c_void, mi_memid_t) -> bool =
    _mi_os_secure_guard_page_set_at;
#[used]
static MI_OS_SECURE_GUARD_PAGE_SET_BEFORE_KEEP: unsafe extern "C" fn(
    *mut c_void,
    mi_memid_t,
) -> bool = _mi_os_secure_guard_page_set_before;
#[used]
static MI_OS_SECURE_GUARD_PAGE_RESET_AT_KEEP: unsafe extern "C" fn(
    *mut c_void,
    mi_memid_t,
) -> bool = _mi_os_secure_guard_page_reset_at;
#[used]
static MI_OS_SECURE_GUARD_PAGE_RESET_BEFORE_KEEP: unsafe extern "C" fn(
    *mut c_void,
    mi_memid_t,
) -> bool = _mi_os_secure_guard_page_reset_before;

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
    if memid.is_null() || offset > size || !alignment.is_multiple_of(unsafe { _mi_os_page_size() })
    {
        return core::ptr::null_mut();
    }
    if offset == 0 {
        return unsafe { _mi_os_alloc_aligned(size, alignment, commit, allow_large, memid) };
    }

    let extra = _mi_align_up(offset, alignment) - offset;
    if size >= usize::MAX - extra {
        return core::ptr::null_mut();
    }
    let oversize = size + extra;
    let start = unsafe { _mi_os_alloc_aligned(oversize, alignment, commit, allow_large, memid) };
    if start.is_null() {
        return core::ptr::null_mut();
    }

    let p = unsafe { start.cast::<u8>().add(extra) }.cast::<c_void>();
    crate::mi_assert!(unsafe {
        crate::internal::_mi_is_aligned(p.cast::<u8>().add(offset).cast(), alignment)
    });
    if commit && extra >= unsafe { _mi_os_page_size() } {
        let mut dsize = 0usize;
        let dstart = mi_os_page_align_areax(false, start, extra, &mut dsize);
        if !dstart.is_null() && dsize > 0 {
            let mut needs_recommit = false;
            let err = unsafe { _mi_prim_decommit(dstart, dsize, &mut needs_recommit) };
            if err != 0 {
                _mi_warning_message_fmt(format_args!(
                    "unable to decommit OS memory (error: {} (0x{:x}), address: {:p}, size: 0x{:x} bytes)\n",
                    err, err, dstart, dsize
                ));
            }
        }
    }
    p
}

#[allow(dead_code)]
#[inline]
unsafe fn mi_os_claim_huge_pages(pages: usize, total_size: *mut usize) -> *mut u8 {
    #[cfg(target_pointer_width = "32")]
    let _ = pages;
    if !total_size.is_null() {
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
                        unsafe { *total_size = size };
                    }
                    return start as *mut u8;
                }
                Err(current) => huge_start = current,
            }
        }
    }
}

#[inline]
#[allow(dead_code)]
unsafe fn mi_os_free_huge_os_pages(p: *mut c_void, mut size: usize, subproc: *mut mi_subproc_t) {
    if p.is_null() || size == 0 {
        return;
    }
    let mut base = p.cast::<u8>();
    while size >= MI_HUGE_OS_PAGE_SIZE {
        unsafe {
            mi_os_prim_free(
                base.cast::<c_void>(),
                MI_HUGE_OS_PAGE_SIZE,
                MI_HUGE_OS_PAGE_SIZE,
                subproc,
            )
        };
        size -= MI_HUGE_OS_PAGE_SIZE;
        base = unsafe { base.add(MI_HUGE_OS_PAGE_SIZE) };
    }
}

/// # Safety
/// Reads process-wide NUMA configuration and may initialize its cached value.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_numa_node_count() -> i32 {
    let mut count = MI_NUMA_NODE_COUNT.load(Ordering::Acquire);
    if count == 0 {
        let ncount = mi_option_get(mi_option_t::mi_option_use_numa_nodes);
        count = if ncount > 0 && ncount < i32::MAX as _ {
            ncount as usize
        } else {
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
    crate::mi_assert_internal!(count > 0 && count <= i32::MAX as usize);
    count as i32
}

#[used]
static MI_OS_NUMA_NODE_COUNT_KEEP: unsafe extern "C" fn() -> i32 = _mi_os_numa_node_count;

#[inline]
unsafe fn mi_os_numa_node_get() -> i32 {
    let numa_count = unsafe { _mi_os_numa_node_count() };
    if numa_count <= 1 {
        return 0;
    }
    let n = unsafe { _mi_prim_numa_node() };
    let mut numa_node = if n < i32::MAX as usize { n as i32 } else { 0 };
    if numa_node >= numa_count {
        numa_node %= numa_count;
    }
    numa_node
}

/// # Safety
/// Reads process-wide NUMA configuration and may query the OS primitive layer.
#[inline(never)]
pub unsafe extern "C" fn _mi_os_numa_node() -> i32 {
    if MI_NUMA_NODE_COUNT.load(Ordering::Relaxed) == 1 {
        0
    } else {
        unsafe { mi_os_numa_node_get() }
    }
}

/// # Safety
/// Any non-null output pointers must be writable. The returned memory must be released through
/// the matching OS huge-page free path.
pub unsafe extern "C" fn _mi_os_alloc_huge_os_pages(
    pages: usize,
    numa_node: i32,
    max_msecs: i64,
    pages_reserved: *mut usize,
    psize: *mut usize,
    memid: *mut mi_memid_t,
) -> *mut c_void {
    if !memid.is_null() {
        unsafe { *memid = _mi_memid_none() };
    }
    if !psize.is_null() {
        unsafe { *psize = 0 };
    }
    if !pages_reserved.is_null() {
        unsafe { *pages_reserved = 0 };
    }
    let mut size = 0usize;
    let start = unsafe { mi_os_claim_huge_pages(pages, &mut size) };
    if start.is_null() {
        return ptr::null_mut();
    }

    let start_t = _mi_clock_start();
    let mut page = 0usize;
    let mut all_zero = true;
    while page < pages {
        let addr = unsafe { start.add(page * MI_HUGE_OS_PAGE_SIZE) }.cast::<c_void>();
        let mut is_zero = false;
        let mut p: *mut c_void = ptr::null_mut();
        let err = unsafe {
            _mi_prim_alloc_huge_os_pages(
                addr,
                MI_HUGE_OS_PAGE_SIZE,
                numa_node,
                &mut is_zero,
                &mut p,
            )
        };
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
                unsafe {
                    mi_os_prim_free(
                        p,
                        MI_HUGE_OS_PAGE_SIZE,
                        MI_HUGE_OS_PAGE_SIZE,
                        ptr::null_mut(),
                    )
                };
            }
            break;
        }

        page += 1;
        let subproc = unsafe { _mi_subproc() };
        if !subproc.is_null() {
            let stats = unsafe { &mut (*subproc.cast::<MiSubprocStatsPrefix>()).stats };
            __mi_stat_increase(ptr::addr_of_mut!(stats.committed), MI_HUGE_OS_PAGE_SIZE);
            __mi_stat_increase(ptr::addr_of_mut!(stats.reserved), MI_HUGE_OS_PAGE_SIZE);
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
    crate::mi_assert_internal!(allocated <= size);
    if !pages_reserved.is_null() {
        unsafe { *pages_reserved = page };
    }
    if !psize.is_null() {
        unsafe { *psize = allocated };
    }
    if page != 0 {
        crate::mi_assert_internal!(!start.is_null());
        if !memid.is_null() {
            unsafe {
                *memid = crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(
                    crate::core_types::OsMemPtr::new(start.cast::<c_void>()),
                    crate::core_types::ReservedBytes(allocated),
                    crate::core_types::InitiallyCommitted(true),
                    crate::core_types::InitiallyZero(all_zero),
                    crate::core_types::Pinned(true),
                )
                .raw;
            }
            crate::mi_assert!((unsafe { (*memid).is_pinned }));
        }
    }
    if page == 0 {
        ptr::null_mut()
    } else {
        start.cast::<c_void>()
    }
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
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
    let (p, memid) = unsafe { mi_os_alloc_aligned_memid(size, alignment, commit, allow_large) };
    if p.is_null() {
        return p;
    }
    if !is_committed.is_null() {
        unsafe { *is_committed = memid.initially_committed };
    }
    if !is_pinned.is_null() {
        unsafe { *is_pinned = memid.is_pinned };
    }
    if !base.is_null()
        || !full_size.is_null()
        || (memid.initially_committed && !memid.initially_zero)
    {
        let (os_base, os_size) = match crate::core_types::MemId::view(memid) {
            crate::core_types::MemIdView::Os(os) => (
                os.base.map(|p| p.as_ptr()).unwrap_or(core::ptr::null_mut()),
                os.size.0,
            ),
            crate::core_types::MemIdView::External(external) => (
                external
                    .base
                    .map(|p| p.as_ptr())
                    .unwrap_or(core::ptr::null_mut()),
                external.size.0,
            ),
            _ => (core::ptr::null_mut(), 0),
        };
        if !base.is_null() {
            unsafe { *base = os_base };
        }
        if !full_size.is_null() {
            unsafe { *full_size = os_size };
        }
        if memid.initially_committed && !memid.initially_zero {
            unsafe { _mi_memzero_aligned(os_base, os_size) };
        }
    }
    p
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_os_commit(p: *mut c_void, size: usize) {
    let _ = unsafe { _mi_os_commit(p, size, core::ptr::null_mut()) };
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_os_free(p: *mut c_void, size: usize) {
    if p.is_null() || size == 0 {
        return;
    }
    let memid = unsafe { _mi_memid_create_os(p, size, true, false, false) };
    unsafe { _mi_os_free(p, size, memid) };
}

/// # Safety
/// Preserves the exported C ABI wrapper from `src/os.c`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_os_decommit(p: *mut c_void, size: usize) {
    let _ = unsafe { _mi_os_decommit(p, size) };
}
