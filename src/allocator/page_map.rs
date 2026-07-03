//! Page map management.

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release, mi_lock_t};
use crate::bitmap::MI_BFIELD_BITS;
#[cfg(target_pointer_width = "32")]
use crate::bitmap::{
    mi_bitmap_init, mi_bitmap_is_clear, mi_bitmap_is_set, mi_bitmap_set, mi_bitmap_size,
    mi_bitmap_t,
};
#[cfg(target_pointer_width = "32")]
use crate::bits::MI_MiB;
use crate::bits::{MI_KiB, MI_PAGE_MAP_FLAT, MI_SIZE_BITS};
use crate::core_types::{Errno, PagePtr, RegisteredPageRange, SliceCount};
#[cfg(target_pointer_width = "32")]
use crate::internal::_mi_memzero;
use crate::internal::{
    _mi_align_up, _mi_divide_up, _mi_is_aligned, _mi_memid_create_os, _mi_memid_none,
    _mi_memzero_aligned, _mi_os_commit, _mi_os_free, _mi_os_free_ex, _mi_os_has_overcommit,
    _mi_os_virtual_address_bits, _mi_page_map_at, _mi_page_map_index, MI_ARENA_SLICE_SHIFT,
    MI_PAGE_MAP_COUNT, MI_PAGE_MAP_SUB_COUNT, MI_PAGE_MAP_SUB_SHIFT, mi_memid_mem_t,
    mi_memid_os_info_t, mi_memid_t, mi_memkind_t, mi_page_area, mi_page_slice_start,
    mi_slice_count_of_size, mi_submap_t,
};
use crate::options::{
    _mi_error_message_fmt, _mi_warning_message_fmt, mi_option_get_clamp, mi_option_is_enabled,
    mi_option_t,
};
use crate::os::{_mi_os_alloc_aligned, _mi_os_zalloc};
use crate::page::MI_LARGE_PAGE_SIZE;
use crate::types::{MI_PAGE_ALIGN, mi_page_t, mi_subproc_t};

mod lookup;

pub(crate) use lookup::{_mi_safe_ptr_page, _mi_safe_ptr_page_registered};

pub const MI_PAGE_MAP_SUB_SIZE: usize =
    MI_PAGE_MAP_SUB_COUNT * core::mem::size_of::<*mut mi_page_t>();
const MI_PAGE_MAP_ENTRIES_PER_CBIT: usize = if MI_PAGE_MAP_COUNT < MI_BFIELD_BITS {
    1
} else {
    MI_PAGE_MAP_COUNT / MI_BFIELD_BITS
};

#[cfg(not(target_pointer_width = "32"))]
// SAFETY[AUDIT:page-map-global-boundary]: process-global page-map base is mutated only
// during allocator lifecycle init/destroy and read by audited page-map helpers.
pub(crate) static mut _mi_page_map: *mut mi_submap_t = ptr::null_mut();

#[cfg(target_pointer_width = "32")]
// SAFETY[AUDIT:page-map-global-boundary]: 32-bit process-global byte page-map base is
// mutated only during allocator lifecycle init/destroy.
pub(crate) static mut _mi_page_map: *mut u8 = ptr::null_mut();

/// # Safety
///
/// The page map must be initialized before callers dereference the returned
/// base pointer. Concurrent teardown must be excluded by allocator lifecycle.
#[cfg(not(target_pointer_width = "32"))]
#[inline(always)]
pub(crate) unsafe fn _mi_page_map_base() -> *mut mi_submap_t {
    // SAFETY[AUDIT:page-map-global-boundary]: callers synchronize allocator lifecycle
    // and only dereference the returned initialized submap base.
    unsafe { _mi_page_map }
}

/// # Safety
///
/// The page map must be initialized before callers dereference the returned
/// base pointer. Concurrent teardown must be excluded by allocator lifecycle.
#[cfg(target_pointer_width = "32")]
#[inline(always)]
pub(crate) unsafe fn _mi_page_map_base() -> *mut u8 {
    // SAFETY[AUDIT:page-map-global-boundary]: callers synchronize allocator lifecycle
    // and only dereference the returned initialized byte page-map base.
    unsafe { _mi_page_map }
}

// SAFETY[AUDIT:page-map-global-boundary]: remaining process-global page-map metadata is
// initialized, published, and cleared only by allocator lifecycle boundaries.
static mut mi_page_map_count: usize = 0;
static mut mi_page_map_max_address: *mut c_void = ptr::null_mut();
static mut mi_page_map_memid: mi_memid_t = mi_memid_t {
    mem: mi_memid_mem_t {
        os: mi_memid_os_info_t {
            base: ptr::null_mut(),
            size: 0,
        },
    },
    memkind: mi_memkind_t::MI_MEM_NONE,
    is_pinned: false,
    initially_committed: false,
    initially_zero: false,
};
static mut mi_page_map_lock: mi_lock_t = mi_lock_t::new();
#[cfg(target_pointer_width = "32")]
static mut mi_page_map_commit: *mut mi_bitmap_t = ptr::null_mut();
#[cfg(not(target_pointer_width = "32"))]
static mi_page_map_commit: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn mi_memid_os_base_size(memid: mi_memid_t) -> (*mut c_void, usize) {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::Os(os) => (
            os.base.map(|p| p.as_ptr()).unwrap_or(ptr::null_mut()),
            os.size.get(),
        ),
        crate::core_types::MemIdView::External(external) => (
            external.base.map(|p| p.as_ptr()).unwrap_or(ptr::null_mut()),
            external.size.get(),
        ),
        _ => {
            crate::internal::mi_assert_internal!(false);
            (ptr::null_mut(), 0)
        }
    }
}

#[inline]
fn mi_page_map_cannot_commit() {
    _mi_warning_message_fmt(format_args!(
        "unable to commit the allocation page-map on-demand\n"
    ));
}

