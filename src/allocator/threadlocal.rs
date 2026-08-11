//! Dynamic thread-local key management.

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::marker::PhantomData;
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
// SAFETY: the empty sentinel is immutable after
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
// SAFETY: declares the C initial-exec TLS cell
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
// SAFETY: reads the hidden Linux TLS cell using
// the same initial-exec `@GOTTPOFF` sequence as the C shim.
#[inline(always)]
unsafe fn mi_linux_x64_thread_locals_get() -> *mut mi_thread_locals_t {
    let tls: *mut mi_thread_locals_t;
    // SAFETY: the inline assembly reads only
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
// SAFETY: writes the hidden Linux TLS cell using
// the same initial-exec `@GOTTPOFF` sequence as the C shim.
#[inline(always)]
unsafe fn mi_linux_x64_thread_locals_set(tls: *mut mi_thread_locals_t) {
    // SAFETY: the inline assembly writes the
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
    // SAFETY: x86_64 Linux reads the current
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
        // SAFETY: non-inline Linux paths read
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
    // SAFETY: x86_64 Linux writes this thread's
    // TLS pointer through the C shim cell.
    unsafe {
        mi_linux_x64_thread_locals_set(tls);
    }
    #[cfg(all(
        target_os = "linux",
        any(feature = "tls-shim", not(feature = "std")),
        not(all(target_arch = "x86_64", not(target_env = "musl")))
    ))]
    // SAFETY: non-inline Linux paths write this
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

struct CurrentThreadLocals {
    ptr: ptr::NonNull<mi_thread_locals_t>,
    _current_thread: PhantomData<*mut ()>,
}

impl CurrentThreadLocals {
    /// # Safety
    /// The current TLS pointer must remain stable and no other
    /// `CurrentThreadLocals` token may remain live until this token is consumed.
    #[inline]
    unsafe fn current_unchecked() -> Self {
        // SAFETY: every TLS backend returns either this thread's live allocated
        // block or the process-lifetime immutable empty sentinel, never null.
        let ptr = unsafe { ptr::NonNull::new_unchecked(thread_locals_ptr()) };
        Self {
            ptr,
            _current_thread: PhantomData,
        }
    }

    #[inline]
    fn count(&self) -> usize {
        // SAFETY: this token keeps the current thread's TLS block stable until
        // it is consumed by expansion or release.
        unsafe { (*self.ptr.as_ptr()).count }
    }

    #[inline]
    fn slot(&self, idx: ThreadLocalSlotIndex) -> Option<ptr::NonNull<mi_tls_slot_t>> {
        if idx.get() >= self.count() {
            return None;
        }
        // SAFETY: the checked index is within this block's flexible slot array.
        Some(unsafe {
            let slots = ptr::addr_of_mut!((*self.ptr.as_ptr()).slots).cast::<mi_tls_slot_t>();
            ptr::NonNull::new_unchecked(slots.add(idx.get()))
        })
    }

    #[inline]
    fn try_set(&self, key: mi_thread_local_t, value: *mut core::ffi::c_void) -> bool {
        let Some(slot) = self.slot(mi_key_index(key)) else {
            return false;
        };
        // SAFETY: `slot` is bounded by this current-thread token and no other
        // thread can access the underlying slot block.
        unsafe {
            ptr::addr_of_mut!((*slot.as_ptr()).value).write(value);
            ptr::addr_of_mut!((*slot.as_ptr()).version).write(mi_key_version(key).get());
        }
        true
    }

    #[inline]
    fn get(&self, key: mi_thread_local_t) -> *mut core::ffi::c_void {
        let Some(slot) = self.slot(mi_key_index(key)) else {
            return ptr::null_mut();
        };
        // SAFETY: `slot` is bounded by this current-thread token and remains
        // stable for the duration of both field reads.
        let version = unsafe { ptr::addr_of!((*slot.as_ptr()).version).read() };
        if mi_key_version(key).get() == version {
            unsafe { ptr::addr_of!((*slot.as_ptr()).value).read() }
        } else {
            ptr::null_mut()
        }
    }

