//! Thread identity and thread-heap TLS association backends.

#[cfg(any(
    miri,
    not(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        target_arch = "x86_64",
        not(target_env = "musl")
    ))
))]
use core::cell::Cell;
use core::ffi::c_void;
use core::ptr;
#[cfg(any(
    miri,
    not(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))
))]
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(any(windows, target_os = "openbsd", target_os = "android"))]
use crate::allocator::options::_mi_error_message_fmt;
use crate::allocator::thread_heap::{_mi_theap_incref_typed, mi_theap_decref_inline};
use crate::internal::atomic::{mi_atomic_do_once_enter, mi_atomic_once_s, mi_atomic_once_t};
use crate::internal::bits::MI_SIZE_SIZE;
#[cfg(windows)]
use crate::platform::MI_TLS_EXPANSION_SLOT;
#[cfg(windows)]
use crate::types::EFAULT;
#[cfg(any(windows, target_os = "openbsd", target_os = "android"))]
use crate::types::Errno;
use crate::types::{ThreadHeapLocal, ThreadHeapPtr, ThreadId};
use crate::types::{mi_theap_t, mi_thread_local_t, mi_threadid_t};

const MI_TLS_DIRECT_SLOTS: usize = 64;
const MI_TLS_EXPANSION_SLOTS: usize = 1024;
const MI_TLS_DIRECT_FIRST: usize = if MI_SIZE_SIZE == 4 {
    0x0E10 / MI_SIZE_SIZE
} else {
    0x1480 / MI_SIZE_SIZE
};
const MI_TLS_INITIAL_SLOT: usize = 5;
const TLS_OUT_OF_INDEXES: u32 = u32::MAX;

pub(crate) static mut _mi_theap_default_slot: usize = MI_TLS_INITIAL_SLOT;
pub(crate) static mut _mi_theap_default_expansion_slot: usize = 0;
pub(crate) static mut _mi_theap_cached_slot: usize = MI_TLS_INITIAL_SLOT;
pub(crate) static mut _mi_theap_cached_expansion_slot: usize = 0;

#[cfg(windows)]
static mut mi_tls_raw_index_default: u32 = TLS_OUT_OF_INDEXES;
#[cfg(windows)]
static mut mi_tls_raw_index_cached: u32 = TLS_OUT_OF_INDEXES;

pub(crate) static mut _mi_theap_default_key: mi_thread_local_t = 0;
pub(crate) static mut _mi_theap_cached_key: mi_thread_local_t = 0;

#[cfg(not(target_os = "linux"))]
pub(crate) static mut __mi_theap_main: *mut mi_theap_t = ptr::null_mut();

#[cfg(any(
    miri,
    not(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))
))]
static NEXT_THREAD_ID: AtomicUsize = AtomicUsize::new(8);

#[cfg(any(
    miri,
    not(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))
))]
thread_local! {
    static THREAD_ID: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(
    miri,
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
thread_local! {
    static THEAP_MAIN: Cell<*mut mi_theap_t> = const { Cell::new(ptr::null_mut()) };
}

// SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
#[cfg(all(target_os = "linux", any(feature = "tls-shim", not(feature = "std"))))]
unsafe extern "C" {
    fn _mi_tls_theap_main_get() -> *mut mi_theap_t;
    #[link_name = "_mi_tls_theap_main_set"]
    fn _mi_tls_theap_main_set_raw(theap: *mut c_void);
}

#[cfg(all(target_os = "linux", any(feature = "tls-shim", not(feature = "std"))))]
#[inline(always)]
unsafe fn _mi_tls_theap_main_set(theap: Option<ThreadHeapPtr>) {
    let theap = theap
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
        .cast();
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe { _mi_tls_theap_main_set_raw(theap) };
}

#[cfg(all(
    not(miri),
    target_os = "linux",
    any(feature = "tls-shim", not(feature = "std")),
    target_arch = "x86_64",
    not(target_env = "musl")
))]
#[inline(always)]
unsafe fn mi_linux_x64_theap_main_get() -> *mut mi_theap_t {
    let theap: *mut mi_theap_t;
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + __mi_theap_main@GOTTPOFF]",
            "mov {theap}, qword ptr fs:[{tls_offset}]",
            tls_offset = lateout(reg) _,
            theap = lateout(reg) theap,
            options(nostack, preserves_flags, readonly),
        );
    }
    theap
}