#[inline]
fn mi_page_map_index(raw: usize) -> Option<crate::core_types::PageMapIndex> {
    crate::core_types::PageMapIndex::new(raw)
}

#[inline]
fn mi_page_map_sub_index(raw: usize) -> Option<crate::core_types::PageMapSubIndex> {
    crate::core_types::PageMapSubIndex::new(raw)
}

#[cfg(target_pointer_width = "32")]
type PageMapGetIndex = (crate::core_types::PageMapIndex, *mut u8, SliceCount);

#[cfg(not(target_pointer_width = "32"))]
type PageMapGetIndex = (
    crate::core_types::PageMapIndex,
    crate::core_types::PageMapSubIndex,
    SliceCount,
);

struct PageMapPageSpan {
    page_start: *mut u8,
    slice_count: SliceCount,
}

unsafe fn mi_page_map_page_span(page: PagePtr) -> Option<PageMapPageSpan> {
    let page = page.as_ptr();
    let mut page_size = 0usize;
    // SAFETY[AUDIT:page-map-global-boundary]: `PagePtr` proves live page metadata and
    // the out-size pointer is stack-local for the area query.
    let page_start = unsafe { mi_page_area(page, &mut page_size) };
    if page_size > MI_LARGE_PAGE_SIZE {
        page_size = MI_LARGE_PAGE_SIZE - MI_ARENA_SLICE_SIZE;
    }
    // SAFETY[AUDIT:page-map-global-boundary]: `PagePtr` proves page slice metadata is
    // initialized for deriving the registered slice span.
    let slice_start = unsafe { mi_page_slice_start(page) };
    let slice_count = mi_slice_count_of_size(page_size)
        + (crate::core_types::ptr_addr_diff(page_start, slice_start) / MI_ARENA_SLICE_SIZE);
    let slice_count = SliceCount::from_usize(slice_count)?;
    Some(PageMapPageSpan {
        page_start,
        slice_count,
    })
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_get_idx_from_span(
    page: PagePtr,
    span: PageMapPageSpan,
) -> Option<PageMapGetIndex> {
    // SAFETY[AUDIT:page-map-global-boundary]: typed page witness bounds the pointer in
    // the 32-bit page-map index domain.
    let idx = unsafe { _mi_page_map_index(page.as_ptr().cast::<c_void>()) };
    Some((mi_page_map_index(idx)?, span.page_start, span.slice_count))
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_get_idx_from_span(
    _page: PagePtr,
    span: PageMapPageSpan,
) -> Option<PageMapGetIndex> {
    let mut sub_idx = 0usize;
    // SAFETY[AUDIT:page-map-global-boundary]: span start is derived from a live page and
    // the stack-local sub-index out pointer is writable.
    let idx = unsafe { _mi_page_map_index(span.page_start.cast::<c_void>(), &mut sub_idx) };
    Some((
        mi_page_map_index(idx)?,
        mi_page_map_sub_index(sub_idx)?,
        span.slice_count,
    ))
}

unsafe fn mi_page_map_get_idx(page: PagePtr) -> Option<PageMapGetIndex> {
    // SAFETY[AUDIT:page-map-global-boundary]: typed page witness scopes page span
    // derivation before conversion into page-map indexes.
    let span = unsafe { mi_page_map_page_span(page) }?;
    // SAFETY[AUDIT:page-map-global-boundary]: span came from the same typed page and is
    // validated into typed index/count domains by the helper.
    unsafe { mi_page_map_get_idx_from_span(page, span) }
}

#[inline]
fn mi_page_map_is_committed(idx: crate::core_types::PageMapIndex, pbit_idx: *mut usize) -> bool {
    let commit = mi_page_map_commit.load(Ordering::Relaxed);
    let bit_idx = idx.get() / MI_PAGE_MAP_ENTRIES_PER_CBIT;
    crate::internal::mi_assert_internal!(bit_idx < MI_BFIELD_BITS);
    if !pbit_idx.is_null() {
        // SAFETY[AUDIT:page-map-global-boundary]: nullable out pointer is checked before
        // writing the derived commit-bit index.
        unsafe { *pbit_idx = bit_idx };
    }
    (commit & (1usize << bit_idx)) != 0
}

#[cfg(target_pointer_width = "32")]
const MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT: usize = MI_ARENA_SLICE_SIZE;

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_ensure_committed(
    idx: crate::core_types::PageMapIndex,
    slice_count: crate::core_types::SliceCount,
) -> bool {
    let slice_count = slice_count.get() as usize;
    let idx = idx.get();
    // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: 32-bit commit bitmap is installed during
    // page-map initialization before range registration.
    if unsafe { !mi_page_map_commit.is_null() } {
        let commit_idx = idx / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT;
        let commit_idx_hi = (idx + slice_count - 1) / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT;
        for i in commit_idx..=commit_idx_hi {
            // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: bitmap index is derived from a
            // validated page-map range and stays within the allocated bitmap.
            if unsafe { mi_bitmap_is_clear(mi_page_map_commit, i) } {
                // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: committed range offsets are
                // within the reserved byte page-map allocation.
                let start = unsafe { _mi_page_map.add(i * MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT) };
                let size = MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT;
                let mut is_zero = false;
                // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: commits exactly one bitmap
                // tracked page-map chunk inside the reserved page-map region.
                if !unsafe { _mi_os_commit(start.cast::<c_void>(), size, &mut is_zero) } {
                    mi_page_map_cannot_commit();
                    return false;
                }
                // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: memid was captured at reserve
                // time and describes whether committed memory is initially zero.
                if !is_zero && !unsafe { mi_page_map_memid.initially_zero } {
                    // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: committed chunk is live
                    // and writable before registration bytes are published.
                    unsafe { _mi_memzero(start.cast::<c_void>(), size) };
                }
                // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: bitmap entry corresponds to
                // the just-committed page-map chunk.
                unsafe { mi_bitmap_set(mi_page_map_commit, i) };
            }
        }
    }
    if cfg!(debug_assertions) {
        // SAFETY[AUDIT:page-map-32bit-commit-bitmap-boundary]: debug sentinel touches validated
        // first/last bytes of the committed 32-bit page-map slice range.
        unsafe {
            *_mi_page_map.add(idx) = 0;
            *_mi_page_map.add(idx + slice_count - 1) = 0;
        }
    }
    true
}

/// # Safety
///
/// Must be called during allocator initialization before concurrent page-map
/// use starts.
#[cfg(target_pointer_width = "32")]
// SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: 32-bit page-map initialization owns the
// process-global reserve/commit state before concurrent allocator use starts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_init() -> bool {
    let mut vbits =
        mi_option_get_clamp(mi_option_t::mi_option_max_vabits, 0, MI_SIZE_BITS as i64) as usize;
    if vbits == 0 {
        // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: platform virtual-address query is
        // read-only during allocator initialization.
        vbits = unsafe { _mi_os_virtual_address_bits() };
        #[cfg(target_arch = "x86_64")]
        if vbits >= 48 {
            vbits = 47;
        }
    }
    if vbits < MI_ARENA_SLICE_SHIFT {
        vbits = MI_ARENA_SLICE_SHIFT;
    }

    // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: initialization has exclusive access to
    // page-map max-address metadata before publication.
    unsafe {
        mi_page_map_max_address = if vbits >= MI_SIZE_BITS {
            (usize::MAX - MI_ARENA_SLICE_SIZE + 1) as *mut c_void
        } else {
            (1usize << vbits) as *mut c_void
        };
    }
    let page_map_size = 1usize << (vbits - MI_ARENA_SLICE_SHIFT);
    let commit =
        page_map_size <= MI_MiB || mi_option_is_enabled(mi_option_t::mi_option_pagemap_commit);
    let commit_bits = _mi_divide_up(page_map_size, MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT);
    let bitmap_size = if commit {
        0
    } else {
        // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: bitmap sizing only reads the derived
        // commit-bit count and writes no caller state with a null out pointer.
        unsafe { mi_bitmap_size(commit_bits, ptr::null_mut()) }
    };
    let reserve_size = bitmap_size + page_map_size;
    // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: initialization reserves the complete
    // page-map backing allocation and records its OS memid in global metadata.
    let base =
        unsafe { _mi_os_alloc_aligned(reserve_size, 1, commit, true, &raw mut mi_page_map_memid) }
            .cast::<u8>();
    if base.is_null() {
        _mi_error_message_fmt(
            Errno::ENOMEM,
            format_args!(
                "unable to reserve virtual memory for the page map ({} KiB)\n",
                page_map_size / MI_KiB
            ),
        );
        return false;
    }
    // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: memid flags describe the just-reserved
    // backing allocation before global page-map publication.
    if unsafe { mi_page_map_memid.initially_committed && !mi_page_map_memid.initially_zero } {
        _mi_warning_message_fmt(format_args!(
            "internal: the page map was committed but not zero initialized!\n"
        ));
        // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: the full reserved page-map backing
        // allocation is committed and writable when it needs zeroing.
        unsafe { _mi_memzero_aligned(base.cast::<c_void>(), reserve_size) };
    }
    if bitmap_size > 0 {
        // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: bitmap storage lives in the reserved
        // prefix and is committed before bitmap initialization.
        unsafe {
            mi_page_map_commit = base.cast::<mi_bitmap_t>();
            if !_mi_os_commit(
                mi_page_map_commit.cast::<c_void>(),
                bitmap_size,
                ptr::null_mut(),
            ) {
                mi_page_map_cannot_commit();
                return false;
            }
            mi_bitmap_init(mi_page_map_commit, commit_bits, true);
        }
    }
    // SAFETY[AUDIT:page-map-32bit-bootstrap-boundary]: initialization publishes the byte map
    // base after reserving any bitmap prefix.
    unsafe {
        _mi_page_map = base.add(bitmap_size);
    }
    // SAFETY[AUDIT:page-map-null-sentinel-boundary]: zero index and one-slice witnesses are
    // constants inside the page-map domains used to reserve null-pointer lookup.
    let zero_index = unsafe { crate::core_types::PageMapIndex::new_unchecked(0) };
    let one_slice = unsafe { crate::core_types::SliceCount::new_unchecked(1) };
    // SAFETY[AUDIT:page-map-null-sentinel-boundary]: initialization commits the null-page
    // sentinel byte before writing it.
    if !commit && !unsafe { mi_page_map_ensure_committed(zero_index, one_slice) } {
        mi_page_map_cannot_commit();
        return false;
    }
    // SAFETY[AUDIT:page-map-null-sentinel-boundary]: null-page sentinel byte is within the
    // committed page-map base range.
    unsafe {
        *_mi_page_map.add(0) = 1;
    }
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:page-map-null-sentinel-boundary]: initialization verifies null lookup
        // through the page-map facade after publishing the sentinel.
        unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null()
    );
    true
}

