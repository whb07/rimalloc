use core::ffi::c_void;
use core::ptr;

use crate::allocator::page::_mi_theap_or_default_typed;
use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::prim::{_mi_theap_default, MI_THEAP_INITASNULL};
use crate::types::{mi_heap_t, mi_theap_t};

use super::{
    _mi_theap_malloc_zero, _mi_theap_malloc_zero_typed, malloc_layout_or_report,
    mi_heap_borrowed_theap_local, mi_heap_malloc_typed, mi_theap_malloc_small_zero,
    mi_theap_malloc_small_zero_typed,
};

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. A nonnull result is allocator-owned
/// and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_small(size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and allocator lifecycle keep the default
    // heap's local queues, direct cache, free lists, and stats exclusively owned
    // while the small allocation runs.
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. `theap` may be null only when lazy
/// thread-heap initialization is enabled; otherwise it must be a live initialized
/// current-thread heap, with exclusive access to its local allocation state. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and heap contract keep the selected
    // current-thread allocation state exclusively owned through dispatch.
    unsafe {
        mi_theap_malloc_small_zero(
            theap,
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// Null returns null. If nonnull, `heap` must identify a live initialized heap
/// owned by the current thread, with a stable heap-to-thread-heap association
/// and no concurrent teardown or reassignment. The caller must have exclusive
/// mutation rights over its local TLD, direct cache, page queues, free lists,
/// statistics, and guarded sampling state; protocol atomic remote-free
/// publication is permitted. Page-map, arena, backing-memory,
/// allocator-configuration, and lifecycle state must remain initialized and
/// stable. Synchronous initialization, deferred-free, collection, arena,
/// guarded-allocation, tracking, or diagnostic callbacks and their opaque
/// arguments must remain valid, must not incompatibly reenter local heap state,
/// and must not unwind. `size` must not exceed `MI_SMALL_SIZE_MAX`. A nonnull
/// result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller proves that nonnull `heap` is live initialized heap
    // metadata; this operation only classifies that raw pointer.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return ptr::null_mut();
    };
    let size = crate::core_types::RequestedSize::new(size);
    // SAFETY: the caller keeps the classified heap current-thread owned with a
    // stable association while the resolver localizes it.
    let Some(theap) = (unsafe { mi_heap_borrowed_theap_local(heap) }) else {
        return ptr::null_mut();
    };
    // SAFETY: the caller's exclusive local-allocation contract remains valid
    // for this terminal bounded allocation operation.
    unsafe {
        mi_theap_malloc_small_zero_typed(
            theap,
            size,
            crate::core_types::Zeroed::No,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `theap` must be null, one of the allocator's static empty/wrong-thread
/// sentinels, or a live aligned allocator current-thread heap whose `heap`
/// pointer field is initialized and readable. Any nonnull input metadata must
/// remain live and readable through resolution. The process/TLS lifecycle must
/// permit current-thread initialization and default-heap lookup. After
/// successful resolution, the selected heap's local TLD, direct cache, page
/// queues, free lists, statistics, and guarded-allocation sampling state must be
/// exclusively mutable for this call; protocol atomic remote-free publication is
/// permitted. Page-map, arena, backing-memory, allocator-configuration, and
/// lifecycle state must remain initialized and stable.
/// Any initialization, deferred-free, collection, arena, guarded-allocation,
/// tracking, or diagnostic callbacks and their opaque arguments reached
/// synchronously must remain valid, must not incompatibly reenter local heap
/// state, and must not unwind. A nonnull result is allocator-owned and may be
/// released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    if theap.is_null() && MI_THEAP_INITASNULL {
        // SAFETY: the raw-boundary contract permits the configured lazy-null
        // resolver state through this terminal unaligned allocation dispatch.
        return unsafe { _mi_theap_malloc_zero(theap, size, false, ptr::null_mut()) };
    }
    let Some(layout) = malloc_layout_or_report(size, false) else {
        return ptr::null_mut();
    };
    // SAFETY: the raw-boundary contract limits `theap` to resolver-accepted
    // states, keeps any nonnull metadata readable through its probe, and permits
    // current-thread initialization and default-heap lookup.
    let Some(theap) = (unsafe { _mi_theap_or_default_typed(theap) }) else {
        return ptr::null_mut();
    };
    // SAFETY: successful resolution supplies an initialized nonnull
    // current-thread heap. The caller keeps local state exclusive and callbacks
    // valid and non-unwinding through this terminal allocation dispatch.
    unsafe {
        _mi_theap_malloc_zero_typed(
            ThreadHeapLocal::new_unchecked(theap),
            layout,
            ptr::null_mut(),
        )
    }
}

/// Allocate `size` bytes from the current thread's default heap.
///
/// A nonnull result is allocator-owned and may be released with `mi_free`.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub extern "C" fn mi_malloc(size: usize) -> *mut c_void {
    // SAFETY: allocator lifecycle supplies the current default/lazy heap and
    // exclusive local allocation epoch for this scalar allocation dispatch.
    unsafe { mi_theap_malloc(_mi_theap_default(), size) }
}

