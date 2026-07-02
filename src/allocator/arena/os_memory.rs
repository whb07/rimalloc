//! Arena reserve, managed OS-memory, and huge-page reservation helpers.
//!
//! The parent arena module keeps allocation/search hot paths; this module owns
//! reserve growth, public managed-memory entrypoints, and huge-page reservation.

use super::*;

pub(super) unsafe fn mi_arena_reserve(
    subproc: SubprocPtr,
    req_size: crate::core_types::ReservedBytes,
    numa_node: crate::core_types::NumaNode,
    allow_large: bool,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    let req_size = req_size.get();
    // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    if arena_count > (MI_MAX_ARENAS - 4) {
        return false;
    }

    let mut arena_reserve = mi_option_get_size(mi_option_t::mi_option_arena_reserve);
    if arena_reserve == 0 {
        return false;
    }

    // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
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

    // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
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
        // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
        unsafe {
            __mi_stat_adjust_decrease_mt(
                ptr::addr_of_mut!((*subproc).stats.committed),
                crate::core_types::CommittedBytes::new(arena_reserve).get(),
            );
        }
    }

    // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
    let mut err = unsafe {
        mi_reserve_os_memory_ex2(
            subproc_ptr,
            crate::core_types::ReservedBytes::new(arena_reserve),
            numa_node,
            crate::core_types::InitiallyCommitted(arena_commit),
            crate::core_types::AllowPinned::new(allow_large),
            crate::core_types::Exclusive::new(false),
            arena_id,
        )
    };
    if err != 0 {
        if adjust {
            // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
            unsafe {
                __mi_stat_adjust_increase_mt(
                    ptr::addr_of_mut!((*subproc).stats.committed),
                    crate::core_types::CommittedBytes::new(arena_reserve).get(),
                );
            }
        }
        let small_arena_reserve = 4 * MI_ARENA_MIN_SIZE;
        if arena_reserve > small_arena_reserve && small_arena_reserve > req_size {
            if adjust {
                // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
                unsafe {
                    __mi_stat_adjust_decrease_mt(
                        ptr::addr_of_mut!((*subproc).stats.committed),
                        crate::core_types::CommittedBytes::new(small_arena_reserve).get(),
                    );
                }
            }
            // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
            err = unsafe {
                mi_reserve_os_memory_ex2(
                    subproc_ptr,
                    crate::core_types::ReservedBytes::new(small_arena_reserve),
                    numa_node,
                    crate::core_types::InitiallyCommitted(arena_commit),
                    crate::core_types::AllowPinned::new(allow_large),
                    crate::core_types::Exclusive::new(false),
                    arena_id,
                )
            };
            if err != 0 && adjust {
                // SAFETY[AUDIT:arena-reserve-growth-boundary]: arena reserve growth computes bounded reserve sizes, lock-protects growth, and balances commit accounting across retries.
                unsafe {
                    __mi_stat_adjust_increase_mt(
                        ptr::addr_of_mut!((*subproc).stats.committed),
                        crate::core_types::CommittedBytes::new(small_arena_reserve).get(),
                    );
                }
            }
        }
    }
    err == 0
}

unsafe fn mi_reserve_os_memory_ex2(
    subproc: SubprocPtr,
    size: crate::core_types::ReservedBytes,
    numa_node: crate::core_types::NumaNode,
    commit: crate::core_types::InitiallyCommitted,
    allow_large: crate::core_types::AllowPinned,
    exclusive: crate::core_types::Exclusive,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    let subproc = subproc.as_ptr();
    let commit = commit.0;
    let allow_large = allow_large.get();
    let exclusive = exclusive.get();
    if !arena_id.is_null() {
        // SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    let size = _mi_align_up(size.get(), MI_ARENA_SLICE_SIZE);
    let mut memid = unsafe { _mi_memid_none() };
    // SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
    let start = unsafe {
        _mi_os_alloc_aligned(size, MI_ARENA_SLICE_ALIGN, commit, allow_large, &mut memid)
    };
    if start.is_null() {
        return ENOMEM;
    }

    // SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
    if unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(subproc),
            OsMemPtr::new_unchecked(start),
            crate::core_types::ReservedBytes::new(size),
            numa_node,
            crate::core_types::Exclusive::new(exclusive),
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
        // SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
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
// SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_os_memory_ex(
    size: usize,
    commit: bool,
    allow_large: bool,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    // SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
    unsafe {
        mi_reserve_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            crate::core_types::ReservedBytes::new(size),
            crate::core_types::NumaNode::any(),
            crate::core_types::InitiallyCommitted(commit),
            crate::core_types::AllowPinned::new(allow_large),
            crate::core_types::Exclusive::new(exclusive),
            arena_id,
        )
    }
}

/// # Safety
/// Matches the C API and may mutate global arena state through the active sub-process.
// SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_os_memory(
    size: usize,
    commit: bool,
    allow_large: bool,
) -> c_int {
    // SAFETY[AUDIT:arena-public-reserve-entry-boundary]: public reserve entrypoints preserve C ABI contracts while registering aligned arena memory with typed memid state.
    unsafe { mi_reserve_os_memory_ex(size, commit, allow_large, false, ptr::null_mut()) }
}

