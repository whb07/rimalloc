//! Mechanical translation of the early `src/theap.c` helper slice.

#![allow(
    dead_code,
    non_camel_case_types,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_void;

use crate::arena::_mi_arenas_collect;
use crate::internal::{
    _mi_deferred_free, _mi_theap_heap, mi_page_all_free, mi_theap_is_initialized,
};
use crate::page::{
    _mi_page_abandon, _mi_page_free, _mi_page_free_collect, _mi_page_is_valid,
    _mi_page_queue_is_valid, _mi_theap_collect_retired, mi_page_theap,
};
use crate::prim::_mi_theap_default;
use crate::stats::_mi_stats_merge_into;
use crate::types::{
    MI_BIN_COUNT, MI_BIN_FULL, MI_SMALL_PAGE_SIZE, mi_heap_area_t, mi_heap_t, mi_page_queue_t,
    mi_page_t, mi_theap_t,
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

pub unsafe extern "C" fn mi_theap_page_is_valid(
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

pub unsafe extern "C" fn mi_theap_page_collect(
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

pub unsafe fn mi_theap_is_valid(theap: *mut mi_theap_t) -> bool {
    crate::mi_assert_internal!(!theap.is_null());
    mi_theap_visit_pages(
        theap,
        Some(mi_theap_page_is_valid),
        true,
        core::ptr::null_mut(),
        core::ptr::null_mut(),
    );
    for bin in 0..MI_BIN_COUNT {
        crate::mi_assert_internal!(_mi_page_queue_is_valid(
            theap,
            core::ptr::addr_of!((*theap).pages[bin]),
        ));
    }
    true
}

pub unsafe fn mi_theap_merge_stats(theap: *mut mi_theap_t) {
    crate::mi_assert_internal!(mi_theap_is_initialized(theap));
    _mi_stats_merge_into(
        core::ptr::addr_of_mut!((*_mi_theap_heap(theap).cast::<mi_heap_t>()).stats),
        core::ptr::addr_of_mut!((*theap).stats),
    );
}

unsafe fn mi_theap_collect_ex(theap: *mut mi_theap_t, mut collect: mi_collect_t) {
    if theap.is_null() || !mi_theap_is_initialized(theap) {
        return;
    }
    crate::mi_assert_expensive!(mi_theap_is_valid(theap));

    let force = collect as i32 >= mi_collect_t::MI_FORCE as i32;
    _mi_deferred_free(theap, force);
    _mi_theap_collect_retired(theap, force);
    mi_theap_visit_pages(
        theap,
        Some(mi_theap_page_collect),
        collect != mi_collect_t::MI_NORMAL,
        core::ptr::addr_of_mut!(collect).cast(),
        core::ptr::null_mut(),
    );
    _mi_arenas_collect(
        collect == mi_collect_t::MI_FORCE,
        collect as i32 >= mi_collect_t::MI_FORCE as i32,
        (*theap).tld,
    );
    mi_theap_merge_stats(theap);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_theap_collect_abandon(theap: *mut mi_theap_t) {
    mi_theap_collect_ex(theap, mi_collect_t::MI_ABANDON);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_collect(theap: *mut mi_theap_t, force: bool) {
    mi_theap_collect_ex(
        theap,
        if force {
            mi_collect_t::MI_FORCE
        } else {
            mi_collect_t::MI_NORMAL
        },
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect(force: bool) {
    mi_theap_collect(_mi_theap_default(), force);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_collect(heap: *mut mi_heap_t, force: bool) {
    mi_theap_collect(crate::heap::mi_heap_theap(heap), force);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_get_default() -> *mut mi_theap_t {
    let mut theap = _mi_theap_default();
    if !mi_theap_is_initialized(theap) {
        crate::init::mi_thread_init();
        theap = _mi_theap_default();
        crate::mi_assert_internal!(mi_theap_is_initialized(theap));
    }
    theap
}