    #[inline]
    fn expand_to_include(self, least_idx: ThreadLocalSlotIndex) -> Option<Self> {
        let count_old = self.count();
        // End the scoped view before entering the allocator, which may access
        // the current thread's TLS while resizing its backing storage.
        let Self { ptr: old, .. } = self;
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
            old.as_ptr().cast()
        };
        // SAFETY: a nonempty token owns this thread's allocation; the empty
        // sentinel maps to null. `size_new` covers the requested slot count.
        let tls = unsafe { mi_rezalloc(old_ptr, size_new).cast::<mi_thread_locals_t>() };
        let ptr = ptr::NonNull::new(tls)?;
        // SAFETY: the allocation is writable and sized for `count` slots.
        unsafe { (*ptr.as_ptr()).count = count };
        set_thread_locals_ptr(ptr.as_ptr());
        Some(Self {
            ptr,
            _current_thread: PhantomData,
        })
    }

    #[inline]
    fn release(self) {
        if self.count() == 0 {
            return;
        }
        // End the scoped view before entering the allocator free path, which
        // may itself consult current-thread TLS.
        let Self { ptr, .. } = self;
        // SAFETY: a positive count distinguishes this thread's allocated block
        // from the immutable empty sentinel.
        unsafe { mi_free(ptr.as_ptr().cast()) };
        set_thread_locals_ptr(thread_locals_empty_ptr());
    }
}

#[inline]
fn thread_locals_size(slot_count: ThreadLocalSlotCount) -> Option<usize> {
    size_of::<mi_thread_locals_t>()
        .checked_add(slot_count.get().checked_mul(size_of::<mi_tls_slot_t>())?)
}

#[inline]
fn bitmap_alloc_size(bit_count: usize) -> usize {
    // SAFETY: null output means the bitmap
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

#[inline(never)]
fn mi_thread_local_set_expand(key: mi_thread_local_t, val: *mut core::ffi::c_void) -> bool {
    if val.is_null() {
        return true;
    }
    let idx = mi_key_index(key);
    // SAFETY: this slow entry holds the only current-thread view and consumes
    // it through expansion before the TLS pointer can change.
    let tls = match unsafe { CurrentThreadLocals::current_unchecked() }.expand_to_include(idx) {
        Some(tls) => tls,
        None => return false,
    };
    crate::internal::mi_debug_assert!(tls.ptr.as_ptr() == thread_locals_ptr());
    crate::internal::mi_debug_assert!(idx.get() < tls.count());
    let was_set = tls.try_set(key, val);
    crate::internal::mi_assert_internal!(was_set);
    true
}

struct ThreadLocalKeepSet(ptr::NonNull<bool>);

impl ThreadLocalKeepSet {
    /// # Safety
    /// `keep_set` must be valid for a `bool` write.
    // SAFETY: raw callback state is converted to
    // a non-null write witness before the typed claim helper sees it.
    #[inline(always)]
    unsafe fn from_raw(keep_set: *mut bool) -> Self {
        crate::internal::mi_assert_internal!(!keep_set.is_null());
        // SAFETY: null was asserted above and
        // callers provide a writable bool slot.
        Self(unsafe { ptr::NonNull::new_unchecked(keep_set) })
    }

    #[inline(always)]
    fn clear(self) {
        // SAFETY: the witness was constructed
        // only from a non-null writable bool slot.
        unsafe {
            *self.0.as_ptr() = false;
        }
    }
}

#[inline]
// SAFETY: C bitmap traversal calls this callback
// with raw arena and keep-set arguments that are immediately wrapped.
unsafe extern "C" fn mi_thread_local_claim_fun(
    slice_index: usize,
    arena: *mut mi_arena_t,
    keep_set: *mut bool,
) -> bool {
    // SAFETY: raw callback arguments are wrapped
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
    // SAFETY: `slots` is the acquired published
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
        // SAFETY: non-null `slots` is the
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
    // SAFETY: allocates aligned bitmap storage
    // for the expanded free-slot bitmap.
    let newslots = unsafe { mi_zalloc_aligned(newsize, MI_BCHUNK_SIZE).cast::<mi_bitmap_t>() };
    if newslots.is_null() {
        return false;
    }

    if !slots.is_null() {
        // SAFETY: `slots` is the previous
        // published bitmap; the null output only asks for its byte size.
        let oldsize = unsafe { mi_bitmap_size(oldcount, ptr::null_mut()) };
        // SAFETY: both bitmaps have at least
        // `oldsize` bytes, and ownership moves to `newslots` before publish.
        unsafe {
            ptr::copy_nonoverlapping(slots.cast::<u8>(), newslots.cast::<u8>(), oldsize);
            mi_free(slots.cast());
        }
    }

    // SAFETY: `newslots` points to writable
    // bitmap storage sized for `newcount`; new bits are published as free.
    unsafe {
        mi_bitmap_init(newslots, newcount, true);
        mi_bitmap_unsafe_setN(newslots, oldcount, newcount - oldcount);
    }
    mi_thread_locals_free.store(newslots, Ordering::Release);
    true
}

// SAFETY: initializes process-global dynamic
// TLS bookkeeping once with acquire/release publication.
#[inline(never)]
pub extern "C" fn _mi_thread_locals_init() {
    if mi_thread_locals_initialized
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        // SAFETY: the once transition gives this
        // thread exclusive initialization access to the static lock.
        unsafe {
            mi_lock_init(ptr::addr_of_mut!(mi_thread_locals_lock));
        }
        mi_thread_locals_free.store(ptr::null_mut(), Ordering::Relaxed);
        mi_thread_locals_version.store(0, Ordering::Relaxed);
    }
}

