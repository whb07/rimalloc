//! Page map management.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc
)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::atomic::{
    mi_atomic_cas_strong_acq_rel, mi_atomic_load_ptr_acquire, mi_lock_acquire, mi_lock_done,
    mi_lock_init, mi_lock_release, mi_lock_t,
};
use crate::bitmap::MI_BFIELD_BITS;
#[cfg(target_pointer_width = "32")]
use crate::bitmap::{
    mi_bitmap_init, mi_bitmap_is_clear, mi_bitmap_is_set, mi_bitmap_set, mi_bitmap_size,
    mi_bitmap_t,
};
#[cfg(target_pointer_width = "32")]
use crate::bits::MI_MiB;
use crate::bits::{MI_KiB, MI_PAGE_MAP_FLAT, MI_SIZE_BITS};
#[cfg(target_pointer_width = "32")]
use crate::internal::_mi_memzero;
use crate::internal::{
    _mi_align_up, _mi_divide_up, _mi_error_message, _mi_is_aligned, _mi_memid_create_os,
    _mi_memid_none, _mi_memzero_aligned, _mi_os_commit, _mi_os_free, _mi_os_free_ex,
    _mi_os_has_overcommit, _mi_os_virtual_address_bits, _mi_page_map_at, _mi_page_map_index,
    MI_ARENA_SLICE_SHIFT, MI_PAGE_MAP_COUNT, MI_PAGE_MAP_SUB_COUNT, MI_PAGE_MAP_SUB_SHIFT,
    mi_memid_mem_t, mi_memid_os_info_t, mi_memid_t, mi_memkind_t, mi_page_area,
    mi_page_slice_start, mi_slice_count_of_size, mi_submap_t,
};
use crate::options::{
    _mi_warning_message_fmt, mi_option_get_clamp, mi_option_is_enabled, mi_option_t,
};
use crate::os::{_mi_os_alloc_aligned, _mi_os_zalloc};
use crate::page::MI_LARGE_PAGE_SIZE;
use crate::types::{ENOMEM, MI_PAGE_ALIGN, mi_page_t, mi_subproc_t};

pub const MI_PAGE_MAP_SUB_SIZE: usize =
    MI_PAGE_MAP_SUB_COUNT * core::mem::size_of::<*mut mi_page_t>();
const MI_PAGE_MAP_ENTRIES_PER_CBIT: usize = if MI_PAGE_MAP_COUNT < MI_BFIELD_BITS {
    1
} else {
    MI_PAGE_MAP_COUNT / MI_BFIELD_BITS
};

#[cfg(not(target_pointer_width = "32"))]
static mut _mi_page_map: *mut mi_submap_t = ptr::null_mut();

#[cfg(target_pointer_width = "32")]
static mut _mi_page_map: *mut u8 = ptr::null_mut();

#[cfg(not(target_pointer_width = "32"))]
#[inline(always)]
pub(crate) unsafe fn _mi_page_map_base() -> *mut mi_submap_t {
    unsafe { _mi_page_map }
}

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
fn mi_page_map_cannot_commit() {
    _mi_warning_message_fmt(format_args!(
        "unable to commit the allocation page-map on-demand\n"
    ));
}

#[inline]
fn mi_page_map_is_committed(idx: usize, pbit_idx: *mut usize) -> bool {
    let commit = mi_page_map_commit.load(Ordering::Relaxed);
    let bit_idx = idx / MI_PAGE_MAP_ENTRIES_PER_CBIT;
    crate::mi_assert_internal!(bit_idx < MI_BFIELD_BITS);
    if !pbit_idx.is_null() {
        unsafe { *pbit_idx = bit_idx };
    }
    (commit & (1usize << bit_idx)) != 0
}

