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

pub const MI_PAGE_MAP_SUB_SIZE: usize =
    MI_PAGE_MAP_SUB_COUNT * core::mem::size_of::<*mut mi_page_t>();
const MI_PAGE_MAP_ENTRIES_PER_CBIT: usize = if MI_PAGE_MAP_COUNT < MI_BFIELD_BITS {
    1
} else {
    MI_PAGE_MAP_COUNT / MI_BFIELD_BITS
};

#[cfg(not(target_pointer_width = "32"))]
pub(crate) static mut _mi_page_map: *mut mi_submap_t = ptr::null_mut();

#[cfg(target_pointer_width = "32")]
pub(crate) static mut _mi_page_map: *mut u8 = ptr::null_mut();

/// # Safety
///
/// The page map must be initialized before callers dereference the returned
/// base pointer. Concurrent teardown must be excluded by allocator lifecycle.
#[cfg(not(target_pointer_width = "32"))]
#[inline(always)]
pub(crate) unsafe fn _mi_page_map_base() -> *mut mi_submap_t {
    unsafe { _mi_page_map }
}

/// # Safety
///
/// The page map must be initialized before callers dereference the returned
/// base pointer. Concurrent teardown must be excluded by allocator lifecycle.
#[cfg(target_pointer_width = "32")]
#[inline(always)]
pub(crate) unsafe fn _mi_page_map_base() -> *mut u8 {
    unsafe { _mi_page_map }
}

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
            crate::mi_assert_internal!(false);
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
    let page_start = unsafe { mi_page_area(page, &mut page_size) };
    if page_size > MI_LARGE_PAGE_SIZE {
        page_size = MI_LARGE_PAGE_SIZE - MI_ARENA_SLICE_SIZE;
    }
    let slice_count = mi_slice_count_of_size(page_size)
        + (crate::core_types::ptr_addr_diff(page_start, unsafe { mi_page_slice_start(page) })
            / MI_ARENA_SLICE_SIZE);
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
    let idx = unsafe { _mi_page_map_index(page.as_ptr().cast::<c_void>()) };
    Some((mi_page_map_index(idx)?, span.page_start, span.slice_count))
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_get_idx_from_span(
    _page: PagePtr,
    span: PageMapPageSpan,
) -> Option<PageMapGetIndex> {
    let mut sub_idx = 0usize;
    let idx = unsafe { _mi_page_map_index(span.page_start.cast::<c_void>(), &mut sub_idx) };
    Some((
        mi_page_map_index(idx)?,
        mi_page_map_sub_index(sub_idx)?,
        span.slice_count,
    ))
}

unsafe fn mi_page_map_get_idx(page: PagePtr) -> Option<PageMapGetIndex> {
    let span = unsafe { mi_page_map_page_span(page) }?;
    unsafe { mi_page_map_get_idx_from_span(page, span) }
}

