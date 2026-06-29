//! Dynamic thread-local key management.

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release, mi_lock_t};
use crate::bitmap::{
    MI_BCHUNK_SIZE, mi_bitmap_init, mi_bitmap_max_bits, mi_bitmap_set, mi_bitmap_size, mi_bitmap_t,
    mi_bitmap_try_find_and_claim, mi_bitmap_unsafe_setN,
};
use crate::bits::MI_SIZE_BITS;
use crate::core_types::{ArenaPtr, SliceIndex, ThreadLocalClaimCallback, ThreadSeq};
use crate::mimalloc::{mi_free, mi_rezalloc, mi_zalloc_aligned};
use crate::types::{mi_arena_t, mi_thread_local_t, mi_thread_locals_t, mi_tls_slot_t};

pub const MI_TLS_IDX_BITS: usize = if MI_SIZE_BITS < 64 {
    MI_SIZE_BITS / 2
} else {
    MI_SIZE_BITS / 4
};
pub const MI_TLS_IDX_MASK: usize = (1usize << MI_TLS_IDX_BITS) - 1;
pub const MI_TLS_IDX_MAX: usize = MI_TLS_IDX_MASK;
pub const MI_TLS_VERSION_MAX: usize = usize::MAX >> MI_TLS_IDX_BITS;

#[cfg(any(
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
struct ThreadLocalsEmpty(mi_thread_locals_t);

#[cfg(any(
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
unsafe impl Sync for ThreadLocalsEmpty {}

#[cfg(any(
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
static mi_thread_locals_empty: ThreadLocalsEmpty = ThreadLocalsEmpty(mi_thread_locals_t {
    count: 0,
    slots: [mi_tls_slot_t {
        version: 0,
        value: ptr::null_mut(),
    }],
});

#[cfg(any(
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
thread_local! {
    pub static mi_thread_locals: core::cell::Cell<*mut mi_thread_locals_t> = const { core::cell::Cell::new(ptr::null_mut()) };
}

#[cfg(all(target_os = "linux", any(feature = "tls-shim", not(feature = "std"))))]
unsafe extern "C" {
    static mi_thread_locals_empty: mi_thread_locals_t;
    fn _mi_tls_thread_locals_get() -> *mut mi_thread_locals_t;
    fn _mi_tls_thread_locals_set(tls: *mut mi_thread_locals_t);
}

#[cfg(all(
    target_os = "linux",
    any(feature = "tls-shim", not(feature = "std")),
    target_arch = "x86_64",
    not(target_env = "musl")
))]
#[inline(always)]
unsafe fn mi_linux_x64_thread_locals_get() -> *mut mi_thread_locals_t {
    let tls: *mut mi_thread_locals_t;
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + mi_thread_locals@GOTTPOFF]",
            "mov {tls}, qword ptr fs:[{tls_offset}]",
            tls_offset = lateout(reg) _,
            tls = lateout(reg) tls,
            options(nostack, preserves_flags, readonly),
        );
    }
    tls
}

#[cfg(all(
    target_os = "linux",
    any(feature = "tls-shim", not(feature = "std")),
    target_arch = "x86_64",
    not(target_env = "musl")
))]
#[inline(always)]
unsafe fn mi_linux_x64_thread_locals_set(tls: *mut mi_thread_locals_t) {
    unsafe {
        core::arch::asm!(
            "mov {tls_offset}, qword ptr [rip + mi_thread_locals@GOTTPOFF]",
            "mov qword ptr fs:[{tls_offset}], {tls}",
            tls_offset = out(reg) _,
            tls = in(reg) tls,
            options(nostack, preserves_flags),
        );
    }
}

