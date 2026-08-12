//! Per-thread heap initialization and teardown lifecycle.

#[cfg(feature = "guarded")]
use core::ffi::c_long;
use core::ffi::c_void;
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::Ordering;

use crate::allocator::options::_mi_error_message_fmt;
#[cfg(feature = "guarded")]
use crate::allocator::options::mi_option_get_clamp;
#[cfg(feature = "guarded")]
use crate::allocator::options::{mi_option_get, mi_option_t};
use crate::allocator::stats::{__mi_stat_decrease_mt, __mi_stat_increase_mt};
use crate::allocator::thread_heap::{_mi_theap_collect_abandon, _mi_theap_create, _mi_theap_free};
use crate::internal::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release};
use crate::internal::memid::mi_memid_t;
use crate::internal::{_mi_meta_free, _mi_meta_zalloc};
use crate::types::{
    Errno, SubprocPtr, ThreadHeapLocal, ThreadHeapPtr, ThreadHeapState, ThreadId, TldPtr,
};
#[cfg(feature = "guarded")]
use crate::types::{
    GuardedConfig, GuardedSampleCount, GuardedSampleRate, GuardedSizeMax, GuardedSizeMin,
};
use crate::types::{mi_subproc_t, mi_theap_t, mi_tld_t};

use super::{
    _mi_is_theap_main, _mi_theap_cached_set, _mi_theap_default_set, _mi_theap_empty_get,
    MI_TLD_INVALID, MiHeapMainPrefix, MiSubprocPrefix, mi_heap_main, mi_heap_main_init,
    mi_is_main_thread_fast, mi_memid_is_static, mi_subproc_heap_main_raw, mi_subproc_prefix,
    mi_theap_main_tls_peek, mi_theap_main_tls_set, subproc_main, theap_main, tld_main,
};

/// # Safety
///
/// A nonnull `theap` must identify live initialized thread-heap metadata for
/// the current thread, with its TLD, subprocess, heap lists, pages, and backing
/// satisfying `done_typed` for the complete teardown. Null requests teardown of
/// the current thread's published default heap. No incompatible concurrent
/// attach, relink, teardown, or non-refcounted metadata access is permitted.
pub(super) unsafe fn done(theap: *mut mi_theap_t) {
    let mut default_theap = theap;
    if default_theap.is_null() {
        default_theap = mi_theap_main_tls_peek();
        if default_theap.is_null() {
            // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
            default_theap = unsafe { crate::platform::_mi_theap_default() };
            crate::internal::mi_assert_internal!(
                // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
                default_theap.is_null() || unsafe { _mi_is_theap_main(default_theap) }
            );
        }
    }

    let Some(default_theap) = ThreadHeapPtr::new(default_theap) else {
        return;
    };
    // SAFETY: thread-done entry resolves the current default thread heap before typed teardown.
    unsafe { done_typed(default_theap) };
}

/// # Safety
///
/// `default_theap` must remain live and initialized through state and locality
/// validation. If it belongs to the current thread, its TLD, subprocess, heap
/// lists, pages, and backing must remain live and satisfy `theaps_done`; this
/// call exclusively owns current-thread teardown. Registered collection,
/// arena, tracking, and diagnostic callbacks and opaque arguments must remain
/// valid for synchronous invocation, tolerate compatible allocator reentry,
/// preserve the teardown topology, and must not unwind.
unsafe fn done_typed(default_theap: ThreadHeapPtr) {
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let theap = match unsafe { default_theap.state() } {
        ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => theap,
        ThreadHeapState::Uninitialized | ThreadHeapState::Deleted => return,
    };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let current = unsafe { ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id()) };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let Ok(default_theap) = (unsafe { theap.try_local(current) }) else {
        return;
    };
    crate::allocator::threadlocal::_mi_thread_locals_thread_done();

    let default_theap = default_theap.ptr().as_ptr();
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let tld = unsafe { (*default_theap).tld };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let subproc = unsafe { (*tld).subproc };
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if heap.is_null() {
        mi_heap_main_init();
        // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
        heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    unsafe {
        __mi_stat_decrease_mt(
            ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
            1,
        )
    };

    // SAFETY: typed thread-done dispatch checks local ownership before stats and cleanup handoff.
    if unsafe { (*tld).thread_id } != crate::platform::_mi_prim_thread_id() {
        return;
    }

    // SAFETY: the successful initialized/local classification and matching TLD
    // thread ID establish the current-thread teardown epoch. The heap/TLD lists,
    // subprocess, pages, backing, and callback state remain live as required by
    // `theaps_done` until it has removed every thread heap.
    let tld_ptr = unsafe { TldPtr::new_unchecked(tld) };
    // SAFETY: the current-thread validation above establishes the complete TLD
    // teardown contract, including exclusive list mutation and callback rules.
    unsafe { theaps_done(tld_ptr) };
    tld_free(tld);
}

