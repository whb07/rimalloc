//! Mechanical translation of `src/alloc-aligned.c`.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc
)]

use core::ffi::c_void;
use core::mem::size_of;

use crate::alloc::{
    _mi_page_malloc_zero_typed, _mi_theap_malloc_guarded, _mi_theap_malloc_zero,
    _mi_theap_malloc_zero_ex, mi_theap_realloc_req,
};
use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::bits::MI_KiB;
#[cfg(not(mi_c_malloc_fast))]
use crate::free::_mi_padding_shrink;
use crate::init::_mi_theap_empty_get;
use crate::internal::{
    _mi_align_up_ptr, _mi_is_power_of_two, _mi_memcpy_aligned, _mi_memzero, _mi_ptr_page,
    MI_BLOCK_TAG_ALIGNED, MI_GUARDED, mi_likely, mi_page_set_has_interior_pointers,
    mi_page_usable_block_size, mi_theap_malloc_use_guarded, mi_unlikely,
};
use crate::options::_mi_error_message_fmt;
use crate::page::{
    MI_MAX_ALLOC_SIZE, MI_PADDING_SIZE, MI_PAGE_OSPAGE_BLOCK_ALIGN2, mi_good_size_typed,
};
use crate::prim::{_mi_heap_theap, _mi_theap_default, MI_THEAP_INITASNULL};
use crate::track::{mi_track_align, mi_track_malloc};
use crate::types::{MI_SMALL_SIZE_MAX, mi_block_t, mi_heap_t, mi_theap_t};

const MI_MAX_ALIGN_SIZE: usize = 16;
const MI_PAGE_MAX_START_BLOCK_ALIGN2: usize = 4 * MI_KiB;
const MI_PAGE_MAX_OVERALLOC_ALIGN: usize = MI_ARENA_SLICE_SIZE;
const MI_EOVERFLOW: i32 = 75;

#[cfg(mi_c_malloc_fast)]
#[inline(always)]
unsafe fn mi_padding_shrink_call(
    page: *const crate::types::mi_page_t,
    block: *const mi_block_t,
    min_size: usize,
) {
    core::arch::asm!(
        "call _mi_padding_shrink",
        in("rdi") page,
        in("rsi") block,
        in("rdx") min_size,
        clobber_abi("C"),
    );
}

#[cfg(not(mi_c_malloc_fast))]
#[inline(always)]
unsafe fn mi_padding_shrink_call(
    page: *const crate::types::mi_page_t,
    block: *const mi_block_t,
    min_size: usize,
) {
    _mi_padding_shrink(page, block, min_size);
}

#[inline]
pub unsafe fn mi_malloc_is_naturally_aligned(size: usize, alignment: usize) -> bool {
    crate::mi_assert_internal!(_mi_is_power_of_two(alignment) && alignment > 0);
    if alignment > size {
        return false;
    }
    let bsize = mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get();
    let ok = (bsize <= MI_PAGE_MAX_START_BLOCK_ALIGN2 && _mi_is_power_of_two(bsize))
        || (alignment == MI_PAGE_OSPAGE_BLOCK_ALIGN2
            && bsize.is_multiple_of(MI_PAGE_OSPAGE_BLOCK_ALIGN2));
    if ok {
        crate::mi_assert_internal!((bsize & (alignment - 1)) == 0);
    }
    ok
}

pub unsafe fn mi_theap_malloc_guarded_aligned(
    mut theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    zero: bool,
) -> *mut c_void {
    if MI_THEAP_INITASNULL && mi_unlikely(theap.is_null()) {
        theap = _mi_theap_empty_get();
    }
    crate::mi_assert_internal!(alignment > 0 && alignment < MI_PAGE_MAX_OVERALLOC_ALIGN);
    if mi_unlikely(
        alignment >= MI_PAGE_MAX_OVERALLOC_ALIGN
            || size > (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE - alignment),
    ) {
        _mi_error_message_fmt(
            MI_EOVERFLOW,
            format_args!(
                "(guarded) aligned allocation request is too large (size {size}, alignment {alignment})\n"
            ),
        );
        return core::ptr::null_mut();
    }
    let oversize = size + alignment - 1;
    let base = _mi_theap_malloc_guarded(theap, oversize, zero);
    if base.is_null() {
        return core::ptr::null_mut();
    }
    let p = _mi_align_up_ptr(base.cast_const(), alignment);
    mi_track_align(base, p, p as usize - base as usize, size);
    crate::mi_assert_internal!(crate::mimalloc::mi_usable_size(p) >= size);
    crate::mi_assert_internal!(crate::internal::_mi_is_aligned(p, alignment));
    p
}