#[inline]
fn thread_locals_empty_ptr() -> *mut mi_thread_locals_t {
    #[cfg(target_os = "linux")]
    {
        #[cfg(any(feature = "tls-shim", not(feature = "std")))]
        {
            ptr::addr_of!(mi_thread_locals_empty).cast_mut()
        }
        #[cfg(all(feature = "std", not(feature = "tls-shim")))]
        {
            ptr::addr_of!(mi_thread_locals_empty.0).cast_mut()
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        ptr::from_ref(&mi_thread_locals_empty.0).cast_mut()
    }
}

static mut mi_thread_locals_lock: mi_lock_t = mi_lock_t::new();
static mi_thread_locals_initialized: AtomicBool = AtomicBool::new(false);
static mi_thread_locals_free: AtomicPtr<mi_bitmap_t> = AtomicPtr::new(ptr::null_mut());
static mi_thread_locals_version: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn thread_locals_ptr() -> *mut mi_thread_locals_t {
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        target_arch = "x86_64",
        not(target_env = "musl")
    ))]
    unsafe {
        mi_linux_x64_thread_locals_get()
    }
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    {
        unsafe { _mi_tls_thread_locals_get() }
    }
    #[cfg(any(
        not(target_os = "linux"),
        all(target_os = "linux", feature = "std", not(feature = "tls-shim"))
    ))]
    {
        mi_thread_locals.with(|slot| {
            let mut tls = slot.get();
            if tls.is_null() {
                tls = thread_locals_empty_ptr();
                slot.set(tls);
            }
            tls
        })
    }
}

#[inline]
fn set_thread_locals_ptr(tls: *mut mi_thread_locals_t) {
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        target_arch = "x86_64",
        not(target_env = "musl")
    ))]
    unsafe {
        mi_linux_x64_thread_locals_set(tls);
    }
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    unsafe {
        _mi_tls_thread_locals_set(tls);
    }
    #[cfg(any(
        not(target_os = "linux"),
        all(target_os = "linux", feature = "std", not(feature = "tls-shim"))
    ))]
    {
        mi_thread_locals.with(|slot| slot.set(tls));
    }
}

#[inline]
fn tls_slots(tls: *mut mi_thread_locals_t) -> *mut mi_tls_slot_t {
    unsafe { (*tls).slots.as_mut_ptr() }
}

#[inline]
fn thread_locals_size(count: usize) -> Option<usize> {
    size_of::<mi_thread_locals_t>().checked_add(count.checked_mul(size_of::<mi_tls_slot_t>())?)
}

#[inline]
fn bitmap_alloc_size(bit_count: usize) -> usize {
    unsafe { mi_bitmap_size(bit_count, ptr::null_mut()) }
}

#[inline]
fn mi_key_index(key: usize) -> usize {
    key & MI_TLS_IDX_MASK
}

#[inline]
fn mi_key_version(key: usize) -> usize {
    key >> MI_TLS_IDX_BITS
}

#[inline]
fn mi_key_create(index: usize, version: usize) -> mi_thread_local_t {
    crate::mi_debug_assert!(version != 0 && version <= MI_TLS_VERSION_MAX);
    crate::mi_debug_assert!(index <= MI_TLS_IDX_MAX);
    let key = (version << MI_TLS_IDX_BITS) | index;
    crate::mi_debug_assert!(key != 0);
    key
}

#[inline]
fn mi_thread_locals_expand(least_idx: usize) -> Option<*mut mi_thread_locals_t> {
    let tls_old = thread_locals_ptr();
    let count_old = unsafe { (*tls_old).count };
    let mut count = if count_old == 0 {
        16
    } else if count_old >= 1024 {
        count_old.saturating_add(1024)
    } else {
        count_old.saturating_mul(2)
    };
    if count <= least_idx {
        count = least_idx + 1;
    }
    if count > MI_TLS_IDX_MAX {
        return None;
    }

    let size_new = thread_locals_size(count)?;
    let old_ptr = if count_old == 0 {
        ptr::null_mut()
    } else {
        tls_old.cast()
    };
    let tls = unsafe { mi_rezalloc(old_ptr, size_new).cast::<mi_thread_locals_t>() };

    if tls.is_null() {
        return None;
    }
    unsafe {
        (*tls).count = count;
    }
    set_thread_locals_ptr(tls);
    Some(tls)
}