#[inline]
fn mi_page_map_is_committed(idx: crate::core_types::PageMapIndex, pbit_idx: *mut usize) -> bool {
    let commit = mi_page_map_commit.load(Ordering::Relaxed);
    let bit_idx = idx.get() / MI_PAGE_MAP_ENTRIES_PER_CBIT;
    crate::mi_assert_internal!(bit_idx < MI_BFIELD_BITS);
    if !pbit_idx.is_null() {
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
    if unsafe { !mi_page_map_commit.is_null() } {
        let commit_idx = idx / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT;
        let commit_idx_hi = (idx + slice_count - 1) / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT;
        for i in commit_idx..=commit_idx_hi {
            if unsafe { mi_bitmap_is_clear(mi_page_map_commit, i) } {
                let start = unsafe { _mi_page_map.add(i * MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT) };
                let size = MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT;
                let mut is_zero = false;
                if !unsafe { _mi_os_commit(start.cast::<c_void>(), size, &mut is_zero) } {
                    mi_page_map_cannot_commit();
                    return false;
                }
                if !is_zero && !unsafe { mi_page_map_memid.initially_zero } {
                    unsafe { _mi_memzero(start.cast::<c_void>(), size) };
                }
                unsafe { mi_bitmap_set(mi_page_map_commit, i) };
            }
        }
    }
    if cfg!(debug_assertions) {
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_init() -> bool {
    let mut vbits =
        mi_option_get_clamp(mi_option_t::mi_option_max_vabits, 0, MI_SIZE_BITS as i64) as usize;
    if vbits == 0 {
        vbits = unsafe { _mi_os_virtual_address_bits() };
        #[cfg(target_arch = "x86_64")]
        if vbits >= 48 {
            vbits = 47;
        }
    }
    if vbits < MI_ARENA_SLICE_SHIFT {
        vbits = MI_ARENA_SLICE_SHIFT;
    }

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
        unsafe { mi_bitmap_size(commit_bits, ptr::null_mut()) }
    };
    let reserve_size = bitmap_size + page_map_size;
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
    if unsafe { mi_page_map_memid.initially_committed && !mi_page_map_memid.initially_zero } {
        _mi_warning_message_fmt(format_args!(
            "internal: the page map was committed but not zero initialized!\n"
        ));
        unsafe { _mi_memzero_aligned(base.cast::<c_void>(), reserve_size) };
    }
    if bitmap_size > 0 {
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
    unsafe {
        _mi_page_map = base.add(bitmap_size);
    }
    let zero_index = unsafe { crate::core_types::PageMapIndex::new_unchecked(0) };
    let one_slice = unsafe { crate::core_types::SliceCount::new_unchecked(1) };
    if !commit && !unsafe { mi_page_map_ensure_committed(zero_index, one_slice) } {
        mi_page_map_cannot_commit();
        return false;
    }
    unsafe {
        *_mi_page_map.add(0) = 1;
    }
    crate::mi_assert_internal!(unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null());
    true
}

/// # Safety
///
/// `subproc` must point to the live sub-process owning the page-map memory.
/// No concurrent page-map users may remain.
#[cfg(target_pointer_width = "32")]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::mi_assert_internal!(!subproc.is_null());
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let (base, size) = mi_memid_os_base_size(unsafe { mi_page_map_memid });
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_register(page: *mut mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    let Some(page) = PagePtr::new(page) else {
        crate::mi_assert_internal!(false);
        return false;
    };
    unsafe { mi_page_map_register_typed(page) }
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool {
    let page_raw = page.as_ptr();
    crate::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    crate::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    let Some((idx, _page_start, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::mi_assert_internal!(false);
        return false;
    };
    let Some(registered) = (unsafe { mi_page_map_register_range_typed(page, idx, slice_count) })
    else {
        return false;
    };
    crate::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());
    crate::mi_debug_assert!(registered.index() == idx);
    crate::mi_debug_assert!(registered.slice_count() == slice_count);
    true
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_register_range_typed(
    page: PagePtr,
    idx: crate::core_types::PageMapIndex,
    slice_count: crate::core_types::SliceCount,
) -> Option<RegisteredPageRange> {
    if !unsafe { mi_page_map_ensure_committed(idx, slice_count) } {
        return None;
    }
    for i in 0..slice_count.get() as usize {
        crate::mi_assert_internal!(i < 128);
        unsafe { *_mi_page_map.add(idx.get() + i) = (i + 1) as u8 };
    }
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `page` must point to a previously registered live page whose slice metadata
/// remains valid for unregistering.
#[cfg(target_pointer_width = "32")]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister(page: *mut mi_page_t) {
    let Some(page) = PagePtr::new(page) else {
        crate::mi_assert_internal!(false);
        return;
    };
    unsafe { mi_page_map_unregister_typed(page) };
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_unregister_typed(page: PagePtr) {
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    let Some((idx, _page_start, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::mi_assert_internal!(false);
        return;
    };
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    let registered = unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) };
    unsafe { mi_page_map_unregister_range_typed(registered) };
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_unregister_range_typed(registered: RegisteredPageRange) {
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister_range(start: *mut c_void, size: usize) {
    let slice_count = _mi_divide_up(size, MI_ARENA_SLICE_SIZE);
    let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
        crate::mi_assert_internal!(size == 0);
        return;
    };
    let index = _mi_page_map_index(start.cast_const());
    let Some(index) = mi_page_map_index(index) else {
        crate::mi_assert_internal!(false);
        return;
    };
    if !unsafe { mi_page_map_ensure_committed(index, slice_count) } {
        return;
    }
    unsafe {
        _mi_memzero(
            _mi_page_map.add(index.get()).cast::<c_void>(),
            slice_count.get() as usize,
        )
    };
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    unsafe { _mi_safe_ptr_page_registered(p) }
        .map(|registered| registered.page().as_ptr())
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
#[inline(always)]
pub(crate) unsafe fn _mi_safe_ptr_page_registered(p: *const c_void) -> Option<RegisteredPageRange> {
    if p >= unsafe { mi_page_map_max_address } {
        return None;
    }
    let idx = _mi_page_map_index(p);
    let Some(idx) = mi_page_map_index(idx) else {
        crate::mi_assert_internal!(false);
        return None;
    };
    if unsafe { !mi_page_map_commit.is_null() }
        && !unsafe {
            mi_bitmap_is_set(
                mi_page_map_commit,
                idx.get() / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT,
            )
        }
    {
        return None;
    }
    let ofs = unsafe { *_mi_page_map.add(idx.get()) as usize };
    if ofs == 0 {
        return None;
    }
    let page = (((crate::core_types::ptr_exposed_addr(p) >> MI_ARENA_SLICE_SHIFT) - ofs + 1)
        << MI_ARENA_SLICE_SHIFT) as *mut mi_page_t;
    let page = PagePtr::new(page)?;
    let sub_idx = unsafe { crate::core_types::PageMapSubIndex::new_unchecked(0) };
    let slice_count = unsafe { SliceCount::from_usize_const(1).unwrap_unchecked() };
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(target_pointer_width = "32")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_ensure_committed(
    idx: crate::core_types::PageMapIndex,
    submap: *mut mi_submap_t,
) -> bool {
    crate::mi_assert_internal!(!submap.is_null() && unsafe { (*submap).is_null() });
    let mut bit_idx = 0usize;
    if !mi_page_map_is_committed(idx, &mut bit_idx) {
        let start = unsafe {
            (&raw mut _mi_page_map)
                .read()
                .cast::<u8>()
                .add(bit_idx * MI_PAGE_MAP_ENTRIES_PER_CBIT * core::mem::size_of::<mi_submap_t>())
        };
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_init() -> bool {
    if MI_PAGE_MAP_FLAT != 0 {
        return false;
    }

    let mut vbits =
        mi_option_get_clamp(mi_option_t::mi_option_max_vabits, 0, MI_SIZE_BITS as i64) as usize;
    if vbits == 0 {
        vbits = unsafe { _mi_os_virtual_address_bits() };
        #[cfg(target_arch = "x86_64")]
        if vbits >= 48 {
            vbits = 47;
        }
    }
    if vbits < MI_PAGE_MAP_SUB_SHIFT + MI_ARENA_SLICE_SHIFT {
        vbits = MI_PAGE_MAP_SUB_SHIFT + MI_ARENA_SLICE_SHIFT;
    }

    unsafe {
        mi_page_map_max_address = if vbits >= MI_SIZE_BITS {
            (usize::MAX - (1usize << MI_ARENA_SLICE_SHIFT) + 1) as *mut c_void
        } else {
            (1usize << vbits) as *mut c_void
        };
        mi_page_map_count = 1usize << (vbits - MI_PAGE_MAP_SUB_SHIFT - MI_ARENA_SLICE_SHIFT);
    }
    crate::mi_assert!(unsafe { mi_page_map_count } <= MI_PAGE_MAP_COUNT);

    let os_page_size = unsafe { crate::os::_mi_os_page_size_typed().get() };
    let page_map_size = _mi_align_up(
        unsafe { mi_page_map_count } * core::mem::size_of::<mi_submap_t>(),
        os_page_size,
    );
    let submap_size = MI_PAGE_MAP_SUB_SIZE;
    let reserve_size = page_map_size + submap_size;
    let commit = page_map_size <= 64 * MI_KiB
        || mi_option_is_enabled(mi_option_t::mi_option_pagemap_commit)
        || unsafe { _mi_os_has_overcommit() };

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
    unsafe {
        _mi_page_map = base;
    }
    if unsafe { mi_page_map_memid.initially_committed && !mi_page_map_memid.initially_zero } {
        unsafe {
            _mi_memzero_aligned(_mi_page_map.cast::<c_void>(), page_map_size);
        }
    }
    mi_page_map_commit.store(
        if unsafe { mi_page_map_memid.initially_committed } {
            usize::MAX
        } else {
            0
        },
        Ordering::Release,
    );

    let sub0 = unsafe { _mi_page_map.cast::<u8>().add(page_map_size) }.cast::<*mut mi_page_t>();
    if !unsafe { mi_page_map_memid.initially_committed }
        && !unsafe { _mi_os_commit(sub0.cast::<c_void>(), submap_size, ptr::null_mut()) }
    {
        mi_page_map_cannot_commit();
        return false;
    }
    if !unsafe { mi_page_map_memid.initially_zero } {
        unsafe { _mi_memzero_aligned(sub0.cast::<c_void>(), submap_size) };
    }

    let mut nullsub: mi_submap_t = ptr::null_mut();
    let zero_index = unsafe { crate::core_types::PageMapIndex::new_unchecked(0) };
    if !unsafe { mi_page_map_ensure_committed(zero_index, &mut nullsub) } {
        mi_page_map_cannot_commit();
        return false;
    }
    unsafe {
        *_mi_page_map.add(0) = sub0;
        mi_lock_init(&raw mut mi_page_map_lock);
    }
    crate::mi_assert_internal!(unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null());
    true
}

/// # Safety
///
/// `subproc` must point to the live sub-process owning the page-map memory.
/// No concurrent page-map users may remain.
#[cfg(not(target_pointer_width = "32"))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::mi_assert_internal!(!subproc.is_null());
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }

    unsafe { mi_lock_done(&raw mut mi_page_map_lock) };
    for idx in 1..unsafe { mi_page_map_count } {
        let Some(map_idx) = mi_page_map_index(idx) else {
            crate::mi_assert_internal!(false);
            continue;
        };
        if mi_page_map_is_committed(map_idx, ptr::null_mut()) {
            let sub = unsafe { _mi_page_map_at(map_idx.get()) };
            if !sub.is_null() {
                let memid = unsafe {
                    _mi_memid_create_os(
                        sub.cast::<c_void>(),
                        MI_PAGE_MAP_SUB_SIZE,
                        true,
                        false,
                        false,
                    )
                };
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
    let (_, size) = mi_memid_os_base_size(unsafe { mi_page_map_memid });
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
    crate::mi_assert_internal!(!submap.is_null() && unsafe { (*submap).is_null() });
    let mut sub = ptr::null_mut();
    if !unsafe { mi_page_map_ensure_committed(idx, &mut sub) } {
        return false;
    }
    if sub.is_null() {
        unsafe { mi_lock_acquire(&raw mut mi_page_map_lock) };
        let atomic_submap =
            crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });
        sub = unsafe { atomic_submap.load(crate::core_types::MemoryOrder::Acquire) };
        if sub.is_null() {
            let mut memid = unsafe { _mi_memid_none() };
            let new_sub = unsafe { _mi_os_zalloc(MI_PAGE_MAP_SUB_SIZE, &raw mut memid) }
                .cast::<*mut mi_page_t>();
            if new_sub.is_null() {
                _mi_warning_message_fmt(format_args!(
                    "internal error: unable to extend the page map\n"
                ));
            } else {
                let mut expect: mi_submap_t = ptr::null_mut();
                if !unsafe { atomic_submap.compare_exchange_acq_rel(&mut expect, new_sub) } {
                    unsafe { _mi_os_free(new_sub.cast::<c_void>(), MI_PAGE_MAP_SUB_SIZE, memid) };
                    sub = expect;
                } else {
                    sub = new_sub;
                }
            }
        }
        unsafe { mi_lock_release(&raw mut mi_page_map_lock) };
        if sub.is_null() {
            return false;
        }
    }
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
        if !unsafe { mi_page_map_ensure_submap_at(idx, &mut sub) } {
            return false;
        }
        crate::mi_assert_internal!(!sub.is_null());
        while slice_count > 0 && map_sub_idx.get() < MI_PAGE_MAP_SUB_COUNT {
            unsafe { *sub.add(map_sub_idx.get()) = page };
            slice_count -= 1;
            map_sub_idx = match mi_page_map_sub_index(map_sub_idx.get() + 1) {
                Some(next) => next,
                None => break,
            };
        }
        if slice_count > 0 {
            let Some(next_idx) = mi_page_map_index(idx.get() + 1) else {
                crate::mi_assert_internal!(false);
                return false;
            };
            idx = next_idx;
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
        if !unsafe { mi_page_map_ensure_submap_at(idx, &mut sub) } {
            return false;
        }
        crate::mi_assert_internal!(!sub.is_null());
        while slice_count > 0 && map_sub_idx.get() < MI_PAGE_MAP_SUB_COUNT {
            unsafe { *sub.add(map_sub_idx.get()) = ptr::null_mut() };
            slice_count -= 1;
            map_sub_idx = match mi_page_map_sub_index(map_sub_idx.get() + 1) {
                Some(next) => next,
                None => break,
            };
        }
        if slice_count > 0 {
            let Some(next_idx) = mi_page_map_index(idx.get() + 1) else {
                crate::mi_assert_internal!(false);
                return false;
            };
            idx = next_idx;
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
    unsafe { mi_page_map_clear_range_prim(idx, sub_idx, slice_count) }
}

/// # Safety
///
/// `page` must point to a live page whose page area and slice metadata are
/// initialized and stable for registration.
#[cfg(not(target_pointer_width = "32"))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_register(page: *mut mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    let Some(page) = PagePtr::new(page) else {
        crate::mi_assert_internal!(false);
        return false;
    };
    unsafe { mi_page_map_register_typed(page) }
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool {
    let page_raw = page.as_ptr();
    crate::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    crate::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    let Some((idx, sub_idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::mi_assert_internal!(false);
        return false;
    };
    let Some(registered) =
        (unsafe { mi_page_map_register_range_typed(page, idx, sub_idx, slice_count) })
    else {
        return false;
    };
    crate::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());
    crate::mi_debug_assert!(registered.index() == idx);
    crate::mi_debug_assert!(registered.sub_index() == sub_idx);
    crate::mi_debug_assert!(registered.slice_count() == slice_count);
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_register_range_typed(
    page: PagePtr,
    idx: crate::core_types::PageMapIndex,
    sub_idx: crate::core_types::PageMapSubIndex,
    slice_count: crate::core_types::SliceCount,
) -> Option<RegisteredPageRange> {
    if !unsafe { mi_page_map_set_range(page, idx, sub_idx, slice_count) } {
        return None;
    }
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister(page: *mut mi_page_t) {
    let Some(page) = PagePtr::new(page) else {
        crate::mi_assert_internal!(false);
        return;
    };
    unsafe { mi_page_map_unregister_typed(page) };
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_unregister_typed(page: PagePtr) {
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    let page_raw = page.as_ptr();
    crate::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let Some((idx, sub_idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::mi_assert_internal!(false);
        return;
    };
    let registered = unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) };
    unsafe { mi_page_map_unregister_range_typed(registered) };
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_unregister_range_typed(registered: RegisteredPageRange) {
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
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister_range(start: *mut c_void, size: usize) {
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let slice_count = _mi_divide_up(size, MI_ARENA_SLICE_SIZE);
    let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count) else {
        crate::mi_assert_internal!(size == 0);
        return;
    };
    let mut sub_idx = 0usize;
    let idx = unsafe { _mi_page_map_index(start.cast_const(), &mut sub_idx) };
    let Some(idx) = mi_page_map_index(idx) else {
        crate::mi_assert_internal!(false);
        return;
    };
    let Some(sub_idx) = mi_page_map_sub_index(sub_idx) else {
        crate::mi_assert_internal!(false);
        return;
    };
    unsafe { mi_page_map_clear_range(idx, sub_idx, slice_count) };
}

#[used]
static MI_PAGE_MAP_UNREGISTER_RANGE_KEEP: unsafe extern "C" fn(*mut c_void, usize) =
    _mi_page_map_unregister_range;

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    unsafe { _mi_safe_ptr_page_registered(p) }
        .map(|registered| registered.page().as_ptr())
        .unwrap_or(ptr::null_mut())
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
#[inline(always)]
pub(crate) unsafe fn _mi_safe_ptr_page_registered(p: *const c_void) -> Option<RegisteredPageRange> {
    if p.is_null() {
        return None;
    }
    if p >= unsafe { mi_page_map_max_address } {
        return None;
    }
    let mut sub_idx = 0usize;
    let idx = unsafe { _mi_page_map_index(p, &mut sub_idx) };
    let Some(idx) = mi_page_map_index(idx) else {
        crate::mi_assert_internal!(false);
        return None;
    };
    let Some(sub_idx) = mi_page_map_sub_index(sub_idx) else {
        crate::mi_assert_internal!(false);
        return None;
    };
    if !mi_page_map_is_committed(idx, ptr::null_mut()) {
        return None;
    }
    let sub = unsafe { _mi_page_map_at(idx.get()) };
    if sub.is_null() {
        return None;
    }
    let page = PagePtr::new(unsafe { *sub.add(sub_idx.get()) })?;
    let slice_count = unsafe { SliceCount::from_usize_const(1).unwrap_unchecked() };
    Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })
}

/// # Safety
///
/// `p` may be null or arbitrary. The page map must be initialized and not
/// concurrently destroyed while this lookup runs.
#[cfg(not(target_pointer_width = "32"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}