#[inline]
pub unsafe fn mi_theap_malloc_zero_no_guarded(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    _mi_theap_malloc_zero(theap, size, zero, usable)
}

pub unsafe fn mi_theap_malloc_zero_aligned_at_overalloc(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    crate::mi_assert_internal!(size <= (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE));
    crate::mi_assert_internal!(alignment != 0 && _mi_is_power_of_two(alignment));

    let p: *mut c_void;
    let oversize: usize;
    if mi_unlikely(alignment > MI_PAGE_MAX_OVERALLOC_ALIGN) {
        if mi_unlikely(offset != 0) {
            _mi_error_message_fmt(
                MI_EOVERFLOW,
                format_args!(
                    "aligned allocation with a large alignment cannot be used with an alignment offset (size {size}, alignment {alignment}, offset {offset})\n"
                ),
            );
            return core::ptr::null_mut();
        }
        oversize = if size <= MI_SMALL_SIZE_MAX {
            MI_SMALL_SIZE_MAX + 1
        } else {
            size
        };
        p = _mi_theap_malloc_zero_ex(theap, oversize, zero, alignment, usable);
        if p.is_null() {
            return core::ptr::null_mut();
        }
    } else {
        crate::mi_assert_internal!(
            size <= (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE)
                && alignment <= MI_PAGE_MAX_OVERALLOC_ALIGN
        );
        crate::mi_assert_internal!(size < usize::MAX - alignment);
        oversize = usize::max(size, MI_MAX_ALIGN_SIZE) + alignment - 1;
        p = mi_theap_malloc_zero_no_guarded(theap, oversize, zero, usable);
        if p.is_null() {
            return core::ptr::null_mut();
        }
    }

    let align_mask = alignment - 1;
    let poffset = (p as usize + offset) & align_mask;
    let adjust = if poffset == 0 { 0 } else { alignment - poffset };
    crate::mi_assert_internal!(adjust < alignment);
    let aligned_p = (p as usize + adjust) as *mut c_void;

    let page = _mi_ptr_page(p);
    if aligned_p != p {
        mi_page_set_has_interior_pointers(page, true);
        if MI_GUARDED != 0 && adjust >= size_of::<mi_block_t>() {
            let block = p.cast::<mi_block_t>();
            (*block).next = MI_BLOCK_TAG_ALIGNED;
        }
        mi_padding_shrink_call(page, p.cast::<mi_block_t>(), adjust + size);
    }

    crate::mi_assert_internal!(mi_page_usable_block_size(page).get() >= adjust + size);
    crate::mi_assert_internal!(((aligned_p as usize) + offset).is_multiple_of(alignment));
    crate::mi_assert_internal!(crate::mimalloc::mi_usable_size(aligned_p) >= size);
    crate::mi_assert_internal!(
        crate::mimalloc::mi_usable_size(p) == crate::mimalloc::mi_usable_size(aligned_p) + adjust
    );

    if p != aligned_p {
        mi_track_align(
            p,
            aligned_p,
            adjust,
            crate::mimalloc::mi_usable_size(aligned_p),
        );
    }
    aligned_p
}

pub unsafe fn mi_theap_malloc_zero_aligned_at_generic(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    crate::mi_assert_internal!(alignment != 0 && _mi_is_power_of_two(alignment));
    if mi_unlikely(size > (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE)) {
        _mi_error_message_fmt(
            MI_EOVERFLOW,
            format_args!(
                "aligned allocation request is too large (size {size}, alignment {alignment})\n"
            ),
        );
        return core::ptr::null_mut();
    }

    if offset == 0 && mi_malloc_is_naturally_aligned(size, alignment) {
        let p = mi_theap_malloc_zero_no_guarded(theap, size, zero, usable);
        crate::mi_assert_internal!(p.is_null() || (p as usize).is_multiple_of(alignment));
        if mi_likely(((p as usize) & (alignment - 1)) == 0) {
            return p;
        }
        crate::mi_assert!(false);
        crate::mimalloc::mi_free(p);
    }

    mi_theap_malloc_zero_aligned_at_overalloc(theap, size, alignment, offset, zero, usable)
}

