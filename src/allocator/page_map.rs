//! Page map management.

#![allow(non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::ptr;
#[cfg(not(target_pointer_width = "32"))]
use core::ptr::NonNull;
#[cfg(not(target_pointer_width = "32"))]
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::allocator::arena::MI_ARENA_SLICE_SIZE;
use crate::allocator::options::{
    _mi_error_message_fmt, _mi_warning_message_fmt, mi_option_get_clamp, mi_option_is_enabled,
    mi_option_t,
};
use crate::allocator::page::MI_LARGE_PAGE_SIZE;
#[cfg(not(target_pointer_width = "32"))]
use crate::atomic::{mi_lock_done, mi_lock_init, mi_lock_t};
#[cfg(not(target_pointer_width = "32"))]
use crate::bitmap::MI_BFIELD_BITS;
#[cfg(target_pointer_width = "32")]
use crate::bitmap::{
    mi_bitmap_init, mi_bitmap_is_clear, mi_bitmap_is_set, mi_bitmap_set, mi_bitmap_size,
    mi_bitmap_t,
};
#[cfg(target_pointer_width = "32")]
use crate::bits::MI_MiB;
#[cfg(not(target_pointer_width = "32"))]
use crate::bits::MI_PAGE_MAP_FLAT;
use crate::bits::{MI_KiB, MI_SIZE_BITS};
#[cfg(target_pointer_width = "32")]
use crate::core_types::PageMapPrimaryIndex;
use crate::core_types::{Errno, PageMapLocation, PagePtr, RegisteredPageRange, SliceCount};
#[cfg(not(target_pointer_width = "32"))]
use crate::internal::memid::_mi_memid_create_os;
use crate::internal::memid::{
    _mi_memid_none, mi_memid_mem_t, mi_memid_os_info_t, mi_memid_t, mi_memkind_t,
};
#[cfg(not(target_pointer_width = "32"))]
use crate::internal::{
    _mi_align_up, _mi_os_free, _mi_os_has_overcommit, _mi_page_map_at, MI_PAGE_MAP_COUNT,
    MI_PAGE_MAP_SUB_SHIFT, mi_submap_t,
};
#[cfg(target_pointer_width = "32")]
use crate::internal::{_mi_divide_up, _mi_memzero};
use crate::internal::{
    _mi_is_aligned, _mi_memzero_aligned, _mi_os_commit, _mi_os_free_ex,
    _mi_os_virtual_address_bits, MI_ARENA_SLICE_SHIFT, MI_PAGE_MAP_SUB_COUNT, mi_page_slice_start,
    mi_slice_count_of_size,
};
use crate::os::_mi_os_alloc_aligned;
#[cfg(not(target_pointer_width = "32"))]
use crate::os::_mi_os_zalloc;
use crate::types::{MI_PAGE_ALIGN, mi_page_t, mi_subproc_t};
#[cfg(not(target_pointer_width = "32"))]
use crate::types::{MiLockGuard, PageMapSubIndex};

mod lookup;

pub(crate) use lookup::{_mi_safe_ptr_page, _mi_safe_ptr_page_registered};

pub const MI_PAGE_MAP_SUB_SIZE: usize =
    MI_PAGE_MAP_SUB_COUNT * core::mem::size_of::<*mut mi_page_t>();
#[cfg(not(target_pointer_width = "32"))]
const MI_PAGE_MAP_ENTRIES_PER_CBIT: usize = if MI_PAGE_MAP_COUNT < MI_BFIELD_BITS {
    1
} else {
    MI_PAGE_MAP_COUNT / MI_BFIELD_BITS
};

#[cfg(not(target_pointer_width = "32"))]
/// Non-owning witness for a published submap allocation. This type does not
/// extend the allocation lifetime or protect against allocator teardown.
#[repr(transparent)]
struct PublishedSubmap(NonNull<*mut mi_page_t>);