#[inline(never)]
fn mi_thread_local_set_expand(key: mi_thread_local_t, val: *mut core::ffi::c_void) -> bool {
    if val.is_null() {
        return true;
    }
    let idx = mi_key_index(key);
    let tls = match mi_thread_locals_expand(idx) {
        Some(tls) => tls,
        None => return false,
    };
    crate::mi_debug_assert!(tls == thread_locals_ptr());
    crate::mi_debug_assert!(idx < unsafe { (*tls).count });
    unsafe {
        let slots = tls_slots(tls);
        (*slots.add(idx)).value = val;
        (*slots.add(idx)).version = mi_key_version(key);
    }
    true
}

struct ThreadLocalKeepSet(ptr::NonNull<bool>);

impl ThreadLocalKeepSet {
    /// # Safety
    /// `keep_set` must be valid for a `bool` write.
    #[inline(always)]
    unsafe fn from_raw(keep_set: *mut bool) -> Self {
        crate::mi_assert_internal!(!keep_set.is_null());
        Self(unsafe { ptr::NonNull::new_unchecked(keep_set) })
    }

    #[inline(always)]
    fn clear(self) {
        unsafe {
            *self.0.as_ptr() = false;
        }
    }
}

#[inline]
unsafe extern "C" fn mi_thread_local_claim_fun(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_set: *mut bool,
) -> bool {
    unsafe {
        mi_thread_local_claim_fun_typed(
            SliceIndex::new(slice_index),
            ArenaPtr::new(arena),
            ThreadLocalKeepSet::from_raw(keep_set),
        )
    }
}

#[inline(always)]
fn mi_thread_local_claim_fun_typed(
    _slice_index: SliceIndex,
    _arena: Option<ArenaPtr>,
    keep_set: ThreadLocalKeepSet,
) -> bool {
    keep_set.clear();
    true
}

fn mi_thread_local_claim() -> mi_thread_local_t {
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    let mut idx = SliceIndex::new(0);
    if slots.is_null() {
        return 0;
    }
    let claim_fun = ThreadLocalClaimCallback::new(mi_thread_local_claim_fun);
    if unsafe { mi_bitmap_try_find_and_claim(slots, ThreadSeq::new(0), &mut idx, claim_fun, None) }
    {
        let mut version = mi_thread_locals_version
            .load(Ordering::Relaxed)
            .wrapping_add(1);
        if version >= MI_TLS_VERSION_MAX {
            version = 1;
        }
        mi_thread_locals_version.store(version, Ordering::Relaxed);
        mi_key_create(idx.get(), version)
    } else {
        0
    }
}

fn mi_thread_local_create_expand() -> bool {
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    let oldcount = if slots.is_null() {
        0
    } else {
        unsafe { mi_bitmap_max_bits(slots) }
    };
    let Some(newcount) = oldcount.checked_add(1024) else {
        return false;
    };
    if newcount > MI_TLS_IDX_MAX {
        return false;
    }
    let newsize = bitmap_alloc_size(newcount);
    let newslots = unsafe { mi_zalloc_aligned(newsize, MI_BCHUNK_SIZE).cast::<mi_bitmap_t>() };
    if newslots.is_null() {
        return false;
    }

    if !slots.is_null() {
        let oldsize = unsafe { mi_bitmap_size(oldcount, ptr::null_mut()) };
        unsafe {
            ptr::copy_nonoverlapping(slots.cast::<u8>(), newslots.cast::<u8>(), oldsize);
            mi_free(slots.cast());
        }
    }

    unsafe {
        mi_bitmap_init(newslots, newcount, true);
        mi_bitmap_unsafe_setN(newslots, oldcount, newcount - oldcount);
    }
    mi_thread_locals_free.store(newslots, Ordering::Release);
    true
}

