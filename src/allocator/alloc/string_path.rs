#![allow(dead_code)]

use core::ffi::{c_char, c_void};
#[cfg(all(unix, not(target_os = "wasi")))]
use core::ffi::{c_int, c_long};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::allocator::page::{_mi_theap_or_default_typed, MI_MAX_ALLOC_SIZE};
use crate::core_types::{BorrowedHeap, ThreadHeapLocal};
use crate::internal::{_mi_memcpy, _mi_strlen, _mi_strnlen};
use crate::mimalloc::mi_free;
use crate::prim::_mi_theap_default;
#[cfg(all(unix, not(target_os = "wasi")))]
use crate::prim::mi_set_errno;
use crate::types::{ENOMEM, mi_heap_t, mi_theap_t};

use super::{mi_heap_borrowed_theap_local, mi_theap_malloc, mi_zalloc};

// SAFETY: these declarations match the Unix libc signatures. Call sites keep
// input C strings readable and any nonnull output buffers writable for the
// duration and extent required by the corresponding libc operation.
#[cfg(all(unix, not(target_os = "wasi")))]
unsafe extern "C" {
    fn pathconf(path: *const c_char, name: c_int) -> c_long;
    fn realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
}

/// # Safety
/// The allocator process/TLS lifecycle must permit current-thread
/// initialization and default-heap lookup, with no concurrent teardown or
/// mutation of this thread's default TLS slot. Synchronous initialization
/// callbacks and their opaque arguments must remain valid, must not unwind,
/// and must not incompatibly reenter thread initialization.
unsafe fn mi_theap_default_local() -> Option<ThreadHeapLocal> {
    // SAFETY: the caller permits current-thread initialization and keeps the
    // default TLS slot stable while it is read and resolved.
    let theap = unsafe { _mi_theap_or_default_typed(_mi_theap_default()) }?;
    // SAFETY: `_mi_theap_or_default_typed` returned a non-null thread-heap
    // pointer for the current/default thread heap.
    Some(unsafe { ThreadHeapLocal::new_unchecked(theap) })
}

/// # Safety
/// `s` must be null or point to a readable NUL-terminated string whose readable
/// prefix remains live and stable through allocation and copying. `theap` must
/// remain a live current-thread heap with exclusive access to its local
/// allocation state; protocol atomic remote-free publication is permitted.
/// Allocator callbacks and their opaque arguments reached synchronously must
/// remain valid, must not unwind, and must not incompatibly reenter heap
/// mutation. A nonnull result is allocator-owned and may be released with
/// `mi_free`.
unsafe fn mi_theap_strdup_typed(theap: ThreadHeapLocal, s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller keeps `s` readable through its NUL terminator.
    let len = unsafe { _mi_strlen(s) };
    if len > MI_MAX_ALLOC_SIZE - 1 {
        return ptr::null_mut();
    }
    // SAFETY: `theap` remains live, current-thread owned, and locally
    // exclusive; the checked bound makes `len + 1` representable.
    let t = unsafe { mi_theap_malloc(theap.ptr().as_ptr(), len + 1) }.cast::<c_char>();
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `s` is readable for the `len` bytes preceding its terminator and
    // `t` is a distinct live allocation writable for `len + 1` bytes. The
    // zero-length copy remains valid, and `t.add(len)` is the final byte.
    unsafe {
        _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
        *t.add(len) = 0;
    }
    t
}