#[cfg(not(target_pointer_width = "32"))]
impl PublishedSubmap {
    /// # Safety
    ///
    /// If `submap` is non-null, it must address all `MI_PAGE_MAP_SUB_COUNT`
    /// slots and remain allocated while the returned value is used. Concurrent
    /// page-map teardown must be excluded. A null pointer is accepted and
    /// returns `None` without being dereferenced.
    #[inline(always)]
    unsafe fn from_raw(submap: mi_submap_t) -> Option<Self> {
        NonNull::new(submap).map(Self)
    }

    #[inline(always)]
    fn slot_ptr(&self, index: PageMapSubIndex) -> NonNull<*mut mi_page_t> {
        // SAFETY: construction proves the full submap allocation is live and
        // the typed index is strictly below `MI_PAGE_MAP_SUB_COUNT`.
        unsafe { NonNull::new_unchecked(self.0.as_ptr().add(index.get())) }
    }
}

/// Snapshot of the commit bit covering one typed top-level page-map index.
#[cfg(not(target_pointer_width = "32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PageMapCommitStatus {
    bit_index: usize,
    committed: bool,
}

#[cfg(not(target_pointer_width = "32"))]
impl PageMapCommitStatus {
    #[inline(always)]
    fn from_index_and_word(idx: crate::core_types::PageMapIndex, commit: usize) -> Self {
        let bit_index = idx.get() / MI_PAGE_MAP_ENTRIES_PER_CBIT;
        crate::internal::mi_assert_internal!(bit_index < MI_BFIELD_BITS);
        Self {
            bit_index,
            committed: (commit & (1usize << bit_index)) != 0,
        }
    }

    #[inline(always)]
    fn is_committed(self) -> bool {
        self.committed
    }

    #[inline(always)]
    fn mask(self) -> usize {
        1usize << self.bit_index
    }

    #[inline(always)]
    fn byte_offset(self) -> usize {
        self.bit_index * MI_PAGE_MAP_ENTRIES_PER_CBIT * core::mem::size_of::<mi_submap_t>()
    }
}

/// Non-owning snapshot of a committed top-level slot. This does not extend the
/// page-map allocation lifetime or protect against lifecycle teardown.
#[cfg(not(target_pointer_width = "32"))]
enum CommittedSubmapSlot {
    Empty,
    Published(PublishedSubmap),
}

#[cfg(not(target_pointer_width = "32"))]
impl CommittedSubmapSlot {
    /// # Safety
    /// If `submap` is non-null, it must be a published full-size submap that
    /// remains live while this snapshot is used. Concurrent teardown must be
    /// excluded. Null is accepted as an empty committed slot.
    #[inline(always)]
    unsafe fn from_raw(submap: mi_submap_t) -> Self {
        // SAFETY: the caller supplies the same conditional allocation and
        // lifecycle proof required by `PublishedSubmap::from_raw`.
        match unsafe { PublishedSubmap::from_raw(submap) } {
            Some(submap) => Self::Published(submap),
            None => Self::Empty,
        }
    }
}

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
#[cfg(not(target_pointer_width = "32"))]
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

#[cfg(not(target_pointer_width = "32"))]
#[inline]
fn mi_page_map_index(raw: usize) -> Option<crate::core_types::PageMapIndex> {
    crate::core_types::PageMapIndex::new(raw)
}

#[cfg(not(target_pointer_width = "32"))]
#[inline]
fn mi_page_map_sub_index(raw: usize) -> Option<crate::core_types::PageMapSubIndex> {
    crate::core_types::PageMapSubIndex::new(raw)
}

#[cfg(target_pointer_width = "32")]
type PageMapGetIndex = (PageMapPrimaryIndex, SliceCount);

#[cfg(not(target_pointer_width = "32"))]
type PageMapGetIndex = (
    crate::core_types::PageMapIndex,
    crate::core_types::PageMapSubIndex,
    SliceCount,
);

struct PageMapPageSpan {
    page_start: crate::types::ptr::PageStart,
    slice_count: SliceCount,
}

