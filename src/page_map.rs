//! Page map management.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc
)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::atomic::{mi_lock_done, mi_lock_init, mi_lock_t};
use crate::bitmap::MI_BFIELD_BITS;
use crate::bits::{MI_KiB, MI_PAGE_MAP_FLAT, MI_SIZE_BITS};
use crate::internal::{
    _mi_align_up, _mi_error_message, _mi_memid_create_os, _mi_memid_none, _mi_memzero_aligned,
    _mi_os_commit, _mi_os_free_ex, _mi_os_has_overcommit, _mi_os_virtual_address_bits,
    _mi_page_map_at, MI_ARENA_SLICE_SHIFT, MI_PAGE_MAP_COUNT, MI_PAGE_MAP_SUB_COUNT,
    MI_PAGE_MAP_SUB_SHIFT, mi_memid_mem_t, mi_memid_os_info_t, mi_memid_t, mi_memkind_t,
    mi_submap_t,
};
use crate::options::{_mi_warning_message, mi_option_get_clamp, mi_option_is_enabled, mi_option_t};
use crate::os::_mi_os_alloc_aligned;
use crate::types::{ENOMEM, mi_page_t, mi_subproc_t};

pub const MI_PAGE_MAP_SUB_SIZE: usize =
    MI_PAGE_MAP_SUB_COUNT * core::mem::size_of::<*mut mi_page_t>();
const MI_PAGE_MAP_ENTRIES_PER_CBIT: usize = if MI_PAGE_MAP_COUNT < MI_BFIELD_BITS {
    1
} else {
    MI_PAGE_MAP_COUNT / MI_BFIELD_BITS
};

#[cfg(not(target_pointer_width = "32"))]
#[unsafe(no_mangle)]
pub static mut _mi_page_map: *mut mi_submap_t = ptr::null_mut();

#[cfg(target_pointer_width = "32")]
#[unsafe(no_mangle)]
pub static mut _mi_page_map: *mut u8 = ptr::null_mut();

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
static mi_page_map_commit: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn mi_page_map_cannot_commit() {
    _mi_warning_message(format_args!(
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_page_map_init() -> bool {
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_page_map_unsafe_destroy(subproc: *mut mi_subproc_t) {
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