pub unsafe fn mi_theap_malloc_zero_aligned_at(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    if mi_unlikely(alignment == 0 || !_mi_is_power_of_two(alignment)) {
        if crate::internal::MI_DEBUG > 0 {
            _mi_error_message_fmt(
                MI_EOVERFLOW,
                format_args!(
                    "aligned allocation requires the alignment to be a power-of-two (size {size}, alignment {alignment})\n"
                ),
            );
        }
        return core::ptr::null_mut();
    }

    if MI_GUARDED != 0
        && !(MI_THEAP_INITASNULL && theap.is_null())
        && offset == 0
        && alignment < MI_PAGE_MAX_OVERALLOC_ALIGN
        && mi_theap_malloc_use_guarded(theap, size)
    {
        return mi_theap_malloc_guarded_aligned(theap, size, alignment, zero);
    }

    if !(MI_THEAP_INITASNULL && theap.is_null())
        && mi_likely(size <= MI_SMALL_SIZE_MAX && alignment <= size)
    {
        let align_mask = alignment - 1;
        let padded = crate::core_types::RequestedSize::new(size).padded_unchecked();
        let page = crate::internal::_mi_theap_get_free_small_page_typed(theap, padded);
        if mi_likely(!page.is_null() && !(*page).free.is_null()) {
            let is_aligned = (((*page).free as usize) + offset) & align_mask == 0;
            if mi_likely(is_aligned) {
                if !usable.is_null() {
                    *usable = mi_page_usable_block_size(page).get();
                }
                let p =
                    _mi_page_malloc_zero_typed(theap, page, padded, zero, core::ptr::null_mut());
                crate::mi_assert_internal!(!p.is_null());
                crate::mi_assert_internal!(((p as usize) + offset).is_multiple_of(alignment));
                mi_track_malloc(p, size, zero);
                return p;
            }
        }
    }

    mi_theap_malloc_zero_aligned_at_generic(theap, size, alignment, offset, zero, usable)
}

