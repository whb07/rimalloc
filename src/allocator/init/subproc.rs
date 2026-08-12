//! Subprocess lifecycle and main-heap association.

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::Ordering;

use crate::allocator::arena::_mi_arenas_unsafe_destroy_all;
use crate::allocator::heap::{_mi_heap_force_destroy, mi_heap_destroy};
use crate::allocator::options::{_mi_options_init, _mi_warning_message_fmt};
use crate::allocator::stats::{__mi_stat_increase_mt, _mi_stats_merge_into, mi_stats_header_init};
use crate::internal::atomic::{mi_lock_done, mi_lock_init};
use crate::internal::memid::mi_memid_t;
use crate::internal::{_mi_meta_free, _mi_meta_zalloc};
use crate::types::{
    BorrowedSubproc, HeapPtr, HeapVisitCallback, HeapVisitCallbackArg, MainHeap, MainSubproc,
    MiLockGuard, SubprocPtr, TldPtr,
};
use crate::types::{mi_heap_t, mi_heap_visit_fun, mi_subproc_id_t, mi_subproc_t, mi_theap_t};

use super::{
    _mi_theap_default_safe, MiHeapMainPrefix, MiSubprocPrefix, SUBPROC_TOTAL_COUNT,
    mi_heap_main_init, mi_memid_is_static, mi_subproc_heap_main_raw, mi_subproc_prefix,
    mi_subproc_prefix_typed, subproc_registry, subprocs_lock, tld_empty,
};

pub(super) unsafe fn heap_main_typed(subproc: BorrowedSubproc) -> Option<HeapPtr> {
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    let subproc = unsafe { mi_subproc_prefix_typed(subproc.ptr()) };
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    HeapPtr::new(unsafe { ptr::addr_of!((*subproc).heap_main).read_unaligned() })
}

pub(super) fn to_id(subproc: Option<SubprocPtr>) -> mi_subproc_id_t {
    mi_subproc_id_t::from_ptr(subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut()))
}

unsafe fn destroy_meta(subproc: SubprocPtr) {
    // SAFETY: the detached token grants exclusive ownership of this metadata.
    let subproc = unsafe { mi_subproc_prefix_typed(subproc) };
    // SAFETY: `subproc` points to metadata allocated by `mi_subproc_new`.
    // SAFETY: the detached token grants exclusive ownership of this metadata.
    unsafe {
        mi_lock_done(ptr::addr_of_mut!((*subproc).arena_reserve_lock));
        mi_lock_done(ptr::addr_of_mut!((*subproc).heaps_lock));
        _mi_meta_free(
            subproc.cast(),
            size_of::<MiSubprocPrefix>(),
            (*subproc).memid,
        );
    }
}

unsafe fn heap_subproc_raw(heap: *const mi_heap_t) -> *mut mi_subproc_t {
    // SAFETY: subproc main-heap loads preserve typed subproc and heap witnesses.
    unsafe { ptr::addr_of!((*(heap.cast::<MiHeapMainPrefix>())).subproc).read_unaligned() }
}

#[cfg(all(not(miri), target_os = "linux", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn main_addr() -> *mut mi_subproc_t {
    let main: *mut mi_subproc_t;
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe {
        core::arch::asm!(
            "lea {main}, [rip + subproc_main]",
            main = lateout(reg) main,
            options(nostack, preserves_flags)
        );
    }
    main
}

#[cfg(any(miri, not(all(target_os = "linux", target_arch = "x86_64"))))]
#[inline(always)]
unsafe fn main_addr() -> *mut mi_subproc_t {
    ptr::addr_of_mut!(super::subproc_main)
        .cast::<MiSubprocPrefix>()
        .cast::<mi_subproc_t>()
}

unsafe fn main_typed() -> MainSubproc {
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe { MainSubproc::new_unchecked(SubprocPtr::new_unchecked(main_addr())) }
}

