//! Mechanical translation of `include/mimalloc/internal.h`.

// Retains upstream internal macro aliases and inline helpers for ABI/layout
// parity across platform and feature combinations.
#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    uninhabited_static
)]

pub(crate) mod atomic;
pub(crate) mod bitmap;
pub(crate) mod bits;
mod block_link;
#[cfg(feature = "guarded")]
pub(crate) mod guarded;
#[cfg(target_arch = "x86_64")]
mod memcpy;
pub(crate) mod memid;
#[cfg(target_arch = "x86_64")]
pub use memcpy::_mi_memcpy;
mod page_flags;
mod page_location;
mod page_metadata;
pub(crate) mod provenance;
pub(crate) mod random;

use core::ffi::{c_char, c_int, c_void};
use core::mem::size_of;
use core::ptr;

pub use self::bits::MI_MiB;
use self::bits::{MI_MAX_VABITS, MI_PAGE_MAP_FLAT, MI_SIZE_SHIFT};
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) use self::random::_mi_random_init;
#[cfg(any(target_os = "macos", windows))]
pub(crate) use self::random::_mi_random_init_weak;
pub(crate) use crate::allocator::alloc::_mi_page_malloc_zero_typed;
use crate::allocator::arena::MI_ARENA_SLICE_SIZE;
pub(crate) use crate::allocator::arena::{
    _mi_arena_memid_is_suitable, _mi_arenas_alloc, _mi_arenas_alloc_aligned, _mi_arenas_free,
};
pub(crate) use crate::allocator::arena_meta::{
    _mi_meta_free, _mi_meta_is_meta_page, _mi_meta_zalloc,
};
pub(crate) use crate::allocator::init::{
    _mi_auto_process_done, _mi_auto_process_init, _mi_is_heap_main, _mi_preloading, _mi_subproc,
    _mi_theap_guarded_init, _mi_theap_options_init, _mi_thread_done, _mi_thread_id,
};
pub(crate) use crate::allocator::page::_mi_deferred_free;
pub use crate::allocator::page::{MI_LARGE_MAX_OBJ_SIZE, MI_OS_PAGE_SIZE, mi_page_is_abandoned};
use crate::allocator::page_map::_mi_page_map_base;
pub(crate) use crate::allocator::page_map::_mi_safe_ptr_page_registered;
pub(crate) use crate::allocator::stats::{
    __mi_stat_adjust_decrease, __mi_stat_adjust_decrease_mt, __mi_stat_adjust_increase_mt,
    __mi_stat_counter_increase, __mi_stat_counter_increase_mt, __mi_stat_decrease,
    __mi_stat_decrease_mt, __mi_stat_increase, __mi_stat_increase_mt,
};
pub(crate) use crate::allocator::thread_heap::_mi_theap_random_next;
use crate::core_types::PageQueuePtr;
#[cfg(feature = "guarded")]
use crate::core_types::{GuardedConfig, GuardedDecision};
pub(crate) use crate::ffi::libc::{
    _mi_strlcpy, _mi_strlen, _mi_strnicmp, _mi_strnlen, _mi_toupper,
};
pub(crate) use crate::platform::os::{
    _mi_os_alloc, _mi_os_commit, _mi_os_commit_ex, _mi_os_free, _mi_os_free_ex,
    _mi_os_good_alloc_size, _mi_os_has_overcommit, _mi_os_has_virtual_reserve,
    _mi_os_page_size_typed, _mi_os_purge_ex, _mi_os_reuse, _mi_os_virtual_address_bits,
};
#[cfg(windows)]
pub(crate) use crate::platform::os::{_mi_os_canuse_large_page, _mi_os_get_aligned_hint};
pub(crate) use crate::platform::{_mi_allocator_done, _mi_allocator_init};
use crate::types::{mi_block_t, mi_heap_t, mi_output_fun, mi_page_queue_t, mi_page_t, mi_theap_t};
#[cfg(test)]
pub use block_link::mi_block_set_nextx;
pub use block_link::{
    mi_block_next, mi_block_nextx, mi_block_set_next, mi_is_in_same_page, mi_page_keys,
};
pub(crate) use block_link::{
    mi_block_set_next_typed, mi_page_set_keys, mi_ptr_decode, mi_ptr_encode, mi_ptr_encode_canary,
};
pub use page_flags::{
    mi_page_flags, mi_page_has_interior_pointers, mi_page_is_abandoned_mapped, mi_page_is_owned,
    mi_page_thread_free,
};
pub use page_location::{
    mi_page_meta_is_separated, mi_page_slice_offset_of, mi_page_slice_start, mi_page_start,
};
pub use page_metadata::{
    mi_page_all_free, mi_page_block_size, mi_page_heap, mi_page_immediate_available,
    mi_page_is_expandable, mi_page_is_full, mi_page_is_mostly_used, mi_page_is_singleton,
};
pub(crate) use page_metadata::{
    mi_page_block_size_typed, mi_page_committed_typed, mi_page_usable_block_size,
};