#[cfg(target_pointer_width = "32")]
const MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT: usize = MI_ARENA_SLICE_SIZE;

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_ensure_committed(idx: usize, slice_count: usize) -> bool {
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

#[cfg(target_pointer_width = "32")]
unsafe fn mi_page_map_get_idx(
    page: *mut mi_page_t,
    page_start: *mut *mut u8,
    slice_count: *mut usize,
) -> usize {
    let mut page_size = 0usize;
    unsafe { *page_start = mi_page_area(page, &mut page_size) };
    if page_size > MI_LARGE_PAGE_SIZE {
        page_size = MI_LARGE_PAGE_SIZE - MI_ARENA_SLICE_SIZE;
    }
    unsafe {
        *slice_count = mi_slice_count_of_size(page_size)
            + (((*page_start) as usize - mi_page_slice_start(page) as usize) / MI_ARENA_SLICE_SIZE);
        _mi_page_map_index(page.cast::<c_void>())
    }
}

#[cfg(target_pointer_width = "32")]
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
        unsafe {
            _mi_error_message(
                ENOMEM,
                c"unable to reserve virtual memory for the page map (%zu KiB)\n".as_ptr(),
                page_map_size / MI_KiB,
            );
        }
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
    if !commit && !unsafe { mi_page_map_ensure_committed(0, 1) } {
        mi_page_map_cannot_commit();
        return false;
    }
    unsafe {
        *_mi_page_map.add(0) = 1;
    }
    crate::mi_assert_internal!(unsafe { crate::internal::_mi_ptr_page(ptr::null()) }.is_null());
    true
}

#[cfg(target_pointer_width = "32")]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::mi_assert_internal!(!subproc.is_null());
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    unsafe {
        _mi_os_free_ex(
            mi_page_map_memid.mem.os.base,
            mi_page_map_memid.mem.os.size,
            true,
            mi_page_map_memid,
            subproc,
        );
        _mi_page_map = ptr::null_mut();
        mi_page_map_commit = ptr::null_mut();
        mi_page_map_max_address = ptr::null_mut();
        mi_page_map_memid = _mi_memid_none();
    }
}

#[cfg(target_pointer_width = "32")]
pub(crate) unsafe extern "C" fn _mi_page_map_register(page: *mut mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_internal!(unsafe {
        _mi_is_aligned(mi_page_slice_start(page).cast::<c_void>(), MI_PAGE_ALIGN)
    });
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    crate::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    let mut page_start = ptr::null_mut();
    let mut slice_count = 0usize;
    let idx = unsafe { mi_page_map_get_idx(page, &mut page_start, &mut slice_count) };
    if !unsafe { mi_page_map_ensure_committed(idx, slice_count) } {
        return false;
    }
    for i in 0..slice_count {
        crate::mi_assert_internal!(i < 128);
        unsafe { *_mi_page_map.add(idx + i) = (i + 1) as u8 };
    }
    true
}

#[cfg(target_pointer_width = "32")]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister(page: *mut mi_page_t) {
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    let mut page_start = ptr::null_mut();
    let mut slice_count = 0usize;
    let idx = unsafe { mi_page_map_get_idx(page, &mut page_start, &mut slice_count) };
    unsafe { _mi_memzero(_mi_page_map.add(idx).cast::<c_void>(), slice_count) };
}

#[cfg(target_pointer_width = "32")]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister_range(start: *mut c_void, size: usize) {
    let slice_count = _mi_divide_up(size, MI_ARENA_SLICE_SIZE);
    let index = _mi_page_map_index(start.cast_const());
    if !unsafe { mi_page_map_ensure_committed(index, slice_count) } {
        return;
    }
    unsafe { _mi_memzero(_mi_page_map.add(index).cast::<c_void>(), slice_count) };
}