/// # Safety
/// If `n` is nonzero, `s` must be null or readable for `n` bytes or through the
/// first NUL byte, whichever comes first, and that readable prefix must remain
/// live and stable through allocation and copying. If `n` is zero, no source
/// byte is accessed and `s` may be null or any nonnull, possibly dangling,
/// pointer value. `theap` must remain a live current-thread heap with exclusive
/// access to its local allocation state; protocol atomic remote-free
/// publication is permitted. Allocator callbacks and their opaque arguments
/// reached synchronously must remain valid, must not unwind, and must not
/// incompatibly reenter heap mutation. A nonnull result is allocator-owned and
/// may be released with `mi_free`.
unsafe fn mi_theap_strndup_typed(
    theap: ThreadHeapLocal,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: for nonzero `n`, the caller keeps `s` readable and stable for the
    // bounded scan, stopping at an earlier NUL byte when present. For zero
    // `n`, `_mi_strnlen` performs no pointer access.
    let len = unsafe { _mi_strnlen(s, n) };
    if len > MI_MAX_ALLOC_SIZE - 1 {
        return ptr::null_mut();
    }
    // SAFETY: `theap` remains live, current-thread owned, and locally
    // exclusive; the checked bound makes `len + 1` representable.
    let t = unsafe { mi_theap_malloc(theap.ptr().as_ptr(), len + 1) }.cast::<c_char>();
    if t.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the bounded scan proves `s` readable for `len` bytes and `t` is a
    // distinct live allocation writable for `len + 1` bytes. The zero-length
    // copy remains valid, and `t.add(len)` is the final byte.
    unsafe {
        _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
        *t.add(len) = 0;
    }
    t
}

/// # Safety
/// `s` must be null or point to a readable NUL-terminated C string whose
/// readable prefix remains live and stable through allocation and copying. The
/// allocator process/TLS lifecycle must permit initialization and keep this
/// thread's default heap live and locally exclusive for allocation; protocol
/// atomic remote-free publication is permitted. Synchronous allocator
/// callbacks and their opaque arguments must remain valid, must not unwind,
/// and must not incompatibly reenter heap mutation. A nonnull result is
/// allocator-owned and may be released with `mi_free`.
// SAFETY: this exported entry preserves the raw string ABI and delegates after
// resolving the allocator's current default heap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strdup(s: *const c_char) -> *mut c_char {
    // SAFETY: allocator lifecycle permits default-heap initialization and
    // keeps the current TLS slot stable. This intentionally occurs before the
    // typed helper's null-string check, matching the original entry ordering.
    let Some(theap) = (unsafe { mi_theap_default_local() }) else {
        return ptr::null_mut();
    };
    // SAFETY: the public contract keeps `s` readable and stable while the
    // resolved default heap remains live and locally exclusive for the
    // duplication transaction.
    unsafe { mi_theap_strdup_typed(theap, s) }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread,
/// with a stable thread-heap association and exclusive access to local
/// allocation state; protocol atomic remote-free publication is permitted.
/// `s` must be null or point to a readable NUL-terminated C string whose
/// readable prefix remains live and stable through allocation and copying.
/// Synchronous allocator callbacks and their opaque arguments must remain
/// valid, must not unwind, and must not incompatibly reenter heap mutation. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
// SAFETY: this exported heap entry validates and localizes the caller's raw
// heap before delegating string access and allocation under its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strdup(heap: *mut mi_heap_t, s: *const c_char) -> *mut c_char {
    // SAFETY: the caller keeps the raw heap live, current-thread owned, and
    // locally exclusive while it is classified and localized. This occurs
    // before the typed helper's null-string check, preserving entry ordering.
    let Some(theap) = (unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_borrowed_theap_local(heap)
    }) else {
        return ptr::null_mut();
    };
    // SAFETY: the public contract keeps `s` readable and stable while the
    // localized heap remains live and locally exclusive for the duplication
    // transaction.
    unsafe { mi_theap_strdup_typed(theap, s) }
}

/// # Safety
/// If `n` is nonzero, `s` must be null or readable for at most `n` bytes up to
/// a NUL terminator, and that readable prefix must remain live and stable
/// through allocation and copying. If `n` is zero, no source byte is accessed
/// and `s` may be null or any nonnull, possibly dangling, pointer value. The
/// allocator process/TLS lifecycle must permit initialization and keep this
/// thread's default heap live and locally exclusive for allocation; protocol
/// atomic remote-free publication is permitted. Synchronous allocator
/// callbacks and their opaque arguments must remain valid, must not unwind,
/// and must not incompatibly reenter heap mutation. A nonnull result is
/// allocator-owned and may be released with `mi_free`.
// SAFETY: this exported entry preserves the bounded raw string ABI and
// delegates after resolving the allocator's current default heap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char {
    // SAFETY: allocator lifecycle permits default-heap initialization and
    // keeps the current TLS slot stable. This intentionally occurs before the
    // typed helper's null-string check, matching the original entry ordering.
    let Some(theap) = (unsafe { mi_theap_default_local() }) else {
        return ptr::null_mut();
    };
    // SAFETY: for nonzero `n`, the public contract keeps `s` readable and
    // stable for its bounded scan and copy; zero `n` performs no source access.
    // The resolved default heap remains live and locally exclusive.
    unsafe { mi_theap_strndup_typed(theap, s, n) }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread,
