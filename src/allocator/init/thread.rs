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
use crate::allocator::thread_heap::{
    _mi_theap_collect_abandon, _mi_theap_create, _mi_theap_free, lists,
};
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
    _mi_is_theap_main, _mi_theap_cached_set, _mi_theap_default_set, MI_TLD_INVALID,
    MiHeapMainPrefix, MiSubprocPrefix, mi_heap_main, mi_heap_main_init, mi_is_main_thread_fast,
    mi_memid_is_static, mi_subproc_heap_main_raw, mi_subproc_prefix, mi_theap_main_tls_peek,
    mi_theap_main_tls_set, subproc_main, theap_main, tld_main,
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
            default_theap = unsafe { crate::platform::_mi_theap_default() };
            crate::internal::mi_assert_internal!(
                default_theap.is_null() || unsafe { _mi_is_theap_main(default_theap) }
            );
        }
    }

    let Some(default_theap) = ThreadHeapPtr::new(default_theap) else {
        return;
    };
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
    let theap = match unsafe { default_theap.state() } {
        ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => theap,
        ThreadHeapState::Uninitialized | ThreadHeapState::Deleted => return,
    };
    let current = unsafe { ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id()) };
    let Ok(default_theap) = (unsafe { theap.try_local(current) }) else {
        return;
    };
    crate::allocator::threadlocal::_mi_thread_locals_thread_done();

    let default_theap = default_theap.ptr().as_ptr();
    let tld = unsafe { (*default_theap).tld };
    let subproc = unsafe { (*tld).subproc };
    let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    if heap.is_null() {
        mi_heap_main_init();
        heap = unsafe { mi_subproc_heap_main_raw(subproc) };
    }
    unsafe {
        __mi_stat_decrease_mt(
            ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
            1,
        )
    };

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
/// Apply the guarded sampling configuration to a current-thread heap.
///
/// # Safety
/// `theap` may be null. A nonnull value must remain live initialized allocator
/// metadata with stable heap/TLD ownership through the locality probe. If it
/// belongs to the current thread, guarded sampling fields and any random state
/// accessed for a zero seed must be exclusively mutable for this call;
/// incompatible reentry, concurrent local allocation/configuration, and
/// teardown are excluded. Protocol remote-free publication remains permitted.
pub(super) unsafe fn guarded_set_sample_rate(
    theap: *mut mi_theap_t,
    sample_rate: usize,
    seed: usize,
) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the raw contract keeps the nonnull thread heap, its stable TLD
    // association, and ownership metadata live through classification. The
    // primitive supplies the executing thread's valid allocator thread ID; on
    // successful locality classification the caller grants exclusive guarded
    // configuration/random-state mutation to the typed operation.
    unsafe {
        let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
        if let Ok(theap) = theap.try_local(current) {
            guarded_set_sample_rate_typed(theap, GuardedSampleRate::new(sample_rate), seed);
        }
    }
}

#[cfg(feature = "guarded")]
/// # Safety
/// `theap` must remain a live current-thread heap with exclusive access to its
/// guarded sampling fields and, when a zero seed and rate greater than one need
/// a random value, its initialized random state.
unsafe fn guarded_set_sample_rate_typed(
    theap: ThreadHeapLocal,
    sample_rate: GuardedSampleRate,
    seed: usize,
) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: the typed witness and function contract keep the guarded fields
    // exclusively writable. A random draw occurs only for a zero seed and rate
    // greater than one, which also makes the modulo divisor nonzero.
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
/// Apply guarded size bounds to a current-thread heap.
///
/// # Safety
/// `theap` may be null. A nonnull value must remain live initialized allocator
/// metadata with stable heap/TLD ownership through the locality probe. If it
/// belongs to the current thread, its guarded size fields must be exclusively
/// mutable for this call; incompatible reentry, concurrent local allocation or
/// configuration, and teardown are excluded. Protocol remote-free publication
/// remains permitted.
pub(super) unsafe fn guarded_set_size_bound(theap: *mut mi_theap_t, min: usize, max: usize) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the raw contract keeps the nonnull heap and stable TLD ownership
    // metadata live through classification. The primitive supplies the current
    // allocator thread ID; a successful classification grants exclusive guarded
    // size-field mutation to the typed operation.
    unsafe {
        let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
        if let Ok(theap) = theap.try_local(current) {
            let min = GuardedSizeMin::new(min);
            let max = GuardedSizeMax::new(if min.get() > max { min.get() } else { max });
            guarded_set_size_bound_typed(theap, min, max);
        }
    }
}

#[cfg(feature = "guarded")]
/// # Safety
/// `theap` must remain a live current-thread heap with exclusive access to its
/// guarded size-bound fields. `min <= max` must hold.
unsafe fn guarded_set_size_bound_typed(
    theap: ThreadHeapLocal,
    min: GuardedSizeMin,
    max: GuardedSizeMax,
) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: the typed witness and contract keep both guarded size fields
    // exclusively writable, and the caller supplies the ordered bounds.
    unsafe {
        (*theap).guarded_size_min = min.get();
        (*theap).guarded_size_max = max.get();
    }
}