#[cfg(feature = "guarded")]
pub(super) unsafe fn guarded_set_sample_rate(
    theap: *mut mi_theap_t,
    sample_rate: usize,
    seed: usize,
) {
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        if let Some(theap) = ThreadHeapPtr::new(theap) {
            let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
            if let Ok(theap) = theap.try_local(current) {
                guarded_set_sample_rate_typed(theap, GuardedSampleRate::new(sample_rate), seed);
            }
        }
    }
}

#[cfg(feature = "guarded")]
unsafe fn guarded_set_sample_rate_typed(
    theap: ThreadHeapLocal,
    sample_rate: GuardedSampleRate,
    seed: usize,
) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        (*theap).guarded_sample_rate = sample_rate.get();
        let mut sample_count = GuardedSampleCount::new(sample_rate.get());
        if sample_rate.get() > 1 {
            let seed = if seed == 0 {
                crate::internal::_mi_theap_random_next(theap)
            } else {
                seed
            };
            sample_count = GuardedSampleCount::new((seed % sample_rate.get()) + 1);
        }
        (*theap).guarded_sample_count = sample_count.get();
    }
}

#[cfg(feature = "guarded")]
pub(super) unsafe fn guarded_set_size_bound(theap: *mut mi_theap_t, min: usize, max: usize) {
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        if let Some(theap) = ThreadHeapPtr::new(theap) {
            let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
            if let Ok(theap) = theap.try_local(current) {
                let min = GuardedSizeMin::new(min);
                let max = GuardedSizeMax::new(if min.get() > max { min.get() } else { max });
                guarded_set_size_bound_typed(theap, min, max);
            }
        }
    }
}

#[cfg(feature = "guarded")]
unsafe fn guarded_set_size_bound_typed(
    theap: ThreadHeapLocal,
    min: GuardedSizeMin,
    max: GuardedSizeMax,
) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        (*theap).guarded_size_min = min.get();
        (*theap).guarded_size_max = max.get();
    }
}

pub(super) unsafe fn guarded_init(theap: *mut mi_theap_t) {
    #[cfg(feature = "guarded")]
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        if let Some(theap) = ThreadHeapPtr::new(theap) {
            let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
            if let Ok(theap) = theap.try_local(current) {
                let sample_rate =
                    mi_option_get_clamp(mi_option_t::mi_option_guarded_sample_rate, 0, c_long::MAX)
                        as usize;
                let mut sample_count = sample_rate;
                if sample_rate > 1 {
                    let seed = mi_option_get(mi_option_t::mi_option_guarded_sample_seed) as usize;
                    let seed = if seed == 0 {
                        crate::internal::_mi_theap_random_next(theap.ptr().as_ptr())
                    } else {
                        seed
                    };
                    sample_count = (seed % sample_rate) + 1;
                }
                let size_min =
                    mi_option_get_clamp(mi_option_t::mi_option_guarded_min, 0, c_long::MAX)
                        as usize;
                let size_max =
                    mi_option_get_clamp(mi_option_t::mi_option_guarded_max, 0, c_long::MAX)
                        as usize;
                let config = GuardedConfig::from_clamped_raw(
                    size_min,
                    if size_min > size_max {
                        size_min
                    } else {
                        size_max
                    },
                    sample_rate,
                    sample_count,
                );
                guarded_init_typed(theap, config);
            }
        }
    }
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(feature = "guarded")]
unsafe fn guarded_init_typed(theap: ThreadHeapLocal, config: GuardedConfig) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        (*theap).guarded_size_min = config.size_min().get();
        (*theap).guarded_size_max = config.size_max().get();
        (*theap).guarded_sample_rate = config.sample_rate().get();
        (*theap).guarded_sample_count = config.sample_count().get();
    }
}