/// with a stable thread-heap association and exclusive access to local
/// allocation state; protocol atomic remote-free publication is permitted.
/// If `n` is nonzero, `s` must be null or readable for at most `n` bytes up to
/// a NUL terminator, and that readable prefix must remain live and stable
/// through allocation and copying. If `n` is zero, no source byte is accessed
/// and `s` may be null or any nonnull, possibly dangling, pointer value.
/// Synchronous allocator callbacks and their opaque arguments must remain
/// valid, must not unwind, and must not incompatibly reenter heap mutation. A
/// nonnull result is allocator-owned and may be released with `mi_free`.
// SAFETY: this exported heap entry validates and localizes the caller's raw
// heap before delegating bounded string access and allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strndup(
    heap: *mut mi_heap_t,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    // SAFETY: the caller keeps the raw heap live, current-thread owned, and
    // locally exclusive while it is classified and localized. This occurs
    // before the typed helper's null-string check, preserving entry ordering.
    let Some(theap) = (unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_borrowed_theap_local(heap)
    }) else {
        return ptr::null_mut();
    };
    // SAFETY: for nonzero `n`, the public contract keeps `s` readable and
    // stable for its bounded scan and copy; zero `n` performs no source access.
    // The localized heap remains live and locally exclusive.
    unsafe { mi_theap_strndup_typed(theap, s, n) }
}

