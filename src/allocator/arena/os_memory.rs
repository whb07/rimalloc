//! Arena reserve, managed OS-memory, and huge-page reservation helpers.
//!
//! The parent arena module keeps allocation/search hot paths; this module owns
//! reserve growth, public managed-memory entrypoints, and huge-page reservation.

use super::metadata::mi_arena_initialize;
use super::*;

fn align_managed_memory(start: *mut c_void, size: usize) -> Option<(*mut c_void, usize)> {
    let address = start.addr();
    let remainder = address % MI_ARENA_SLICE_SIZE;
    if remainder == 0 {
        return Some((start, size));
    }
    let adjustment = MI_ARENA_SLICE_SIZE - remainder;
    let aligned_address = address.checked_add(adjustment)?;
    if adjustment >= size || size - adjustment < MI_ARENA_SLICE_SIZE {
        return None;
    }
    Some((start.map_addr(|_| aligned_address), size - adjustment))
}

/// Commit the backing bytes for one already-claimed arena slice range.
///
/// # Safety
/// `arena` must remain live and initialized, and `range` must identify slices
/// within its backing that this allocation transition exclusively claimed.
/// `commit_size` and `already_committed` must accurately describe that range's
/// OS commitment and accounting. The backing must remain valid for the selected
/// commit primitive. Any stored commit callback and opaque argument must remain
/// valid for synchronous invocation, tolerate concurrent arena commits, not
/// retain the temporary `is_zero` pointer, and not unwind. Arena teardown and
/// non-protocol bitmap mutation must not race this operation.
pub(super) unsafe fn mi_arena_commit(
    arena: ArenaPtr,
    range: crate::types::SliceRange,
    commit_size: crate::types::CommittedBytes,
    is_zero: &mut bool,
    already_committed: crate::types::CommittedBytes,
) -> bool {
    let arena_raw = arena.as_ptr();
    // SAFETY: the claimed range is within the caller-stabilized arena backing;
    // its start slice therefore projects a valid commit-range address.
    let start = unsafe {
        mi_arena_slice_start(arena, range.start())
            .as_ptr()
            .cast::<c_void>()
    };
    let size = commit_size.get();
    // SAFETY: the live initialized arena prefix keeps its callback field
    // readable and stable for this synchronous commit attempt.
    let commit_fun = unsafe { (*arena_raw.cast::<MiArenaPrefix>()).commit_fun };
    if let Some(commit_fun) = commit_fun {
        // SAFETY: the contract keeps the callback and opaque argument valid and
        // non-unwinding, the range writable, and `is_zero` uniquely borrowed.
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
        // SAFETY: the contract couples the live backing range with both byte
        // counts; the extended primitive updates only the supplied zero flag.
        unsafe { _mi_os_commit_ex(start, size, is_zero, already_committed) }
    } else {
        // SAFETY: the claimed live backing covers `size`, and the caller keeps
        // OS configuration initialized while the zero flag is updated.
        unsafe { _mi_os_commit(start, size, is_zero) }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ArenaOsAllocRequest {
    pub(super) size: usize,
    pub(super) alignment: usize,
    pub(super) align_offset: usize,
}

/// Allocate aligned OS backing and return its matching initialized MemId.
///
/// # Safety
/// Allocator OS configuration and primitive callbacks must be initialized and
/// remain stable for this synchronous attempt. A non-null `req_arena_id` is
/// treated only as a value that disables OS fallback and is never dereferenced.
/// On success the caller is responsible for releasing the returned backing with
/// the returned MemId. This function does not otherwise establish a Rust
/// ownership or lifetime guarantee for the pointer.
pub(super) unsafe fn mi_arena_os_alloc_aligned(
    request: ArenaOsAllocRequest,
    commit: bool,
    allow_large: bool,
    req_arena_id: mi_arena_id_t,
) -> Result<(NonNull<c_void>, mi_memid_t), mi_memid_t> {
    let mut memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();
    if mi_option_is_enabled(mi_option_t::mi_option_disallow_os_alloc) || !req_arena_id.is_null() {
        mi_set_errno(ENOMEM);
        return Err(memid);
    }

    let start = if request.align_offset > 0 {
        // SAFETY: initialized OS configuration accepts this request; the local
        // MemId output is uniquely writable and remains paired with the result.
        unsafe {
            _mi_os_alloc_aligned_at_offset(
                request.size,
                request.alignment,
                request.align_offset,
                commit,
                allow_large,
                &mut memid,
            )
        }
    } else {
        // SAFETY: initialized OS configuration accepts this request; the local
        // MemId output is uniquely writable and remains paired with the result.
        unsafe {
            _mi_os_alloc_aligned(
                request.size,
                request.alignment,
                commit,
                allow_large,
                &mut memid,
            )
        }
    };
    match NonNull::new(start) {
        Some(start) => Ok((start, memid)),
        None => Err(memid),
    }
}

/// Write an OS allocation result through the allocator's raw MemId ABI.
///
/// The returned pointer is the successful non-null address or null on failure;
/// `memid` receives the matching result state in both cases.
pub(super) fn mi_write_arena_os_allocation_result(
    result: Result<(NonNull<c_void>, mi_memid_t), mi_memid_t>,
    memid: &mut mi_memid_t,
) -> *mut c_void {
    let (start, allocated_memid) = match result {
        Ok((start, memid)) => (start.as_ptr(), memid),
        Err(memid) => (ptr::null_mut(), memid),
    };
    *memid = allocated_memid;
    start
}

/// Grow a subprocess arena registry by reserving OS backing.
///
/// # Safety
/// `subproc` must remain live with initialized registry, reserve-lock, and
/// atomic statistics fields. The caller must serialize registry growth with the
/// subprocess reserve lock. Allocator OS state and callbacks must remain valid
/// and non-unwinding. Subprocess/arena teardown must not race; concurrent arena
/// claims may proceed through their atomic protocols.
pub(super) unsafe fn mi_arena_reserve(
    subproc: SubprocPtr,
    req_size: crate::types::ReservedBytes,
    numa_node: crate::types::NumaNode,
    allow_large: bool,
) -> bool {
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    let req_size = req_size.get();
    // SAFETY: the live subprocess owns an initialized atomic arena count, and
    // the caller-held reserve lock excludes registry teardown during the load.
    let arena_count = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    if arena_count > (MI_MAX_ARENAS - 4) {
        return false;
    }

    let mut arena_reserve = mi_option_get_size(mi_option_t::mi_option_arena_reserve);
    if arena_reserve == 0 {
        return false;
    }

    // SAFETY: allocator OS configuration is initialized and stable while the
    // reserve policy snapshots virtual-reservation support.
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

    // SAFETY: allocator OS configuration remains initialized while eager-commit
    // policy snapshots overcommit support.
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
        // SAFETY: the live subprocess committed counter is atomic-compatible;
        // this pre-adjustment prevents double charging eager overcommit.
        unsafe {
            __mi_stat_adjust_decrease_mt(
                ptr::addr_of_mut!((*subproc).stats.committed),
                crate::types::CommittedBytes::new(arena_reserve).get(),
            );
        }
    }

    // SAFETY: the reserve lock stabilizes the live subprocess registry and OS
    // callbacks through allocation, registration, and possible rollback.
    let mut err = unsafe {
        mi_reserve_os_memory_ex2(
            subproc_ptr,
            crate::types::ReservedBytes::new(arena_reserve),
            numa_node,
            crate::types::InitiallyCommitted(arena_commit),
            crate::types::AllowPinned::new(allow_large),
            crate::types::Exclusive::new(false),
            ptr::null_mut(),
        )
    };
    if err != 0 {
        if adjust {
            // SAFETY: failed registration did not consume the pre-adjustment;
            // restore it through the subprocess's atomic-compatible counter.
            unsafe {
                __mi_stat_adjust_increase_mt(
                    ptr::addr_of_mut!((*subproc).stats.committed),
                    crate::types::CommittedBytes::new(arena_reserve).get(),
                );
            }
        }
        let small_arena_reserve = 4 * MI_ARENA_MIN_SIZE;
        if arena_reserve > small_arena_reserve && small_arena_reserve > req_size {
            if adjust {
                // SAFETY: the smaller eager-overcommit retry needs the same
                // atomic pre-adjustment against the live subprocess counter.
                unsafe {
                    __mi_stat_adjust_decrease_mt(
                        ptr::addr_of_mut!((*subproc).stats.committed),
                        crate::types::CommittedBytes::new(small_arena_reserve).get(),
                    );
                }
            }
            // SAFETY: the reserve lock and function contract remain in force
            // for the bounded smaller allocation and registration retry.
            err = unsafe {
                mi_reserve_os_memory_ex2(
                    subproc_ptr,
                    crate::types::ReservedBytes::new(small_arena_reserve),
                    numa_node,
                    crate::types::InitiallyCommitted(arena_commit),
                    crate::types::AllowPinned::new(allow_large),
                    crate::types::Exclusive::new(false),
                    ptr::null_mut(),
                )
            };
            if err != 0 && adjust {
                // SAFETY: failed smaller registration leaves its pre-adjustment
                // unconsumed, so restore the live atomic-compatible counter.
                unsafe {
                    __mi_stat_adjust_increase_mt(
                        ptr::addr_of_mut!((*subproc).stats.committed),
                        crate::types::CommittedBytes::new(small_arena_reserve).get(),
                    );
                }
            }
        }
    }
    err == 0
}

/// Reserve aligned OS backing and register it in `subproc` as an arena.
///
/// # Safety
/// `subproc` must remain live with an initialized registry and statistics.
/// Allocator OS configuration and primitive callbacks must remain valid for
/// allocation, registration, and rollback. If non-null, `arena_id` must be
/// aligned and uniquely writable for one identifier. Registry/subprocess
/// teardown must not race; successful registration transfers backing lifetime
/// management to the arena registry.
unsafe fn mi_reserve_os_memory_ex2(
    subproc: SubprocPtr,
    size: crate::types::ReservedBytes,
    numa_node: crate::types::NumaNode,
    commit: crate::types::InitiallyCommitted,
    allow_large: crate::types::AllowPinned,
    exclusive: crate::types::Exclusive,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    let subproc = subproc.as_ptr();
    let commit = commit.0;
    let allow_large = allow_large.get();
    let exclusive = exclusive.get();
    if !arena_id.is_null() {
        // SAFETY: the caller keeps the optional arena-id output aligned and
        // uniquely writable for initialization.
        unsafe { *arena_id = _mi_arena_id_none() };
    }

    let size = _mi_align_up(size.get(), MI_ARENA_SLICE_SIZE);
    let mut memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();
    // SAFETY: allocator OS state is initialized; the local MemId output is
    // uniquely writable and will remain paired with this allocation.
    let start = unsafe {
        _mi_os_alloc_aligned(size, MI_ARENA_SLICE_ALIGN, commit, allow_large, &mut memid)
    };
    if start.is_null() {
        return ENOMEM;
    }

    // SAFETY: the fresh OS allocation and MemId describe the exact live backing;
    // the subprocess registry and optional output remain valid for registration.
    if unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(subproc),
            OsMemPtr::new_unchecked(start),
            crate::types::ReservedBytes::new(size),
            numa_node,
            crate::types::Exclusive::new(exclusive),
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    } {
        crate::allocator::options::_mi_verbose_message(format_args!(
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
        // SAFETY: registration failed without taking ownership; the live OS
        // allocation is released using its exact size, commit state, and MemId.
        unsafe { _mi_os_free_ex(start, size, commit, memid, ptr::null_mut()) };
        crate::allocator::options::_mi_verbose_message(format_args!(
            "failed to reserve {} KiB memory\n",
            _mi_divide_up(size, 1024)
        ));
        ENOMEM
    }
}

/// # Safety
/// If non-null, `arena_id` must be aligned and uniquely writable for one
/// identifier for the duration of the call.
// SAFETY: this exported name and C signature match the public mimalloc reserve ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_os_memory_ex(
    size: usize,
    commit: bool,
    allow_large: bool,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    // SAFETY: the optional output validity is the public contract; the active
    // subprocess and OS state are allocator-owned and remain live for the call.
    unsafe {
        mi_reserve_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            crate::types::ReservedBytes::new(size),
            crate::types::NumaNode::any(),
            crate::types::InitiallyCommitted(commit),
            crate::types::AllowPinned::new(allow_large),
            crate::types::Exclusive::new(exclusive),
            arena_id,
        )
    }
}

