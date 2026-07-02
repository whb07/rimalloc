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

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ThreadLocalSlotIndex(usize);

impl ThreadLocalSlotIndex {
    #[inline(always)]
    const fn new(raw: usize) -> Self {
        Self(raw)
    }

    #[inline(always)]
    const fn get(self) -> usize {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ThreadLocalSlotCount(usize);

impl ThreadLocalSlotCount {
    #[inline(always)]
    const fn new(raw: usize) -> Self {
        Self(raw)
    }

    #[inline(always)]
    const fn get(self) -> usize {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ThreadLocalKeyVersion(usize);

impl ThreadLocalKeyVersion {
    #[inline(always)]
    const fn new(version: usize) -> Self {
        Self(version)
    }

    #[inline(always)]
    const fn get(self) -> usize {
        self.0
    }
}

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
// SAFETY[AUDIT:threadlocal-empty-sentinel-boundary]: the empty sentinel is immutable after
// static initialization and only exposes null/zero slot metadata.
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
// SAFETY[AUDIT:threadlocal-tls-shim-boundary]: declares the C initial-exec TLS cell
// and accessors owned by `src/tls_shim.c`.
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
// SAFETY[AUDIT:threadlocal-tls-shim-boundary]: reads the hidden Linux TLS cell using
// the same initial-exec `@GOTTPOFF` sequence as the C shim.
#[inline(always)]
unsafe fn mi_linux_x64_thread_locals_get() -> *mut mi_thread_locals_t {
    let tls: *mut mi_thread_locals_t;
    // SAFETY[AUDIT:threadlocal-tls-shim-boundary]: the inline assembly reads only
    // the thread-local `mi_thread_locals` slot and writes the output register.
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
// SAFETY[AUDIT:threadlocal-tls-shim-boundary]: writes the hidden Linux TLS cell using
// the same initial-exec `@GOTTPOFF` sequence as the C shim.
#[inline(always)]
unsafe fn mi_linux_x64_thread_locals_set(tls: *mut mi_thread_locals_t) {
    // SAFETY[AUDIT:threadlocal-tls-shim-boundary]: the inline assembly writes the
    // caller-provided TLS pointer into this thread's TLS slot.
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
    // SAFETY[AUDIT:threadlocal-tls-shim-boundary]: x86_64 Linux reads the current
    // thread's TLS pointer from the C shim cell.
    unsafe {
        mi_linux_x64_thread_locals_get()
    }
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    {
        // SAFETY[AUDIT:threadlocal-tls-shim-boundary]: non-inline Linux paths read
        // the current thread's TLS pointer through the C shim accessor.
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
    // SAFETY[AUDIT:threadlocal-tls-shim-boundary]: x86_64 Linux writes this thread's
    // TLS pointer through the C shim cell.
    unsafe {
        mi_linux_x64_thread_locals_set(tls);
    }
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    // SAFETY[AUDIT:threadlocal-tls-shim-boundary]: non-inline Linux paths write this
    // thread's TLS pointer through the C shim accessor.
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
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: callers pass a live thread-local
    // slot block; this helper only returns its flexible slot array pointer.
    unsafe { (*tls).slots.as_mut_ptr() }
}

#[inline]
fn thread_locals_size(slot_count: ThreadLocalSlotCount) -> Option<usize> {
    size_of::<mi_thread_locals_t>()
        .checked_add(slot_count.get().checked_mul(size_of::<mi_tls_slot_t>())?)
}

#[inline]
fn bitmap_alloc_size(bit_count: usize) -> usize {
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: null output means the bitmap
    // helper computes size only and does not publish a chunk count.
    unsafe { mi_bitmap_size(bit_count, ptr::null_mut()) }
}

#[inline]
fn mi_key_index(key: mi_thread_local_t) -> ThreadLocalSlotIndex {
    ThreadLocalSlotIndex::new(key & MI_TLS_IDX_MASK)
}

#[inline]
fn mi_key_version(key: mi_thread_local_t) -> ThreadLocalKeyVersion {
    ThreadLocalKeyVersion::new(key >> MI_TLS_IDX_BITS)
}

#[inline]
fn mi_key_create(
    slot_index: ThreadLocalSlotIndex,
    version: ThreadLocalKeyVersion,
) -> mi_thread_local_t {
    crate::internal::mi_debug_assert!(version.get() != 0 && version.get() <= MI_TLS_VERSION_MAX);
    crate::internal::mi_debug_assert!(slot_index.get() <= MI_TLS_IDX_MAX);
    let key = (version.get() << MI_TLS_IDX_BITS) | slot_index.get();
    crate::internal::mi_debug_assert!(key != 0);
    key
}

#[inline]
fn mi_thread_locals_expand(least_idx: ThreadLocalSlotIndex) -> Option<*mut mi_thread_locals_t> {
    let tls_old = thread_locals_ptr();
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: `thread_locals_ptr` returns the
    // current thread's live TLS block or the immutable empty sentinel.
    let count_old = unsafe { (*tls_old).count };
    let mut count = if count_old == 0 {
        16
    } else if count_old >= 1024 {
        count_old.saturating_add(1024)
    } else {
        count_old.saturating_mul(2)
    };
    if count <= least_idx.get() {
        count = least_idx.get() + 1;
    }
    if count > MI_TLS_IDX_MAX {
        return None;
    }

    let size_new = thread_locals_size(ThreadLocalSlotCount::new(count))?;
    let old_ptr = if count_old == 0 {
        ptr::null_mut()
    } else {
        tls_old.cast()
    };
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: `old_ptr` is either null or the
    // current thread's owned TLS block; `size_new` was checked above.
    let tls = unsafe { mi_rezalloc(old_ptr, size_new).cast::<mi_thread_locals_t>() };

    if tls.is_null() {
        return None;
    }
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: `tls` is writable newly allocated
    // or resized TLS storage large enough for `count` slots.
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
    crate::internal::mi_debug_assert!(tls == thread_locals_ptr());
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: expansion ensures `idx` is within
    // the current thread-local slot count.
    crate::internal::mi_debug_assert!(idx.get() < unsafe { (*tls).count });
    // SAFETY[AUDIT:threadlocal-slot-block-boundary]: `idx` is in bounds and the TLS
    // block belongs to this thread.
    unsafe {
        let slots = tls_slots(tls);
        (*slots.add(idx.get())).value = val;
        (*slots.add(idx.get())).version = mi_key_version(key).get();
    }
    true
}

struct ThreadLocalKeepSet(ptr::NonNull<bool>);

impl ThreadLocalKeepSet {
    /// # Safety
    /// `keep_set` must be valid for a `bool` write.
    // SAFETY[AUDIT:threadlocal-claim-callback-boundary]: raw callback state is converted to
    // a non-null write witness before the typed claim helper sees it.
    #[inline(always)]
    unsafe fn from_raw(keep_set: *mut bool) -> Self {
        crate::internal::mi_assert_internal!(!keep_set.is_null());
        // SAFETY[AUDIT:threadlocal-claim-callback-boundary]: null was asserted above and
        // callers provide a writable bool slot.
        Self(unsafe { ptr::NonNull::new_unchecked(keep_set) })
    }

    #[inline(always)]
    fn clear(self) {
        // SAFETY[AUDIT:threadlocal-claim-callback-boundary]: the witness was constructed
        // only from a non-null writable bool slot.
        unsafe {
            *self.0.as_ptr() = false;
        }
    }
}

#[inline]
// SAFETY[AUDIT:threadlocal-claim-callback-boundary]: C bitmap traversal calls this callback
// with raw arena and keep-set arguments that are immediately wrapped.
unsafe extern "C" fn mi_thread_local_claim_fun(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_set: *mut bool,
) -> bool {
    // SAFETY[AUDIT:threadlocal-claim-callback-boundary]: raw callback arguments are wrapped
    // into typed witnesses before mutation.
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
    // SAFETY[AUDIT:threadlocal-key-claim-boundary]: `slots` is the acquired published
    // free bitmap, `idx` is a valid out slot, and the callback only clears its
    // keep flag through a typed witness.
    if unsafe { mi_bitmap_try_find_and_claim(slots, ThreadSeq::new(0), &mut idx, claim_fun, None) }
    {
        let mut version = mi_thread_locals_version
            .load(Ordering::Relaxed)
            .wrapping_add(1);
        if version >= MI_TLS_VERSION_MAX {
            version = 1;
        }
        mi_thread_locals_version.store(version, Ordering::Relaxed);
        mi_key_create(
            ThreadLocalSlotIndex::new(idx.get()),
            ThreadLocalKeyVersion::new(version),
        )
    } else {
        0
    }
}

fn mi_thread_local_create_expand() -> bool {
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    let oldcount = if slots.is_null() {
        0
    } else {
        // SAFETY[AUDIT:threadlocal-free-bitmap-boundary]: non-null `slots` is the
        // acquired published bitmap.
        unsafe { mi_bitmap_max_bits(slots) }
    };
    let Some(newcount) = oldcount.checked_add(1024) else {
        return false;
    };
    if newcount > MI_TLS_IDX_MAX {
        return false;
    }
    let newsize = bitmap_alloc_size(newcount);
    // SAFETY[AUDIT:threadlocal-free-bitmap-boundary]: allocates aligned bitmap storage
    // for the expanded free-slot bitmap.
    let newslots = unsafe { mi_zalloc_aligned(newsize, MI_BCHUNK_SIZE).cast::<mi_bitmap_t>() };
    if newslots.is_null() {
        return false;
    }

    if !slots.is_null() {
        // SAFETY[AUDIT:threadlocal-free-bitmap-boundary]: `slots` is the previous
        // published bitmap; the null output only asks for its byte size.
        let oldsize = unsafe { mi_bitmap_size(oldcount, ptr::null_mut()) };
        // SAFETY[AUDIT:threadlocal-free-bitmap-boundary]: both bitmaps have at least
        // `oldsize` bytes, and ownership moves to `newslots` before publish.
        unsafe {
            ptr::copy_nonoverlapping(slots.cast::<u8>(), newslots.cast::<u8>(), oldsize);
            mi_free(slots.cast());
        }
    }

    // SAFETY[AUDIT:threadlocal-free-bitmap-boundary]: `newslots` points to writable
    // bitmap storage sized for `newcount`; new bits are published as free.
    unsafe {
        mi_bitmap_init(newslots, newcount, true);
        mi_bitmap_unsafe_setN(newslots, oldcount, newcount - oldcount);
    }
    mi_thread_locals_free.store(newslots, Ordering::Release);
    true
}

// SAFETY[AUDIT:threadlocal-global-manager-boundary]: initializes process-global dynamic
// TLS bookkeeping once with acquire/release publication.
#[inline(never)]
pub extern "C" fn _mi_thread_locals_init() {
    if mi_thread_locals_initialized
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        // SAFETY[AUDIT:threadlocal-global-manager-boundary]: the once transition gives this
        // thread exclusive initialization access to the static lock.
        unsafe {
            mi_lock_init(ptr::addr_of_mut!(mi_thread_locals_lock));
        }
        mi_thread_locals_free.store(ptr::null_mut(), Ordering::Relaxed);
        mi_thread_locals_version.store(0, Ordering::Relaxed);
    }
}

#[used]
static MI_THREAD_LOCALS_INIT_KEEP: extern "C" fn() = _mi_thread_locals_init;

// SAFETY[AUDIT:threadlocal-global-manager-boundary]: tears down the global free bitmap
// under the static lock when dynamic TLS slots were allocated.
#[inline(never)]
pub extern "C" fn _mi_thread_locals_done() {
    if mi_thread_locals_free.load(Ordering::Acquire).is_null() {
        return;
    }
    // SAFETY[AUDIT:threadlocal-global-manager-boundary]: global bitmap teardown is
    // serialized by the static lock.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let slots = mi_thread_locals_free.swap(ptr::null_mut(), Ordering::AcqRel);
    if !slots.is_null() {
        // SAFETY[AUDIT:threadlocal-global-manager-boundary]: `slots` was removed from the
        // global publication point and is now owned by this teardown path.
        unsafe {
            mi_free(slots.cast());
        }
    }
    // SAFETY[AUDIT:threadlocal-global-manager-boundary]: releases and finalizes the static
    // lock after the global free bitmap has been unpublished.
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
        mi_lock_done(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
}

#[used]
static MI_THREAD_LOCALS_DONE_KEEP: extern "C" fn() = _mi_thread_locals_done;

// SAFETY[AUDIT:threadlocal-key-access-boundary]: stores a value into this thread's TLS
// slot block or expands that block before storing.
#[inline(never)]
pub extern "C" fn _mi_thread_local_set(
    key: mi_thread_local_t,
    val: *mut core::ffi::c_void,
) -> bool {
    let tls = thread_locals_ptr();
    crate::internal::mi_debug_assert!(!tls.is_null());
    crate::internal::mi_debug_assert!(key != 0);
    let idx = mi_key_index(key);
    // SAFETY[AUDIT:threadlocal-key-access-boundary]: `tls` is this thread's TLS block;
    // versioned slot writes are in bounds or delegated to expansion.
    unsafe {
        if idx.get() < (*tls).count {
            let slots = tls_slots(tls);
            (*slots.add(idx.get())).value = val;
            (*slots.add(idx.get())).version = mi_key_version(key).get();
            true
        } else {
            mi_thread_local_set_expand(key, val)
        }
    }
}

#[used]
static MI_THREAD_LOCAL_SET_KEEP: extern "C" fn(mi_thread_local_t, *mut core::ffi::c_void) -> bool =
    _mi_thread_local_set;

// SAFETY[AUDIT:threadlocal-key-access-boundary]: reads a value from this thread's TLS
// slot block only when the key version still matches.
#[inline(never)]
pub extern "C" fn _mi_thread_local_get(key: mi_thread_local_t) -> *mut core::ffi::c_void {
    let tls = thread_locals_ptr();
    crate::internal::mi_debug_assert!(!tls.is_null());
    crate::internal::mi_debug_assert!(key != 0);
    let idx = mi_key_index(key);
    // SAFETY[AUDIT:threadlocal-key-access-boundary]: `tls` is this thread's TLS block;
    // reads are in bounds and stale key versions return null.
    unsafe {
        if idx.get() < (*tls).count {
            let slots = tls_slots(tls);
            if mi_key_version(key).get() == (*slots.add(idx.get())).version {
                (*slots.add(idx.get())).value
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

// SAFETY[AUDIT:threadlocal-thread-done-boundary]: frees only this thread's dynamically
// allocated TLS slot block and restores the empty sentinel.
#[inline(never)]
pub extern "C" fn _mi_thread_locals_thread_done() {
    let tls = thread_locals_ptr();
    // SAFETY[AUDIT:threadlocal-thread-done-boundary]: `tls` is the current thread's TLS
    // pointer; positive count distinguishes allocated blocks from the sentinel.
    if unsafe { !tls.is_null() && (*tls).count > 0 } {
        // SAFETY[AUDIT:threadlocal-thread-done-boundary]: a positive count means this
        // thread owns a dynamically allocated TLS block.
        unsafe {
            mi_free(tls.cast());
        }
        set_thread_locals_ptr(thread_locals_empty_ptr());
    }
}

#[used]
static MI_THREAD_LOCALS_THREAD_DONE_KEEP: extern "C" fn() = _mi_thread_locals_thread_done;

// SAFETY[AUDIT:threadlocal-key-create-boundary]: allocates a versioned TLS key under
// the global bitmap lock, expanding the bitmap when needed.
pub extern "C" fn _mi_thread_local_create() -> mi_thread_local_t {
    _mi_thread_locals_init();
    // SAFETY[AUDIT:threadlocal-key-create-boundary]: key creation and bitmap expansion
    // are serialized by the static lock.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let mut key = mi_thread_local_claim();
    if key == 0 && mi_thread_local_create_expand() {
        key = mi_thread_local_claim();
    }
    // SAFETY[AUDIT:threadlocal-key-create-boundary]: releases the lock acquired for key
    // claim/expansion.
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    key
}

// SAFETY[AUDIT:threadlocal-key-free-boundary]: returns a versioned key's slot to the
// global free bitmap while holding the bitmap lock.
#[inline(never)]
pub extern "C" fn _mi_thread_local_free(key: mi_thread_local_t) {
    if key == 0 {
        return;
    }
    let idx = mi_key_index(key);
    // SAFETY[AUDIT:threadlocal-key-free-boundary]: slot free mutates the global free
    // bitmap while serialized by the static lock.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    // SAFETY[AUDIT:threadlocal-key-free-boundary]: non-null `slots` is the acquired
    // published free bitmap and can be queried while the lock is held.
    if !slots.is_null() && idx.get() < unsafe { mi_bitmap_max_bits(slots) } {
        // SAFETY[AUDIT:threadlocal-key-free-boundary]: `idx` is within the bitmap and
        // marks the slot as free for a future versioned key.
        unsafe {
            mi_bitmap_set(slots, idx.get());
        }
    }
    // SAFETY[AUDIT:threadlocal-key-free-boundary]: releases the lock acquired for
    // free bitmap mutation.
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
}

#[used]
static MI_THREAD_LOCAL_FREE_KEEP: extern "C" fn(mi_thread_local_t) = _mi_thread_local_free;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_helpers_split_index_and_version_domains() {
        let key = mi_key_create(ThreadLocalSlotIndex::new(17), ThreadLocalKeyVersion::new(3));
        assert_eq!(mi_key_index(key).get(), 17);
        assert_eq!(mi_key_version(key).get(), 3);
    }

    #[test]
    fn keep_set_witness_clears_claim_flag() {
        let mut keep_flag = true;
        // SAFETY[AUDIT:threadlocal-claim-callback-boundary]: the test passes a live
        // stack-local bool as the callback keep-set slot.
        let keep_set = unsafe { ThreadLocalKeepSet::from_raw(&mut keep_flag) };
        assert!(mi_thread_local_claim_fun_typed(
            SliceIndex::new(0),
            None,
            keep_set
        ));
        assert!(!keep_flag);
    }
}