pub(super) unsafe fn options_init(theap: *mut mi_theap_t) {
    if let Some(theap) = ThreadHeapPtr::new(theap) {
        // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
        let current = unsafe { ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id()) };
        // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
        if let Ok(theap) = unsafe { theap.try_local(current) } {
            // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
            unsafe { options_init_typed(theap) };
        }
    }
}

unsafe fn options_init_typed(theap: ThreadHeapLocal) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: guarded allocation and option setup mutate only validated thread-heap configuration.
    unsafe {
        let policy = crate::allocator::policy::ActivePolicy::thread_heap_policy();
        (*theap).allow_page_reclaim = policy.allow_page_reclaim();
        (*theap).allow_page_abandon = policy.allow_page_abandon();
        (*theap).page_full_retain = policy.page_full_retain().get();
    }
}

pub(super) fn tld_main_init() {
    // SAFETY: mutates the statically allocated main-thread tld during one-time lazy init.
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        if tld_main.thread_id == 0 {
            tld_main.thread_id = crate::platform::_mi_prim_thread_id();
            mi_lock_init(ptr::addr_of_mut!(tld_main.theaps_lock));
        }
    }
}

pub(super) fn theap_main_init() {
    // SAFETY: mutates the statically allocated main theap during one-time lazy init.
    // SAFETY: main thread-heap static metadata is initialized once before publication.
    unsafe {
        if !mi_memid_is_static(theap_main.memid) {
            theap_main.memid =
                crate::types::MemId::<crate::types::StaticMem>::static_mem().into_raw();
            #[cfg(any(target_os = "macos", windows))]
            crate::internal::_mi_random_init_weak(ptr::addr_of_mut!(theap_main.random));
            #[cfg(not(any(target_os = "macos", windows)))]
            crate::internal::_mi_random_init(ptr::addr_of_mut!(theap_main.random));
            theap_main.cookie =
                crate::internal::_mi_theap_random_next(ptr::addr_of_mut!(theap_main));
            super::_mi_theap_options_init(ptr::addr_of_mut!(theap_main));
            super::_mi_theap_guarded_init(ptr::addr_of_mut!(theap_main));
        }
    }
}