/// Reserve OS memory and register it in the active subprocess arena registry.
#[unsafe(no_mangle)]
pub extern "C" fn mi_reserve_os_memory(size: usize, commit: bool, allow_large: bool) -> c_int {
    // SAFETY: this value-only wrapper supplies no output pointer; the public
    // reserve boundary owns the active-subprocess lookup and registration.
    unsafe { mi_reserve_os_memory_ex(size, commit, allow_large, false, ptr::null_mut()) }
}

/// # Safety
/// `start..start+size` must be caller-controlled backing that remains reserved
/// for allocator use until every resulting arena is destroyed. The committed,
/// large, and zero flags must describe that backing accurately. Backing teardown
/// and non-protocol mutation must not race allocator use.
// SAFETY: this exported name and C signature match the public managed-memory ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_manage_os_memory(
    start: *mut c_void,
    size: usize,
    is_committed: bool,
    is_large: bool,
    is_zero: bool,
    numa_node: c_int,
) -> bool {
    // SAFETY: the caller's backing and state flags satisfy the extended
    // boundary; no callback or arena-id output is supplied by this wrapper.
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

/// Register caller-managed backing as one or more arenas.
///
/// # Safety
/// `subproc` must remain live with an initialized registry and statistics.
/// `start` and `size` must describe caller-controlled backing that remains live,
/// writable as required by `memid`, and reserved for allocator use until every
/// resulting arena is destroyed. `memid` must describe that exact backing and
/// its committed, zero, pinned, and release policy. Any commit callback and
/// opaque argument must remain valid and thread-safe for later concurrent arena
/// invocations, must not retain temporary output pointers, and must not unwind.
/// If non-null, `arena_id` must be aligned and uniquely
/// writable for one identifier. Registry/backing teardown or relinking must not
/// race registration; concurrent registry growth may use its atomic protocol.
#[allow(clippy::too_many_arguments)]
unsafe fn mi_manage_os_memory_ex2(
    subproc: SubprocPtr,
    start: OsMemPtr,
    size: crate::types::ReservedBytes,
    numa_node: crate::types::NumaNode,
    exclusive: crate::types::Exclusive,
    mut memid: mi_memid_t,
    commit_fun: Option<mi_commit_fun_t>,
    commit_fun_arg: *mut c_void,
    arena_id: *mut mi_arena_id_t,
) -> bool {
    let subproc = subproc.as_ptr();
    let start = start.as_ptr();
    let size = size.get();
    let exclusive = exclusive.get();
    if !arena_id.is_null() {
        // SAFETY: the caller keeps the optional arena-id output aligned and
        // uniquely writable for initialization.
        unsafe { *arena_id = _mi_arena_id_none() };
    }
    let Some((mut start, size)) = align_managed_memory(start, size) else {
        crate::allocator::options::_mi_warning_message_fmt(format_args!(
            "after alignment, the size of the arena becomes too small (memory at {:p} with size {})\n",
            start, size
        ));
        return false;
    };

    let mut total_slice_count = (size / MI_ARENA_SLICE_SIZE / MI_BCHUNK_BITS) * MI_BCHUNK_BITS;
    let total_size = mi_size_of_slices(total_slice_count);
    if total_size < MI_ARENA_MIN_SIZE {
        crate::allocator::options::_mi_warning_message_fmt(format_args!(
            "cannot use OS memory since it is not large enough (size {} KiB, minimum required is {} KiB)",
            size / crate::internal::bits::MI_KiB,
            MI_ARENA_MIN_SIZE / crate::internal::bits::MI_KiB
        ));
        return false;
    }

    let mut parent: *mut mi_arena_t = ptr::null_mut();
    while total_slice_count > 0 {
        let mut slice_count = total_slice_count;
        if slice_count > MI_BITMAP_MAX_BIT_COUNT {
            slice_count = MI_BITMAP_MAX_BIT_COUNT;
        }
        let Some(typed_slice_count) = crate::types::SliceCount::from_usize(slice_count) else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };

        // SAFETY: checked slice geometry keeps this remaining segment within
        // the caller-owned backing; the root/parent, callback state, registry,
        // and optional root output satisfy `mi_arena_initialize`'s contract.
        let arena = unsafe {
            mi_arena_initialize(
                SubprocPtr::new_unchecked(subproc),
                OsMemPtr::new_unchecked(start),
                typed_slice_count,
                parent,
                crate::types::ReservedBytes::new(if parent.is_null() { total_size } else { 0 }),
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
            // SAFETY: `parent` was returned by the successful root
            // initialization above and remains live under the registration
            // epoch. The failed child leaves the parent's size field exclusively
            // adjustable, and the assertion guards the subtraction.
            unsafe {
                crate::internal::mi_assert!(
                    mi_size_of_slices(total_slice_count)
                        <= (*parent.cast::<MiArenaPrefix>()).total_size
                );
                (*parent.cast::<MiArenaPrefix>()).total_size -=
                    mi_size_of_slices(total_slice_count);
            }
            return true;
        }

        if parent.is_null() {
            parent = arena;
            memid = crate::types::MemId::<crate::types::NoneMem>::none().into_raw();
        }
        crate::internal::mi_assert!(slice_count <= total_slice_count);
        total_slice_count -= slice_count;
        // SAFETY: `slice_count` is bounded by the remaining slice count, so this
        // provenance-preserving advance stays within or one-past the backing.
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
/// `start..start+size` must be caller-controlled backing that remains reserved
/// for allocator use until every resulting arena is destroyed. The committed,
/// pinned, and zero flags must describe it accurately. If non-null, `arena_id`
/// must be aligned and uniquely writable for one identifier. Backing teardown
/// and non-protocol mutation must not race allocator use.
// SAFETY: this exported name and C signature match the extended managed-memory ABI.
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
    let memid = crate::types::MemId::<crate::types::ExternalMem>::external(
        crate::types::OsMemPtr::new(start),
        crate::types::ReservedBytes::new(size),
        crate::types::InitiallyCommitted(is_committed),
        crate::types::InitiallyZero(is_zero),
        crate::types::Pinned(is_pinned),
    )
    .into_raw();
    let Some(start_ptr) = crate::types::OsMemPtr::new(start) else {
        return false;
    };
    // SAFETY: the public contract keeps external backing and optional output
    // valid; allocator-owned subprocess state remains live during registration.
    unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            start_ptr,
            crate::types::ReservedBytes::new(size),
            crate::types::NumaNode::new(numa_node),
            crate::types::Exclusive::new(exclusive),
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    }
}

// SAFETY: this exported name and C signature match the callback managed-memory ABI.
#[unsafe(no_mangle)]
/// # Safety
/// `start..start+size` must be caller-controlled backing reserved for allocator
/// use until every resulting arena is destroyed, and the state flags must
/// describe it accurately. If supplied, `commit_fun` and `commit_fun_arg` must
/// remain valid and thread-safe for later concurrent arena commits, must not
/// retain temporary output pointers, and must not unwind. If non-null,
/// `arena_id` must be aligned and uniquely writable for one identifier.
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
    let memid = crate::types::MemId::<crate::types::ExternalMem>::external(
        crate::types::OsMemPtr::new(start),
        crate::types::ReservedBytes::new(size),
        crate::types::InitiallyCommitted(is_committed),
        crate::types::InitiallyZero(is_zero),
        crate::types::Pinned(is_pinned),
    )
    .into_raw();
    let Some(start_ptr) = crate::types::OsMemPtr::new(start) else {
        return false;
    };
    // SAFETY: the external backing, persistent callback state, and optional
    // output satisfy the private registration epoch's contract.
    unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            start_ptr,
            crate::types::ReservedBytes::new(size),
            crate::types::NumaNode::new(numa_node),
            crate::types::Exclusive::new(exclusive),
            memid,
            commit_fun,
            commit_fun_arg,
            arena_id,
        )
    }
}