/// # Safety
///
/// `subproc` must point to the live sub-process owning the page-map memory.
/// No concurrent page-map users may remain.
#[cfg(target_pointer_width = "32")]
// SAFETY[AUDIT:page-map-teardown-boundary]: unsafe destroy runs after page-map users are
// quiesced and owns global pointer/memid teardown.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::internal::mi_assert_internal!(!subproc.is_null());
    // SAFETY[AUDIT:page-map-teardown-boundary]: destroy checks initialized global base
    // before freeing the backing allocation.
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    // SAFETY[AUDIT:page-map-teardown-boundary]: memid was recorded by initialization and
    // still describes the page-map backing allocation at teardown.
    let (base, size) = mi_memid_os_base_size(unsafe { mi_page_map_memid });
    // SAFETY[AUDIT:page-map-teardown-boundary]: destroy frees the recorded backing range
    // and clears process-global page-map state while users are quiesced.
    unsafe {
        _mi_os_free_ex(base, size, true, mi_page_map_memid, subproc);
        _mi_page_map = ptr::null_mut();
        mi_page_map_commit = ptr::null_mut();
        mi_page_map_max_address = ptr::null_mut();
        mi_page_map_memid = _mi_memid_none();
    }
}

/// # Safety
///
/// `page` must point to a live page whose page area and slice metadata are
/// initialized and stable for registration.
#[cfg(target_pointer_width = "32")]
// SAFETY[AUDIT:page-map-page-register-boundary]: C ABI registration validates the raw page
// pointer into `PagePtr` before touching page-map state.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_register(page: *mut mi_page_t) -> bool {
    crate::internal::mi_assert_internal!(!page.is_null());
    let Some(page) = PagePtr::new(page) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY[AUDIT:page-map-page-register-boundary]: `PagePtr` proves non-null page metadata
    // for the typed registration path.
    unsafe { mi_page_map_register_typed(page) }
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool {
    let page_raw = page.as_ptr();
    // SAFETY[AUDIT:page-map-page-register-boundary]: typed page metadata exposes a stable
    // slice start for page-alignment validation.
    crate::internal::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    // SAFETY[AUDIT:page-map-page-register-boundary]: lazy initialization is serialized by
    // allocator lifecycle before registration proceeds.
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    // SAFETY[AUDIT:page-map-page-register-boundary]: initialized page-map base must exist
    // after successful lazy initialization.
    crate::internal::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    // SAFETY[AUDIT:page-map-page-register-boundary]: typed page metadata is converted once
    // into page-map index and slice-count domains.
    let Some((idx, _page_start, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY[AUDIT:page-map-page-register-boundary]: typed index/count witnesses bound the
    // 32-bit byte-map range registration.
    let Some(registered) = (unsafe { mi_page_map_register_range_typed(page, idx, slice_count) })
    else {
        return false;
    };
    crate::internal::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());
    crate::internal::mi_debug_assert!(registered.index() == idx);
    crate::internal::mi_debug_assert!(registered.slice_count() == slice_count);
    true
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_register_range_typed(
    page: PagePtr,
    idx: crate::core_types::PageMapIndex,
    slice_count: crate::core_types::SliceCount,
) -> Option<RegisteredPageRange> {
    // SAFETY[AUDIT:page-map-commit-boundary]: range commit is bounded by the typed
    // index and slice-count domain.
    if !unsafe { mi_page_map_ensure_committed(idx, slice_count) } {
        return None;
    }
    for i in 0..slice_count.get() as usize {
        crate::internal::mi_assert_internal!(i < 128);
        // SAFETY[AUDIT:page-map-range-register-boundary]: each registration byte is inside the
        // committed range described by `idx` and `slice_count`.
        unsafe { *_mi_page_map.add(idx.get() + i) = (i + 1) as u8 };
    }
    // SAFETY[AUDIT:page-map-range-register-boundary]: 32-bit page map has a single sub-index
    // domain, so zero is the only valid sub-index witness.
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    // SAFETY[AUDIT:page-map-range-register-boundary]: range bytes were just registered for the
    // page, so the witness matches the published page-map state.
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `page` must point to a previously registered live page whose slice metadata
/// remains valid for unregistering.
#[cfg(target_pointer_width = "32")]
// SAFETY[AUDIT:page-map-page-unregister-boundary]: C ABI unregister validates the raw page into
// a typed page witness before clearing the registered range.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister(page: *mut mi_page_t) {
    let Some(page) = PagePtr::new(page) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: typed page witness scopes the unregister
    // path to previously registered page metadata.
    unsafe { mi_page_map_unregister_typed(page) };
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_unregister_typed(page: PagePtr) {
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: unregister requires an initialized global
    // page-map base before deriving a range.
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: typed page metadata is converted into the
    // 32-bit registration index and slice-count domains.
    let Some((idx, _page_start, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: zero sub-index is the 32-bit witness for
    // the registered page range.
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: the page was previously registered for
    // this derived range, producing a valid unregister witness.
    let registered = unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: unregister witness bounds the byte range
    // clear.
    unsafe { mi_page_map_unregister_range_typed(registered) };
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_unregister_range_typed(registered: RegisteredPageRange) {
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: registered range witness bounds the
    // committed 32-bit byte-map region that is cleared.
    unsafe {
        _mi_memzero(
            _mi_page_map.add(registered.index().get()).cast::<c_void>(),
            registered.slice_count().get() as usize,
        )
    };
}

/// # Safety
///
/// `start..start + size` must describe a page-map range previously registered
/// or otherwise safe to clear during allocator teardown.
#[cfg(target_pointer_width = "32")]
// SAFETY[AUDIT:page-map-range-unregister-boundary]: C ABI range unregister validates raw start
// and size into typed page-map index/count domains before clearing.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister_range(start: *mut c_void, size: usize) {
    let slice_count = _mi_divide_up(size, MI_ARENA_SLICE_SIZE);
    let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
        crate::internal::mi_assert_internal!(size == 0);
        return;
    };
    let index = _mi_page_map_index(start.cast_const());
    let Some(index) = mi_page_map_index(index) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:page-map-commit-boundary]: typed index/count bounds the commit check
    // for the 32-bit byte-map range.
    if !unsafe { mi_page_map_ensure_committed(index, slice_count) } {
        return;
    }
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: committed typed range is cleared from the
    // byte-map backing store.
    unsafe {
        _mi_memzero(
            _mi_page_map.add(index.get()).cast::<c_void>(),
            slice_count.get() as usize,
        )
    };
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_ensure_committed(
    idx: crate::core_types::PageMapIndex,
    submap: *mut mi_submap_t,
) -> bool {
    // SAFETY[AUDIT:page-map-top-level-lazy-commit-boundary]: caller provides a writable out slot that
    // is still null before commit/publication fills it.
    crate::internal::mi_assert_internal!(!submap.is_null() && unsafe { (*submap).is_null() });
    let mut bit_idx = 0usize;
    if !mi_page_map_is_committed(idx, &mut bit_idx) {
        // SAFETY[AUDIT:page-map-top-level-lazy-commit-boundary]: committed bit index maps to a
        // page-map chunk inside the reserved top-level submap table.
        let start = unsafe {
            (&raw mut _mi_page_map)
                .read()
                .cast::<u8>()
                .add(bit_idx * MI_PAGE_MAP_ENTRIES_PER_CBIT * core::mem::size_of::<mi_submap_t>())
        };
        // SAFETY[AUDIT:page-map-top-level-lazy-commit-boundary]: commits exactly the top-level
        // page-map chunk represented by the validated commit bit.
        if !unsafe {
            _mi_os_commit(
                start.cast::<c_void>(),
                MI_PAGE_MAP_ENTRIES_PER_CBIT * core::mem::size_of::<mi_submap_t>(),
                ptr::null_mut(),
            )
        } {
            mi_page_map_cannot_commit();
            return false;
        }
        mi_page_map_commit.fetch_or(1usize << bit_idx, Ordering::AcqRel);
    }
    // SAFETY[AUDIT:page-map-top-level-lazy-commit-boundary]: committed top-level index can be read via
    // `_mi_page_map_at` and published to the caller's out slot.
    unsafe {
        *submap = _mi_page_map_at(idx.get());
    }
    true
}

/// # Safety
///
/// Must be called during allocator initialization before concurrent page-map
/// use starts.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY[AUDIT:page-map-global-bootstrap-boundary]: non-32-bit page-map initialization owns the
// top-level table and first submap before concurrent allocator use starts.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_init() -> bool {
    if MI_PAGE_MAP_FLAT != 0 {
        return false;
    }

    let mut vbits =
        mi_option_get_clamp(mi_option_t::mi_option_max_vabits, 0, MI_SIZE_BITS as i64) as usize;
    if vbits == 0 {
        // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: platform virtual-address query is
        // read-only during allocator initialization.
        vbits = unsafe { _mi_os_virtual_address_bits() };
        #[cfg(target_arch = "x86_64")]
        if vbits >= 48 {
            vbits = 47;
        }
    }
    if vbits < MI_PAGE_MAP_SUB_SHIFT + MI_ARENA_SLICE_SHIFT {
        vbits = MI_PAGE_MAP_SUB_SHIFT + MI_ARENA_SLICE_SHIFT;
    }

    // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: initialization has exclusive access to
    // max-address and count globals before publishing the page-map base.
    unsafe {
        mi_page_map_max_address = if vbits >= MI_SIZE_BITS {
            (usize::MAX - (1usize << MI_ARENA_SLICE_SHIFT) + 1) as *mut c_void
        } else {
            (1usize << vbits) as *mut c_void
        };
        mi_page_map_count = 1usize << (vbits - MI_PAGE_MAP_SUB_SHIFT - MI_ARENA_SLICE_SHIFT);
    }
    // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: page-map count was just derived from
    // virtual-address bits and is still owned by initialization.
    crate::internal::mi_assert!(unsafe { mi_page_map_count } <= MI_PAGE_MAP_COUNT);

    // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: OS page-size query is read-only during
    // initialization.
    let os_page_size = unsafe { crate::os::_mi_os_page_size_typed().get() };
    let page_map_size = _mi_align_up(
        // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: page-map count was initialized above
        // and bounds the top-level table reservation.
        unsafe { mi_page_map_count } * core::mem::size_of::<mi_submap_t>(),
        os_page_size,
    );
    let submap_size = MI_PAGE_MAP_SUB_SIZE;
    let reserve_size = page_map_size + submap_size;
    let commit = page_map_size <= 64 * MI_KiB
        || mi_option_is_enabled(mi_option_t::mi_option_pagemap_commit)
        // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: overcommit capability query is
        // read-only and only influences reservation commit policy.
        || unsafe { _mi_os_has_overcommit() };

    // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: initialization reserves the top-level
    // table plus first submap and records the OS memid in global metadata.
    let base =
        unsafe { _mi_os_alloc_aligned(reserve_size, 1, commit, true, &raw mut mi_page_map_memid) }
            as *mut mi_submap_t;
    if base.is_null() {
        _mi_error_message_fmt(
            Errno::ENOMEM,
            format_args!(
                "unable to reserve virtual memory for the page map ({} KiB)\n",
                page_map_size / MI_KiB
            ),
        );
        return false;
    }
    // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: initialization publishes the top-level
    // base only after successful reservation.
    unsafe {
        _mi_page_map = base;
    }
    // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: memid flags describe the just-reserved
    // top-level page-map allocation before concurrent publication.
    if unsafe { mi_page_map_memid.initially_committed && !mi_page_map_memid.initially_zero } {
        // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: committed top-level table is writable
        // and sized by `page_map_size`.
        unsafe {
            _mi_memzero_aligned(_mi_page_map.cast::<c_void>(), page_map_size);
        }
    }
    mi_page_map_commit.store(
        // SAFETY[AUDIT:page-map-global-bootstrap-boundary]: initial commit flags come from the
        // page-map backing allocation memid captured above.
        if unsafe { mi_page_map_memid.initially_committed } {
            usize::MAX
        } else {
            0
        },
        Ordering::Release,
    );

    // SAFETY[AUDIT:page-map-initial-submap-boundary]: first submap is carved from the reserved
    // allocation immediately after the aligned top-level table.
    let sub0 = unsafe { _mi_page_map.cast::<u8>().add(page_map_size) }.cast::<*mut mi_page_t>();
    // SAFETY[AUDIT:page-map-initial-submap-boundary]: initial commit flags describe whether
    // the first submap range needs an explicit commit.
    if !unsafe { mi_page_map_memid.initially_committed }
        // SAFETY[AUDIT:page-map-initial-submap-boundary]: commits the first submap range before
        // it is published at top-level index zero.
        && !unsafe { _mi_os_commit(sub0.cast::<c_void>(), submap_size, ptr::null_mut()) }
    {
        mi_page_map_cannot_commit();
        return false;
    }
    // SAFETY[AUDIT:page-map-initial-submap-boundary]: initial zeroing flag determines whether
    // the first committed submap range needs explicit clearing.
    if !unsafe { mi_page_map_memid.initially_zero } {
        unsafe { _mi_memzero_aligned(sub0.cast::<c_void>(), submap_size) };
    }

    let mut nullsub: mi_submap_t = ptr::null_mut();
    // SAFETY[AUDIT:page-map-null-sentinel-boundary]: zero index is within the top-level
    // page-map domain for the null-pointer sentinel submap.
    let zero_index = unsafe { crate::core_types::PageMapIndex::new_unchecked(0) };
    // SAFETY[AUDIT:page-map-null-sentinel-boundary]: initialization commits/publishes the
    // top-level zero entry before installing the first submap.
    if !unsafe { mi_page_map_ensure_committed(zero_index, &mut nullsub) } {
        mi_page_map_cannot_commit();
        return false;
    }
    // SAFETY[AUDIT:page-map-null-sentinel-boundary]: top-level index zero and the lock global
    // are exclusively owned during initialization.
    unsafe {
        *_mi_page_map.add(0) = sub0;
        mi_lock_init(&raw mut mi_page_map_lock);
    }
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:page-map-null-sentinel-boundary]: initialization verifies null lookup
        // through the page-map facade after publishing index zero.
        unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null()
    );
    true
}

/// # Safety
///
/// `subproc` must point to the live sub-process owning the page-map memory.
/// No concurrent page-map users may remain.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY[AUDIT:page-map-teardown-boundary]: unsafe destroy runs after page-map users are
// quiesced and owns lock, submap, and top-level table teardown.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::internal::mi_assert_internal!(!subproc.is_null());
    // SAFETY[AUDIT:page-map-teardown-boundary]: destroy checks initialized global base
    // before tearing down submaps and the top-level table.
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }

    // SAFETY[AUDIT:page-map-teardown-boundary]: no page-map users remain, so the global
    // growth lock can be destroyed before clearing submaps.
    unsafe { mi_lock_done(&raw mut mi_page_map_lock) };
    // SAFETY[AUDIT:page-map-teardown-boundary]: page-map count is stable during teardown
    // and bounds the top-level submap scan.
    for idx in 1..unsafe { mi_page_map_count } {
        let Some(map_idx) = mi_page_map_index(idx) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        if mi_page_map_is_committed(map_idx, ptr::null_mut()) {
            // SAFETY[AUDIT:page-map-teardown-boundary]: committed top-level index can be
            // loaded through the audited page-map accessor.
            let sub = unsafe { _mi_page_map_at(map_idx.get()) };
            if !sub.is_null() {
                // SAFETY[AUDIT:page-map-teardown-boundary]: non-null submap was allocated
                // as a page-map submap and can be described by an OS memid.
                let memid = unsafe {
                    _mi_memid_create_os(
                        sub.cast::<c_void>(),
                        crate::core_types::ReservedBytes::new(MI_PAGE_MAP_SUB_SIZE),
                        true,
                        false,
                        false,
                    )
                };
                // SAFETY[AUDIT:page-map-teardown-boundary]: destroy frees the submap and
                // clears its top-level slot while users are quiesced.
                unsafe {
                    _mi_os_free_ex(
                        sub.cast::<c_void>(),
                        MI_PAGE_MAP_SUB_SIZE,
                        true,
                        memid,
                        subproc,
                    );
                    *_mi_page_map.add(idx) = ptr::null_mut();
                }
            }
        }
    }
    // SAFETY[AUDIT:page-map-teardown-boundary]: memid was recorded by initialization and
    // still describes the top-level page-map allocation at teardown.
    let (_, size) = mi_memid_os_base_size(unsafe { mi_page_map_memid });
    // SAFETY[AUDIT:page-map-teardown-boundary]: destroy frees the top-level allocation
    // and clears process-global page-map metadata.
    unsafe {
        _mi_os_free_ex(
            _mi_page_map.cast::<c_void>(),
            size,
            true,
            mi_page_map_memid,
            subproc,
        );
        _mi_page_map = ptr::null_mut();
        mi_page_map_count = 0;
        mi_page_map_memid = _mi_memid_none();
        mi_page_map_max_address = ptr::null_mut();
    }
    mi_page_map_commit.store(0, Ordering::Release);
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_ensure_submap_at(
    idx: crate::core_types::PageMapIndex,
    submap: *mut mi_submap_t,
) -> bool {
    // SAFETY[AUDIT:page-map-commit-boundary]: caller provides a writable null out slot
    // for the published or newly allocated submap.
    crate::internal::mi_assert_internal!(!submap.is_null() && unsafe { (*submap).is_null() });
    let mut sub = ptr::null_mut();
    // SAFETY[AUDIT:page-map-commit-boundary]: typed top-level index bounds the commit
    // check and initial submap slot load.
    if !unsafe { mi_page_map_ensure_committed(idx, &mut sub) } {
        return false;
    }
    if sub.is_null() {
        // SAFETY[AUDIT:page-map-submap-publication-boundary]: global page-map growth lock protects
        // submap allocation/publication for this slot.
        unsafe { mi_lock_acquire(&raw mut mi_page_map_lock) };
        // SAFETY[AUDIT:page-map-submap-publication-boundary]: typed index bounds the top-level slot
        // reinterpreted as an audited atomic pointer field.
        let atomic_submap =
            crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });
        // SAFETY[AUDIT:page-map-submap-publication-boundary]: acquire load observes any submap
        // published for this top-level slot.
        sub = unsafe { atomic_submap.load(crate::core_types::MemoryOrder::Acquire) };
        if sub.is_null() {
            // SAFETY[AUDIT:page-map-submap-publication-boundary]: local memid starts empty and will
            // be filled by the OS zero-allocation helper on success.
            let mut memid = unsafe { _mi_memid_none() };
            // SAFETY[AUDIT:page-map-submap-publication-boundary]: allocates a zeroed submap-sized
            // backing range for publication in exactly one top-level slot.
            let new_sub = unsafe { _mi_os_zalloc(MI_PAGE_MAP_SUB_SIZE, &raw mut memid) }
                .cast::<*mut mi_page_t>();
            if new_sub.is_null() {
                _mi_warning_message_fmt(format_args!(
                    "internal error: unable to extend the page map\n"
                ));
            } else {
                let mut expect: mi_submap_t = ptr::null_mut();
                // SAFETY[AUDIT:page-map-submap-publication-boundary]: CAS publishes the new submap
                // only if the slot remained null under the growth lock.
                if !unsafe { atomic_submap.compare_exchange_acq_rel(&mut expect, new_sub) } {
                    // SAFETY[AUDIT:page-map-submap-publication-boundary]: failed publication means
                    // another submap owns the slot, so the new backing range is freed.
                    unsafe { _mi_os_free(new_sub.cast::<c_void>(), MI_PAGE_MAP_SUB_SIZE, memid) };
                    sub = expect;
                } else {
                    sub = new_sub;
                }
            }
        }
        // SAFETY[AUDIT:page-map-submap-publication-boundary]: releases the global page-map growth
        // lock after slot publication or failed allocation.
        unsafe { mi_lock_release(&raw mut mi_page_map_lock) };
        if sub.is_null() {
            return false;
        }
    }
    // SAFETY[AUDIT:page-map-submap-publication-boundary]: non-null submap is published to the
    // caller-provided out slot.
    unsafe { *submap = sub };
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_set_range_prim(
    page: PagePtr,
    mut idx: crate::core_types::PageMapIndex,
    mut sub_idx: crate::core_types::PageMapSubIndex,
    slice_count: crate::core_types::SliceCount,
) -> bool {
    let page = page.as_ptr();
    let mut slice_count = slice_count.get() as usize;
    while slice_count > 0 {
        let mut map_sub_idx = sub_idx;
        let mut sub: mi_submap_t = ptr::null_mut();
        // SAFETY[AUDIT:page-map-commit-boundary]: typed top-level index and out slot
        // bound submap commit/allocation before page publication.
        if !unsafe { mi_page_map_ensure_submap_at(idx, &mut sub) } {
            return false;
        }
        crate::internal::mi_assert_internal!(!sub.is_null());
        while slice_count > 0 && map_sub_idx.get() < MI_PAGE_MAP_SUB_COUNT {
            // SAFETY[AUDIT:page-map-range-primitive-boundary]: typed sub-index bounds the write
            // inside the published submap slot range.
            unsafe { *sub.add(map_sub_idx.get()) = page };
            slice_count -= 1;
            map_sub_idx = match mi_page_map_sub_index(map_sub_idx.get() + 1) {
                Some(next) => next,
                None => break,
            };
        }
        if slice_count > 0 {
            let Some(next_idx) = mi_page_map_index(idx.get() + 1) else {
                crate::internal::mi_assert_internal!(false);
                return false;
            };
            idx = next_idx;
            // SAFETY[AUDIT:page-map-range-primitive-boundary]: advancing to the next top-level
            // submap resets to the first valid sub-index.
            sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
        }
    }
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_clear_range_prim(
    mut idx: crate::core_types::PageMapIndex,
    mut sub_idx: crate::core_types::PageMapSubIndex,
    slice_count: crate::core_types::SliceCount,
) -> bool {
    let mut slice_count = slice_count.get() as usize;
    while slice_count > 0 {
        let mut map_sub_idx = sub_idx;
        let mut sub: mi_submap_t = ptr::null_mut();
        // SAFETY[AUDIT:page-map-range-primitive-boundary]: typed top-level index and out slot
        // bound submap commit/allocation before clearing entries.
        if !unsafe { mi_page_map_ensure_submap_at(idx, &mut sub) } {
            return false;
        }
        crate::internal::mi_assert_internal!(!sub.is_null());
        while slice_count > 0 && map_sub_idx.get() < MI_PAGE_MAP_SUB_COUNT {
            // SAFETY[AUDIT:page-map-range-primitive-boundary]: typed sub-index bounds the clear
            // inside the published submap slot range.
            unsafe { *sub.add(map_sub_idx.get()) = ptr::null_mut() };
            slice_count -= 1;
            map_sub_idx = match mi_page_map_sub_index(map_sub_idx.get() + 1) {
                Some(next) => next,
                None => break,
            };
        }
        if slice_count > 0 {
            let Some(next_idx) = mi_page_map_index(idx.get() + 1) else {
                crate::internal::mi_assert_internal!(false);
                return false;
            };
            idx = next_idx;
            // SAFETY[AUDIT:page-map-range-primitive-boundary]: advancing to the next top-level
            // submap resets to the first valid sub-index.
            sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
        }
    }
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_set_range(
    page: PagePtr,
    idx: crate::core_types::PageMapIndex,
    sub_idx: crate::core_types::PageMapSubIndex,
    slice_count: crate::core_types::SliceCount,
) -> bool {
    // SAFETY[AUDIT:page-map-range-register-boundary]: typed range bounds every submap write; on
    // failure, the same typed range is cleared for rollback.
    if !unsafe { mi_page_map_set_range_prim(page, idx, sub_idx, slice_count) } {
        unsafe { mi_page_map_clear_range_prim(idx, sub_idx, slice_count) };
        return false;
    }
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_clear_range(
    idx: crate::core_types::PageMapIndex,
    sub_idx: crate::core_types::PageMapSubIndex,
    slice_count: crate::core_types::SliceCount,
) -> bool {
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: typed range bounds the submap clear
    // operation.
    unsafe { mi_page_map_clear_range_prim(idx, sub_idx, slice_count) }
}

/// # Safety
///
/// `page` must point to a live page whose page area and slice metadata are
/// initialized and stable for registration.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY[AUDIT:page-map-page-register-boundary]: C ABI registration validates the raw page
// pointer into `PagePtr` before touching page-map state.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_register(page: *mut mi_page_t) -> bool {
    crate::internal::mi_assert_internal!(!page.is_null());
    let Some(page) = PagePtr::new(page) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY[AUDIT:page-map-page-register-boundary]: `PagePtr` proves non-null page metadata
    // for the typed registration path.
    unsafe { mi_page_map_register_typed(page) }
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool {
    let page_raw = page.as_ptr();
    // SAFETY[AUDIT:page-map-page-register-boundary]: typed page metadata exposes a stable
    // slice start for page-alignment validation.
    crate::internal::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    // SAFETY[AUDIT:page-map-page-register-boundary]: lazy initialization is serialized by
    // allocator lifecycle before registration proceeds.
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    // SAFETY[AUDIT:page-map-page-register-boundary]: initialized page-map base must exist
    // after successful lazy initialization.
    crate::internal::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    // SAFETY[AUDIT:page-map-page-register-boundary]: typed page metadata is converted once
    // into page-map index, sub-index, and slice-count domains.
    let Some((idx, sub_idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    // SAFETY[AUDIT:page-map-page-register-boundary]: typed index/sub-index/count witnesses
    // bound the non-32-bit submap range registration.
    let Some(registered) =
        (unsafe { mi_page_map_register_range_typed(page, idx, sub_idx, slice_count) })
    else {
        return false;
    };
    crate::internal::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());
    crate::internal::mi_debug_assert!(registered.index() == idx);
    crate::internal::mi_debug_assert!(registered.sub_index() == sub_idx);
    crate::internal::mi_debug_assert!(registered.slice_count() == slice_count);
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_register_range_typed(
    page: PagePtr,
    idx: crate::core_types::PageMapIndex,
    sub_idx: crate::core_types::PageMapSubIndex,
    slice_count: crate::core_types::SliceCount,
) -> Option<RegisteredPageRange> {
    // SAFETY[AUDIT:page-map-range-register-boundary]: typed index/sub-index/count witnesses
    // bound all submap slot writes for the page.
    if !unsafe { mi_page_map_set_range(page, idx, sub_idx, slice_count) } {
        return None;
    }
    // SAFETY[AUDIT:page-map-range-register-boundary]: the range was just registered for this
    // page, so the witness matches the published page-map state.
    let registered = unsafe {
        crate::core_types::RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)
    };
    Some(registered)
}

/// # Safety
///
/// `page` must point to a previously registered live page whose slice metadata
/// remains valid for unregistering.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY[AUDIT:page-map-page-unregister-boundary]: C ABI unregister validates the raw page into
// a typed page witness before clearing the registered range.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister(page: *mut mi_page_t) {
    let Some(page) = PagePtr::new(page) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: typed page witness scopes the unregister
    // path to previously registered page metadata.
    unsafe { mi_page_map_unregister_typed(page) };
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_unregister_typed(page: PagePtr) {
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: unregister requires an initialized global
    // page-map base before deriving a range.
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    let page_raw = page.as_ptr();
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: typed page metadata exposes a stable
    // slice start for page-alignment validation.
    crate::internal::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: null base check preserves C-compatible
    // no-op behavior if teardown already cleared the map.
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: typed page metadata is converted into the
    // non-32-bit registration index/sub-index/count domains.
    let Some((idx, sub_idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: the page was previously registered for
    // this derived range, producing a valid unregister witness.
    let registered = unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) };
    // SAFETY[AUDIT:page-map-page-unregister-boundary]: unregister witness bounds the submap
    // range clear.
    unsafe { mi_page_map_unregister_range_typed(registered) };
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_unregister_range_typed(registered: RegisteredPageRange) {
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: registered range witness bounds the
    // submap range that is cleared.
    unsafe {
        mi_page_map_clear_range(
            registered.index(),
            registered.sub_index(),
            registered.slice_count(),
        )
    };
}

/// # Safety
///
/// `start..start + size` must describe a page-map range previously registered
/// or otherwise safe to clear during allocator teardown.
#[cfg(not(target_pointer_width = "32"))]
// SAFETY[AUDIT:page-map-range-unregister-boundary]: C ABI range unregister validates raw start
// and size into typed page-map index/sub-index/count domains before clearing.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister_range(start: *mut c_void, size: usize) {
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: null base check preserves C-compatible
    // no-op behavior if teardown already cleared the map.
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let slice_count = _mi_divide_up(size, MI_ARENA_SLICE_SIZE);
    let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
        crate::internal::mi_assert_internal!(size == 0);
        return;
    };
    let mut sub_idx = 0usize;
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: raw start and stack-local sub-index out
    // pointer are converted to typed page-map domains below.
    let idx = unsafe { _mi_page_map_index(start.cast_const(), &mut sub_idx) };
    let Some(idx) = mi_page_map_index(idx) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    let Some(sub_idx) = mi_page_map_sub_index(sub_idx) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:page-map-range-unregister-boundary]: typed index/sub-index/count witnesses
    // bound the non-32-bit range clear.
    unsafe { mi_page_map_clear_range(idx, sub_idx, slice_count) };
}

// SAFETY[AUDIT:page-map-range-unregister-boundary]: retained function pointer keeps the local C
// ABI range-unregister boundary linked without exporting it dynamically.
#[used]
static MI_PAGE_MAP_UNREGISTER_RANGE_KEEP: unsafe extern "C" fn(*mut c_void, usize) =
    _mi_page_map_unregister_range;