pub const MI_INTERNAL_H: () = ();
pub const _Atomic: () = ();

pub const MI_DEBUG: usize = if cfg!(debug_assertions) { 2 } else { 0 };
pub const MI_SECURE: usize = if cfg!(feature = "secure") { 4 } else { 0 };
pub const MI_TRACK_ASAN: usize = crate::allocator::tracking::MI_TRACK_ASAN;
pub const MI_TRACK_ENABLED: usize = crate::allocator::tracking::MI_TRACK_ENABLED;
pub const MI_TRACK_ENABLED_FULL: usize = 0;
pub const MI_GUARDED: usize = if cfg!(feature = "guarded") { 1 } else { 0 };
pub const MI_PAGE_META_IS_SEPARATED: usize = if MI_PAGE_MAP_FLAT != 0 { 0 } else { 1 };
pub const MI_BIG_ENDIAN: usize = if cfg!(target_endian = "big") { 1 } else { 0 };
pub const __wasi__: usize = if cfg!(target_os = "wasi") { 1 } else { 0 };

pub const mi_decl_cache_align: usize = 64;
pub const mi_decl_forceinline: () = ();
pub const mi_decl_thread: () = ();
pub const mi_decl_weak: () = ();
pub const mi_decl_cold: () = ();
pub const mi_decl_noinline: () = ();
pub const mi_decl_noreturn: () = ();
pub const mi_decl_hidden: () = ();
pub const mi_decl_externc: () = ();
pub const mi_decl_maybe_unused: () = ();
pub const MI_INIT4: usize = 4;
pub const MI_INIT8: usize = 8;
pub const MI_INIT16: usize = 16;
pub const MI_INIT32: usize = 32;
pub const MI_INIT64: usize = 64;
pub const MI_INIT128: usize = 128;
pub const MI_INIT256: usize = 256;
pub const MI_INIT74: usize = 74;
pub const MI_INIT5: usize = 5;
pub const MI_INIT6: usize = 6;

pub const MI_PAGE_MAP_SUB_SHIFT: usize = 13;
pub const MI_PAGE_MAP_SUB_COUNT: usize = 1usize << MI_PAGE_MAP_SUB_SHIFT;
pub const MI_PAGE_MAP_SHIFT: usize = MI_MAX_VABITS - MI_PAGE_MAP_SUB_SHIFT - MI_ARENA_SLICE_SHIFT;
pub const MI_PAGE_MAP_COUNT: usize = 1usize << MI_PAGE_MAP_SHIFT;

pub const MI_BLOCK_TAG_ALIGNED: usize = 0;
pub const MI_BLOCK_TAG_GUARDED: usize = !MI_BLOCK_TAG_ALIGNED;

pub type mi_submap_t = *mut *mut mi_page_t;

#[cold]
pub(crate) fn _mi_rust_assert_fail(assertion: *const c_char, fname: *const c_char, line: u32) -> ! {
    // SAFETY: runtime assertion shims forward caller-proven C strings into the audited abort path.
    unsafe { crate::allocator::options::_mi_assert_fail(assertion, fname, line, ptr::null()) }
}

macro_rules! mi_debug_assert {
    ($expr:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 && !$expr {
            $crate::internal::_mi_rust_assert_fail(
                concat!(stringify!($expr), "\0").as_ptr().cast(),
                concat!(file!(), "\0").as_ptr().cast(),
                line!(),
            )
        }
    }};
    ($expr:expr, $($arg:tt)+) => {{
        $crate::internal::mi_debug_assert!($expr);
    }};
}
pub(crate) use mi_debug_assert;