#[cfg(all(
    not(miri),
    target_os = "linux",
    any(feature = "tls-shim", not(feature = "std")),
    target_arch = "x86_64",
    not(target_env = "musl")
))]
#[inline(always)]
unsafe fn mi_linux_x64_theap_main_set(theap: Option<ThreadHeapPtr>) {
    let theap = theap.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + __mi_theap_main@GOTTPOFF]",
            "mov qword ptr fs:[{tls_offset}], {theap}",
            tls_offset = out(reg) _,
            theap = in(reg) theap,
            options(nostack, preserves_flags),
        );
    }
}

#[inline]
pub(crate) fn mi_theap_main_tls_peek() -> *mut mi_theap_t {
    #[cfg(miri)]
    {
        THEAP_MAIN.with(Cell::get)
    }
    #[cfg(all(
        not(miri),
        target_os = "linux",
        feature = "std",
        not(feature = "tls-shim")
    ))]
    {
        THEAP_MAIN.with(Cell::get)
    }
    #[cfg(all(
        not(miri),
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        target_arch = "x86_64",
        not(target_env = "musl")
    ))]
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe {
        mi_linux_x64_theap_main_get()
    }
    #[cfg(all(
        not(miri),
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe {
        _mi_tls_theap_main_get()
    }
    #[cfg(not(any(miri, target_os = "linux")))]
    {
        THEAP_MAIN.with(Cell::get)
    }
}

#[inline]
pub(crate) fn mi_theap_main_tls_set(theap: Option<ThreadHeapPtr>) {
    #[cfg(miri)]
    {
        let theap = theap.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        THEAP_MAIN.with(|slot| slot.set(theap));
    }
    #[cfg(all(
        not(miri),
        target_os = "linux",
        feature = "std",
        not(feature = "tls-shim")
    ))]
    {
        let theap = theap.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        THEAP_MAIN.with(|slot| slot.set(theap));
    }
    #[cfg(all(
        not(miri),
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        target_arch = "x86_64",
        not(target_env = "musl")
    ))]
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe {
        mi_linux_x64_theap_main_set(theap);
    }
    #[cfg(all(
        not(miri),
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    // SAFETY: main thread-heap TLS access publishes only typed thread-heap pointers.
    unsafe {
        _mi_tls_theap_main_set(theap);
    }
    #[cfg(not(any(miri, target_os = "linux")))]
    {
        let theap = theap.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        THEAP_MAIN.with(|slot| slot.set(theap));
    }
}

#[inline]
#[cfg(any(
    miri,
    not(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))
))]
pub(crate) fn fallback_thread_id() -> mi_threadid_t {
    THREAD_ID.with(|slot| {
        let tid = slot.get();
        if tid != 0 {
            tid
        } else {
            let tid = NEXT_THREAD_ID.fetch_add(4, Ordering::Relaxed);
            slot.set(tid);
            tid
        }
    })
}

#[inline]
pub(super) fn current_thread_id() -> mi_threadid_t {
    crate::platform::_mi_prim_thread_id()
}

// SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
#[cfg(any(target_os = "openbsd", target_os = "android"))]
unsafe extern "C" {
    fn pthread_key_delete(key: mi_thread_local_t) -> i32;
}

// SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
#[cfg(windows)]
unsafe extern "system" {
    fn TlsAlloc() -> u32;
    fn TlsFree(dw_tls_index: u32) -> i32;
    fn TlsSetValue(dw_tls_index: u32, lp_tls_value: *mut c_void) -> i32;
}