#[cfg(all(unix, not(target_os = "wasi")))]
fn mi_path_max() -> usize {
    static PATH_MAX: AtomicUsize = AtomicUsize::new(0);
    let pmax = PATH_MAX.load(Ordering::Acquire);
    if pmax != 0 {
        return pmax;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const _PC_PATH_MAX: c_int = 4;
    #[cfg(target_vendor = "apple")]
    const _PC_PATH_MAX: c_int = 5;

    // SAFETY: the static `"/"` string is NUL-terminated and readable for the
    // synchronous libc query; `_PC_PATH_MAX` is the target's path-limit key.
    let m = unsafe { pathconf(c"/".as_ptr(), _PC_PATH_MAX) };
    let pmax = if m <= 0 {
        4096
    } else if m < 256 {
        256
    } else if m > (64 * 1024) {
        64 * 1024
    } else {
        m as usize
    };
    let _ = PATH_MAX.compare_exchange(0, pmax, Ordering::AcqRel, Ordering::Acquire);
    PATH_MAX.load(Ordering::Acquire)
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must be a readable NUL-terminated path. A nonnull `resolved_name`
/// must satisfy libc `realpath`'s output-buffer requirements. When
/// `resolved_name` is null, `theap` must remain a live current-thread heap with
/// exclusive access to its local allocation state while the temporary result
/// is duplicated; protocol atomic remote-free publication is permitted. The
/// allocator configuration and default heap used for the temporary buffer must
/// remain live, and allocator callbacks invoked by allocation or free must be
/// valid for synchronous use, must not unwind, and must not reenter in a way
/// that mutates these heap operations incompatibly.
unsafe fn mi_theap_realpath_typed(
    theap: ThreadHeapLocal,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    if !resolved_name.is_null() {
        // SAFETY: the caller supplies the readable input path and a nonnull
        // output buffer satisfying libc `realpath`'s requirements.
        return unsafe { realpath(fname, resolved_name) };
    }

    let n = mi_path_max();
    // `n + 1` cannot overflow because `mi_path_max` clamps `n` to at most 64 KiB.
    let buf = mi_zalloc(n + 1).cast::<c_char>();
    if buf.is_null() {
        mi_set_errno(ENOMEM);
        return ptr::null_mut();
    }

    // SAFETY: `fname` remains a readable C string; `buf` is a live writable
    // `n + 1` byte allocation. The caller's current-thread heap witness and
    // exclusive local allocation access cover duplication, and `buf` remains
    // live until the matching allocator free after the duplicate attempt.
    unsafe {
        let rname = realpath(fname, buf);
        let result = mi_theap_strndup_typed(theap, rname, n);
        mi_free(buf.cast::<c_void>());
        result
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must point to a readable NUL-terminated path. If `resolved_name` is
/// non-null, it must satisfy libc `realpath`'s output-buffer requirements. When
/// `resolved_name` is null, `theap` may be null or a supported allocator
/// default/lazy sentinel; otherwise it must identify a live current-thread heap
/// with exclusive access to its local allocation state for the call. Protocol
/// atomic remote-free publication is permitted. In that null-output case,
/// allocator callbacks and opaque arguments used by allocation or release must
/// remain valid for synchronous invocation, must not unwind, and must not
/// reenter with incompatible heap mutation.
// SAFETY: this exported raw entry preserves the allocator's thread-heap
// sentinel ABI and delegates path and output-buffer access under its contract.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_theap_realpath(
    theap: *mut mi_theap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    if !resolved_name.is_null() {
        // SAFETY: the public contract supplies the readable input path and the
        // caller-owned output storage required by libc `realpath`.
        return unsafe { realpath(fname, resolved_name) };
    }

    // SAFETY: a null/lazy/default `theap` is handled by the allocator; any
    // nonnull heap is kept live, current-thread owned, and locally exclusive by
    // the caller while it is classified and converted to a typed witness.
    let theap = unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        ThreadHeapLocal::new_unchecked(theap)
    };

    // SAFETY: the raw entry contract keeps `fname` readable and the selected
    // current-thread heap/configuration live and locally exclusive through the
    // temporary allocation, duplication, and release transaction.
    unsafe { mi_theap_realpath_typed(theap, fname, resolved_name) }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread,
/// with a stable thread-heap association and exclusive access to local
/// allocation state for the call; protocol atomic remote-free publication is
/// permitted. `fname` must point to a readable NUL-terminated path. If
/// `resolved_name` is non-null, it must satisfy libc `realpath`'s output-buffer
/// requirements. When `resolved_name` is null, allocator callbacks and opaque
/// arguments used by allocation or release must remain valid for synchronous
/// invocation, must not unwind, and must not reenter with incompatible heap
/// mutation.
// SAFETY: this exported heap entry validates and localizes the caller's raw
// heap before delegating path and output-buffer access under its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realpath(
    heap: *mut mi_heap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    // SAFETY: the caller keeps the raw heap live and current-thread owned while
    // it is classified and its stable local thread-heap association is read.
    let Some(theap) = (unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_borrowed_theap_local(heap)
    }) else {
        return ptr::null_mut();
    };

    // SAFETY: the public contract keeps `fname` and any output buffer valid;
    // the localized current-thread heap remains live and locally exclusive
    // through direct libc delegation or temporary allocation and release.
    unsafe { mi_theap_realpath_typed(theap, fname, resolved_name) }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must point to a readable NUL-terminated path. If `resolved_name` is
/// non-null, it must satisfy libc `realpath`'s output-buffer requirements. When
/// `resolved_name` is null, allocator callbacks and opaque arguments used by
/// allocation or release must remain valid for synchronous invocation, must
/// not unwind, and must not reenter with incompatible mutation of the current
/// default heap.
// SAFETY: this exported default-heap entry delegates path and output-buffer
// access under its contract and allocator lifecycle supplies the local heap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realpath(
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    if !resolved_name.is_null() {
        // SAFETY: the public contract supplies the readable input path and the
        // caller-owned output storage required by libc `realpath`.
        return unsafe { realpath(fname, resolved_name) };
    }

    // SAFETY: allocator lifecycle initialization keeps the default heap and
    // thread-local association live; this entry runs with exclusive access to
    // the current thread's local allocation state.
    let Some(theap) = (unsafe { mi_theap_default_local() }) else {
        return ptr::null_mut();
    };

    // SAFETY: the public contract keeps `fname` readable, and the localized
    // default heap/configuration remain live and locally exclusive through the
    // temporary allocation, duplication, and release transaction.
    unsafe { mi_theap_realpath_typed(theap, fname, resolved_name) }
}