macro_rules! mi_debug_assert_eq {
    ($left:expr, $right:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 {
            let left = &$left;
            let right = &$right;
            if !(*left == *right) {
                $crate::internal::_mi_rust_assert_fail(
                    concat!(stringify!($left), " == ", stringify!($right), "\0")
                        .as_ptr()
                        .cast(),
                    concat!(file!(), "\0").as_ptr().cast(),
                    line!(),
                )
            }
        }
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        $crate::internal::mi_debug_assert_eq!($left, $right);
    }};
}
pub(crate) use mi_debug_assert_eq;

macro_rules! mi_assert {
    ($expr:expr $(,)?) => {{
        if $crate::internal::MI_DEBUG > 0 {
            $crate::internal::mi_debug_assert!($expr);
        }
    }};
    ($expr:expr, $($arg:tt)+) => {{
        if $crate::internal::MI_DEBUG > 0 {
            $crate::internal::mi_debug_assert!($expr, $($arg)+);
        }
    }};
}
pub(crate) use mi_assert;

macro_rules! mi_assert_internal {
    ($($arg:tt)*) => {{
        if $crate::internal::MI_DEBUG > 1 {
            $crate::internal::mi_assert!($($arg)*);
        }
    }};
}
pub(crate) use mi_assert_internal;

macro_rules! mi_assert_expensive {
    ($($arg:tt)*) => {{
        if $crate::internal::MI_DEBUG > 2 {
            $crate::internal::mi_assert!($($arg)*);
        }
    }};
}
pub(crate) use mi_assert_expensive;

pub const MI_ARENA_SLICE_SHIFT: usize = 13 + MI_SIZE_SHIFT;
pub const MI_BCHUNK_BITS_SHIFT: usize = 6 + MI_SIZE_SHIFT;

#[inline]
pub const fn mi_unlikely(x: bool) -> bool {
    x
}

#[inline]
pub const fn mi_likely(x: bool) -> bool {
    x
}

#[inline]
pub const fn MI_UNUSED<T>(_x: &T) {}

#[inline]
pub const fn MI_UNUSED_RELEASE<T>(_x: &T) {}

/// # Safety
/// `x` must be valid for writes of one `T`.
#[inline]
pub unsafe fn _mi_memzero_var<T>(x: *mut T) {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        ptr::write_bytes(x, 0, 1);
    }
}

// SAFETY: internal declarations preserve translated C helper signatures without dereferencing pointers.
unsafe extern "C" {
    pub fn _mi_snprintf(buf: *mut c_char, buflen: usize, fmt: *const c_char, ...) -> c_int;

    pub fn _mi_fputs(
        out: Option<mi_output_fun>,
        arg: *mut c_void,
        prefix: *const c_char,
        message: *const c_char,
    );
    pub fn _mi_fprintf(out: Option<mi_output_fun>, arg: *mut c_void, fmt: *const c_char, ...);
    pub fn _mi_raw_message(fmt: *const c_char, ...);
    pub fn _mi_message(fmt: *const c_char, ...);
    pub fn _mi_options_init();
    pub fn _mi_options_post_init();

    pub fn _mi_is_redirected() -> bool;
    pub fn _mi_is_main_thread() -> bool;
    pub fn _mi_thread_seq_id() -> usize;

    pub fn _mi_malloc_generic(
        theap: *mut mi_theap_t,
        size: usize,
        zero_huge_alignment: usize,
        usable: *mut usize,
    ) -> *mut c_void;
}

#[inline]
pub const fn _mi_is_power_of_two(x: usize) -> bool {
    (x & x.wrapping_sub(1)) == 0
}

#[inline]
pub fn _mi_is_aligned(p: *const c_void, alignment: usize) -> bool {
    crate::core_types::ptr_is_aligned(p, alignment)
}

#[inline]
pub const fn _mi_align_up(sz: usize, alignment: usize) -> usize {
    let mask = alignment.wrapping_sub(1);
    if alignment != 0 && (alignment & mask) == 0 {
        (sz + mask) & !mask
    } else if alignment == 0 {
        sz
    } else {
        ((sz + mask) / alignment) * alignment
    }
}