/// # Safety
/// Matches the C API and forwards to the current arena-management entrypoint.
// SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_manage_os_memory(
    start: *mut c_void,
    size: usize,
    is_committed: bool,
    is_large: bool,
    is_zero: bool,
    numa_node: c_int,
) -> bool {
    // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
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
    subproc: SubprocPtr,
    start: OsMemPtr,
    size: crate::core_types::ReservedBytes,
    numa_node: crate::core_types::NumaNode,
    exclusive: crate::core_types::Exclusive,
    mut memid: mi_memid_t,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let subproc = subproc.as_ptr();
    let mut start = start.as_ptr();
    let mut size = size.get();
    let exclusive = exclusive.get();
    if !arena_id.is_null() {
        // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
        unsafe { *arena_id = _mi_arena_id_none() };
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
        let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };

        // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
        let arena = unsafe {
            mi_arena_initialize(
                SubprocPtr::new_unchecked(subproc),
                OsMemPtr::new_unchecked(start),
                typed_slice_count,
                parent,
                crate::core_types::ReservedBytes::new(if parent.is_null() {
                    total_size
                } else {
                    0
                }),
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
            crate::internal::mi_assert!(
                mi_size_of_slices(total_slice_count)
                    // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
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
            memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();
        }
        crate::internal::mi_assert!(slice_count <= total_slice_count);
        total_slice_count -= slice_count;
        // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
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
// SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
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
        crate::core_types::ReservedBytes::new(size),
        crate::core_types::InitiallyCommitted(is_committed),
        crate::core_types::InitiallyZero(is_zero),
        crate::core_types::Pinned(is_pinned),
    )
    .into_raw();
    let Some(start_ptr) = crate::core_types::OsMemPtr::new(start) else {
        return false;
    };
    // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
    unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            start_ptr,
            crate::core_types::ReservedBytes::new(size),
            crate::core_types::NumaNode::new(numa_node),
            crate::core_types::Exclusive::new(exclusive),
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    }
}

// SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
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
        crate::core_types::ReservedBytes::new(size),
        crate::core_types::InitiallyCommitted(is_committed),
        crate::core_types::InitiallyZero(is_zero),
        crate::core_types::Pinned(is_pinned),
    )
    .into_raw();
    let Some(start_ptr) = crate::core_types::OsMemPtr::new(start) else {
        return false;
    };
    // SAFETY[AUDIT:arena-managed-os-memory-boundary]: managed OS-memory entrypoints preserve C ABI contracts while registering caller-owned memory with typed memid state.
    unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            start_ptr,
            crate::core_types::ReservedBytes::new(size),
            crate::core_types::NumaNode::new(numa_node),
            crate::core_types::Exclusive::new(exclusive),
            memid,
            commit_fun,
            commit_fun_arg,
            arena_id,
        )
    }
}

/// # Safety
/// Matches the C API and may reserve/manage huge-page backed arena memory.
// SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages_at_ex(
    pages: usize,
    numa_node: c_int,
    timeout_msecs: usize,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    if !arena_id.is_null() {
        // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
        unsafe { *arena_id = ptr::null_mut() };
    }
    if pages == 0 {
        return 0;
    }
    let numa_node = if numa_node < -1 {
        crate::core_types::NumaNode::any()
    } else {
        crate::core_types::NumaNode::new(numa_node)
    }
    // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
    .normalize(unsafe { _mi_os_numa_node_count() });
    let numa_node_raw = numa_node.get();
    let mut hsize = 0usize;
    let mut pages_reserved = 0usize;
    let mut memid = unsafe { _mi_memid_none() };
    // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
    let p = unsafe {
        _mi_os_alloc_huge_os_pages(
            pages,
            numa_node_raw,
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
        numa_node_raw, pages_reserved, pages
    ));

    // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
    if !unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            OsMemPtr::new_unchecked(p),
            crate::core_types::ReservedBytes::new(hsize),
            numa_node,
            crate::core_types::Exclusive::new(exclusive),
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    } {
        // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
        unsafe { _mi_os_free(p, hsize, memid) };
        return ENOMEM;
    }
    0
}

/// # Safety
/// Matches the C API and reserves huge-page backed arena memory on a specific NUMA node.
// SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages_at(
    pages: usize,
    numa_node: c_int,
    timeout_msecs: usize,
) -> c_int {
    // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
    unsafe {
        mi_reserve_huge_os_pages_at_ex(pages, numa_node, timeout_msecs, false, ptr::null_mut())
    }
}

/// # Safety
/// Matches the C API and reserves huge-page backed arena memory interleaved across NUMA nodes.
// SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
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
        // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
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
        // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
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
// SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
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
        // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
        unsafe { *pages_reserved = 0 };
    }
    let err =
        unsafe { mi_reserve_huge_os_pages_interleave(pages, 0, (max_secs * 1000.0) as usize) };
    if err == 0 && !pages_reserved.is_null() {
        // SAFETY[AUDIT:arena-huge-reservation-boundary]: huge-page reservation entrypoints normalize NUMA input, reserve huge OS memory, and register or release it atomically.
        unsafe { *pages_reserved = pages };
    }
    err
}