#[inline]
pub unsafe fn mi_theap_malloc_aligned_at(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_malloc_zero_aligned_at(theap, size, alignment, offset, false, core::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc_aligned(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_malloc_aligned_at(theap, size, alignment, 0)
}

#[inline]
pub unsafe fn mi_theap_zalloc_aligned_at(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_malloc_zero_aligned_at(theap, size, alignment, offset, true, core::ptr::null_mut())
}

#[inline]
pub unsafe fn mi_theap_zalloc_aligned(
    theap: *mut mi_theap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_zalloc_aligned_at(theap, size, alignment, 0)
}

#[inline]
pub unsafe fn mi_theap_calloc_aligned_at(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        mi_theap_zalloc_aligned_at(theap, req.layout().requested().get(), alignment, offset)
    } else {
        core::ptr::null_mut()
    }
}

#[inline]
pub unsafe fn mi_theap_calloc_aligned(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_calloc_aligned_at(theap, count, size, alignment, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_aligned_at(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_malloc_aligned_at(_mi_theap_default(), size, alignment, offset)
}

#[cfg(not(mi_c_malloc_fast))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void {
    mi_theap_malloc_aligned(_mi_theap_default(), size, alignment)
}

#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_aligned(_size: usize, _alignment: usize) -> *mut c_void {
    core::arch::naked_asm!(
        "endbr64",
        "mov rax, qword ptr [rip + __mi_theap_default@GOTTPOFF]",
        "mov rdx, rsi",
        "mov rsi, rdi",
        "mov rdi, qword ptr fs:[rax]",
        "jmp {target}",
        target = sym mi_theap_malloc_aligned,
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc_aligned(
    size: usize,
    alignment: usize,
    block_size: *mut usize,
) -> *mut c_void {
    mi_theap_malloc_zero_aligned_at(_mi_theap_default(), size, alignment, 0, false, block_size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_aligned_at(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_zalloc_aligned_at(_mi_theap_default(), size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void {
    mi_theap_zalloc_aligned(_mi_theap_default(), size, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc_aligned(
    size: usize,
    alignment: usize,
    block_size: *mut usize,
) -> *mut c_void {
    mi_theap_malloc_zero_aligned_at(_mi_theap_default(), size, alignment, 0, true, block_size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_calloc_aligned_at(
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_calloc_aligned_at(_mi_theap_default(), count, size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_calloc_aligned(
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_calloc_aligned(_mi_theap_default(), count, size, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_aligned_at(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_malloc_aligned_at(_mi_heap_theap(heap), size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_aligned(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_malloc_aligned(_mi_heap_theap(heap), size, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_aligned_at(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_zalloc_aligned_at(_mi_heap_theap(heap), size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_aligned(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_zalloc_aligned(_mi_heap_theap(heap), size, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc_aligned_at(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_calloc_aligned_at(_mi_heap_theap(heap), count, size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc_aligned(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_calloc_aligned(_mi_heap_theap(heap), count, size, alignment)
}

pub unsafe fn mi_theap_realloc_zero_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
) -> *mut c_void {
    if alignment == 0 {
        return core::ptr::null_mut();
    }
    let zeroed = if zero {
        crate::core_types::Zeroed::Yes
    } else {
        crate::core_types::Zeroed::No
    };
    let Ok(layout) =
        crate::core_types::LayoutRequest::aligned_at(newsize, alignment, offset, zeroed)
    else {
        return core::ptr::null_mut();
    };
    mi_theap_realloc_zero_aligned_at_req(theap, p, layout, offset)
}

unsafe fn mi_theap_realloc_zero_aligned_at_req(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
    offset: usize,
) -> *mut c_void {
    let newsize = layout.requested().get();
    let alignment = layout.alignment().get();
    let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;

    crate::mi_assert!(alignment > 0);
    if alignment <= size_of::<usize>() && offset == 0 {
        return mi_theap_realloc_req(theap, p, layout);
    }
    if p.is_null() {
        return mi_theap_malloc_zero_aligned_at(
            theap,
            newsize,
            alignment,
            offset,
            zero,
            core::ptr::null_mut(),
        );
    }
    let size = crate::mimalloc::mi_usable_size(p);
    if newsize <= size
        && newsize >= (size - (size / 2))
        && (p as usize + offset).is_multiple_of(alignment)
    {
        return p;
    }

    let newp = mi_theap_malloc_aligned_at(theap, newsize, alignment, offset);
    if !newp.is_null() {
        if zero && newsize > size {
            let start = if size >= size_of::<isize>() {
                size - size_of::<isize>()
            } else {
                0
            };
            _mi_memzero((newp as *mut u8).add(start).cast(), newsize - start);
        }
        _mi_memcpy_aligned(newp, p, usize::min(newsize, size));
        crate::mimalloc::mi_free(p);
    }
    newp
}

#[inline]
pub unsafe fn mi_theap_realloc_zero_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    zero: bool,
) -> *mut c_void {
    crate::mi_assert!(alignment > 0);
    mi_theap_realloc_zero_aligned_at(theap, p, newsize, alignment, 0, zero)
}

#[inline]
pub unsafe fn mi_theap_realloc_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_realloc_zero_aligned_at(theap, p, newsize, alignment, offset, false)
}

#[inline]
pub unsafe fn mi_theap_realloc_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_realloc_zero_aligned(theap, p, newsize, alignment, false)
}

#[inline]
pub unsafe fn mi_theap_rezalloc_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_realloc_zero_aligned_at(theap, p, newsize, alignment, offset, true)
}

#[inline]
pub unsafe fn mi_theap_rezalloc_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_realloc_zero_aligned(theap, p, newsize, alignment, true)
}

#[inline]
pub unsafe fn mi_theap_recalloc_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(newcount, size) {
        mi_theap_rezalloc_aligned_at(theap, p, req.layout().requested().get(), alignment, offset)
    } else {
        core::ptr::null_mut()
    }
}

#[inline]
pub unsafe fn mi_theap_recalloc_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_recalloc_aligned_at(theap, p, newcount, size, alignment, 0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realloc_aligned_at(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_realloc_aligned_at(_mi_theap_default(), p, newsize, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realloc_aligned(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_realloc_aligned(_mi_theap_default(), p, newsize, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_rezalloc_aligned_at(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_rezalloc_aligned_at(_mi_theap_default(), p, newsize, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_rezalloc_aligned(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_rezalloc_aligned(_mi_theap_default(), p, newsize, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc_aligned_at(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_recalloc_aligned_at(_mi_theap_default(), p, newcount, size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc_aligned(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_recalloc_aligned(_mi_theap_default(), p, newcount, size, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc_aligned_at(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_realloc_aligned_at(_mi_heap_theap(heap), p, newsize, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc_aligned(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_realloc_aligned(_mi_heap_theap(heap), p, newsize, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc_aligned_at(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_rezalloc_aligned_at(_mi_heap_theap(heap), p, newsize, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc_aligned(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_rezalloc_aligned(_mi_heap_theap(heap), p, newsize, alignment)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc_aligned_at(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    mi_theap_recalloc_aligned_at(_mi_heap_theap(heap), p, newcount, size, alignment, offset)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc_aligned(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    mi_theap_recalloc_aligned(_mi_heap_theap(heap), p, newcount, size, alignment)
}