#[inline]
pub const fn _mi_align_down(sz: usize, alignment: usize) -> usize {
    let mask = alignment.wrapping_sub(1);
    if alignment != 0 && (alignment & mask) == 0 {
        sz & !mask
    } else if alignment == 0 {
        sz
    } else {
        (sz / alignment) * alignment
    }
}

#[inline]
pub fn _mi_align_up_ptr(p: *const c_void, alignment: usize) -> *mut c_void {
    crate::core_types::ptr_align_up(p, alignment)
}

#[inline]
pub fn _mi_align_down_ptr(p: *const c_void, alignment: usize) -> *mut c_void {
    crate::core_types::ptr_align_down(p, alignment)
}

#[inline]
pub const fn _mi_divide_up(size: usize, divider: usize) -> usize {
    if divider == 0 {
        size
    } else {
        size.div_ceil(divider)
    }
}

#[inline]
pub const fn _mi_clamp(sz: usize, min: usize, max: usize) -> usize {
    if sz < min {
        min
    } else if sz > max {
        max
    } else {
        sz
    }
}

/// # Safety
/// `p..p + size` must be valid for reads.
#[inline]
pub unsafe fn mi_mem_is_zero(p: *const c_void, size: usize) -> bool {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        let bytes = p.cast::<u8>();
        let mut i = 0;
        while i < size {
            if *bytes.add(i) != 0 {
                return false;
            }
            i += 1;
        }
        true
    }
}

#[inline]
pub const fn _mi_wsize_from_size(size: usize) -> usize {
    crate::core_types::WordCount::from_size(size).get()
}

#[inline(always)]
pub(crate) fn mi_direct_bin_from_size(size: usize) -> crate::core_types::DirectBin {
    let idx = crate::core_types::WordCount::from_size(size).get();
    crate::internal::mi_debug_assert!(idx < crate::types::MI_PAGES_DIRECT);
    crate::internal::mi_debug_assert!(u16::try_from(idx).is_ok());
    // SAFETY: direct bin derivation checks page queue table bounds before unchecked construction.
    unsafe { crate::core_types::DirectBin::new_unchecked(idx as u16) }
}

#[inline(always)]
pub(crate) fn mi_direct_bin_from_padded(
    padded: crate::core_types::PaddedSize,
) -> crate::core_types::DirectBin {
    mi_direct_bin_from_size(padded.get())
}

#[inline]
pub fn mi_mul_overflow(count: usize, size: usize, total: &mut usize) -> bool {
    match count.checked_mul(size) {
        Some(product) => {
            *total = product;
            false
        }
        None => {
            *total = usize::MAX;
            true
        }
    }
}

/// # Safety
/// `theap` must point to initialized thread-heap metadata.
#[inline]
pub unsafe fn _mi_theap_heap(theap: *const mi_theap_t) -> *mut mi_heap_t {
    // SAFETY: thread-heap and allocation shims project typed heap metadata from caller-proven raw pointers.
    unsafe {
        crate::core_types::AtomicPtrField::from_raw(ptr::addr_of!((*theap).heap).cast_mut())
            .load(crate::core_types::MemoryOrder::Acquire)
    }
}

/// # Safety
/// `theap` must be null or point to thread-heap metadata allocated by this allocator.
#[inline]
pub unsafe fn mi_theap_is_initialized(theap: *const mi_theap_t) -> bool {
    // SAFETY: thread-heap and allocation shims project typed heap metadata from caller-proven raw pointers.
    !theap.is_null() && !unsafe { _mi_theap_heap(theap) }.is_null()
}

/// # Safety
/// `padded` must derive from a normalized request no larger than
/// `MI_SMALL_SIZE_MAX`, so its direct bin is in range. `theap` and its
/// initialized direct cache must remain live and locally owned for the call.
#[inline]
pub(crate) unsafe fn _mi_theap_get_free_small_page_typed(
    theap: crate::core_types::ThreadHeapLocal,
    padded: crate::core_types::PaddedSize,
) -> Option<crate::core_types::AvailablePage> {
    // SAFETY: thread-heap and allocation shims project typed heap metadata from caller-proven raw pointers.
    unsafe {
        let bin = mi_direct_bin_from_padded(padded);
        theap.direct_available_page(bin)
    }
}