#[cfg(windows)]
unsafe fn mi_win_tls_slot_alloc(
    slot: *mut usize,
    extended: *mut usize,
    raw_index: *mut u32,
) -> bool {
    // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
    let index = unsafe { TlsAlloc() };
    // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
    unsafe { raw_index.write(index) };
    if index == TLS_OUT_OF_INDEXES {
        // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            extended.write(0);
            slot.write(0);
        }
        false
    } else if usize::try_from(index)
        .ok()
        .is_some_and(|i| i < MI_TLS_DIRECT_SLOTS)
    {
        // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            extended.write(0);
            slot.write(index as usize + MI_TLS_DIRECT_FIRST);
        }
        true
    } else if usize::try_from(index)
        .ok()
        .is_some_and(|i| i < MI_TLS_DIRECT_SLOTS + MI_TLS_EXPANSION_SLOTS - 1)
    {
        // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            extended.write(index as usize - MI_TLS_DIRECT_SLOTS);
            slot.write(MI_TLS_EXPANSION_SLOT);
        }
        true
    } else {
        // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            _mi_error_message_fmt(
                Errno::EFAULT,
                format_args!("returned tls index was too high ({})\n", index),
            );
            let _ = TlsFree(index);
            raw_index.write(TLS_OUT_OF_INDEXES);
            extended.write(0);
            slot.write(0);
        }
        false
    }
}

#[cfg(windows)]
unsafe fn mi_win_tls_slot_free(raw_index: *mut u32) {
    // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
    let index = unsafe { raw_index.read() };
    if index != TLS_OUT_OF_INDEXES {
        // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            let _ = TlsFree(index);
            raw_index.write(TLS_OUT_OF_INDEXES);
        }
    }
}

#[cfg(windows)]
unsafe fn mi_win_tls_slot_set(slot: usize, extended_slot: usize, value: *mut c_void) {
    crate::internal::mi_assert_internal!(
        (slot >= MI_TLS_DIRECT_FIRST && slot < MI_TLS_DIRECT_FIRST + MI_TLS_DIRECT_SLOTS)
            || slot == MI_TLS_EXPANSION_SLOT
    );
    if slot < MI_TLS_DIRECT_FIRST + MI_TLS_DIRECT_SLOTS {
        crate::platform::mi_prim_tls_slot_set(slot, value);
    } else {
        crate::internal::mi_assert_internal!(extended_slot < MI_TLS_EXPANSION_SLOTS);
        // SAFETY: Windows TLS slot setup and teardown use process-global TLS metadata under one-time init.
        let _ = unsafe { TlsSetValue((extended_slot + MI_TLS_DIRECT_SLOTS) as u32, value) };
    }
}

#[cfg(not(windows))]
unsafe fn mi_win_tls_slot_set(_slot: usize, _extended_slot: usize, _value: *mut c_void) {}

#[cfg(any(target_os = "openbsd", target_os = "android"))]
unsafe fn mi_pthread_key_create(pkey: *mut mi_thread_local_t) -> i32 {
    let key = crate::allocator::threadlocal::_mi_thread_local_create();
    if key == 0 {
        EFAULT
    } else {
        // SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
        unsafe { pkey.write(key) };
        0
    }
}

pub(super) fn mi_tls_slots_init() {
    static TLS_SLOTS_INIT: mi_atomic_once_t = mi_atomic_once_s::new();
    if let Some(_once) =
        // SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
        unsafe { mi_atomic_do_once_enter(ptr::addr_of!(TLS_SLOTS_INIT).cast_mut()) }
    {
        #[cfg(windows)]
        // SAFETY: one-time initialization of process-global TLS slot metadata.
        // SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            let mut ok = mi_win_tls_slot_alloc(
                ptr::addr_of_mut!(_mi_theap_default_slot),
                ptr::addr_of_mut!(_mi_theap_default_expansion_slot),
                ptr::addr_of_mut!(mi_tls_raw_index_default),
            );
            if ok {
                ok = mi_win_tls_slot_alloc(
                    ptr::addr_of_mut!(_mi_theap_cached_slot),
                    ptr::addr_of_mut!(_mi_theap_cached_expansion_slot),
                    ptr::addr_of_mut!(mi_tls_raw_index_cached),
                );
            }
            if !ok {
                _mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!(
                        "unable to allocate fast TLS user slot (0x{:x})\n",
                        _mi_theap_cached_slot
                    ),
                );
            }
        }

        #[cfg(any(target_os = "openbsd", target_os = "android"))]
        // SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
        unsafe {
            let mut err = mi_pthread_key_create(ptr::addr_of_mut!(_mi_theap_default_key));
            if err == 0 {
                err = mi_pthread_key_create(ptr::addr_of_mut!(_mi_theap_cached_key));
            }
            if err != 0 {
                _mi_error_message_fmt(
                    Errno::EFAULT,
                    format_args!("unable to allocate pthread keys (error {})\n", err),
                );
            }
        }
    }
}

