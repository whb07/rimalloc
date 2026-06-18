//! Core allocation / free paths + libc overrides

#![allow(dead_code, clippy::missing_safety_doc, unsafe_op_in_unsafe_fn)]

use core::ffi::{c_char, c_int, c_long, c_void};
use core::mem::size_of;
use core::{
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::free::_mi_usable_size_typed;
use crate::internal::{
    __mi_stat_adjust_decrease, __mi_stat_counter_increase, __mi_stat_increase, _mi_align_up,
    _mi_is_aligned, _mi_memcpy, _mi_memzero, _mi_memzero_aligned, _mi_os_page_size, _mi_ptr_page,
    _mi_strlen, _mi_strnlen, _mi_theap_get_free_small_page_typed, _mi_theap_heap, _mi_thread_id,
    MI_GUARDED, MI_LARGE_MAX_OBJ_SIZE, MI_PADDING, MI_PADDING_CHECK, mi_likely, mi_page_block_size,
    mi_page_heap, mi_page_keys, mi_page_set_has_interior_pointers, mi_page_slice_start,
    mi_page_start, mi_page_usable_block_size, mi_ptr_encode_canary, mi_theap_is_initialized,
    mi_theap_malloc_use_guarded, mi_unlikely,
};
use crate::mimalloc::{mi_free, mi_malloc_aligned};
use crate::options::{_mi_error_message_fmt, mi_option_is_enabled, mi_option_t};
use crate::page::{
    _mi_malloc_generic_typed, MI_MAX_ALLOC_SIZE, mi_bin_from_usable, mi_good_size_typed,
};
use crate::prim::{_mi_heap_theap, _mi_theap_default, MI_THEAP_INITASNULL};
use crate::track::{
    MI_TRACK_ENABLED, mi_track_align, mi_track_malloc, mi_track_mem_defined, mi_track_mem_undefined,
};
use crate::types::{
    ENOMEM, EOVERFLOW, MI_DEBUG_UNINIT, MI_MAX_ALIGN_SIZE, MI_PADDING_SIZE, MI_SMALL_SIZE_MAX,
    MI_STAT, mi_block_t, mi_heap_t, mi_padding_t, mi_page_t, mi_theap_t,
};

unsafe extern "C" {
    fn abort() -> !;
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

pub(crate) unsafe fn _mi_page_malloc_zero_typed(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    padded: crate::core_types::PaddedSize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    let size = padded.get();
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
        let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
            padded,
            if zero {
                crate::core_types::Zeroed::Yes
            } else {
                crate::core_types::Zeroed::No
            },
        );
        return _mi_malloc_generic_typed(
            theap,
            layout,
            crate::core_types::MallocGenericFlags::new(
                if zero {
                    crate::core_types::Zeroed::Yes
                } else {
                    crate::core_types::Zeroed::No
                },
                crate::core_types::MaybeHugeAlignment::none(),
            ),
            usable,
        );
    }
    crate::mi_assert_internal!(!block.is_null() && _mi_ptr_page(block.cast()) == page);
    if !usable.is_null() {
        *usable = mi_page_usable_block_size(page).get();
    }

    // Pop the head block off the page's available free list via the typed
    // wrapper. Zero-cost: compiles to the same `mi_block_next` + store as the
    // raw form below it replaces.
    // SAFETY: `page` is a valid page with a non-null `free` head (`block`).
    let available = unsafe { crate::core_types::AvailableFreeList::from_raw(block) };
    let (allocated, next_list) = unsafe {
        available.pop(crate::core_types::AvailablePage(
            crate::core_types::PagePtr::new_unchecked(page),
        ))
    };
    (*page).free = next_list
        .0
        .map(|b| b.as_ptr())
        .unwrap_or(core::ptr::null_mut());
    let block = allocated.0.as_ptr();
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

    let usable_size = mi_page_usable_block_size(page);
    let bsize = usable_size.get();
    mi_track_mem_undefined(block.cast(), bsize);

    if MI_STAT > 0 && bsize <= MI_LARGE_MAX_OBJ_SIZE {
        __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.malloc_normal), bsize);
        if MI_STAT > 1 {
            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_normal_count), 1);
            let bin = mi_bin_from_usable(usable_size).as_usize();
            __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.malloc_bins[bin]), 1);
            __mi_stat_increase(
                ptr::addr_of_mut!((*theap).stats.malloc_requested),
                size - MI_PADDING_SIZE,
            );
        }
    }

    if !zero {
        crate::core_types::BlockTag::ALIGNED
            .write_to(crate::core_types::BlockPtr::new_unchecked(block));
        if cfg!(debug_assertions) && MI_TRACK_ENABLED == 0 {
            memset_nonhuge(page, block.cast(), bsize);
        }
    } else if !(*page).free_is_zero {
        _mi_memzero_aligned(block.cast(), bsize);
    } else {
        crate::core_types::BlockTag::ALIGNED
            .write_to(crate::core_types::BlockPtr::new_unchecked(block));
        mi_track_mem_defined(block.cast(), bsize);
    }

    if MI_PADDING != 0 {
        let padding = block.cast::<u8>().add(bsize).cast::<mi_padding_t>();
        let delta = (padding as usize).wrapping_sub(block as usize) - (size - MI_PADDING_SIZE);
        if cfg!(debug_assertions) {
            crate::mi_assert_internal!(delta <= bsize && bsize >= (size - MI_PADDING_SIZE + delta));
        }
        mi_track_mem_defined(padding.cast(), size_of::<mi_padding_t>());
        let keys = mi_page_keys(page);
        (*padding).canary = mi_ptr_encode_canary(page.cast(), block.cast(), &keys);
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

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_malloc_zero(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    size: usize,
    zero: bool,
) -> *mut c_void {
    _mi_page_malloc_zero_typed(
        theap,
        page,
        crate::core_types::PaddedSize::new_unchecked(size),
        zero,
        ptr::null_mut(),
    )
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

    // Compute the padded size once via the typed `PaddedSize` boundary so both
    // the page lookup and the page-malloc use the same value, instead of
    // open-coding `size + MI_PADDING_SIZE` twice. Uses the branchless unchecked
    // path: `size <= MI_SMALL_SIZE_MAX` (≤ 1024, guarded upstream) and is
    // normalized to non-zero above, so the sum cannot overflow or yield zero.
    let padded = crate::core_types::RequestedSize::new(size).padded_unchecked();
    let page = _mi_theap_get_free_small_page_typed(theap, padded);
    let p = _mi_page_malloc_zero_typed(theap, page, padded, zero, usable);
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

    if MI_GUARDED != 0
        && huge_alignment == 0
        && !theap.is_null()
        && mi_unlikely(mi_theap_malloc_use_guarded(theap, size))
    {
        return _mi_theap_malloc_guarded(theap, size, zero);
    }

    // Construct the typed layout once at the generic boundary. On overflow
    // (request too large), report and return null.
    let layout = match crate::core_types::LayoutRequest::malloc(size) {
        Ok(layout) => layout,
        Err(_) => {
            _mi_error_message_fmt(
                EOVERFLOW,
                format_args!("allocation request is too large ({size} bytes)\n"),
            );
            return ptr::null_mut();
        }
    };
    let flags = crate::core_types::MallocGenericFlags::new(
        if zero {
            crate::core_types::Zeroed::Yes
        } else {
            crate::core_types::Zeroed::No
        },
        crate::core_types::MaybeHugeAlignment::new(huge_alignment),
    );
    let p = _mi_malloc_generic_typed(theap, layout, flags, usable);
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
pub unsafe extern "C" fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    mi_theap_malloc_small_zero(theap, size, false, ptr::null_mut())
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

#[inline(always)]
pub(crate) unsafe extern "C" fn _mi_theap_malloc_zero_ex(
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

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_malloc_zero(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    _mi_theap_malloc_zero_ex(theap, size, zero, 0, usable)
}

#[used]
static MI_THEAP_MALLOC_ZERO_EX_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    usize,
    bool,
    usize,
    *mut usize,
) -> *mut c_void = _mi_theap_malloc_zero_ex;

#[used]
static MI_THEAP_MALLOC_ZERO_KEEP: unsafe extern "C" fn(
    *mut mi_theap_t,
    usize,
    bool,
    *mut usize,
) -> *mut c_void = _mi_theap_malloc_zero;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    _mi_theap_malloc_zero(theap, size, false, ptr::null_mut())
}

#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_malloc(size: usize) -> *mut c_void {
    mi_theap_malloc(_mi_theap_default(), size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(size) {
        mi_theap_malloc_zero_nonnull(
            _mi_heap_theap(heap),
            layout.requested().get(),
            false,
            0,
            ptr::null_mut(),
        )
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_small(size: usize) -> *mut c_void {
    mi_theap_malloc_small_zero(_mi_theap_default(), size, true, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    mi_theap_malloc_small_zero(theap, size, true, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    mi_theap_malloc_small_zero_nonnull(_mi_heap_theap(heap), size, true, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    _mi_theap_malloc_zero(theap, size, true, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc(size: usize) -> *mut c_void {
    _mi_theap_malloc_zero(_mi_theap_default(), size, true, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    mi_theap_malloc_zero_nonnull(_mi_heap_theap(heap), size, true, 0, ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_calloc(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_calloc_req(theap, req)
    } else {
        ptr::null_mut()
    }
}

unsafe fn mi_theap_calloc_req(
    theap: *mut mi_theap_t,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    mi_theap_zalloc(theap, req.layout().requested().get())
}

#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_calloc(count: usize, size: usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_calloc_req(_mi_theap_default(), req)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_calloc_req(_mi_heap_theap(heap), req)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc_small(size: usize, usable: *mut usize) -> *mut c_void {
    mi_theap_malloc_small_zero(_mi_theap_default(), size, false, usable)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc_small(size: usize, usable: *mut usize) -> *mut c_void {
    mi_theap_malloc_small_zero(_mi_theap_default(), size, true, usable)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_umalloc(
    theap: *mut mi_theap_t,
    size: usize,
    usable: *mut usize,
) -> *mut c_void {
    _mi_theap_malloc_zero_ex(theap, size, false, 0, usable)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc(size: usize, usable: *mut usize) -> *mut c_void {
    mi_theap_umalloc(_mi_theap_default(), size, usable)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc(size: usize, usable: *mut usize) -> *mut c_void {
    _mi_theap_malloc_zero_ex(_mi_theap_default(), size, true, 0, usable)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_ucalloc(count: usize, size: usize, usable: *mut usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_uzalloc(req.layout().requested().get(), usable)
    } else {
        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_malloc_good_size(size: usize) -> usize {
    mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()
}

unsafe fn mi_theap_mallocn(theap: *mut mi_theap_t, count: usize, size: usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_mallocn_req(theap, req)
    } else {
        ptr::null_mut()
    }
}

unsafe fn mi_theap_mallocn_req(
    theap: *mut mi_theap_t,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    mi_theap_malloc(theap, req.total().get())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_mallocn(count: usize, size: usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_mallocn_req(_mi_theap_default(), req)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_mallocn(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_mallocn_req(_mi_heap_theap(heap), req)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_expand(p: *mut c_void, newsize: usize) -> *mut c_void {
    if MI_PADDING != 0 {
        let _ = p;
        let _ = newsize;
        return ptr::null_mut();
    }
    if p.is_null() {
        return ptr::null_mut();
    }
    let page = _mi_ptr_page(p);
    if page.is_null() {
        return ptr::null_mut();
    }
    let size = _mi_usable_size_typed(p, page);
    if newsize > size.get() {
        return ptr::null_mut();
    }
    p
}

unsafe fn mi_validate_ptr_page(
    p: *const c_void,
    msg: *const c_char,
) -> crate::core_types::PointerLookup {
    crate::free::mi_validate_pointer(p.cast_mut(), msg)
}

unsafe fn mi_realloc_old_allocation(
    p: *mut c_void,
    msg: *const c_char,
) -> Option<crate::core_types::OldAllocation> {
    match mi_validate_ptr_page(p, msg) {
        crate::core_types::PointerLookup::Valid {
            user,
            page,
            location,
        } => {
            let block = location.block();
            let usable = _mi_usable_size_typed(user.as_ptr(), page.as_ptr());
            // SAFETY: `PointerLookup::Valid` proves `user` belongs to `page` and
            // supplies the block start derived by the validation boundary.
            Some(crate::core_types::OldAllocation::new(
                user, usable, page, block,
            ))
        }
        crate::core_types::PointerLookup::Null
        | crate::core_types::PointerLookup::Invalid { .. } => None,
    }
}

pub(crate) unsafe extern "C" fn _mi_theap_realloc_zero(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    zero: bool,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    let old = if p.is_null() {
        None
    } else {
        let Some(old) = mi_realloc_old_allocation(p, c"mi_realloc".as_ptr()) else {
            if !usable_pre.is_null() {
                *usable_pre = 0;
            }
            return ptr::null_mut();
        };
        Some(old)
    };
    mi_theap_realloc_apply(theap, p, old, newsize, zero, usable_pre, usable_post)
}

unsafe fn mi_theap_realloc_apply(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    old: Option<crate::core_types::OldAllocation>,
    newsize: usize,
    zero: bool,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    let (page, size) = if let Some(old) = old {
        let page = old.page().as_ptr();
        let size = old.usable();
        if !usable_pre.is_null() {
            *usable_pre = mi_page_usable_block_size(page).get();
        }
        (page, size.get())
    } else {
        if !usable_pre.is_null() {
            *usable_pre = 0;
        }
        (ptr::null_mut(), 0usize)
    };

    if mi_unlikely(
        newsize <= size
            && newsize >= (size / 2)
            && newsize > 0
            && mi_page_heap(page) == _mi_theap_heap(theap),
    ) {
        crate::mi_assert_internal!(!p.is_null());
        if !usable_post.is_null() {
            *usable_post = mi_page_usable_block_size(page).get();
        }
        return p;
    }

    let newp = mi_theap_umalloc(theap, newsize, usable_post);
    if mi_likely(!newp.is_null()) {
        if zero && newsize > size {
            let start = if size >= size_of::<isize>() {
                size - size_of::<isize>()
            } else {
                0
            };
            _mi_memzero(newp.cast::<u8>().add(start).cast(), newsize - start);
        } else if newsize == 0 {
            *newp.cast::<u8>() = 0;
        }
        if mi_likely(!p.is_null()) {
            let copysize = if newsize > size { size } else { newsize };
            mi_track_mem_defined(p, copysize);
            _mi_memcpy(newp, p, copysize);
            mi_free(p);
        }
    }
    newp
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_realloc(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
        mi_theap_realloc_req(theap, p, layout)
    } else {
        ptr::null_mut()
    }
}

pub(crate) unsafe fn mi_theap_realloc_req(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    let req = crate::core_types::ReallocRequest::new(
        crate::core_types::UserPtr::new(p),
        None,
        layout,
        layout.zeroed(),
    );
    mi_theap_realloc_request(theap, req, ptr::null_mut(), ptr::null_mut())
}

unsafe fn mi_theap_realloc_request(
    theap: *mut mi_theap_t,
    req: crate::core_types::ReallocRequest,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    let p = req
        .ptr()
        .map(|ptr| ptr.as_ptr())
        .unwrap_or(core::ptr::null_mut());
    let old = if p.is_null() {
        None
    } else {
        let Some(old) = mi_realloc_old_allocation(p, c"mi_realloc".as_ptr()) else {
            if !usable_pre.is_null() {
                *usable_pre = 0;
            }
            return ptr::null_mut();
        };
        Some(old)
    };
    let req = req.with_old(old);
    let layout = req.new_layout();
    mi_theap_realloc_apply(
        theap,
        p,
        req.old(),
        layout.requested().get(),
        req.zero_new_tail() == crate::core_types::Zeroed::Yes,
        usable_pre,
        usable_post,
    )
}

unsafe fn mi_theap_reallocn(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_reallocn_req(theap, p, req)
    } else {
        ptr::null_mut()
    }
}

unsafe fn mi_theap_reallocn_req(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(req.total().get()) {
        mi_theap_realloc_req(theap, p, layout)
    } else {
        ptr::null_mut()
    }
}

unsafe fn mi_theap_reallocf_req(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    let newp = mi_theap_realloc_req(theap, p, layout);
    if newp.is_null() && !p.is_null() {
        mi_free(p);
    }
    newp
}

unsafe fn mi_theap_rezalloc(theap: *mut mi_theap_t, p: *mut c_void, newsize: usize) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::zalloc(newsize) {
        mi_theap_realloc_req(theap, p, layout)
    } else {
        ptr::null_mut()
    }
}

unsafe fn mi_theap_recalloc(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_recalloc_req(theap, p, req)
    } else {
        ptr::null_mut()
    }
}

unsafe fn mi_theap_recalloc_req(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    mi_theap_realloc_req(theap, p, req.layout())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
        mi_theap_realloc_req(_mi_theap_default(), p, layout)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocn(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_reallocn_req(_mi_theap_default(), p, req)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_urealloc(
    p: *mut c_void,
    newsize: usize,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    _mi_theap_realloc_zero(
        _mi_theap_default(),
        p,
        newsize,
        false,
        usable_pre,
        usable_post,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
        mi_theap_reallocf_req(_mi_theap_default(), p, layout)
    } else {
        if !p.is_null() {
            crate::free::mi_free(p);
        }
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_rezalloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::zalloc(newsize) {
        mi_theap_realloc_req(_mi_theap_default(), p, layout)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    mi_theap_recalloc(_mi_theap_default(), p, count, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
        mi_theap_realloc_req(_mi_heap_theap(heap), p, layout)
    } else {
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_reallocn(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    mi_theap_reallocn(_mi_heap_theap(heap), p, count, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_reallocf(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
        mi_theap_reallocf_req(_mi_heap_theap(heap), p, layout)
    } else {
        if !p.is_null() {
            crate::free::mi_free(p);
        }
        core::ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    mi_theap_rezalloc(_mi_heap_theap(heap), p, newsize)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    mi_theap_recalloc(_mi_heap_theap(heap), p, count, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strdup(s: *const c_char) -> *mut c_char {
    mi_theap_strdup(_mi_theap_default(), s)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strdup(heap: *mut mi_heap_t, s: *const c_char) -> *mut c_char {
    mi_theap_strdup(_mi_heap_theap(heap), s)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char {
    mi_theap_strndup(_mi_theap_default(), s, n)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strndup(
    heap: *mut mi_heap_t,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    mi_theap_strndup(_mi_heap_theap(heap), s, n)
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_realpath(
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

#[cfg(all(unix, not(target_os = "wasi")))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realpath(
    heap: *mut mi_heap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    mi_theap_realpath(_mi_heap_theap(heap), fname, resolved_name)
}

#[cfg(all(unix, not(target_os = "wasi")))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realpath(
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    mi_theap_realpath(_mi_theap_default(), fname, resolved_name)
}

type StdNewHandler = Option<unsafe extern "C" fn()>;

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_get_new_handler() -> StdNewHandler {
    type GetNewHandler = unsafe extern "C" fn() -> StdNewHandler;

    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    let symbol = dlsym(ptr::null_mut(), c"_ZSt15get_new_handlerv".as_ptr());
    if symbol.is_null() {
        None
    } else {
        let get_new_handler = core::mem::transmute::<*mut c_void, GetNewHandler>(symbol);
        get_new_handler()
    }
}

#[cfg(not(all(unix, not(target_os = "wasi"))))]
#[allow(non_snake_case)]
unsafe fn _ZSt15get_new_handlerv() -> StdNewHandler {
    None
}

#[cfg(not(all(unix, not(target_os = "wasi"))))]
unsafe fn mi_get_new_handler() -> StdNewHandler {
    _ZSt15get_new_handlerv()
}

unsafe fn mi_try_new_handler(nothrow: bool) -> bool {
    let h = mi_get_new_handler();
    if let Some(h) = h {
        h();
        true
    } else {
        crate::internal::_mi_error_message(ENOMEM, c"out of memory in 'new'".as_ptr());
        if !nothrow {
            abort();
        }
        false
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn mi_theap_try_new(
    theap: *mut mi_theap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    let mut p = mi_theap_malloc(theap, size);
    while p.is_null() && mi_try_new_handler(nothrow) {
        p = mi_theap_malloc(theap, size);
    }
    p
}

#[inline(never)]
unsafe extern "C" fn mi_try_new(size: usize, nothrow: bool) -> *mut c_void {
    mi_theap_try_new(_mi_theap_default(), size, nothrow)
}

#[inline(never)]
unsafe extern "C" fn mi_heap_try_new(
    heap: *mut mi_heap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    mi_theap_try_new(_mi_heap_theap(heap), size, nothrow)
}

#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn mi_theap_alloc_new(
    theap: *mut mi_theap_t,
    size: usize,
) -> *mut c_void {
    let p = mi_theap_malloc(theap, size);
    if mi_unlikely(p.is_null()) {
        mi_theap_try_new(theap, size, false)
    } else {
        p
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new(size: usize) -> *mut c_void {
    mi_theap_alloc_new(_mi_theap_default(), size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    let p = mi_heap_malloc(heap, size);
    if mi_unlikely(p.is_null()) {
        mi_heap_try_new(heap, size, false)
    } else {
        p
    }
}

unsafe fn mi_theap_alloc_new_n(theap: *mut mi_theap_t, count: usize, size: usize) -> *mut c_void {
    match crate::core_types::CallocRequest::new(count, size) {
        Ok(req) => mi_theap_alloc_new(theap, req.total().get()),
        Err(_) => {
            mi_try_new_handler(false);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_n(count: usize, size: usize) -> *mut c_void {
    mi_theap_alloc_new_n(_mi_theap_default(), count, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new_n(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    mi_theap_alloc_new_n(_mi_heap_theap(heap), count, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_nothrow(size: usize) -> *mut c_void {
    let p = mi_malloc(size);
    if mi_unlikely(p.is_null()) {
        mi_try_new(size, true)
    } else {
        p
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_aligned(size: usize, alignment: usize) -> *mut c_void {
    let mut p: *mut c_void;
    loop {
        p = mi_malloc_aligned(size, alignment);
        if !p.is_null() || !mi_try_new_handler(false) {
            return p;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_aligned_nothrow(size: usize, alignment: usize) -> *mut c_void {
    let mut p: *mut c_void;
    loop {
        p = mi_malloc_aligned(size, alignment);
        if !p.is_null() || !mi_try_new_handler(true) {
            return p;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    let mut q: *mut c_void;
    loop {
        q = mi_realloc(p, newsize);
        if !q.is_null() || !mi_try_new_handler(false) {
            return q;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_reallocn(
    p: *mut c_void,
    newcount: usize,
    size: usize,
) -> *mut c_void {
    match crate::core_types::CallocRequest::new(newcount, size) {
        Ok(req) => mi_new_realloc(p, req.total().get()),
        Err(_) => {
            mi_try_new_handler(false);
            ptr::null_mut()
        }
    }
}

#[used]
static MI_THEAP_TRY_NEW_KEEP: unsafe extern "C" fn(*mut mi_theap_t, usize, bool) -> *mut c_void =
    mi_theap_try_new;

#[used]
static MI_TRY_NEW_KEEP: unsafe extern "C" fn(usize, bool) -> *mut c_void = mi_try_new;

#[used]
static MI_HEAP_TRY_NEW_KEEP: unsafe extern "C" fn(*mut mi_heap_t, usize, bool) -> *mut c_void =
    mi_heap_try_new;

unsafe fn mi_block_ptr_set_guarded(block: *mut mi_block_t, obj_size: usize) -> *mut c_void {
    if MI_GUARDED == 0 {
        let _ = obj_size;
        return block.cast();
    }
    let page = _mi_ptr_page(block.cast());
    mi_page_set_has_interior_pointers(page, true);
    crate::core_types::BlockTag::GUARDED
        .write_to(crate::core_types::BlockPtr::new_unchecked(block));

    let block_size = mi_page_block_size(page);
    let os_page_size = _mi_os_page_size();
    crate::mi_assert_internal!(block_size >= obj_size + os_page_size + size_of::<mi_block_t>());
    if block_size < obj_size + os_page_size + size_of::<mi_block_t>() {
        mi_free(block.cast());
        return ptr::null_mut();
    }

    let guard_page = block.cast::<u8>().add(block_size - os_page_size);
    crate::mi_assert_internal!(_mi_is_aligned(block.cast(), os_page_size));
    crate::mi_assert_internal!(_mi_is_aligned(guard_page.cast(), os_page_size));
    if !(*page).memid.is_pinned && _mi_is_aligned(guard_page.cast(), os_page_size) {
        let ok = crate::os::_mi_os_protect(guard_page.cast(), os_page_size);
        if mi_unlikely(!ok) {
            crate::options::_mi_warning_message_fmt(format_args!(
                "failed to set a guard page behind an object (object {block:p} of size {block_size})\n"
            ));
        }
    } else {
        crate::options::_mi_warning_message_fmt(format_args!(
            "unable to set a guard page behind an object due to pinned memory (large OS pages?) (object {block:p} of size {block_size})\n"
        ));
    }

    let mut offset = block_size - os_page_size - obj_size;
    crate::mi_assert_internal!(offset > size_of::<mi_block_t>());
    if offset > crate::types::MI_PAGE_MAX_OVERALLOC_ALIGN {
        offset = crate::types::MI_PAGE_MAX_OVERALLOC_ALIGN;
    }
    let p = block.cast::<u8>().add(offset).cast::<c_void>();
    crate::mi_assert_internal!(p.cast::<u8>() == guard_page.sub(obj_size));
    mi_track_align(block.cast(), p, offset, obj_size);
    mi_track_mem_defined(block.cast(), size_of::<mi_block_t>());
    p
}

pub(crate) unsafe extern "C" fn _mi_theap_malloc_guarded(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
) -> *mut c_void {
    if MI_GUARDED == 0 {
        let _ = mi_block_ptr_set_guarded as unsafe fn(*mut mi_block_t, usize) -> *mut c_void;
        return _mi_theap_malloc_zero(theap, size, zero, ptr::null_mut());
    }
    if mi_unlikely(size >= MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE) {
        _mi_error_message_fmt(
            EOVERFLOW,
            format_args!("(guarded) allocation request is too large ({size} bytes)\n"),
        );
        return ptr::null_mut();
    }

    let os_page_size = _mi_os_page_size();
    let obj_size = if mi_option_is_enabled(mi_option_t::mi_option_guarded_precise) {
        size
    } else {
        _mi_align_up(size, MI_MAX_ALIGN_SIZE)
    };
    let bsize = _mi_align_up(
        _mi_align_up(obj_size, MI_MAX_ALIGN_SIZE) + size_of::<mi_block_t>(),
        MI_MAX_ALIGN_SIZE,
    );
    let req_size = _mi_align_up(bsize + os_page_size, os_page_size);
    let layout = crate::core_types::LayoutRequest::from_padded_unchecked(
        crate::core_types::PaddedSize::new_unchecked(req_size),
        crate::core_types::Zeroed::No,
    );
    let block = _mi_malloc_generic_typed(
        theap,
        layout,
        crate::core_types::MallocGenericFlags::new(
            crate::core_types::Zeroed::No,
            crate::core_types::MaybeHugeAlignment::none(),
        ),
        ptr::null_mut(),
    )
    .cast::<mi_block_t>();
    if block.is_null() {
        return ptr::null_mut();
    }
    let p = mi_block_ptr_set_guarded(block, obj_size);
    if p.is_null() {
        return p;
    }
    if zero {
        _mi_memzero_aligned(p, obj_size);
    }

    mi_track_malloc(p, obj_size, zero);
    let theap = if mi_theap_is_initialized(theap) {
        theap
    } else {
        _mi_theap_default()
    };
    if mi_theap_is_initialized(theap) {
        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_guarded_count), 1);
        if MI_STAT > 1 {
            __mi_stat_adjust_decrease(ptr::addr_of_mut!((*theap).stats.malloc_requested), req_size);
            __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.malloc_requested), size);
        }
    }
    p
}