#[cfg(not(target_pointer_width = "32"))]
/// # Safety
/// The page map must be initialized and remain live for the returned load.
/// `idx` must also be below the active runtime page-map count and identify a
/// committed top-level slot, as established by a registered range or an
/// address below `mi_page_map_max_address`.
#[inline]
pub unsafe fn _mi_page_map_at(idx: crate::core_types::PageMapPrimaryIndex) -> mi_submap_t {
    // SAFETY: page-map lookup helpers convert allocator addresses through validated submap indexes.
    unsafe {
        crate::core_types::AtomicPtrField::from_raw(_mi_page_map_base().add(idx.get()))
            .load(crate::core_types::MemoryOrder::Acquire)
    }
}

#[cfg(target_pointer_width = "32")]
/// # Safety
/// `p` must be a pointer in the allocator's page-map domain, and `valid` must be null or writable.
#[inline]
pub unsafe fn _mi_ptr_page_ex(p: *const c_void, valid: *mut bool) -> *mut mi_page_t {
    let location = crate::core_types::PageMapLocation::from_ptr(p);
    let idx = location.index().get();
    let ofs = *_mi_page_map_base().add(idx) as usize;
    if !valid.is_null() {
        *valid = ofs != 0;
    }
    (((crate::core_types::ptr_exposed_addr(p) >> MI_ARENA_SLICE_SHIFT) + 1 - ofs)
        << MI_ARENA_SLICE_SHIFT) as *mut mi_page_t
}

#[cfg(target_pointer_width = "32")]
/// # Safety
/// `p` must be a pointer in the allocator's page-map domain.
#[inline]
pub unsafe fn _mi_checked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    let mut valid = false;
    let page = _mi_ptr_page_ex(p, &mut valid);
    if valid { page } else { ptr::null_mut() }
}

#[cfg(target_pointer_width = "32")]
/// # Safety
/// `p` must be a pointer in the allocator's page-map domain.
#[inline]
pub unsafe fn _mi_unchecked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    _mi_ptr_page_ex(p, ptr::null_mut())
}

#[cfg(not(target_pointer_width = "32"))]
/// # Safety
/// `p` must be a pointer in the allocator's page-map domain.
#[inline]
pub unsafe fn _mi_unchecked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    // SAFETY: page-map lookup helpers convert allocator addresses through validated submap indexes.
    unsafe {
        let location = crate::core_types::PageMapLocation::from_ptr(p).unwrap_unchecked();
        _mi_page_map_at(location.index())
            .add(location.sub_index().get())
            .read()
    }
}

#[cfg(not(target_pointer_width = "32"))]
/// # Safety
/// `p` must be a pointer in the allocator's page-map domain.
#[inline]
pub unsafe fn _mi_checked_ptr_page(p: *const c_void) -> *mut mi_page_t {
    // SAFETY: page-map lookup helpers convert allocator addresses through validated submap indexes.
    unsafe {
        let Some(location) = crate::core_types::PageMapLocation::from_ptr(p) else {
            return ptr::null_mut();
        };
        let sub = _mi_page_map_at(location.index());
        if sub.is_null() {
            ptr::null_mut()
        } else {
            *sub.add(location.sub_index().get())
        }
    }
}

/// # Safety
/// `p` must be null or a pointer allocated by this allocator.
#[inline]
pub unsafe fn _mi_ptr_page(p: *const c_void) -> *mut mi_page_t {
    // SAFETY: page-map lookup helpers convert allocator addresses through validated submap indexes.
    unsafe {
        crate::internal::mi_assert_internal!(
            p.is_null() || crate::mimalloc::mi_is_in_heap_region(p)
        );
        if MI_DEBUG != 0 || MI_SECURE != 0 || cfg!(target_vendor = "apple") {
            _mi_checked_ptr_page(p)
        } else {
            _mi_unchecked_ptr_page(p)
        }
    }
}

#[inline]
pub const fn mi_page_info_size() -> usize {
    _mi_align_up(size_of::<mi_page_t>(), 16)
}