pub(super) fn main_init() {
    // SAFETY: one-time process initialization for local subproc scaffolding.
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe {
        let main = main_addr();
        let main_prefix = mi_subproc_prefix(main);
        if !mi_memid_is_static((*main_prefix).memid) {
            _mi_options_init();
            (*main_prefix).memid =
                crate::types::MemId::<crate::types::StaticMem>::static_mem().into_raw();
            (*main_prefix).heaps = ptr::addr_of_mut!(super::heap_main).cast::<mi_heap_t>();
            (*main_prefix).heap_total_count.store(1, Ordering::Relaxed);
            (*main_prefix).heap_count.store(1, Ordering::Relaxed);
            (*main_prefix).heap_main = ptr::addr_of_mut!(super::heap_main).cast::<mi_heap_t>();
            __mi_stat_increase_mt(ptr::addr_of_mut!((*main_prefix).stats.heaps), 1);
            mi_stats_header_init(ptr::addr_of_mut!((*main_prefix).stats));
            mi_lock_init(ptr::addr_of_mut!((*main_prefix).arena_reserve_lock));
            mi_lock_init(ptr::addr_of_mut!((*main_prefix).heaps_lock));
            mi_lock_init(ptr::addr_of_mut!(subprocs_lock));
            mi_lock_init(ptr::addr_of_mut!(tld_empty.theaps_lock));
        }
    }
}

unsafe fn destroy_detached(detached: subproc_registry::DetachedSubproc) {
    let subproc = detached.ptr();
    let subproc_raw = subproc.as_ptr();
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    let main_subproc = unsafe { main_typed().ptr() };
    let is_main = ptr::eq(subproc_raw, main_subproc.as_ptr());
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    let subproc_prefix = unsafe { mi_subproc_prefix_typed(subproc) };
    // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
    let borrowed_subproc = unsafe { BorrowedSubproc::new(subproc) };
    loop {
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        let heaps_lock = unsafe { MiLockGuard::acquire(subproc.heaps_lock()) };
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        let heap = unsafe { (*subproc_prefix).heaps };
        drop(heaps_lock);
        if heap.is_null() {
            break;
        }
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        let main_heap = unsafe { heap_main_typed(borrowed_subproc) }
            .map(HeapPtr::as_ptr)
            .unwrap_or(ptr::null_mut());
        if !ptr::eq(heap, main_heap) {
            // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
            unsafe { mi_heap_destroy(heap) };
        } else {
            // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
            unsafe { _mi_heap_force_destroy(heap) };
            break;
        }
    }

    {
        let subproc = subproc_raw;
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        unsafe { _mi_arenas_unsafe_destroy_all(subproc) };
    }

    if is_main {
        let subproc = subproc_raw;
        // SAFETY: subproc destruction drains heaps, arenas, and page maps under registry ordering.
        unsafe { crate::allocator::page_map::_mi_page_map_unsafe_destroy(subproc) };
    } else {
        // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
        let main_subproc = unsafe { mi_subproc_prefix_typed(main_subproc) };
        // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
        unsafe {
            _mi_stats_merge_into(
                ptr::addr_of_mut!((*main_subproc).stats),
                ptr::addr_of_mut!((*subproc_prefix).stats),
            );
            destroy_meta(subproc);
        }
    }
}

pub(super) unsafe fn unsafe_destroy_all() {
    main_init();
    // SAFETY: registry teardown walks remaining subprocs while holding the registry lock.
    let main_subproc = unsafe { super::_mi_subproc_main() };
    // SAFETY: registry teardown walks remaining subprocs while holding the registry lock.
    // SAFETY: `main_init` initialized the registry lock.
    let registry = unsafe { subproc_registry::SubprocRegistryGuard::acquire() };
    while let Some(detached) = registry.detach_first_except(main_subproc) {
        // SAFETY: the detached token proves this live node was unlinked once;
        // retaining the guard preserves the existing destroy-all lock order.
        unsafe { destroy_detached(detached) };
    }
    drop(registry);

    // Preserve the existing final main-subproc detach after all other nodes.
    // SAFETY: `main_init` initialized the registry lock.
    let registry = unsafe { subproc_registry::SubprocRegistryGuard::acquire() };
    let main = registry.detach(main_subproc);
    drop(registry);
    if let Some(main) = main {
        // SAFETY: the detached token proves main was linked and unlinked once.
        unsafe { destroy_detached(main) };
    }
}

pub(super) unsafe fn main_ptr() -> *mut mi_subproc_t {
    // SAFETY: main subproc and main heap publication initialize process-global static metadata once.
    unsafe { main_typed().ptr().as_ptr() }
}