#[cfg(target_pointer_width = "32")]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    if p >= unsafe { mi_page_map_max_address } {
        return ptr::null_mut();
    }
    let idx = _mi_page_map_index(p);
    if unsafe { !mi_page_map_commit.is_null() }
        && !unsafe {
            mi_bitmap_is_set(mi_page_map_commit, idx / MI_PAGE_MAP_ENTRIES_PER_COMMIT_BIT)
        }
    {
        return ptr::null_mut();
    }
    let ofs = unsafe { *_mi_page_map.add(idx) as usize };
    if ofs == 0 {
        return ptr::null_mut();
    }
    ((((p as usize) >> MI_ARENA_SLICE_SHIFT) - ofs + 1) << MI_ARENA_SLICE_SHIFT) as *mut mi_page_t
}

#[cfg(target_pointer_width = "32")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_ensure_committed(idx: usize, submap: *mut mi_submap_t) -> bool {
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
        *submap = _mi_page_map_at(idx);
    }
    true
}

#[cfg(not(target_pointer_width = "32"))]
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

    let os_page_size = unsafe { crate::os::_mi_os_page_size() };
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
        unsafe {
            _mi_error_message(
                ENOMEM,
                c"unable to reserve virtual memory for the page map (%zu KiB)\n".as_ptr(),
                page_map_size / MI_KiB,
            );
        }
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
    if !unsafe { mi_page_map_ensure_committed(0, &mut nullsub) } {
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

#[cfg(not(target_pointer_width = "32"))]
pub(crate) unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
    crate::mi_assert_internal!(!subproc.is_null());
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    if unsafe { _mi_page_map }.is_null() {
        return;
    }

    unsafe { mi_lock_done(&raw mut mi_page_map_lock) };
    for idx in 1..unsafe { mi_page_map_count } {
        if mi_page_map_is_committed(idx, ptr::null_mut()) {
            let sub = unsafe { _mi_page_map_at(idx) };
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
                    _mi_os_free_ex(memid.mem.os.base, memid.mem.os.size, true, memid, subproc);
                    *_mi_page_map.add(idx) = ptr::null_mut();
                }
            }
        }
    }
    unsafe {
        _mi_os_free_ex(
            _mi_page_map.cast::<c_void>(),
            mi_page_map_memid.mem.os.size,
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
unsafe fn mi_page_map_ensure_submap_at(idx: usize, submap: *mut mi_submap_t) -> bool {
    crate::mi_assert_internal!(!submap.is_null() && unsafe { (*submap).is_null() });
    let mut sub = ptr::null_mut();
    if !unsafe { mi_page_map_ensure_committed(idx, &mut sub) } {
        return false;
    }
    if sub.is_null() {
        unsafe { mi_lock_acquire(&raw mut mi_page_map_lock) };
        sub = unsafe { mi_atomic_load_ptr_acquire(_mi_page_map.add(idx)) };
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
                if !unsafe {
                    mi_atomic_cas_strong_acq_rel(
                        _mi_page_map.add(idx).cast::<usize>(),
                        (&raw mut expect).cast::<usize>(),
                        new_sub as usize,
                    )
                } {
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
    page: *mut mi_page_t,
    mut idx: usize,
    mut sub_idx: usize,
    mut slice_count: usize,
) -> bool {
    while slice_count > 0 {
        let mut sub: mi_submap_t = ptr::null_mut();
        if !unsafe { mi_page_map_ensure_submap_at(idx, &mut sub) } {
            return false;
        }
        crate::mi_assert_internal!(!sub.is_null());
        while slice_count > 0 && sub_idx < MI_PAGE_MAP_SUB_COUNT {
            unsafe { *sub.add(sub_idx) = page };
            slice_count -= 1;
            sub_idx += 1;
        }
        idx += 1;
        sub_idx = 0;
    }
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_set_range(
    page: *mut mi_page_t,
    idx: usize,
    sub_idx: usize,
    slice_count: usize,
) -> bool {
    if !unsafe { mi_page_map_set_range_prim(page, idx, sub_idx, slice_count) } {
        if !page.is_null() {
            unsafe { mi_page_map_set_range_prim(ptr::null_mut(), idx, sub_idx, slice_count) };
        }
        return false;
    }
    true
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn mi_page_map_get_idx(
    page: *mut mi_page_t,
    sub_idx: *mut usize,
    slice_count: *mut usize,
) -> usize {
    let mut page_size = 0usize;
    let page_start = unsafe { mi_page_area(page, &mut page_size) };
    if page_size > MI_LARGE_PAGE_SIZE {
        page_size = MI_LARGE_PAGE_SIZE - MI_ARENA_SLICE_SIZE;
    }
    unsafe {
        *slice_count = mi_slice_count_of_size(page_size)
            + ((page_start as usize - mi_page_slice_start(page) as usize) / MI_ARENA_SLICE_SIZE);
    }
    unsafe { _mi_page_map_index(page_start.cast::<c_void>(), sub_idx) }
}

#[cfg(not(target_pointer_width = "32"))]
pub(crate) unsafe extern "C" fn _mi_page_map_register(page: *mut mi_page_t) -> bool {
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_internal!(unsafe {
        _mi_is_aligned(mi_page_slice_start(page).cast::<c_void>(), MI_PAGE_ALIGN)
    });
    if unsafe { _mi_page_map }.is_null() && !unsafe { _mi_page_map_init() } {
        return false;
    }
    crate::mi_assert!(!(unsafe { _mi_page_map }).is_null());
    let mut slice_count = 0usize;
    let mut sub_idx = 0usize;
    let idx = unsafe { mi_page_map_get_idx(page, &mut sub_idx, &mut slice_count) };
    unsafe { mi_page_map_set_range(page, idx, sub_idx, slice_count) }
}

#[cfg(not(target_pointer_width = "32"))]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister(page: *mut mi_page_t) {
    crate::mi_assert_internal!(!(unsafe { _mi_page_map }).is_null());
    crate::mi_assert_internal!(!page.is_null());
    crate::mi_assert_internal!(unsafe {
        _mi_is_aligned(mi_page_slice_start(page).cast::<c_void>(), MI_PAGE_ALIGN)
    });
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let mut slice_count = 0usize;
    let mut sub_idx = 0usize;
    let idx = unsafe { mi_page_map_get_idx(page, &mut sub_idx, &mut slice_count) };
    unsafe { mi_page_map_set_range(ptr::null_mut(), idx, sub_idx, slice_count) };
}

#[cfg(not(target_pointer_width = "32"))]
pub(crate) unsafe extern "C" fn _mi_page_map_unregister_range(start: *mut c_void, size: usize) {
    if unsafe { _mi_page_map }.is_null() {
        return;
    }
    let slice_count = _mi_divide_up(size, MI_ARENA_SLICE_SIZE);
    let mut sub_idx = 0usize;
    let idx = unsafe { _mi_page_map_index(start.cast_const(), &mut sub_idx) };
    unsafe { mi_page_map_set_range(ptr::null_mut(), idx, sub_idx, slice_count) };
}

#[cfg(not(target_pointer_width = "32"))]
pub(crate) unsafe extern "C" fn _mi_safe_ptr_page(p: *const c_void) -> *mut mi_page_t {
    if p.is_null() {
        return ptr::null_mut();
    }
    if p >= unsafe { mi_page_map_max_address } {
        return ptr::null_mut();
    }
    let mut sub_idx = 0usize;
    let idx = unsafe { _mi_page_map_index(p, &mut sub_idx) };
    if !mi_page_map_is_committed(idx, ptr::null_mut()) {
        return ptr::null_mut();
    }
    let sub = unsafe { _mi_page_map_at(idx) };
    if sub.is_null() {
        return ptr::null_mut();
    }
    unsafe { *sub.add(sub_idx) }
}

#[cfg(not(target_pointer_width = "32"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_is_in_heap_region(p: *const c_void) -> bool {
    !unsafe { _mi_safe_ptr_page(p) }.is_null()
}