pub(super) fn mi_tls_slots_done() {
    #[cfg(windows)]
    // SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
    unsafe {
        mi_win_tls_slot_free(ptr::addr_of_mut!(mi_tls_raw_index_default));
        mi_win_tls_slot_free(ptr::addr_of_mut!(mi_tls_raw_index_cached));
    }

    #[cfg(any(target_os = "openbsd", target_os = "android"))]
    // SAFETY: platform TLS slot/key setup and teardown use process-global TLS metadata under one-time init.
    unsafe {
        if _mi_theap_default_key != 0 {
            let _ = pthread_key_delete(_mi_theap_default_key);
            _mi_theap_default_key = 0;
        }
        if _mi_theap_cached_key != 0 {
            let _ = pthread_key_delete(_mi_theap_cached_key);
            _mi_theap_cached_key = 0;
        }
    }
}

pub(super) unsafe fn theap_default_set(theap: *mut mi_theap_t) {
    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    let theap_old = unsafe { crate::platform::_mi_theap_default() };
    crate::internal::mi_assert_internal!(!theap.is_null());
    crate::internal::mi_assert_internal!(
        // SAFETY: default thread-heap association validates local ownership before TLS publication.
        unsafe { (*(*theap).tld).thread_id } == 0
            // SAFETY: default thread-heap association validates local ownership before TLS publication.
            || unsafe { (*(*theap).tld).thread_id } == crate::allocator::init::_mi_thread_id()
    );
    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    let theap = unsafe { ThreadHeapPtr::new_unchecked(theap) };
    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    let current = unsafe { ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id()) };
    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    let theap = match unsafe { theap.try_local(current) } {
        Ok(theap) => theap,
        // SAFETY: default thread-heap association validates local ownership before TLS publication.
        Err(theap) => unsafe { ThreadHeapLocal::new_unchecked(theap) },
    };

    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    unsafe { theap_default_set_typed(theap, ThreadHeapPtr::new(theap_old)) };
}

unsafe fn theap_default_set_typed(theap: ThreadHeapLocal, theap_old: Option<ThreadHeapPtr>) {
    let theap_ptr = theap.ptr();
    let theap_raw = theap_ptr.as_ptr();
    mi_tls_slots_init();
    if crate::platform::MI_TLS_MODEL_THREAD_LOCAL {
        crate::platform::mi_prim_thread_local_default_set(Some(theap_ptr));
    } else if crate::platform::MI_TLS_MODEL_FIXED_SLOT {
        crate::platform::mi_prim_tls_slot_set(
            crate::platform::MI_TLS_MODEL_FIXED_SLOT_DEFAULT,
            theap_raw.cast(),
        );
    } else if crate::platform::MI_TLS_MODEL_DYNAMIC_WIN32 {
        // SAFETY: default thread-heap publication writes the validated local heap pointer into the selected TLS backend.
        unsafe {
            mi_win_tls_slot_set(
                _mi_theap_default_slot,
                _mi_theap_default_expansion_slot,
                theap_raw.cast(),
            );
        }
    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    } else if crate::platform::MI_TLS_MODEL_DYNAMIC_PTHREADS
        // SAFETY: _mi_theap_default_key is the TLS key initialized during allocator startup; reading it only checks initialization.
        && unsafe { _mi_theap_default_key } != 0
    {
        let _ = crate::allocator::threadlocal::_mi_thread_local_set(
            // SAFETY: default thread-heap publication writes the validated local heap pointer into the selected TLS backend.
            unsafe { _mi_theap_default_key },
            theap_raw.cast(),
        );
    }

    // SAFETY: default thread-heap association validates local ownership before TLS publication.
    if unsafe { crate::internal::mi_theap_is_initialized(theap_raw) } {
        // SAFETY: default thread-heap publication writes the validated local heap pointer into the selected TLS backend.
        unsafe { crate::platform::_mi_prim_thread_associate_default_theap(theap_raw) };
        // SAFETY: default thread-heap association validates local ownership before TLS publication.
        if unsafe {
            crate::allocator::init::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap_raw))
        } {
            mi_theap_main_tls_set(Some(theap_ptr));
        }
    }

    if let Some(theap_old) = theap_old {
        let theap_old = theap_old.as_ptr();
        // SAFETY: default thread-heap association validates local ownership before TLS publication.
        if unsafe { crate::internal::mi_theap_is_initialized(theap_old) }
            // SAFETY: default thread-heap association validates local ownership before TLS publication.
            && unsafe {
                crate::allocator::init::_mi_is_heap_main(crate::internal::_mi_theap_heap(theap_old))
            }
        {
            mi_theap_main_tls_set(ThreadHeapPtr::new(theap_old));
        }
    }
}