/// # Safety
/// `page` must identify live initialized page metadata whose allocation area, slice metadata, and
/// backing remain readable and stable through the returned span's use. The area's start must be at
/// or after its slice start in the same allocation, and the capped page size plus their address
/// difference must be representable as a nonzero `SliceCount`. Concurrent page teardown, metadata
/// relink, or backing release must be excluded.
unsafe fn mi_page_map_page_span(page: PagePtr) -> Option<PageMapPageSpan> {
    // SAFETY: `PagePtr` proves live initialized page metadata for the
    // registered block-area range.
    let area = unsafe { page.area()? };
    let page_start = area.start();
    let mut page_size = area.byte_len();
    let page = page.as_ptr();
    if page_size > MI_LARGE_PAGE_SIZE {
        page_size = MI_LARGE_PAGE_SIZE - MI_ARENA_SLICE_SIZE;
    }
    let slice_start = unsafe { mi_page_slice_start(page) };
    let slice_count = mi_slice_count_of_size(page_size)
        + (crate::core_types::ptr_addr_diff(page_start.as_ptr(), slice_start)
            / MI_ARENA_SLICE_SIZE);
    let slice_count = SliceCount::from_usize(slice_count)?;
    Some(PageMapPageSpan {
        page_start,
        slice_count,
    })
}

#[cfg(target_pointer_width = "32")]
fn mi_page_map_get_idx_from_span(page: PagePtr, span: PageMapPageSpan) -> Option<PageMapGetIndex> {
    let location = PageMapLocation::from_ptr(page.as_ptr().cast::<c_void>());
    Some((location.index(), span.slice_count))
}

#[cfg(not(target_pointer_width = "32"))]
fn mi_page_map_get_idx_from_span(_page: PagePtr, span: PageMapPageSpan) -> Option<PageMapGetIndex> {
    let location = PageMapLocation::from_ptr(span.page_start.as_ptr().cast::<c_void>())?;
    Some((location.index(), location.sub_index(), span.slice_count))
}

/// # Safety
/// `page` must identify live initialized page metadata whose allocation area, slice start, size, and
/// backing remain readable and stable through coordinate derivation. Its area and slice start must
/// belong to the same allocation with the area start no earlier than the slice start. Concurrent
/// page-map teardown, page metadata relink, or backing release must be excluded.
unsafe fn mi_page_map_get_idx(page: PagePtr) -> Option<PageMapGetIndex> {
    let span = unsafe { mi_page_map_page_span(page) }?;
    mi_page_map_get_idx_from_span(page, span)
}

#[cfg(not(target_pointer_width = "32"))]
#[inline]
fn mi_page_map_commit_status(idx: crate::core_types::PageMapIndex) -> PageMapCommitStatus {
    let commit = mi_page_map_commit.load(Ordering::Relaxed);
    PageMapCommitStatus::from_index_and_word(idx, commit)
}

#[cfg(target_pointer_width = "32")]
const MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT: usize = MI_ARENA_SLICE_SIZE;

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_ensure_committed(
    idx: PageMapPrimaryIndex,
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
                let commit = unsafe { _mi_os_commit(start.cast::<c_void>(), size) };
                if !commit.success() {
                    mi_page_map_cannot_commit();
                    return false;
                }
                if !commit.initially_zero().0 && !unsafe { mi_page_map_memid.initially_zero } {
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
            if !_mi_os_commit(mi_page_map_commit.cast::<c_void>(), bitmap_size).success() {
                mi_page_map_cannot_commit();
                return false;
            }
            mi_bitmap_init(mi_page_map_commit, commit_bits, true);
        }
    }
    unsafe {
        _mi_page_map = base.add(bitmap_size);
    }
    let zero_location = PageMapLocation::from_ptr(ptr::null());
    let zero_index = zero_location.index();
    let one_slice = unsafe { crate::core_types::SliceCount::new_unchecked(1) };
    if !commit && !unsafe { mi_page_map_ensure_committed(zero_index, one_slice) } {
        mi_page_map_cannot_commit();
        return false;
    }
    unsafe {
        *_mi_page_map.add(0) = 1;
    }
    crate::internal::mi_assert_internal!(
        unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null()
    );
    true
}