#[inline(always)]
fn tld_alloc_non_main(main_subproc: *mut mi_subproc_t) -> *mut mi_tld_t {
    let Some(main_subproc) = SubprocPtr::new(main_subproc) else {
        return ptr::null_mut();
    };
    tld_alloc_non_main_typed(main_subproc)
        .map(TldPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

#[inline(always)]
fn tld_alloc_non_main_typed(main_subproc: SubprocPtr) -> Option<TldPtr> {
    // SAFETY: allocates a fresh zeroed tld block and initializes its local fields.
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        let mut memid = mi_memid_t::default();
        let tld_raw =
            _mi_meta_zalloc(size_of::<mi_tld_t>(), ptr::addr_of_mut!(memid)).cast::<mi_tld_t>();
        let Some(tld) = TldPtr::new(tld_raw) else {
            _mi_error_message_fmt(
                Errno::ENOMEM,
                format_args!("unable to allocate memory for thread local data\n"),
            );
            return None;
        };
        ptr::write(tld.memid_ptr(), memid);
        tld.set_thread_heaps(None);
        mi_lock_init(tld.thread_heaps_lock());
        tld.set_subproc(Some(main_subproc));
        tld.set_numa_node(crate::platform::os::_mi_os_numa_node_typed());
        tld.set_thread_id(Some(ThreadId::new_unchecked(
            crate::platform::_mi_prim_thread_id(),
        )));
        let thread_seq = main_subproc
            .thread_total_count_cell()
            .fetch_add(1, Ordering::Relaxed);
        tld.set_thread_seq(crate::types::ThreadSeq::new(thread_seq));
        tld.set_is_in_threadpool(crate::platform::_mi_prim_thread_is_in_threadpool());
        main_subproc
            .thread_count_cell()
            .fetch_add(1, Ordering::Relaxed);
        Some(tld)
    }
}

#[inline(never)]
fn tld_free(tld: *mut mi_tld_t) {
    if tld.is_null() || tld == MI_TLD_INVALID {
        return;
    }
    // SAFETY: `tld` must point to a valid local `mi_tld_t`.
    // SAFETY: TLD allocation/free and per-thread metadata counters use typed TLD and subproc witnesses.
    unsafe {
        mi_lock_done(ptr::addr_of_mut!((*tld).theaps_lock));
        if !(*tld).subproc.is_null() {
            let subproc = mi_subproc_prefix((*tld).subproc);
            (*subproc).thread_count.fetch_sub(1, Ordering::Relaxed);
        }
        if !ptr::eq(tld, ptr::addr_of_mut!(tld_main)) {
            _mi_meta_free(tld.cast::<c_void>(), size_of::<mi_tld_t>(), (*tld).memid);
        }
    }
}

/// Mark the current thread's allocator metadata as belonging to a threadpool.
/// If current-thread initialization cannot provide a live thread heap, this is
/// a no-op.
pub(super) fn set_in_threadpool() {
    let theap = super::_mi_theap_default_safe();
    // SAFETY: the safe default lookup returns an allocator-owned live heap or a
    // static uninitialized sentinel; probing the heap pointer distinguishes
    // those cases before any TLD projection.
    if !unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return;
    }
    // SAFETY: successful initialization proves this is the current thread's
    // live nonsentinel heap with initialized TLD metadata; only this thread
    // mutates its threadpool flag during the thread lifecycle.
    unsafe { (*(*theap).tld).is_in_threadpool = true };
}

/// Return the allocator-managed default thread heap for the current thread,
/// initializing its thread-local state when necessary. On initialization
/// failure, the static empty sentinel is returned. The raw pointer carries no
/// lifetime or dereference permission.
pub(super) fn default_safe() -> *mut mi_theap_t {
    // SAFETY: the bootstrap TLS lookup tolerates unset dynamic keys/slots and
    // normalizes a null current-thread value to the static empty sentinel; any
    // nonnull TLS value is allocator-owned current-thread metadata.
    let theap = unsafe { crate::platform::_mi_theap_default() };
    // SAFETY: the allocator-owned TLS value is a static sentinel or live
    // current-thread heap whose heap-pointer field is readable for this probe.
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    // Initialize this thread before repeating the allocator-owned TLS lookup.
    super::mi_thread_init();
    // SAFETY: initialization either published this thread's live default heap
    // or left the static empty sentinel when no heap could be provided; the
    // lookup reads only that current-thread slot.
    let theap = unsafe { crate::platform::_mi_theap_default() };
    // SAFETY: the post-initialization TLS value is either the live current-thread
    // default heap or the static empty sentinel, and its heap-pointer field is
    // readable for this assertion.
    crate::internal::mi_assert_internal!(unsafe {
        crate::internal::mi_theap_is_initialized(theap)
    });
    theap
}