/// # Safety
/// If non-null, `arena_id` must be aligned and uniquely writable for one
/// identifier for the duration of the call.
// SAFETY: this exported name and C signature match the huge-page reserve ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages_at_ex(
    pages: usize,
    numa_node: c_int,
    timeout_msecs: usize,
    exclusive: bool,
    arena_id: *mut mi_arena_id_t,
) -> c_int {
    if !arena_id.is_null() {
        // SAFETY: the caller keeps the optional arena-id output aligned and
        // uniquely writable for its required failure-state initialization.
        unsafe { *arena_id = ptr::null_mut() };
    }
    if pages == 0 {
        return 0;
    }
    let numa_node = if numa_node < -1 {
        crate::types::NumaNode::any()
    } else {
        crate::types::NumaNode::new(numa_node)
    }
    .normalize(_mi_os_numa_node_count());
    let numa_node_raw = numa_node.get();
    let mut hsize = 0usize;
    let mut pages_reserved = 0usize;
    let mut memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();
    // SAFETY: OS huge-page state is initialized; all three local outputs are
    // uniquely writable and remain paired with the returned allocation.
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
        crate::allocator::options::_mi_warning_message_fmt(format_args!(
            "failed to reserve {} GiB huge pages\n",
            pages
        ));
        return ENOMEM;
    }
    crate::allocator::options::_mi_verbose_message(format_args!(
        "numa node {}: reserved {} GiB huge pages (of the {} GiB requested)\n",
        numa_node_raw, pages_reserved, pages
    ));

    // SAFETY: the successful huge allocation, reported size, and MemId describe
    // the exact live backing; registry and optional output remain valid.
    if !unsafe {
        mi_manage_os_memory_ex2(
            SubprocPtr::new_unchecked(_mi_subproc()),
            OsMemPtr::new_unchecked(p),
            crate::types::ReservedBytes::new(hsize),
            numa_node,
            crate::types::Exclusive::new(exclusive),
            memid,
            None,
            ptr::null_mut(),
            arena_id,
        )
    } {
        // SAFETY: failed registration did not take ownership; release the live
        // huge-page allocation with its exact reported size and MemId.
        unsafe { _mi_os_free(p, hsize, memid) };
        return ENOMEM;
    }
    0
}