unsafe fn current_typed() -> BorrowedSubproc {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    let theap = unsafe { crate::platform::_mi_theap_default_raw() };
    let subproc = if theap.is_null() {
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        unsafe { main_typed().ptr() }
    } else {
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        let tld = unsafe { TldPtr::new_unchecked((*theap).tld) };
        // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
        unsafe { tld.subproc() }.unwrap_or_else(|| unsafe { main_typed().ptr() })
    };
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    unsafe { BorrowedSubproc::new(subproc) }
}

pub(super) unsafe fn current_ptr() -> *mut mi_subproc_t {
    // SAFETY: current lookup preserves the non-initializing raw TLS path.
    unsafe { current_typed().ptr().as_ptr() }
}

pub(super) unsafe fn main_id() -> mi_subproc_id_t {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    to_id(Some(unsafe { main_typed().ptr() }))
}

pub(super) unsafe fn current_id() -> mi_subproc_id_t {
    // SAFETY: subproc ID conversion and current/main subproc lookup use typed subproc witnesses.
    to_id(Some(unsafe { current_typed().ptr() }))
}

pub(super) unsafe fn ensure_heap_main(subproc: *mut mi_subproc_t) -> *mut mi_heap_t {
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    let subproc = unsafe { BorrowedSubproc::new(SubprocPtr::new_unchecked(subproc)) };
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    unsafe { ensure_heap_main_typed(subproc).ptr().as_ptr() }
}

unsafe fn ensure_heap_main_typed(subproc: BorrowedSubproc) -> MainHeap {
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    let heap = unsafe { heap_main_typed(subproc) };
    if let Some(heap) = heap {
        // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
        return unsafe { MainHeap::new_unchecked(heap) };
    }
    mi_heap_main_init();
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    let heap = unsafe { heap_main_typed(subproc) };
    crate::internal::mi_assert_internal!(heap.is_some());
    let Some(heap) = heap else {
        // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
        return unsafe { MainHeap::new_unchecked(HeapPtr::new_unchecked(super::mi_heap_main())) };
    };
    // SAFETY: subproc main-heap export initializes and returns a typed main-heap witness.
    unsafe { MainHeap::new_unchecked(heap) }
}

pub(super) unsafe fn heap_main() -> *mut mi_heap_t {
    // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
    let theap = unsafe { crate::platform::_mi_theap_default_raw() };
    let subproc = if theap.is_null() {
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        unsafe { main_addr() }
    } else {
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        let tld = unsafe { (*theap).tld };
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        unsafe { (*tld).subproc }
    };
    // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
    let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if heap.is_null() {
        mi_heap_main_init();
        // SAFETY: current main-heap lookup preserves raw default-thread and subproc association semantics.
        heap = unsafe { mi_subproc_heap_main_raw(subproc) };
        crate::internal::mi_assert_internal!(!heap.is_null());
    }
    heap
}