#[inline]
// SAFETY: page owner helpers resolve heap, thread heap, and queue pointers from caller-proven metadata.
pub(crate) unsafe fn mi_page_queue(
    theap: *const mi_theap_t,
    block_size: crate::core_types::BlockSize,
) -> *mut mi_page_queue_t {
    unsafe {
        let size = block_size.get();
        let pq = ptr::addr_of!(
            (*(theap as *mut mi_theap_t)).pages
                [crate::allocator::page::mi_bin_from_block_size(block_size).as_usize()]
        )
        .cast_mut();
        if size <= MI_LARGE_MAX_OBJ_SIZE {
            let queue = PageQueuePtr::new_unchecked(pq);
            crate::internal::mi_assert_internal!(
                queue
                    .block_size()
                    .is_some_and(|block_size| block_size.get() <= MI_LARGE_MAX_OBJ_SIZE)
            );
        }
        pq
    }
}

/// # Safety
/// `page` must point to mutable page metadata owned by this allocator.
#[inline]
pub(crate) unsafe fn mi_page_set_has_interior_pointers(
    page: crate::core_types::PagePtr,
    has_aligned: bool,
) {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    unsafe {
        page.thread_id_cell().set_flags(
            crate::core_types::PageFlags::HAS_INTERIOR_POINTERS,
            has_aligned,
        );
    }
}

/// # Safety
/// `page` must point to mutable abandoned-mapped page metadata owned by this allocator.
#[inline]
pub(crate) unsafe fn mi_page_clear_abandoned_mapped(page: crate::core_types::PagePtr) {
    // SAFETY: page flag and owner-id helpers use typed atomic thread-id fields for state updates.
    unsafe {
        crate::internal::mi_assert_internal!(mi_page_is_abandoned_mapped(page.as_ptr()));
        let atomic_tid = page.thread_id_cell();
        let mut old = atomic_tid.load();
        loop {
            let new = old.with_owner(crate::core_types::PageOwnerId::Abandoned);
            if atomic_tid.compare_exchange_owner(&mut old, new) {
                break;
            }
        }
    }
}

/// # Safety
/// `page` must be abandoned page metadata whose thread-free ownership may be claimed.
#[inline]
pub(crate) unsafe fn mi_page_claim_ownership(
    page: crate::core_types::AbandonedPage,
) -> Option<crate::core_types::OwnedAbandonedPage> {
    // SAFETY: thread-free ownership word helpers preserve atomic head and ownership encoding invariants.
    unsafe {
        let page_ptr = page.page();
        let atomic_tf = page_ptr.thread_free_cell();
        if atomic_tf.claim_acq_rel() {
            Some(crate::core_types::assume_owned_abandoned_page(page_ptr))
        } else {
            None
        }
    }
}

/// # Safety
/// `block` and `p` must refer to the same allocated block when guarded allocation is enabled.
#[inline]
pub unsafe fn mi_block_ptr_is_guarded(block: *const mi_block_t, p: *const c_void) -> bool {
    // SAFETY: thread-heap and allocation shims project typed heap metadata from caller-proven raw pointers.
    unsafe {
        if MI_GUARDED == 0 {
            MI_UNUSED(&block);
            MI_UNUSED(&p);
            return false;
        }
        let offset = p.cast::<u8>().offset_from(block.cast::<u8>());
        offset >= size_of::<mi_block_t>() as isize
            && crate::core_types::BlockLink::is_guarded_raw(block)
    }
}

/// # Safety
/// `theap` must be local to the current thread.
#[inline]
pub(crate) unsafe fn mi_theap_malloc_use_guarded(
    theap: crate::core_types::ThreadHeapLocal,
    size: crate::core_types::EffectiveSize,
) -> bool {
    if MI_GUARDED == 0 {
        MI_UNUSED(&theap);
        MI_UNUSED(&size);
        return false;
    }
    #[cfg(feature = "guarded")]
    {
        // SAFETY: thread-heap and allocation shims project typed heap metadata from caller-proven raw pointers.
        unsafe {
            let theap = theap.ptr().as_ptr();
            let size = size.get();
            let config = GuardedConfig::from_clamped_raw(
                (*theap).guarded_size_min,
                (*theap).guarded_size_max,
                (*theap).guarded_sample_rate,
                (*theap).guarded_sample_count,
            );
            let decision = config.decide(size);
            if let Some(next_sample_count) = decision.next_sample_count() {
                (*theap).guarded_sample_count = next_sample_count.get();
            }
            match decision {
                GuardedDecision::UseGuarded { .. } => true,
                GuardedDecision::SkipAndUpdate { .. } | GuardedDecision::SkipUnchanged => false,
            }
        }
    }
    #[cfg(not(feature = "guarded"))]
    {
        false
    }
}