pub(super) unsafe fn theap_cached_set(theap: *mut mi_theap_t) {
    // SAFETY: cached thread-heap association validates local ownership before TLS publication.
    let prev = unsafe { crate::platform::_mi_theap_cached() };
    if ptr::eq(prev, theap) {
        return;
    }
    let theap = ThreadHeapPtr::new(theap).map(|theap| {
        // SAFETY: cached thread-heap association validates local ownership before TLS publication.
        let current = unsafe { ThreadId::new_unchecked(crate::platform::_mi_prim_thread_id()) };
        // SAFETY: cached thread-heap association validates local ownership before TLS publication.
        match unsafe { theap.try_local(current) } {
            Ok(theap) => theap,
            // SAFETY: cached thread-heap association validates local ownership before TLS publication.
            Err(theap) => unsafe { ThreadHeapLocal::new_unchecked(theap) },
        }
    });

    // SAFETY: cached thread-heap association validates local ownership before TLS publication.
    unsafe { theap_cached_set_typed(theap, ThreadHeapPtr::new(prev)) };
}

unsafe fn theap_cached_set_typed(theap: Option<ThreadHeapLocal>, prev: Option<ThreadHeapPtr>) {
    let theap_ptr = theap.map(ThreadHeapLocal::ptr);
    let theap_raw = theap_ptr
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut());
    mi_tls_slots_init();
    if crate::platform::MI_TLS_MODEL_THREAD_LOCAL {
        crate::platform::mi_prim_thread_local_cached_set(theap_ptr);
    } else if crate::platform::MI_TLS_MODEL_FIXED_SLOT {
        crate::platform::mi_prim_tls_slot_set(
            crate::platform::MI_TLS_MODEL_FIXED_SLOT_CACHED,
            theap_raw.cast(),
        );
    } else if crate::platform::MI_TLS_MODEL_DYNAMIC_WIN32 {
        // SAFETY: cached thread-heap association validates local ownership before TLS publication.
        unsafe {
            mi_win_tls_slot_set(
                _mi_theap_cached_slot,
                _mi_theap_cached_expansion_slot,
                theap_raw.cast(),
            );
        }
    // SAFETY: cached thread-heap association validates local ownership before TLS publication.
    } else if crate::platform::MI_TLS_MODEL_DYNAMIC_PTHREADS && unsafe { _mi_theap_cached_key } != 0
    {
        let _ = crate::allocator::threadlocal::_mi_thread_local_set(
            // SAFETY: cached thread-heap association validates local ownership before TLS publication.
            unsafe { _mi_theap_cached_key },
            theap_raw.cast(),
        );
    }

    // SAFETY: TLS now holds the new live thread-heap pointer before its AcqRel
    // increment. The caller retains the new heap's existing reference through
    // this point, and the old TLS reference remains live until decremented.
    // Preserve new incref before old decref; the latter is terminal, so `prev`
    // is not accessed afterward.
    unsafe {
        if let Some(theap) = theap_ptr {
            _mi_theap_incref_typed(theap);
        }
        if let Some(prev) = prev {
            mi_theap_decref_inline(prev);
        }
    }
}