/// # Safety
///
/// `subproc` must point to the live sub-process owning the page-map memory.
/// No concurrent page-map users may remain.
#[cfg(target_pointer_width = "32")]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::internal::mi_assert_internal!(!subproc.is_null());
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
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
    crate::internal::mi_assert_internal!(!page.is_null());
    let Some(page) = PagePtr::new(page) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    unsafe { mi_page_map_register_typed(page) }
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool {
    let page_raw = page.as_ptr();
    crate::internal::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    crate::internal::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    let Some((idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
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
    idx: PageMapPrimaryIndex,
    slice_count: crate::core_types::SliceCount,
) -> Option<RegisteredPageRange> {
    if !unsafe { mi_page_map_ensure_committed(idx, slice_count) } {
        return None;
    }
    for i in 0..slice_count.get() as usize {
        crate::internal::mi_assert_internal!(i < 128);
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
        crate::internal::mi_assert_internal!(false);
        return;
    };
    unsafe { mi_page_map_unregister_typed(page) };
}

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_unregister_typed(page: PagePtr) {
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    let Some((idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
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
/// The process-global page map must be initialized and remain live. `idx` must
/// be below the active runtime page-map count. Concurrent unsafe destruction
/// must be excluded while the returned slot snapshot is used.
#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_ensure_committed(
    idx: crate::core_types::PageMapIndex,
) -> Option<CommittedSubmapSlot> {
    let commit = mi_page_map_commit_status(idx);
    if !commit.is_committed() {
        let start = unsafe {
            (&raw mut _mi_page_map)
                .read()
                .cast::<u8>()
                .add(commit.byte_offset())
        };
        if !unsafe {
            _mi_os_commit(
                start.cast::<c_void>(),
                MI_PAGE_MAP_ENTRIES_PER_CBIT * core::mem::size_of::<mi_submap_t>(),
            )
            .success()
        } {
            mi_page_map_cannot_commit();
            return None;
        }
        mi_page_map_commit.fetch_or(commit.mask(), Ordering::AcqRel);
    }
    // SAFETY: the committed slot load through the audited accessor is null or
    // a published full-size submap, and the caller excludes concurrent
    // lifecycle teardown.
    Some(unsafe { CommittedSubmapSlot::from_raw(_mi_page_map_at(idx)) })
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
    crate::internal::mi_assert!(unsafe { mi_page_map_count } <= MI_PAGE_MAP_COUNT);

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
        && !unsafe { _mi_os_commit(sub0.cast::<c_void>(), submap_size) }.success()
    {
        mi_page_map_cannot_commit();
        return false;
    }
    if !unsafe { mi_page_map_memid.initially_zero } {
        unsafe { _mi_memzero_aligned(sub0.cast::<c_void>(), submap_size) };
    }

    let zero_index = unsafe { crate::core_types::PageMapIndex::new_unchecked(0) };
    if unsafe { mi_page_map_ensure_committed(zero_index) }.is_none() {
        mi_page_map_cannot_commit();
        return false;
    }
    unsafe {
        *_mi_page_map.add(0) = sub0;
        mi_lock_init(&raw mut mi_page_map_lock);
    }
    crate::internal::mi_assert_internal!(
        unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null()
    );
    true
}

/// # Safety
///
/// `subproc` must point to the live sub-process owning the page-map memory.
/// No concurrent page-map users may remain.
#[cfg(not(target_pointer_width = "32"))]
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::internal::mi_assert_internal!(!subproc.is_null());
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }

    unsafe { mi_lock_done(&raw mut mi_page_map_lock) };
    for idx in 1..unsafe { mi_page_map_count } {
        let Some(map_idx) = mi_page_map_index(idx) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        if mi_page_map_commit_status(map_idx).is_committed() {
            let sub = unsafe { _mi_page_map_at(map_idx) };
            if !sub.is_null() {
                let memid = unsafe {
                    _mi_memid_create_os(
                        sub.cast::<c_void>(),
                        crate::core_types::ReservedBytes::new(MI_PAGE_MAP_SUB_SIZE),
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
/// # Safety
///
/// The process-global page map and growth lock must be initialized and remain
/// live. Concurrent unsafe page-map destruction must be excluded, and the
/// returned witness must not outlive its backing page-map allocation.
unsafe fn mi_page_map_ensure_submap_at(
    idx: crate::core_types::PageMapIndex,
) -> Option<PublishedSubmap> {
    let committed = unsafe { mi_page_map_ensure_committed(idx) }?;
    if let CommittedSubmapSlot::Published(submap) = committed {
        return Some(submap);
    }
    let sub;
    {
        let lock = unsafe { &*ptr::addr_of!(mi_page_map_lock) };
        // SAFETY: the process-global page-map lock remains live throughout
        // allocator operation and the guard releases it on every exit path.
        let _guard = MiLockGuard::acquire(lock);
        let atomic_submap =
            crate::core_types::AtomicPtrField::from_raw(unsafe { _mi_page_map.add(idx.get()) });
        let mut loaded = unsafe { atomic_submap.load(crate::core_types::MemoryOrder::Acquire) };
        if loaded.is_null() {
            let mut memid = _mi_memid_none();
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
                    loaded = expect;
                } else {
                    loaded = new_sub;
                }
            }
        }
        if loaded.is_null() {
            return None;
        }
        sub = loaded;
    }
    // SAFETY: the non-null submap was loaded from or published into the
    // process-global page map and remains live until lifecycle teardown.
    unsafe { PublishedSubmap::from_raw(sub) }
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
        // SAFETY: typed top-level index bounds submap commit/allocation before
        // page publication, with lifecycle teardown excluded by the caller.
        let Some(sub) = (unsafe { mi_page_map_ensure_submap_at(idx) }) else {
            return false;
        };
        while slice_count > 0 && map_sub_idx.get() < MI_PAGE_MAP_SUB_COUNT {
            unsafe { *sub.slot_ptr(map_sub_idx).as_ptr() = page };
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
        // SAFETY: typed top-level index bounds submap commit/allocation before
        // clearing entries, with lifecycle teardown excluded by the caller.
        let Some(sub) = (unsafe { mi_page_map_ensure_submap_at(idx) }) else {
            return false;
        };
        while slice_count > 0 && map_sub_idx.get() < MI_PAGE_MAP_SUB_COUNT {
            unsafe { *sub.slot_ptr(map_sub_idx).as_ptr() = ptr::null_mut() };
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
    crate::internal::mi_assert_internal!(!page.is_null());
    let Some(page) = PagePtr::new(page) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    unsafe { mi_page_map_register_typed(page) }
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool {
    let page_raw = page.as_ptr();
    crate::internal::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    crate::internal::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    let Some((idx, sub_idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
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
        crate::internal::mi_assert_internal!(false);
        return;
    };
    unsafe { mi_page_map_unregister_typed(page) };
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_unregister_typed(page: PagePtr) {
    crate::internal::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    let page_raw = page.as_ptr();
    crate::internal::mi_assert_internal!(unsafe {
        _mi_is_aligned(
            mi_page_slice_start(page_raw).cast::<c_void>(),
            MI_PAGE_ALIGN,
        )
    });
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let Some((idx, sub_idx, slice_count)) = (unsafe { mi_page_map_get_idx(page) }) else {
        crate::internal::mi_assert_internal!(false);
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

#[cfg(all(test, not(target_pointer_width = "32")))]
mod tests {
    use super::*;
    use crate::types::PageMapIndex;
    use core::mem::size_of;

    #[test]
    fn commit_status_maps_chunk_edges_and_commit_words() {
        let status = PageMapCommitStatus::from_index_and_word(
            PageMapIndex::new(0).expect("zero is a valid top-level index"),
            0,
        );
        assert!(!status.is_committed());
        assert_eq!(status.mask(), 1);
        assert_eq!(status.byte_offset(), 0);

        let last_in_first_chunk = (MI_PAGE_MAP_ENTRIES_PER_CBIT - 1).min(MI_PAGE_MAP_COUNT - 1);
        let status = PageMapCommitStatus::from_index_and_word(
            PageMapIndex::new(last_in_first_chunk)
                .expect("last index in the first commit chunk is valid"),
            1,
        );
        assert!(status.is_committed());
        assert_eq!(status.mask(), 1);

        if MI_PAGE_MAP_COUNT > MI_PAGE_MAP_ENTRIES_PER_CBIT {
            let rollover = PageMapCommitStatus::from_index_and_word(
                PageMapIndex::new(MI_PAGE_MAP_ENTRIES_PER_CBIT)
                    .expect("first index in the second commit chunk is valid"),
                1,
            );
            assert!(!rollover.is_committed());
            assert_eq!(rollover.mask(), 2);
            assert_eq!(
                rollover.byte_offset(),
                MI_PAGE_MAP_ENTRIES_PER_CBIT * size_of::<mi_submap_t>()
            );
            let committed = PageMapCommitStatus::from_index_and_word(
                PageMapIndex::new(MI_PAGE_MAP_ENTRIES_PER_CBIT)
                    .expect("first index in the second commit chunk is valid"),
                2,
            );
            assert!(committed.is_committed());
        }

        let last_index = MI_PAGE_MAP_COUNT - 1;
        let final_clear = PageMapCommitStatus::from_index_and_word(
            PageMapIndex::new(last_index).expect("final top-level index is valid"),
            0,
        );
        assert!(!final_clear.is_committed());
        let final_status = PageMapCommitStatus::from_index_and_word(
            PageMapIndex::new(last_index).expect("final top-level index is valid"),
            usize::MAX,
        );
        assert!(final_status.is_committed());
        assert_eq!(
            final_status.mask(),
            1usize << (last_index / MI_PAGE_MAP_ENTRIES_PER_CBIT)
        );
    }

    #[test]
    fn committed_submap_slot_classifies_empty_slot() {
        // SAFETY: null is explicitly accepted as an empty committed slot.
        assert!(matches!(
            unsafe { CommittedSubmapSlot::from_raw(ptr::null_mut()) },
            CommittedSubmapSlot::Empty
        ));
    }

    #[test]
    fn published_submap_rejects_null() {
        // SAFETY: null is explicitly accepted for rejection without being
        // dereferenced or wrapped as a published submap.
        assert!(unsafe { PublishedSubmap::from_raw(ptr::null_mut()) }.is_none());
    }

    #[test]
    fn published_submap_preserves_pointer_layout_and_option_niche() {
        assert_eq!(size_of::<PublishedSubmap>(), size_of::<mi_submap_t>());
        assert_eq!(
            size_of::<Option<PublishedSubmap>>(),
            size_of::<mi_submap_t>()
        );
    }

    #[test]
    fn published_submap_addresses_first_and_last_typed_slots() {
        let mut slots = std::vec![ptr::null_mut(); MI_PAGE_MAP_SUB_COUNT].into_boxed_slice();
        let base = slots.as_mut_ptr();
        // SAFETY: the boxed slice provides every submap slot and remains live
        // until after all derived slot pointers are checked.
        let committed = unsafe { CommittedSubmapSlot::from_raw(base) };
        let CommittedSubmapSlot::Published(submap) = committed else {
            panic!("boxed full-size submap must classify as published");
        };
        let first = PageMapSubIndex::new(0).expect("zero is a valid submap index");
        let last = PageMapSubIndex::new(MI_PAGE_MAP_SUB_COUNT - 1)
            .expect("last in-range submap index must be valid");

        assert_eq!(submap.slot_ptr(first).as_ptr(), base);
        // SAFETY: the boxed slice contains exactly `MI_PAGE_MAP_SUB_COUNT`
        // elements, so its last element is in bounds.
        assert_eq!(submap.slot_ptr(last).as_ptr(), unsafe {
            base.add(MI_PAGE_MAP_SUB_COUNT - 1)
        });
    }
}