/// # Safety
/// `dst..dst + n` must be valid for writes.
#[inline]
pub unsafe fn mi_rep_stosb(dst: *mut c_void, val: u8, n: usize) {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            core::arch::asm!(
                "rep stosb",
                inout("rdi") dst => _,
                inout("rcx") n => _,
                in("al") val,
                options(nostack, preserves_flags),
            );
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            ptr::write_bytes(dst.cast::<u8>(), val, n);
        }
    }
}

#[inline]
pub const fn mi_slice_count_of_size(size: usize) -> usize {
    _mi_divide_up(size, MI_ARENA_SLICE_SIZE)
}

#[inline]
pub const fn mi_size_of_slices(bcount: usize) -> usize {
    bcount * MI_ARENA_SLICE_SIZE
}

#[inline]
pub(crate) const fn mi_committed_bytes_of_slices(
    bcount: usize,
) -> crate::core_types::CommittedBytes {
    crate::core_types::CommittedBytes::new(mi_size_of_slices(bcount))
}

#[inline]
pub const fn _mi_random_shuffle(mut x: usize) -> usize {
    if x == 0 {
        x = 17;
    }
    #[cfg(target_pointer_width = "64")]
    {
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
    }
    #[cfg(target_pointer_width = "32")]
    {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;
    }
    x
}

#[inline]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn mi_cpu_stosb_max() -> usize {
    let max: usize;
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        core::arch::asm!(
            "mov {max}, qword ptr [rip + _mi_cpu_stosb_max]",
            max = lateout(reg) max,
            options(nostack, readonly, preserves_flags)
        );
    }
    max
}

#[inline]
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
unsafe fn mi_cpu_stosb_max() -> usize {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe { crate::allocator::init::_mi_cpu_stosb_max }
}

/// # Safety
/// For `n > 0`, `src` must point to `n` initialized readable bytes and `dst` to
/// `n` writable bytes; the regions must be disjoint. No alignment is required.
/// For `n == 0`, neither pointer is accessed and each may be null or dangling.
#[cfg(not(target_arch = "x86_64"))]
#[inline]
pub unsafe fn _mi_memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    // SAFETY: the caller provides the initialized source and disjoint writable
    // destination ranges required by copy_nonoverlapping.
    unsafe {
        ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), n);
    }
    dst
}

/// # Safety
/// `dst..dst + n` must be valid for writes.
#[inline]
pub unsafe fn _mi_memset(dst: *mut c_void, val: i32, n: usize) {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            if n <= mi_cpu_stosb_max() {
                mi_rep_stosb(dst, val as u8, n);
                return;
            }
        }
        ptr::write_bytes(dst, val as u8, n);
    }
}

/// # Safety
/// `dst..dst + n` must be valid for writes and aligned as required by callers of the aligned
/// helper.
#[inline]
pub unsafe fn _mi_memset_aligned(dst: *mut c_void, val: i32, n: usize) {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        _mi_memset(dst, val, n);
    }
}

/// # Safety
/// `dst..dst + n` must be valid for writes.
#[inline]
pub unsafe fn _mi_memzero(dst: *mut c_void, n: usize) {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        _mi_memset(dst, 0, n);
    }
}

/// # Safety
/// `dst..dst + n` must be valid for writes and aligned as required by callers of the aligned
/// helper.
#[inline]
pub unsafe fn _mi_memzero_aligned(dst: *mut c_void, n: usize) {
    // SAFETY: copy, fill, and zero helpers operate on caller-proven byte ranges and architecture thresholds.
    unsafe {
        _mi_memset_aligned(dst, 0, n);
    }
}
