//! Mechanical translation of the early `src/theap.c` helper slice.

#![allow(
    dead_code,
    non_camel_case_types,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;

use crate::internal::mi_page_all_free;
use crate::page::{
    _mi_page_abandon, _mi_page_free, _mi_page_free_collect, _mi_page_is_valid, mi_page_theap,
};
use crate::types::{
    MI_BIN_FULL, MI_SMALL_PAGE_SIZE, mi_heap_area_t, mi_page_queue_t, mi_page_t, mi_theap_t,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum mi_collect_t {
    MI_NORMAL = 0,
    MI_FORCE = 1,
    MI_ABANDON = 2,
}

pub const MI_MAX_BLOCKS: usize = MI_SMALL_PAGE_SIZE / core::mem::size_of::<*const c_void>();

pub type theap_page_visitor_fun = unsafe extern "C" fn(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct mi_theap_area_ex_s {
    pub area: mi_heap_area_t,
    pub page: *mut mi_page_t,
}

pub type mi_theap_area_ex_t = mi_theap_area_ex_s;

#[inline]
pub unsafe fn mi_fast_divide(n: usize, magic: u64, shift: usize) -> usize {
    crate::mi_assert_internal!(u32::try_from(n).is_ok());
    let hi = ((n as u64) * magic) >> 32;
    ((hi + n as u64) >> shift) as usize
}

pub unsafe fn mi_theap_visit_pages(
    theap: *mut mi_theap_t,
    fn_page: Option<theap_page_visitor_fun>,
    include_full: bool,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    if theap.is_null() || (*theap).page_count == 0 {
        return false;
    }

    let mut count = 0usize;
    let total = (*theap).page_count;
    let max_bin = if include_full {
        MI_BIN_FULL
    } else {
        MI_BIN_FULL - 1
    };

    for i in 0..=max_bin {
        let pq = core::ptr::addr_of_mut!((*theap).pages[i]);
        let mut page = (*pq).first;
        while !page.is_null() {
            let next = (*page).next;
            crate::mi_assert_internal!(mi_page_theap(page) == theap);
            if crate::internal::MI_DEBUG > 1 {
                count += 1;
            }
            if let Some(visitor) = fn_page
                && !visitor(theap, pq, page, arg1, arg2)
            {
                return false;
            }
            page = next;
        }
    }

    crate::mi_assert_internal!(!include_full || crate::internal::MI_DEBUG <= 1 || count == total);
    true
}

pub unsafe fn mi_theap_page_is_valid(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    crate::MI_UNUSED!(arg1);
    crate::MI_UNUSED!(arg2);
    crate::MI_UNUSED!(pq);
    crate::mi_assert_internal!(mi_page_theap(page) == theap);
    crate::mi_assert_expensive!(_mi_page_is_valid(page));
    true
}

pub unsafe fn mi_theap_page_collect(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg_collect: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    crate::MI_UNUSED!(arg2);
    crate::MI_UNUSED!(theap);
    crate::mi_assert_internal!(mi_theap_page_is_valid(
        theap,
        pq,
        page,
        core::ptr::null_mut(),
        core::ptr::null_mut()
    ));

    let collect = *(arg_collect as *const mi_collect_t);
    _mi_page_free_collect(page, collect as i32 >= mi_collect_t::MI_FORCE as i32);

    if mi_page_all_free(page) {
        if collect as i32 >= mi_collect_t::MI_FORCE as i32 || (*page).retire_expire == 0 {
            _mi_page_free(page, pq);
        }
    } else if collect == mi_collect_t::MI_ABANDON {
        _mi_page_abandon(page, pq);
    }

    true
}