#[inline(never)]
pub extern "C" fn _mi_thread_locals_init() {
    if mi_thread_locals_initialized
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        unsafe {
            mi_lock_init(ptr::addr_of_mut!(mi_thread_locals_lock));
        }
        mi_thread_locals_free.store(ptr::null_mut(), Ordering::Relaxed);
        mi_thread_locals_version.store(0, Ordering::Relaxed);
    }
}

#[used]
static MI_THREAD_LOCALS_INIT_KEEP: extern "C" fn() = _mi_thread_locals_init;

#[inline(never)]
pub extern "C" fn _mi_thread_locals_done() {
    if mi_thread_locals_free.load(Ordering::Acquire).is_null() {
        return;
    }
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let slots = mi_thread_locals_free.swap(ptr::null_mut(), Ordering::AcqRel);
    if !slots.is_null() {
        unsafe {
            mi_free(slots.cast());
        }
    }
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
        mi_lock_done(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
}

#[used]
static MI_THREAD_LOCALS_DONE_KEEP: extern "C" fn() = _mi_thread_locals_done;

#[inline(never)]
pub extern "C" fn _mi_thread_local_set(
    key: mi_thread_local_t,
    val: *mut core::ffi::c_void,
) -> bool {
    let tls = thread_locals_ptr();
    crate::mi_debug_assert!(!tls.is_null());
    crate::mi_debug_assert!(key != 0);
    let idx = mi_key_index(key);
    unsafe {
        if idx < (*tls).count {
            let slots = tls_slots(tls);
            (*slots.add(idx)).value = val;
            (*slots.add(idx)).version = mi_key_version(key);
            true
        } else {
            mi_thread_local_set_expand(key, val)
        }
    }
}

#[used]
static MI_THREAD_LOCAL_SET_KEEP: extern "C" fn(mi_thread_local_t, *mut core::ffi::c_void) -> bool =
    _mi_thread_local_set;

#[inline(never)]
pub extern "C" fn _mi_thread_local_get(key: mi_thread_local_t) -> *mut core::ffi::c_void {
    let tls = thread_locals_ptr();
    crate::mi_debug_assert!(!tls.is_null());
    crate::mi_debug_assert!(key != 0);
    let idx = mi_key_index(key);
    unsafe {
        if idx < (*tls).count {
            let slots = tls_slots(tls);
            if mi_key_version(key) == (*slots.add(idx)).version {
                (*slots.add(idx)).value
            } else {
                ptr::null_mut()
            }
        } else {
            ptr::null_mut()
        }
    }
}

#[used]
static MI_THREAD_LOCAL_GET_KEEP: extern "C" fn(mi_thread_local_t) -> *mut core::ffi::c_void =
    _mi_thread_local_get;

#[inline(never)]
pub extern "C" fn _mi_thread_locals_thread_done() {
    let tls = thread_locals_ptr();
    if unsafe { !tls.is_null() && (*tls).count > 0 } {
        unsafe {
            mi_free(tls.cast());
        }
        set_thread_locals_ptr(thread_locals_empty_ptr());
    }
}

#[used]
static MI_THREAD_LOCALS_THREAD_DONE_KEEP: extern "C" fn() = _mi_thread_locals_thread_done;

pub extern "C" fn _mi_thread_local_create() -> mi_thread_local_t {
    _mi_thread_locals_init();
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let mut key = mi_thread_local_claim();
    if key == 0 && mi_thread_local_create_expand() {
        key = mi_thread_local_claim();
    }
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    key
}

#[inline(never)]
pub extern "C" fn _mi_thread_local_free(key: mi_thread_local_t) {
    if key == 0 {
        return;
    }
    let idx = mi_key_index(key);
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    if !slots.is_null() && idx < unsafe { mi_bitmap_max_bits(slots) } {
        unsafe {
            mi_bitmap_set(slots, idx);
        }
    }
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
}

#[used]
static MI_THREAD_LOCAL_FREE_KEEP: extern "C" fn(mi_thread_local_t) = _mi_thread_local_free;
