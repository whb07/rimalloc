//! Core allocation / free paths + libc overrides

#![allow(dead_code, clippy::missing_safety_doc, unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_int, c_long, c_void};
use core::mem::size_of;
use core::{
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::internal::{
    __mi_stat_counter_increase, __mi_stat_increase, _mi_is_aligned, _mi_memcpy,
    _mi_memzero_aligned, _mi_ptr_page, _mi_strlen, _mi_strnlen, _mi_theap_get_free_small_page,
    _mi_thread_id, MI_LARGE_MAX_OBJ_SIZE, MI_PADDING, MI_PADDING_CHECK, mi_block_next,
    mi_count_size_overflow, mi_page_slice_start, mi_page_start, mi_page_usable_block_size,
    mi_ptr_encode_canary, mi_theap_malloc_use_guarded, mi_unlikely,
};
use crate::mimalloc::{mi_free, mi_theap_malloc, mi_theap_realloc, mi_zalloc};
use crate::page::{_mi_bin, MI_MAX_ALLOC_SIZE, mi_good_size};
use crate::prim::{_mi_heap_theap, _mi_theap_default, MI_THEAP_INITASNULL};
use crate::track::{
    MI_TRACK_ENABLED, mi_track_malloc, mi_track_mem_defined, mi_track_mem_undefined,
};
use crate::types::{
    ENOMEM, MI_DEBUG_UNINIT, MI_MAX_ALIGN_SIZE, MI_PADDING_SIZE, MI_SMALL_SIZE_MAX, MI_STAT,
    mi_block_t, mi_heap_t, mi_padding_t, mi_page_t, mi_theap_t,
};

unsafe extern "C" {
    fn _mi_malloc_generic(
        theap: *mut mi_theap_t,
        size: usize,
        zero_huge_alignment: usize,
        usable: *mut usize,
    ) -> *mut c_void;
    fn _mi_theap_malloc_guarded(theap: *mut mi_theap_t, size: usize, zero: bool) -> *mut c_void;
}

pub const _DEFAULT_SOURCE: () = ();

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe extern "C" {
    fn pathconf(path: *const c_char, name: c_int) -> c_long;
    fn realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    *__errno_location() = err;
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    *__error() = err;
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

unsafe fn mi_theap_strdup(theap: *mut mi_theap_t, s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    let len = _mi_strlen(s);
    if len > MI_MAX_ALLOC_SIZE - 1 {
        return ptr::null_mut();
    }
    let t = mi_theap_malloc(theap, len + 1).cast::<c_char>();
    if t.is_null() {
        return ptr::null_mut();
    }
    _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
    *t.add(len) = 0;
    t
}

unsafe fn mi_theap_strndup(theap: *mut mi_theap_t, s: *const c_char, n: usize) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    let len = _mi_strnlen(s, n);
    if len > MI_MAX_ALLOC_SIZE - 1 {
        return ptr::null_mut();
    }
    let t = mi_theap_malloc(theap, len + 1).cast::<c_char>();
    if t.is_null() {
        return ptr::null_mut();
    }
    _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
    *t.add(len) = 0;
    t
}

unsafe fn mi_page_malloc_zero(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    if (*page).block_size != 0 {
        crate::mi_assert_internal!(crate::internal::mi_page_block_size(page) >= size);
        crate::mi_assert_internal!(_mi_is_aligned(
            mi_page_slice_start(page).cast(),
            crate::page::MI_OS_PAGE_SIZE
        ));
        crate::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);
    }

    let block = (*page).free;
    if mi_unlikely(block.is_null()) {
        return _mi_malloc_generic(theap, size, usize::from(zero), usable);
    }
    crate::mi_assert_internal!(!block.is_null() && _mi_ptr_page(block.cast()) == page);
    if !usable.is_null() {
        *usable = mi_page_usable_block_size(page);
    }

    (*page).free = mi_block_next(page, block);
    (*page).used = (*page).used.wrapping_add(1);
    crate::mi_assert_internal!((*page).free.is_null() || _mi_ptr_page((*page).free.cast()) == page);
    crate::mi_assert_internal!(
        (*page).block_size < MI_MAX_ALIGN_SIZE || _mi_is_aligned(block.cast(), MI_MAX_ALIGN_SIZE)
    );

    if cfg!(debug_assertions) && (*page).free_is_zero && size > size_of::<mi_block_t>() {
        crate::mi_assert_expensive!(crate::internal::mi_mem_is_zero(
            block.add(1).cast(),
            size - size_of::<mi_block_t>()
        ));
    }

    let bsize = mi_page_usable_block_size(page);
    mi_track_mem_undefined(block.cast(), bsize);

    if MI_STAT > 0 && bsize <= MI_LARGE_MAX_OBJ_SIZE {
        __mi_stat_increase(&mut (*theap).stats.malloc_normal, bsize);
        if MI_STAT > 1 {
            __mi_stat_counter_increase(&mut (*theap).stats.malloc_normal_count, 1);
            let bin = _mi_bin(bsize);
            __mi_stat_increase(&mut (*theap).stats.malloc_bins[bin], 1);
            __mi_stat_increase(&mut (*theap).stats.malloc_requested, size - MI_PADDING_SIZE);
        }
    }

    if !zero {
        (*block).next = 0;
        if cfg!(debug_assertions) && MI_TRACK_ENABLED == 0 {
            memset_nonhuge(page, block.cast(), bsize);
        }
    } else if !(*page).free_is_zero {
        _mi_memzero_aligned(block.cast(), bsize);
    } else {
        (*block).next = 0;
        mi_track_mem_defined(block.cast(), bsize);
    }

    if MI_PADDING != 0 {
        let padding = block.cast::<u8>().add(bsize).cast::<mi_padding_t>();
        let delta = (padding as usize).wrapping_sub(block as usize) - (size - MI_PADDING_SIZE);
        if cfg!(debug_assertions) {
            crate::mi_assert_internal!(delta <= bsize && bsize >= (size - MI_PADDING_SIZE + delta));
        }
        mi_track_mem_defined(padding.cast(), size_of::<mi_padding_t>());
        (*padding).canary = mi_ptr_encode_canary(page.cast(), block.cast(), &(*page).keys);
        (*padding).delta = delta as u32;
        if MI_PADDING_CHECK != 0 && !crate::page::mi_page_is_huge(page) {
            let fill = padding.cast::<u8>().sub(delta);
            let maxpad = delta.min(MI_MAX_ALIGN_SIZE);
            for i in 0..maxpad {
                *fill.add(i) = crate::types::MI_DEBUG_PADDING;
            }
        }
    }

    block.cast()
}