/// Return the current thread's theap associated with the main heap, initializing
/// and caching its TLS association when necessary. On initialization failure,
/// the static empty sentinel is returned. The raw pointer carries no lifetime
/// or dereference permission.
pub(super) fn main_safe() -> *mut mi_theap_t {
    let mut theap = mi_theap_main_tls_peek();
    if theap.is_null() {
        // Initialize this thread before reading and caching its main-heap TLS
        // association.
        super::mi_thread_init();
        // SAFETY: initialization published a static sentinel or live current-
        // thread default heap through the allocator-owned TLS slot.
        theap = unsafe { crate::platform::_mi_theap_default() };
        crate::internal::mi_assert_internal!(!theap.is_null());
        // SAFETY: the allocator-owned TLS value remains live while its
        // initialized heap pointer and process-main identity are inspected.
        if unsafe { crate::internal::mi_theap_is_initialized(theap) }
            // SAFETY: the successful initialization probe makes the heap
            // pointer field readable; process-main metadata remains published.
            && unsafe { crate::internal::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) }
        {
            mi_theap_main_tls_set(ThreadHeapPtr::new(theap));
        }
    }
    crate::internal::mi_assert_internal!(
        !theap.is_null()
            // SAFETY: the cached or newly published allocator-owned TLS value
            // remains live for this final identity check; successful
            // initialization makes it initialized, while failure leaves the
            // static empty sentinel.
            && unsafe { crate::internal::mi_theap_is_initialized(theap) }
            // SAFETY: the preceding initialized probe makes its heap pointer
            // readable; process-main metadata remains published and stable.
            && unsafe { crate::internal::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap)) }
    );
    theap
}

#[inline(always)]
pub(super) unsafe fn init_default() -> *mut mi_theap_t {
    // SAFETY: thread init installs a default heap only after process and TLD setup.
    let mut theap = unsafe { crate::platform::_mi_theap_default() };
    // SAFETY: thread init installs a default heap only after process and TLD setup.
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    if mi_is_main_thread_fast() {
        mi_heap_main_init();
        theap = ptr::addr_of_mut!(theap_main);
    } else {
        let tld = tld_alloc_non_main(
            ptr::addr_of_mut!(subproc_main)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>(),
        );
        if tld.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: thread init installs a default heap only after process and TLD setup.
        theap = unsafe { _mi_theap_create(mi_heap_main(), tld) };
    }
    // SAFETY: thread init installs a default heap only after process and TLD setup.
    unsafe { _mi_theap_default_set(theap) };
    theap
}