/// Initialize guarded configuration for a current-thread heap.
///
/// # Safety
/// `theap` may be null. With `guarded` enabled, a nonnull value must remain live
/// initialized allocator metadata with stable heap/TLD ownership through the
/// locality probe. A current-thread heap's guarded fields and random state must
/// be exclusively mutable. Allocator option storage and any registered
/// diagnostic callback plus opaque argument must remain valid; callback unwind
/// and incompatible reentry, concurrent local allocation/configuration, and
/// teardown are excluded. Concurrent atomic option updates and protocol
/// remote-free publication remain permitted. Without `guarded`, the pointer is
/// not accessed.
pub(super) unsafe fn guarded_init(theap: *mut mi_theap_t) {
    #[cfg(feature = "guarded")]
    {
        let Some(theap) = ThreadHeapPtr::new(theap) else {
            return;
        };
        // SAFETY: the raw contract keeps this nonnull heap and stable TLD
        // ownership metadata live through the probe; the primitive supplies
        // the executing thread's valid allocator thread ID.
        let theap = unsafe {
            let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
            let Ok(theap) = theap.try_local(current) else {
                return;
            };
            theap
        };

        let sample_rate =
            mi_option_get_clamp(mi_option_t::mi_option_guarded_sample_rate, 0, c_long::MAX)
                as usize;
        let mut sample_count = sample_rate;
        if sample_rate > 1 {
            let seed = mi_option_get(mi_option_t::mi_option_guarded_sample_seed) as usize;
            // SAFETY: locality classification above proves current-thread
            // ownership, and the contract grants exclusive access to the live
            // heap's random state when the configured seed requests one.
            let seed = if seed == 0 {
                unsafe { crate::internal::_mi_theap_random_next(theap.ptr().as_ptr()) }
            } else {
                seed
            };
            sample_count = (seed % sample_rate) + 1;
        }
        let size_min =
            mi_option_get_clamp(mi_option_t::mi_option_guarded_min, 0, c_long::MAX) as usize;
        let size_max =
            mi_option_get_clamp(mi_option_t::mi_option_guarded_max, 0, c_long::MAX) as usize;
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
        // SAFETY: the successful locality probe and caller contract keep the
        // heap live and its guarded fields exclusively mutable; the safe policy
        // calculations above produced a normalized configuration.
        unsafe { guarded_init_typed(theap, config) };
    }
    #[cfg(not(feature = "guarded"))]
    {
        let _ = theap;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(feature = "guarded")]
/// # Safety
/// `theap` must remain a live current-thread heap whose guarded configuration
/// fields are exclusively mutable for this initialization.
unsafe fn guarded_init_typed(theap: ThreadHeapLocal, config: GuardedConfig) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: the typed witness and function contract keep the four guarded
    // fields exclusively writable; `GuardedConfig` carries normalized bounds
    // and sampling values.
    unsafe {
        (*theap).guarded_size_min = config.size_min().get();
        (*theap).guarded_size_max = config.size_max().get();
        (*theap).guarded_sample_rate = config.sample_rate().get();
        (*theap).guarded_sample_count = config.sample_count().get();
    }
}

/// Initialize allocation policy fields for a current-thread heap.
///
/// # Safety
/// `theap` may be null. A nonnull value must remain live initialized allocator
/// metadata with stable heap/TLD ownership through the locality probe. If it
/// belongs to the current thread, its policy fields must be exclusively mutable;
/// allocator option storage and any registered diagnostic callback plus opaque
/// argument must remain valid. Callback unwind and incompatible reentry,
/// concurrent local allocation/configuration, and teardown are excluded.
/// Concurrent atomic option updates and protocol remote-free publication remain
/// permitted.
pub(super) unsafe fn options_init(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the raw contract keeps the nonnull heap and stable TLD ownership
    // metadata live. The primitive supplies the current allocator thread ID;
    // successful classification grants exclusive policy-field mutation through
    // the typed operation.
    unsafe {
        let current = ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id());
        if let Ok(theap) = theap.try_local(current) {
            options_init_typed(theap);
        }
    }
}