#[inline]
unsafe fn memset_nonhuge(page: *mut mi_page_t, block: *mut c_void, bsize: usize) {
    if !crate::page::mi_page_is_huge(page) {
        ptr::write_bytes(block, MI_DEBUG_UNINIT, bsize);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_page_malloc_zero(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    size: usize,
    zero: bool,
) -> *mut c_void {
    mi_page_malloc_zero(theap, page, size, zero, ptr::null_mut())
}

unsafe fn mi_theap_malloc_small_zero_nonnull(
    theap: *mut mi_theap_t,
    mut size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    crate::mi_assert!(!theap.is_null());
    crate::mi_assert!(size <= MI_SMALL_SIZE_MAX);
    if cfg!(debug_assertions) {
        let tid = _mi_thread_id();
        crate::mi_assert!((*(*theap).tld).thread_id == 0 || (*(*theap).tld).thread_id == tid);
    }
    if (MI_PADDING != 0 || crate::internal::MI_GUARDED != 0) && mi_unlikely(size == 0) {
        size = size_of::<*mut c_void>();
    }
    if crate::internal::MI_GUARDED != 0 && mi_unlikely(mi_theap_malloc_use_guarded(theap, size)) {
        return _mi_theap_malloc_guarded(theap, size, zero);
    }

    let page = _mi_theap_get_free_small_page(theap, size + MI_PADDING_SIZE);
    let p = mi_page_malloc_zero(theap, page, size + MI_PADDING_SIZE, zero, usable);
    mi_track_malloc(p, size, zero);

    if cfg!(debug_assertions) && !p.is_null() && zero {
        crate::mi_assert_expensive!(crate::internal::mi_mem_is_zero(p, size));
    }
    p
}

unsafe fn mi_theap_malloc_generic(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    crate::mi_assert!(
        theap.is_null()
            || (*(*theap).tld).thread_id == 0
            || (*(*theap).tld).thread_id == _mi_thread_id()
    );
    crate::mi_assert!((huge_alignment & 1) == 0);
    let p = _mi_malloc_generic(
        theap,
        size + MI_PADDING_SIZE,
        usize::from(zero) | huge_alignment,
        usable,
    );
    mi_track_malloc(p, size, zero);
    if cfg!(debug_assertions) && !p.is_null() && zero {
        crate::mi_assert_expensive!(crate::internal::mi_mem_is_zero(p, size));
    }
    p
}

unsafe fn mi_theap_malloc_small_zero(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    if MI_THEAP_INITASNULL && !theap.is_null() {
        mi_theap_malloc_small_zero_nonnull(theap, size, zero, usable)
    } else if MI_THEAP_INITASNULL {
        mi_theap_malloc_generic(theap, size, zero, 0, usable)
    } else {
        mi_theap_malloc_small_zero_nonnull(theap, size, zero, usable)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_small(size: usize) -> *mut c_void {
    mi_theap_malloc_small_zero(_mi_theap_default(), size, false, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    mi_theap_malloc_small_zero_nonnull(_mi_heap_theap(heap), size, false, ptr::null_mut())
}

unsafe fn mi_theap_malloc_zero_nonnull(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    if size <= MI_SMALL_SIZE_MAX {
        crate::mi_assert_internal!(huge_alignment == 0);
        mi_theap_malloc_small_zero_nonnull(theap, size, zero, usable)
    } else {
        mi_theap_malloc_generic(theap, size, zero, huge_alignment, usable)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _mi_theap_malloc_zero_ex(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    if (!theap.is_null() || !MI_THEAP_INITASNULL) && size <= MI_SMALL_SIZE_MAX {
        crate::mi_assert_internal!(huge_alignment == 0);
        mi_theap_malloc_small_zero_nonnull(theap, size, zero, usable)
    } else {
        mi_theap_malloc_generic(theap, size, zero, huge_alignment, usable)
    }
}

pub extern "C" fn mi_malloc_good_size(size: usize) -> usize {
    mi_good_size(size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_mallocn(count: usize, size: usize) -> *mut c_void {
    let mut total = 0usize;
    if mi_count_size_overflow(count, size, &mut total) {
        return ptr::null_mut();
    }
    mi_theap_malloc(_mi_theap_default(), total)
}

unsafe fn mi_theap_reallocf(theap: *mut mi_theap_t, p: *mut c_void, newsize: usize) -> *mut c_void {
    let newp = mi_theap_realloc(theap, p, newsize);
    if newp.is_null() && !p.is_null() {
        mi_free(p);
    }
    newp
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strdup(s: *const c_char) -> *mut c_char {
    mi_theap_strdup(_mi_theap_default(), s)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char {
    mi_theap_strndup(_mi_theap_default(), s, n)
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_path_max() -> usize {
    static PATH_MAX: AtomicUsize = AtomicUsize::new(0);
    let pmax = PATH_MAX.load(Ordering::Acquire);
    if pmax != 0 {
        return pmax;
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const _PC_PATH_MAX: c_int = 4;
    #[cfg(target_vendor = "apple")]
    const _PC_PATH_MAX: c_int = 5;

    let m = pathconf(c"/".as_ptr(), _PC_PATH_MAX);
    let pmax = if m <= 0 {
        4096
    } else if m < 256 {
        256
    } else if m > (64 * 1024) {
        64 * 1024
    } else {
        m as usize
    };
    let _ = PATH_MAX.compare_exchange(0, pmax, Ordering::AcqRel, Ordering::Acquire);
    PATH_MAX.load(Ordering::Acquire)
}

#[cfg(all(unix, not(target_os = "wasi")))]
pub unsafe fn mi_theap_realpath(
    theap: *mut mi_theap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    if !resolved_name.is_null() {
        return realpath(fname, resolved_name);
    }

    let n = mi_path_max();
    let buf = mi_zalloc(n + 1).cast::<c_char>();
    if buf.is_null() {
        mi_set_errno(ENOMEM);
        return ptr::null_mut();
    }
    let rname = realpath(fname, buf);
    let result = mi_theap_strndup(theap, rname, n);
    mi_free(buf.cast::<c_void>());
    result
}