/// # Safety
/// If nonnull, `heap` must identify live initialized current-thread heap
/// metadata with a stable association to its thread heap. The caller must keep
/// the selected local TLD, direct cache, page queues, free lists, statistics,
/// and guarded-allocation sampling state exclusively mutable for this call;
/// protocol atomic remote-free publication is permitted. Page-map, arena,
/// backing-memory, allocator-configuration, and lifecycle state must remain
/// initialized and stable. Any synchronous collection, deferred-free, arena,
/// guarded-allocation, tracking, or diagnostic callbacks and opaque arguments
/// reached during allocation must remain valid, must not incompatibly reenter
/// local heap state, and must not unwind. A nonnull result is allocator-owned
/// and may be released with `mi_free`.
// SAFETY: this raw boundary first classifies caller-proven live heap metadata,
// then validates the layout before terminal typed allocation dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: the raw-boundary contract keeps any nonnull heap metadata live,
    // current-thread owned, and stable through this classification.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return ptr::null_mut();
    };
    let Ok(layout) = crate::core_types::LayoutRequest::malloc(size) else {
        return core::ptr::null_mut();
    };
    // SAFETY: the caller-proven classified heap and validated layout retain the
    // local-exclusivity, lifecycle, and callback obligations required by
    // terminal allocation.
    unsafe { mi_heap_malloc_typed(heap, layout) }
}

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. A nonnull result is allocator-owned
/// and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_small(size: usize) -> *mut c_void {
    // SAFETY: the caller-provided bound and allocator lifecycle keep the default
    // heap's local queues, direct cache, free lists, and stats exclusively owned
    // while the zeroing small allocation runs.
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::Yes,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. `theap` may be null only when lazy
/// thread-heap initialization is enabled; otherwise it must be a live initialized
/// current-thread heap, with exclusive access to its local allocation state. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    if theap.is_null() && MI_THEAP_INITASNULL {
        // SAFETY: the raw-boundary contract permits the configured lazy-null
        // resolver state through this terminal bounded zeroing dispatch.
        return unsafe {
            mi_theap_malloc_small_zero(
                theap,
                crate::core_types::RequestedSize::new(size),
                crate::core_types::Zeroed::Yes,
                ptr::null_mut(),
            )
        };
    }
    let size = crate::core_types::RequestedSize::new(size);
    // SAFETY: the raw-boundary contract limits `theap` to resolver-accepted
    // states, keeps any nonnull metadata readable through its probe, and permits
    // current-thread initialization and default-heap lookup.
    let Some(theap) = (unsafe { _mi_theap_or_default_typed(theap) }) else {
        return ptr::null_mut();
    };
    // SAFETY: successful resolution supplies an initialized nonnull
    // current-thread heap. The caller keeps local state exclusive and callbacks
    // valid and non-unwinding through this terminal bounded zeroing dispatch.
    unsafe {
        mi_theap_malloc_small_zero_typed(
            ThreadHeapLocal::new_unchecked(theap),
            size,
            crate::core_types::Zeroed::Yes,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown. The caller must have exclusive
/// mutation rights over its local allocation state; protocol atomic remote-free
/// publication is permitted. `size` must not exceed `MI_SMALL_SIZE_MAX`. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    // SAFETY: the caller keeps the heap live and current-thread owned while it
    // is classified, localized, and used for the bounded zeroing allocation.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_typed(
            theap,
            size,
            crate::core_types::Zeroed::Yes,
            ptr::null_mut(),
        )
    }
}