/// # Safety
/// `theap` must remain a live current-thread heap whose allocation-policy fields
/// are exclusively mutable. Global option storage and any registered diagnostic
/// callback plus opaque argument must remain valid; callback unwind and
/// incompatible reentry are excluded. Concurrent atomic option updates remain
/// permitted.
unsafe fn options_init_typed(theap: ThreadHeapLocal) {
    let theap = theap.ptr().as_ptr();
    // SAFETY: the typed witness and function contract keep this live local
    // heap's policy fields exclusively writable. The policy snapshot uses live
    // option storage and valid, non-unwinding diagnostics before the three
    // ordered stores.
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
        if !mi_memid_is_static(theap_main.0.memid) {
            theap_main.0.memid =
                crate::types::MemId::<crate::types::StaticMem>::static_mem().into_raw();
            #[cfg(any(target_os = "macos", windows))]
            crate::internal::_mi_random_init_weak(ptr::addr_of_mut!(theap_main.0.random));
            #[cfg(not(any(target_os = "macos", windows)))]
            crate::internal::_mi_random_init(ptr::addr_of_mut!(theap_main.0.random));
            theap_main.0.cookie = crate::internal::_mi_theap_random_next(
                ptr::addr_of_mut!(theap_main).cast::<mi_theap_t>(),
            );
            // SAFETY: one-time main-heap initialization owns this live static
            // metadata and its policy fields. Option storage and diagnostics
            // remain valid and non-unwinding, and no incompatible reentry,
            // allocation, or teardown can observe it before publication.
            super::_mi_theap_options_init(ptr::addr_of_mut!(theap_main).cast::<mi_theap_t>());
            // SAFETY: the same unpublished current-thread static remains live
            // with exclusive guarded/random configuration access, after options
            // initialization and before any allocator use.
            super::_mi_theap_guarded_init(ptr::addr_of_mut!(theap_main).cast::<mi_theap_t>());
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
        let thread_seq = crate::types::atomic::AtomicUsizeField::from_cell(
            &*main_subproc.thread_total_count_field(),
        )
        .fetch_add(1, Ordering::Relaxed);
        tld.set_thread_seq(crate::types::ThreadSeq::new(thread_seq));
        tld.set_is_in_threadpool(crate::platform::_mi_prim_thread_is_in_threadpool());
        crate::types::atomic::AtomicUsizeField::from_cell(&*main_subproc.thread_count_field())
            .fetch_add(1, Ordering::Relaxed);
        crate::allocator::page::MI_PROCESS_MULTI_THREADED.store(true, Ordering::Relaxed);
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
pub(super) fn init_default() -> *mut mi_theap_t {
    // SAFETY: `mi_process_init_inner` reaches this point after establishing the
    // primitive and allocator TLS scaffolding, while `init` first completes
    // process initialization. Both are the current thread's initialization
    // epoch; the lookup returns live local metadata or the static empty sentinel.
    let mut theap = unsafe { crate::platform::_mi_theap_default() };
    // SAFETY: the TLS lookup returns live allocator metadata or the static
    // sentinel, whose initialized heap-pointer field is readable for this probe.
    if unsafe { crate::internal::mi_theap_is_initialized(theap) } {
        return theap;
    }
    if mi_is_main_thread_fast() {
        mi_heap_main_init();
        theap = ptr::addr_of_mut!(theap_main).cast::<mi_theap_t>();
    } else {
        let tld = tld_alloc_non_main(
            ptr::addr_of_mut!(subproc_main)
                .cast::<MiSubprocPrefix>()
                .cast::<mi_subproc_t>(),
        );
        if tld.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: `tld_alloc_non_main` returned a fresh current-thread TLD
        // associated with the initialized main subproc, and `mi_heap_main`
        // returns its live main heap. Process initialization keeps their policy,
        // list, option, random, guarded, stats, and callback state stable; this
        // current-thread initialization epoch excludes teardown, relink, and
        // incompatible callback reentry or unwind. Failure publishes nothing
        // and leaves the fresh TLD exclusively owned here; success transfers
        // that TLD into the newly linked thread heap.
        let new_theap = unsafe { _mi_theap_create(mi_heap_main(), tld) };
        if new_theap.is_null() {
            // `_mi_theap_create` published nothing, so the fresh TLD still has
            // no heap-list consumers. Roll back its initialized lock, subproc
            // thread count, and metadata backing exactly once.
            tld_free(tld);
            return ptr::null_mut();
        }
        theap = new_theap;
    }
    // SAFETY: `theap` is the initialized static main thread heap or the newly
    // created heap for this current thread. This lifecycle epoch exclusively
    // owns publication to the current thread's default TLS slot.
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
    let empty = crate::platform::__mi_theap_empty();
    // SAFETY: `empty` is the allocator's live uninitialized sentinel, and this
    // current-thread teardown exclusively owns these TLS publications. Clear
    // the destructor association before releasing metadata: publishing the
    // empty sentinel alone leaves the old pthread/FLS value registered.
    unsafe {
        crate::platform::_mi_prim_thread_associate_default_theap(ptr::null_mut());
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
        // list lock is acquired by the callee and this TLD list capability is
        // borrowed for the unlink. No current-node access follows the call.
        unsafe {
            lists::with_tld_list(tld, |list| {
                let mut theap = tld
                    .thread_heaps()
                    .map_or(ptr::null_mut(), ThreadHeapPtr::as_ptr);
                while !theap.is_null() {
                    let next = (*theap).tnext;
                    crate::internal::mi_assert_internal!((*theap).page_count == 0);
                    if !_mi_theap_free(
                        ThreadHeapPtr::new_unchecked(theap),
                        lists::HeldThreadHeapList::Tld(list),
                    ) {
                        all_freed = false;
                    }
                    theap = next;
                }
            });
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
    if init_default().is_null() {
        return;
    }
    let heap = mi_heap_main();
    // SAFETY: `heap` is the live subprocess main-heap prefix; its atomic thread
    // statistic is initialized and remains valid during this lifecycle update.
    unsafe {
        __mi_stat_increase_mt(
            ptr::addr_of_mut!((*(heap.cast::<MiHeapMainPrefix>())).stats.threads),
            1,
        )
    };
}