// SAFETY: tears down the global free bitmap
// under the static lock when dynamic TLS slots were allocated.
#[inline(never)]
pub extern "C" fn _mi_thread_locals_done() {
    if mi_thread_locals_free.load(Ordering::Acquire).is_null() {
        return;
    }
    // SAFETY: global bitmap teardown is
    // serialized by the static lock.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let slots = mi_thread_locals_free.swap(ptr::null_mut(), Ordering::AcqRel);
    if !slots.is_null() {
        // SAFETY: `slots` was removed from the
        // global publication point and is now owned by this teardown path.
        unsafe {
            mi_free(slots.cast());
        }
    }
    // SAFETY: releases and finalizes the static
    // lock after the global free bitmap has been unpublished.
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
        mi_lock_done(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
}

// SAFETY: stores a value into this thread's TLS
// slot block or expands that block before storing.
#[inline(never)]
pub extern "C" fn _mi_thread_local_set(
    key: mi_thread_local_t,
    val: *mut core::ffi::c_void,
) -> bool {
    crate::internal::mi_debug_assert!(key != 0);
    let was_set = {
        // SAFETY: this entry creates the sole operation-scoped view of the
        // current thread's stable TLS pointer.
        let tls = unsafe { CurrentThreadLocals::current_unchecked() };
        tls.try_set(key, val)
    };
    if was_set {
        true
    } else {
        mi_thread_local_set_expand(key, val)
    }
}

// SAFETY: reads a value from this thread's TLS
// slot block only when the key version still matches.
#[inline(never)]
pub extern "C" fn _mi_thread_local_get(key: mi_thread_local_t) -> *mut core::ffi::c_void {
    crate::internal::mi_debug_assert!(key != 0);
    // SAFETY: this entry creates the sole operation-scoped view of the current
    // thread's stable TLS pointer and performs no reallocating call.
    unsafe { CurrentThreadLocals::current_unchecked() }.get(key)
}

// SAFETY: frees only this thread's dynamically
// allocated TLS slot block and restores the empty sentinel.
#[inline(never)]
pub extern "C" fn _mi_thread_locals_thread_done() {
    // SAFETY: thread teardown creates the sole current-thread view and consumes
    // it while releasing any allocated block.
    unsafe { CurrentThreadLocals::current_unchecked() }.release();
}

// SAFETY: allocates a versioned TLS key under
// the global bitmap lock, expanding the bitmap when needed.
pub extern "C" fn _mi_thread_local_create() -> mi_thread_local_t {
    _mi_thread_locals_init();
    // SAFETY: key creation and bitmap expansion
    // are serialized by the static lock.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let mut key = mi_thread_local_claim();
    if key == 0 && mi_thread_local_create_expand() {
        key = mi_thread_local_claim();
    }
    // SAFETY: releases the lock acquired for key
    // claim/expansion.
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    key
}

// SAFETY: returns a versioned key's slot to the
// global free bitmap while holding the bitmap lock.
#[inline(never)]
pub extern "C" fn _mi_thread_local_free(key: mi_thread_local_t) {
    if key == 0 {
        return;
    }
    let idx = mi_key_index(key);
    // SAFETY: slot free mutates the global free
    // bitmap while serialized by the static lock.
    unsafe {
        mi_lock_acquire(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    // SAFETY: non-null `slots` is the acquired
    // published free bitmap and can be queried while the lock is held.
    if !slots.is_null() && idx.get() < unsafe { mi_bitmap_max_bits(slots) } {
        // SAFETY: `idx` is within the bitmap and
        // marks the slot as free for a future versioned key.
        unsafe {
            mi_bitmap_set(slots, idx.get());
        }
    }
    // SAFETY: releases the lock acquired for
    // free bitmap mutation.
    unsafe {
        mi_lock_release(ptr::addr_of_mut!(mi_thread_locals_lock));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(CurrentThreadLocals: Copy, Clone, Send, Sync);

    #[repr(C)]
    struct ThreadLocalsFixture {
        count: usize,
        slots: [mi_tls_slot_t; 3],
    }

    impl ThreadLocalsFixture {
        /// # Safety
        /// No other token or access to this fixture may overlap the returned token.
        unsafe fn token(&mut self) -> CurrentThreadLocals {
            CurrentThreadLocals {
                ptr: ptr::NonNull::from_mut(self).cast(),
                _current_thread: PhantomData,
            }
        }
    }

    #[test]
    fn key_helpers_split_index_and_version_domains() {
        let key = mi_key_create(ThreadLocalSlotIndex::new(17), ThreadLocalKeyVersion::new(3));
        assert_eq!(mi_key_index(key).get(), 17);
        assert_eq!(mi_key_version(key).get(), 3);
    }

    #[test]
    fn keep_set_witness_clears_claim_flag() {
        let mut keep_flag = true;
        // SAFETY: the test passes a live
        // stack-local bool as the callback keep-set slot.
        let keep_set = unsafe { ThreadLocalKeepSet::from_raw(&mut keep_flag) };
        assert!(mi_thread_local_claim_fun_typed(
            SliceIndex::new(0),
            None,
            keep_set
        ));
        assert!(!keep_flag);
    }

    #[test]
    fn current_thread_locals_bounds_and_versions_slot_access() {
        let mut fixture = ThreadLocalsFixture {
            count: 3,
            slots: [mi_tls_slot_t::default(); 3],
        };
        let value = ptr::without_provenance_mut::<core::ffi::c_void>(0x1000);
        let key = mi_key_create(ThreadLocalSlotIndex::new(1), ThreadLocalKeyVersion::new(2));
        let stale = mi_key_create(ThreadLocalSlotIndex::new(1), ThreadLocalKeyVersion::new(3));
        let out_of_bounds =
            mi_key_create(ThreadLocalSlotIndex::new(3), ThreadLocalKeyVersion::new(2));

        // SAFETY: this is the sole token for `fixture` and it is dropped before
        // the fixture fields are inspected directly.
        {
            let tls = unsafe { fixture.token() };
            assert!(tls.try_set(key, value));
            assert_eq!(tls.get(key), value);
            assert!(tls.get(stale).is_null());
            assert!(!tls.try_set(out_of_bounds, value));
            assert!(tls.get(out_of_bounds).is_null());
        }
        assert_eq!(fixture.slots[1].version, 2);
        assert_eq!(fixture.slots[1].value, value);

        fixture.count = 0;
        // SAFETY: the prior token is out of scope and this is the sole token
        // for the fixture during the empty-block checks.
        let empty = unsafe { fixture.token() };
        assert!(!empty.try_set(key, value));
        assert!(empty.get(key).is_null());
    }

    #[test]
    fn null_out_of_range_set_does_not_expand_current_tls() {
        let before = thread_locals_ptr();
        let key = mi_key_create(ThreadLocalSlotIndex::new(17), ThreadLocalKeyVersion::new(2));
        assert!(mi_thread_local_set_expand(key, ptr::null_mut()));
        assert_eq!(thread_locals_ptr(), before);
    }

    #[test]
    #[cfg(not(miri))]
    fn current_thread_locals_expansion_preserves_slots_and_release_restores_sentinel() {
        std::thread::spawn(|| {
            assert_eq!(thread_locals_ptr(), thread_locals_empty_ptr());

            let first_key =
                mi_key_create(ThreadLocalSlotIndex::new(1), ThreadLocalKeyVersion::new(2));
            let last_key =
                mi_key_create(ThreadLocalSlotIndex::new(17), ThreadLocalKeyVersion::new(3));
            let first_value = ptr::without_provenance_mut::<core::ffi::c_void>(0x1000);
            let last_value = ptr::without_provenance_mut::<core::ffi::c_void>(0x2000);

            assert!(_mi_thread_local_set(first_key, first_value));
            assert!(_mi_thread_local_set(last_key, last_value));
            assert_eq!(_mi_thread_local_get(first_key), first_value);
            assert_eq!(_mi_thread_local_get(last_key), last_value);
            // SAFETY: no other current-thread token is live and these operations do
            // not change the TLS pointer.
            {
                let tls = unsafe { CurrentThreadLocals::current_unchecked() };
                assert!(tls.count() > 17);
            }

            _mi_thread_locals_thread_done();
            assert_eq!(thread_locals_ptr(), thread_locals_empty_ptr());
        })
        .join()
        .expect("isolated TLS test thread completes");
    }
}