pub(super) unsafe fn is_heap_main(heap: *const mi_heap_t) -> bool {
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let subproc = unsafe { heap_subproc_raw(heap) };
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if main_heap.is_null() {
        mi_heap_main_init();
        // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
        main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    ptr::eq(main_heap, heap.cast_mut())
}

pub(super) unsafe fn is_theap_main(theap: *const mi_theap_t) -> bool {
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    if !unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return false;
    }
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let heap = unsafe { crate::internal::_mi_theap_heap(theap) };
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let subproc = unsafe { heap_subproc_raw(heap) };
    // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
    let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if main_heap.is_null() {
        mi_heap_main_init();
        // SAFETY: main-heap predicates compare validated heap and thread-heap associations.
        main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    ptr::eq(main_heap, heap)
}

/// Allocate, initialize, and publish a new subprocess, returning its opaque ID.
/// A null ID reports metadata-allocation failure.
pub(super) fn new() -> mi_subproc_id_t {
    main_init();
    let seq = SUBPROC_TOTAL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let mut memid = mi_memid_t::default();
    // SAFETY: `memid` is writable for the allocation result. The allocator's
    // process lifecycle and metadata callbacks remain initialized and do not
    // unwind or incompatibly reenter subprocess creation during this call.
    let subproc_ptr = unsafe {
        _mi_meta_zalloc(size_of::<MiSubprocPrefix>(), ptr::addr_of_mut!(memid))
            .cast::<MiSubprocPrefix>()
    };
    if subproc_ptr.is_null() {
        // SAFETY: a null pointer is the allocator's opaque allocation-failure ID.
        return unsafe { super::_mi_subproc_to_id(ptr::null_mut()) };
    }
    // SAFETY: `_mi_meta_zalloc` returned writable storage for the complete
    // prefix and `memid` describes that backing. The node remains unpublished
    // and exclusively owned while its initialized value is written.
    unsafe {
        subproc_ptr.write(MiSubprocPrefix::new_with_memid(
            crate::types::SubprocSeq::new(seq),
            memid,
        ))
    };
    // SAFETY: both lock words belong to the initialized, unpublished prefix and
    // have not previously been initialized or exposed to another thread.
    unsafe {
        mi_lock_init(ptr::addr_of_mut!((*subproc_ptr).arena_reserve_lock));
        mi_lock_init(ptr::addr_of_mut!((*subproc_ptr).heaps_lock));
    }
    // SAFETY: `main_init` initialized the registry lock.
    let registry = unsafe { subproc_registry::SubprocRegistryGuard::acquire() };
    // SAFETY: the nonnull prefix is fully initialized and still unlinked. The
    // registry guard serializes its one-time intrusive-list publication; the
    // metadata backing remains live until a later successful detach destroys it.
    unsafe {
        registry.push_front(SubprocPtr::new_unchecked(
            subproc_ptr.cast::<mi_subproc_t>(),
        ))
    };
    // SAFETY: publication completed under the registry guard, so this live
    // node's address is now a valid opaque allocator ID.
    unsafe { super::_mi_subproc_to_id(subproc_ptr.cast::<mi_subproc_t>()) }
}

pub(super) unsafe fn destroy(subproc_id: mi_subproc_id_t) {
    main_init();
    let candidate = subproc_id.as_ptr::<mi_subproc_t>();
    // SAFETY: main subproc address lookup does not dereference the candidate.
    if candidate.is_null() || ptr::eq(candidate, unsafe { main_addr() }) {
        return;
    }
    // SAFETY: `main_init` initialized the registry lock.
    let registry = unsafe { subproc_registry::SubprocRegistryGuard::acquire() };
    let detached = registry.detach(candidate);
    drop(registry);
    if let Some(detached) = detached {
        // SAFETY: the detached token proves the opaque ID named a live linked
        // node and that it was unlinked exactly once.
        unsafe { destroy_detached(detached) };
    }
}

pub(super) unsafe fn add_current_thread(subproc_id: mi_subproc_id_t) {
    let candidate = subproc_id.as_ptr::<mi_subproc_t>();
    if candidate.is_null() {
        return;
    }
    let theap = _mi_theap_default_safe();
    if theap.is_null() {
        return;
    }
    // SAFETY: subproc thread/heap counters and stats merges use typed subproc metadata witnesses.
    let Some(tld) = TldPtr::new(unsafe { (*theap).tld }) else {
        return;
    };
    // SAFETY: process initialization keeps main metadata and registry lock live.
    let main = unsafe { main_typed() };
    // SAFETY: `_mi_theap_default_safe` returned the current thread's live,
    // writable TLD, which presently must belong to main for reassignment.
    let result = unsafe {
        let registry = subproc_registry::SubprocRegistryGuard::acquire();
        registry.assign_current_thread(candidate, tld, main)
    };
    if matches!(
        result,
        subproc_registry::AssignCurrentThreadResult::AlreadyAssigned
    ) {
        _mi_warning_message_fmt(format_args!(
            "unable to add thread to the subprocess as it was already in another subprocess (id: {:?})\n",
            candidate
        ));
    }
}

pub(super) unsafe fn visit_heaps(
    subproc_id: mi_subproc_id_t,
    visitor: Option<mi_heap_visit_fun>,
    arg: *mut c_void,
) -> bool {
    let visitor = HeapVisitCallback::new(visitor);
    if !visitor.is_some() {
        return false;
    };
    let arg = HeapVisitCallbackArg::from_raw(arg);
    let candidate = subproc_id.as_ptr::<mi_subproc_t>();
    if candidate.is_null() {
        return false;
    }
    main_init();
    // SAFETY: `main_init` initialized the registry lock. Acquiring the heap
    // lock before releasing it hands target liveness to the returned token.
    let registry = unsafe { subproc_registry::SubprocRegistryGuard::acquire() };
    let Some(heaps) = registry.lock_heaps(candidate) else {
        return false;
    };
    drop(registry);
    // SAFETY: the public entrypoint requires a valid callback and argument for
    // this call, including its reentrancy and no-unwind obligations.
    unsafe { heaps.visit(visitor, arg) }
}