/// # Safety
///
/// `tld` must identify live, initialized TLD metadata owned by the current
/// thread, and the caller must exclusively own its teardown. Its initialized
/// thread-heap lock and list must remain valid; every linked heap must belong to
/// this TLD and remain live with stable list links, pages, page backing, heap
/// association, refcount, and MemId until its terminal free attempt. No
/// concurrent heap attach, list relink, teardown, or non-refcounted metadata
/// access is permitted; allocator-protocol remote publication and existing
/// refcount holders may delay deletion. The TLD's subprocess and atomic stats
/// must remain live through every retry. Registered collection, release, arena,
/// tracking, and diagnostic callbacks and opaque arguments must remain valid
/// for synchronous invocation, tolerate compatible allocator reentry without
/// mutating this teardown topology, and must not unwind. A successfully freed
/// heap must not be accessed again.
unsafe fn theaps_done(tld: TldPtr) {
    // SAFETY: current-thread teardown resets the published TLS entries before
    // visiting or releasing any heap that they could otherwise expose.
    let empty = unsafe { _mi_theap_empty_get() };
    // SAFETY: `empty` is the allocator's live uninitialized sentinel, and this
    // current-thread teardown exclusively owns all three TLS publications.
    unsafe {
        _mi_theap_default_set(empty);
        _mi_theap_cached_set(empty);
        mi_theap_main_tls_set(None);
    }

    // SAFETY: the caller keeps the initialized TLD/list live and grants this
    // current thread exclusive teardown mutation. Snapshot each next link
    // before callback-capable abandonment; abandonment preserves list links
    // while draining the current node's pages.
    unsafe {
        mi_lock_acquire(tld.thread_heaps_lock());
        let mut theap = tld
            .thread_heaps()
            .map_or(ptr::null_mut(), ThreadHeapPtr::as_ptr);
        while !theap.is_null() {
            let next = (*theap).tnext;
            _mi_theap_collect_abandon(theap);
            crate::internal::mi_assert_internal!((*theap).page_count == 0);
            theap = next;
        }
        mi_lock_release(tld.thread_heaps_lock());
    }

    loop {
        let mut all_freed = true;
        // SAFETY: the TLD list lock serializes traversal and unlinking. Snapshot
        // `tnext` before `_mi_theap_free`: success unlinks and may release the
        // current node, while failure leaves it live for a later retry. The heap
        // list lock is acquired by the callee (`true`) and this TLD list lock is
        // already held (`false`). No current-node access follows the call.
        unsafe {
            mi_lock_acquire(tld.thread_heaps_lock());
            let mut theap = tld
                .thread_heaps()
                .map_or(ptr::null_mut(), ThreadHeapPtr::as_ptr);
            while !theap.is_null() {
                let next = (*theap).tnext;
                crate::internal::mi_assert_internal!((*theap).page_count == 0);
                if !_mi_theap_free(theap, true, false) {
                    all_freed = false;
                }
                theap = next;
            }
            mi_lock_release(tld.thread_heaps_lock());
        }

        if !all_freed {
            // SAFETY: the caller keeps the TLD and its nonnull subprocess
            // association stable through retries; projection copies the pointer
            // value before addressing the live atomic wait counter.
            let subproc = unsafe { tld.subproc().unwrap_unchecked() };
            // SAFETY: the subprocess stats remain live atomic-compatible storage;
            // this diagnostic update does not mutate the protected TLD list.
            unsafe {
                crate::internal::__mi_stat_counter_increase_mt(
                    ptr::addr_of_mut!((*subproc.stats()).heaps_delete_wait),
                    1,
                )
            };
            // SAFETY: the primitive yield has completed process initialization
            // and performs no access through the retained TLD or heap pointers.
            unsafe { crate::platform::_mi_prim_thread_yield() };
        } else {
            // SAFETY: the final locked traversal observed every node released;
            // the initialized TLD remains live for this list-head assertion.
            crate::internal::mi_assert_internal!(unsafe { tld.thread_heaps() }.is_none());
            break;
        }
    }

    // SAFETY: teardown published the empty sentinel before releasing heaps and
    // no later phase changes the current thread's default TLS entry.
    crate::internal::mi_assert_internal!(unsafe { crate::platform::_mi_theap_default() } == empty);
    // SAFETY: `empty` is the allocator's live sentinel and its initialized-state
    // probe is valid after all thread heaps have been removed.
    crate::internal::mi_assert_internal!(!unsafe {
        crate::internal::mi_theap_is_initialized(empty)
    });
}

/// Initialize allocator state for the current thread if necessary. Failure to
/// allocate thread-local metadata leaves the thread uninitialized.
pub(super) fn init() {
    // SAFETY: process initialization owns its one-time global publication and
    // is valid to request from this current-thread initialization boundary.
    unsafe { super::mi_process_init() };
    // SAFETY: process initialization completed (or another initializer
    // published it); the primitive/TLS query reads current-thread state.
    if unsafe { crate::platform::_mi_thread_is_initialized() } {
        return;
    }
    // SAFETY: this is the current thread's initialization epoch; process state
    // is initialized and no concurrent actor mutates this thread's TLS/TLD.
    if unsafe { init_default() }.is_null() {
        return;
    }
    // SAFETY: successful default initialization published the current
    // subprocess main heap and keeps it live through the stats update.
    let heap = unsafe { mi_heap_main() };
    // SAFETY: `heap` is the live subprocess main-heap prefix; its atomic thread
    // statistic is initialized and remains valid during this lifecycle update.
    unsafe {
        __mi_stat_increase_mt(
            ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
            1,
        )
    };
}