/// Reserve huge-page backed arena memory on a specific NUMA node.
#[unsafe(no_mangle)]
pub extern "C" fn mi_reserve_huge_os_pages_at(
    pages: usize,
    numa_node: c_int,
    timeout_msecs: usize,
) -> c_int {
    // SAFETY: this value-only wrapper supplies no output pointer; the extended
    // boundary owns OS allocation and active-subprocess registration.
    unsafe {
        mi_reserve_huge_os_pages_at_ex(pages, numa_node, timeout_msecs, false, ptr::null_mut())
    }
}

/// Reserve huge-page backed arena memory interleaved across NUMA nodes.
#[unsafe(no_mangle)]
pub extern "C" fn mi_reserve_huge_os_pages_interleave(
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
        _mi_os_numa_node_count()
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
        let err = mi_reserve_huge_os_pages_at(node_pages, numa_node, timeout_per);
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
/// If non-null, `pages_reserved` must be aligned and uniquely writable for one
/// `usize` for the duration of the call.
// SAFETY: this exported name and C signature match the deprecated huge-page reserve ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reserve_huge_os_pages(
    pages: usize,
    max_secs: f64,
    pages_reserved: *mut usize,
) -> c_int {
    crate::allocator::options::_mi_warning_message_fmt(format_args!(
        "mi_reserve_huge_os_pages is deprecated: use mi_reserve_huge_os_pages_interleave/at instead\n"
    ));
    if !pages_reserved.is_null() {
        // SAFETY: the caller keeps the optional count output aligned and
        // uniquely writable for failure-state initialization.
        unsafe { *pages_reserved = 0 };
    }
    let err = mi_reserve_huge_os_pages_interleave(pages, 0, (max_secs * 1000.0) as usize);
    if err == 0 && !pages_reserved.is_null() {
        // SAFETY: the same optional output remains uniquely writable and the
        // successful compatibility path reports the requested page count.
        unsafe { *pages_reserved = pages };
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_memory_alignment_checks_size_and_address_boundaries() {
        let slice = MI_ARENA_SLICE_SIZE;
        let aligned = ptr::without_provenance_mut::<c_void>(slice);
        assert_eq!(align_managed_memory(aligned, slice), Some((aligned, slice)));

        let misaligned = ptr::without_provenance_mut::<c_void>(1);
        let (start, size) = align_managed_memory(misaligned, 2 * slice).unwrap();
        assert_eq!(start.addr(), slice);
        assert_eq!(size, slice + 1);
        assert_eq!(align_managed_memory(misaligned, slice), None);

        let overflowing = ptr::without_provenance_mut::<c_void>(usize::MAX);
        assert_eq!(align_managed_memory(overflowing, usize::MAX), None);
    }
}
