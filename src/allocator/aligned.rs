//! Mechanical translation of `src/alloc-aligned.c`.

// Retains upstream C allocation variants that are reached only by selected ABI
// exports or feature/platform combinations.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::ffi::c_void;
use core::mem::size_of;

use crate::alloc::{
    _mi_theap_malloc_guarded, _mi_theap_malloc_zero, _mi_theap_malloc_zero_ex,
    calloc_request_or_report,
};
use crate::arena::MI_ARENA_SLICE_SIZE;
use crate::bits::MI_KiB;
use crate::core_types::{BorrowedHeap, Errno};
#[cfg(not(mi_c_malloc_fast))]
use crate::free::_mi_padding_shrink;
use crate::internal::{
    _mi_align_up_ptr, _mi_is_power_of_two, _mi_memcpy_aligned, _mi_memzero, _mi_ptr_page,
    MI_GUARDED, mi_likely, mi_page_set_has_interior_pointers, mi_page_usable_block_size,
    mi_unlikely,
};
use crate::options::_mi_error_message_fmt;
use crate::page::{
    MI_MAX_ALLOC_SIZE, MI_PADDING_SIZE, MI_PAGE_OSPAGE_BLOCK_ALIGN2, mi_good_size_typed,
};
use crate::prim::{_mi_heap_theap, _mi_theap_default, MI_THEAP_INITASNULL};
use crate::track::mi_track_align;
use crate::types::{MI_SMALL_SIZE_MAX, mi_block_t, mi_heap_t, mi_theap_t};

const MI_MAX_ALIGN_SIZE: usize = 16;
const MI_PAGE_MAX_START_BLOCK_ALIGN2: usize = 4 * MI_KiB;
const MI_PAGE_MAX_OVERALLOC_ALIGN: usize = MI_ARENA_SLICE_SIZE;

mod thread_heap;

#[allow(unused_imports)]
pub use thread_heap::{
    mi_theap_calloc_aligned, mi_theap_calloc_aligned_at, mi_theap_malloc_aligned,
    mi_theap_malloc_aligned_at, mi_theap_malloc_zero_aligned_at, mi_theap_zalloc_aligned,
    mi_theap_zalloc_aligned_at,
};
pub(crate) use thread_heap::{mi_theap_calloc_aligned_at_req, mi_theap_malloc_zero_aligned_at_req};

#[cfg(mi_c_malloc_fast)]
#[inline(always)]
unsafe fn mi_padding_shrink_call(
    page: *const crate::types::mi_page_t,
    block: *const mi_block_t,
    min_size: usize,
) {
    // SAFETY[AUDIT:aligned-padding-shrink-boundary]: aligned padding-shrink shims preserve C ABI and page/block witness requirements.
    unsafe {
        core::arch::asm!(
            "call _mi_padding_shrink",
            in("rdi") page,
            in("rsi") block,
            in("rdx") min_size,
            clobber_abi("C"),
        );
    }
}

#[cfg(not(mi_c_malloc_fast))]
#[inline(always)]
unsafe fn mi_padding_shrink_call(
    page: *const crate::types::mi_page_t,
    block: *const mi_block_t,
    min_size: usize,
) {
    // SAFETY[AUDIT:aligned-padding-shrink-boundary]: aligned padding-shrink shims preserve C ABI and page/block witness requirements.
    unsafe { _mi_padding_shrink(page, block, min_size) };
}

#[inline]
fn mi_malloc_is_naturally_aligned(
    size: crate::core_types::RequestedSize,
    alignment: crate::core_types::Alignment,
) -> bool {
    let requested = size;
    let size = requested.get();
    let alignment = alignment.get();
    if alignment > size {
        return false;
    }
    let bsize = mi_good_size_typed(requested).get();
    let ok = (bsize <= MI_PAGE_MAX_START_BLOCK_ALIGN2 && _mi_is_power_of_two(bsize))
        || (alignment == MI_PAGE_OSPAGE_BLOCK_ALIGN2
            && bsize.is_multiple_of(MI_PAGE_OSPAGE_BLOCK_ALIGN2));
    if ok {
        crate::internal::mi_assert_internal!((bsize & (alignment - 1)) == 0);
    }
    ok
}

fn aligned_layout_or_report(
    size: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
) -> Option<(
    crate::core_types::LayoutRequest,
    crate::core_types::Alignment,
    crate::core_types::AlignmentOffset,
)> {
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        if crate::internal::MI_DEBUG > 0 {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation requires the alignment to be a power-of-two (size {size}, alignment {alignment})\n"
                ),
            );
        }
        return None;
    };
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let zeroed = if zero {
        crate::core_types::Zeroed::Yes
    } else {
        crate::core_types::Zeroed::No
    };
    match crate::core_types::LayoutRequest::aligned_at(size, alignment.get(), offset, zeroed) {
        Ok(layout) => Some((layout, alignment, offset)),
        Err(_) => {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation request is too large (size {size}, alignment {})\n",
                    alignment.get()
                ),
            );
            None
        }
    }
}

/// # Safety
/// `theap` must be initialized and local to the current thread.
/// `alignment` must be valid for aligned allocation.
unsafe fn mi_theap_malloc_guarded_aligned(
    theap: crate::core_types::ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    let size = layout.requested().get();
    let alignment = alignment.get();
    let offset = offset.get();
    let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;
    let theap = theap.ptr().as_ptr();
    crate::internal::mi_assert_internal!(alignment > 0 && alignment < MI_PAGE_MAX_OVERALLOC_ALIGN);
    crate::internal::mi_assert_internal!(offset == 0);
    if mi_unlikely(
        alignment >= MI_PAGE_MAX_OVERALLOC_ALIGN
            || size > (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE - alignment),
    ) {
        _mi_error_message_fmt(
            Errno::EOVERFLOW,
            format_args!(
                "(guarded) aligned allocation request is too large (size {size}, alignment {alignment})\n"
            ),
        );
        return core::ptr::null_mut();
    }
    let oversize = size + alignment - 1;
    // SAFETY[AUDIT:aligned-guarded-allocation-boundary]: guarded aligned allocation preserves layout, tracking, and usable-size witnesses.
    let base = unsafe { _mi_theap_malloc_guarded(theap, oversize, zero) };
    if base.is_null() {
        return core::ptr::null_mut();
    }
    let p = _mi_align_up_ptr(base.cast_const(), alignment);
    // SAFETY[AUDIT:aligned-guarded-allocation-boundary]: guarded aligned allocation preserves layout, tracking, and usable-size witnesses.
    unsafe {
        mi_track_align(base, p, p as usize - base as usize, size);
    }
    // SAFETY[AUDIT:aligned-guarded-allocation-boundary]: guarded aligned allocation preserves layout, tracking, and usable-size witnesses.
    crate::internal::mi_assert_internal!(unsafe { crate::mimalloc::mi_usable_size(p) } >= size);
    crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(p, alignment));
    p
}

#[inline]
/// # Safety
/// `theap` must be initialized and local to the current thread.
/// If `usable` is non-null it must be writable for one `usize`.
pub(crate) unsafe fn mi_theap_malloc_zero_no_guarded(
    theap: crate::core_types::ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
    usable: *mut usize,
) -> *mut c_void {
    let size = layout.requested().get();
    let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;
    crate::internal::mi_assert_internal!(alignment.get() > 0);
    crate::internal::mi_assert_internal!(offset.get() < alignment.get());
    // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
    unsafe { _mi_theap_malloc_zero(theap.ptr().as_ptr(), size, zero, usable) }
}

/// # Safety
/// `theap` must be `None` only when lazy thread-heap initialization is enabled.
/// `alignment` must be a non-zero power of two, and `usable` must be writable when non-null.
unsafe fn mi_theap_malloc_zero_aligned_at_overalloc(
    theap: Option<crate::core_types::ThreadHeapLocal>,
    layout: crate::core_types::LayoutRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
    usable: *mut usize,
) -> *mut c_void {
    let size = layout.requested().get();
    let alignment_raw = alignment.get();
    let offset_raw = offset.get();
    let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;
    crate::internal::mi_assert_internal!(size <= (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE));
    crate::internal::mi_assert_internal!(alignment_raw != 0 && _mi_is_power_of_two(alignment_raw));

    let p: *mut c_void;
    let oversize: usize;
    if mi_unlikely(alignment_raw > MI_PAGE_MAX_OVERALLOC_ALIGN) {
        if mi_unlikely(offset_raw != 0) {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation with a large alignment cannot be used with an alignment offset (size {size}, alignment {alignment_raw}, offset {offset_raw})\n"
                ),
            );
            return core::ptr::null_mut();
        }
        oversize = if size <= MI_SMALL_SIZE_MAX {
            MI_SMALL_SIZE_MAX + 1
        } else {
            size
        };
        let theap = theap
            .map(|theap| theap.ptr().as_ptr())
            .unwrap_or(core::ptr::null_mut());
        // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
        p = unsafe { _mi_theap_malloc_zero_ex(theap, oversize, zero, alignment_raw, usable) };
        if p.is_null() {
            return core::ptr::null_mut();
        }
    } else {
        crate::internal::mi_assert_internal!(
            size <= (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE)
                && alignment_raw <= MI_PAGE_MAX_OVERALLOC_ALIGN
        );
        crate::internal::mi_assert_internal!(size < usize::MAX - alignment_raw);
        oversize = usize::max(size, MI_MAX_ALIGN_SIZE) + alignment_raw - 1;
        let no_guarded_layout = if zero {
            crate::core_types::LayoutRequest::zalloc(oversize)
        } else {
            crate::core_types::LayoutRequest::malloc(oversize)
        };
        let Ok(no_guarded_layout) = no_guarded_layout else {
            crate::internal::mi_assert_internal!(false);
            return core::ptr::null_mut();
        };
        p = if let Some(theap) = theap {
            // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
            unsafe {
                mi_theap_malloc_zero_no_guarded(theap, no_guarded_layout, alignment, offset, usable)
            }
        } else {
            // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
            unsafe { _mi_theap_malloc_zero(core::ptr::null_mut(), oversize, zero, usable) }
        };
        if p.is_null() {
            return core::ptr::null_mut();
        }
    }

    let align_mask = alignment_raw - 1;
    let poffset = (p as usize + offset_raw) & align_mask;
    let adjust = if poffset == 0 {
        0
    } else {
        alignment_raw - poffset
    };
    crate::internal::mi_assert_internal!(adjust < alignment_raw);
    let aligned_p = (p as usize + adjust) as *mut c_void;

    // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
    let page = unsafe { _mi_ptr_page(p) };
    if aligned_p != p {
        // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
        unsafe {
            mi_page_set_has_interior_pointers(crate::core_types::PagePtr::new_unchecked(page), true)
        };
        if MI_GUARDED != 0 && adjust >= size_of::<mi_block_t>() {
            let block = p.cast::<mi_block_t>();
            // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
            let block = unsafe { crate::core_types::BlockPtr::new_unchecked(block) };
            // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
            unsafe {
                crate::core_types::BlockTag::write_aligned_to(block);
            }
        }
        // SAFETY[AUDIT:aligned-padding-shrink-boundary]: aligned padding-shrink shims preserve C ABI and page/block witness requirements.
        unsafe { mi_padding_shrink_call(page, p.cast::<mi_block_t>(), adjust + size) };
    }

    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
        unsafe { mi_page_usable_block_size(page) }.get() >= adjust + size
    );
    crate::internal::mi_assert_internal!(
        ((aligned_p as usize) + offset_raw).is_multiple_of(alignment_raw)
    );
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
        unsafe { crate::mimalloc::mi_usable_size(aligned_p) } >= size
    );
    crate::internal::mi_assert_internal!(
        // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
        unsafe { crate::mimalloc::mi_usable_size(p) }
            // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
            == unsafe { crate::mimalloc::mi_usable_size(aligned_p) } + adjust
    );

    if p != aligned_p {
        // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
        let usable_size = unsafe { crate::mimalloc::mi_usable_size(aligned_p) };
        // SAFETY[AUDIT:aligned-overalloc-adjust-boundary]: over-allocation adjustment preserves page, block, interior-pointer, and usable-size invariants.
        unsafe { mi_track_align(p, aligned_p, adjust, usable_size) };
    }
    aligned_p
}

/// # Safety
/// `theap` must be `None` only when lazy thread-heap initialization is enabled.
/// `alignment` must be a non-zero power of two, and `usable` must be writable when non-null.
unsafe fn mi_theap_malloc_zero_aligned_at_generic(
    theap: Option<crate::core_types::ThreadHeapLocal>,
    layout: crate::core_types::LayoutRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
    usable: *mut usize,
) -> *mut c_void {
    let size = layout.requested().get();
    let alignment_raw = alignment.get();
    let offset_raw = offset.get();
    crate::internal::mi_assert_internal!(alignment_raw != 0 && _mi_is_power_of_two(alignment_raw));
    if mi_unlikely(size > (MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE)) {
        _mi_error_message_fmt(
            Errno::EOVERFLOW,
            format_args!(
                "aligned allocation request is too large (size {size}, alignment {alignment_raw})\n"
            ),
        );
        return core::ptr::null_mut();
    }

    if let Some(local_theap) = theap
        && offset_raw == 0
        && mi_malloc_is_naturally_aligned(layout.requested(), alignment)
    {
        // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
        let p = unsafe {
            mi_theap_malloc_zero_no_guarded(local_theap, layout, alignment, offset, usable)
        };
        crate::internal::mi_assert_internal!(
            p.is_null() || (p as usize).is_multiple_of(alignment_raw)
        );
        if mi_likely(((p as usize) & (alignment_raw - 1)) == 0) {
            return p;
        }
        crate::internal::mi_assert!(false);
        // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
        unsafe { crate::mimalloc::mi_free(p) };
    }

    // SAFETY[AUDIT:aligned-no-guard-allocation-boundary]: no-guard aligned allocation delegates through validated layout, alignment, and usable out-pointer witnesses.
    unsafe { mi_theap_malloc_zero_aligned_at_overalloc(theap, layout, alignment, offset, usable) }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. Returned memory must be released through this
/// allocator.
// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_aligned_at(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
    unsafe { mi_theap_malloc_aligned_at(_mi_theap_default(), size, alignment, offset) }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. Returned memory must be released through this
/// allocator.
// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
#[cfg(not(mi_c_malloc_fast))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
    unsafe { mi_theap_malloc_aligned(_mi_theap_default(), size, alignment) }
}

// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
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

/// # Safety
/// `alignment` must be valid for aligned allocation. If `block_size` is non-null it must be
/// writable for one `usize`.
// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc_aligned(
    size: usize,
    alignment: usize,
    block_size: *mut usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
    unsafe {
        mi_theap_malloc_zero_aligned_at(_mi_theap_default(), size, alignment, 0, false, block_size)
    }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. Returned memory must be released through this
/// allocator.
// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_aligned_at(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
    unsafe { mi_theap_zalloc_aligned_at(_mi_theap_default(), size, alignment, offset) }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. Returned memory must be released through this
/// allocator.
// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
    unsafe { mi_theap_zalloc_aligned(_mi_theap_default(), size, alignment) }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. If `block_size` is non-null it must be
/// writable for one `usize`.
// SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc_aligned(
    size: usize,
    alignment: usize,
    block_size: *mut usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-allocation-export-boundary]: global aligned allocation exports forward raw C ABI inputs into checked aligned requests.
    unsafe {
        mi_theap_malloc_zero_aligned_at(_mi_theap_default(), size, alignment, 0, true, block_size)
    }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. Returned memory must be released through this
/// allocator.
// SAFETY[AUDIT:aligned-global-calloc-export-boundary]: global aligned calloc exports forward count/size/alignment inputs into checked calloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_calloc_aligned_at(
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-calloc-export-boundary]: global aligned calloc exports forward count/size/alignment inputs into checked calloc requests.
    unsafe { mi_theap_calloc_aligned_at(_mi_theap_default(), count, size, alignment, offset) }
}

/// # Safety
/// `alignment` must be valid for aligned allocation. Returned memory must be released through this
/// allocator.
// SAFETY[AUDIT:aligned-global-calloc-export-boundary]: global aligned calloc exports forward count/size/alignment inputs into checked calloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_calloc_aligned(
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-global-calloc-export-boundary]: global aligned calloc exports forward count/size/alignment inputs into checked calloc requests.
    unsafe { mi_theap_calloc_aligned(_mi_theap_default(), count, size, alignment) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_aligned_at(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some((layout, alignment, offset)) =
        aligned_layout_or_report(size, alignment, offset, false)
    else {
        return core::ptr::null_mut();
    };
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    unsafe { mi_heap_malloc_aligned_at_typed(heap, layout, alignment, offset) }
}

unsafe fn mi_heap_malloc_aligned_at_typed(
    heap: BorrowedHeap,
    layout: crate::core_types::LayoutRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    let theap = unsafe { _mi_heap_theap(heap.ptr().as_ptr()) };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    unsafe {
        mi_theap_malloc_zero_aligned_at_req(theap, layout, alignment, offset, core::ptr::null_mut())
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_aligned(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, 0, false)
    else {
        return core::ptr::null_mut();
    };
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    unsafe { mi_heap_malloc_aligned_at_typed(heap, layout, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_aligned_at(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, offset, true)
    else {
        return core::ptr::null_mut();
    };
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    unsafe { mi_heap_malloc_aligned_at_typed(heap, layout, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_aligned(
    heap: *mut mi_heap_t,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some((layout, alignment, offset)) = aligned_layout_or_report(size, alignment, 0, true)
    else {
        return core::ptr::null_mut();
    };
    // SAFETY[AUDIT:aligned-heap-allocation-export-boundary]: heap aligned allocation exports borrow heap handles before checked aligned dispatch.
    unsafe { mi_heap_malloc_aligned_at_typed(heap, layout, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc_aligned_at(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(request) = calloc_request_or_report(count, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        if crate::internal::MI_DEBUG > 0 {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation requires the alignment to be a power-of-two (size {}, alignment {alignment})\n",
                    request.total().get()
                ),
            );
        }
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(offset);
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    unsafe { mi_heap_calloc_aligned_at_typed(heap, request, alignment, offset) }
}

/// # Safety
/// `heap` must reference a live heap owned by this allocator.
unsafe fn mi_heap_calloc_aligned_at_typed(
    heap: BorrowedHeap,
    request: crate::core_types::CallocRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    let theap = unsafe { _mi_heap_theap(heap.ptr().as_ptr()) };
    let theap = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    unsafe { mi_theap_calloc_aligned_at_req(theap, request, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc_aligned(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(request) = calloc_request_or_report(count, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        if crate::internal::MI_DEBUG > 0 {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!(
                    "aligned allocation requires the alignment to be a power-of-two (size {}, alignment {alignment})\n",
                    request.total().get()
                ),
            );
        }
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    // SAFETY[AUDIT:aligned-calloc-product-boundary]: aligned calloc paths use checked product requests before heap or thread-heap dispatch.
    unsafe { mi_heap_calloc_aligned_at_typed(heap, request, alignment, offset) }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_realloc_zero_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
    zero: bool,
) -> *mut c_void {
    let Some(alignment_typed) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    if alignment_typed.get() <= size_of::<usize>() && offset == 0 {
        let layout = if zero {
            crate::core_types::LayoutRequest::zalloc(newsize)
        } else {
            crate::core_types::LayoutRequest::malloc(newsize)
        };
        return match layout {
            // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
            Ok(layout) => unsafe { crate::alloc::mi_theap_realloc_layout(theap, p, layout) },
            Err(_) => core::ptr::null_mut(),
        };
    }
    let zeroed = if zero {
        crate::core_types::Zeroed::Yes
    } else {
        crate::core_types::Zeroed::No
    };
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let Ok(layout) =
        crate::core_types::LayoutRequest::aligned_at(newsize, alignment, offset, zeroed)
    else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment_typed, offset) }
}

unsafe fn mi_theap_realloc_zero_aligned_at_req(
    theap: *mut mi_theap_t,
    request: crate::core_types::ReallocRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    let layout = request.new_layout();
    let newsize = layout.requested().get();
    let alignment_raw = alignment.get();
    let offset_raw = offset.get();
    let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;
    let p = request
        .ptr()
        .map(|ptr| ptr.as_ptr())
        .unwrap_or(core::ptr::null_mut());

    crate::internal::mi_assert!(alignment_raw > 0);
    if alignment_raw <= size_of::<usize>() && offset_raw == 0 {
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        return unsafe { crate::alloc::mi_theap_realloc_req(theap, request) };
    }
    if p.is_null() {
        let theap = if MI_THEAP_INITASNULL && theap.is_null() {
            None
        } else {
            // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
            Some(unsafe {
                crate::core_types::ThreadHeapLocal::new_unchecked(
                    crate::core_types::ThreadHeapPtr::new_unchecked(theap),
                )
            })
        };
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        return unsafe {
            mi_theap_malloc_zero_aligned_at_req(
                theap,
                layout,
                alignment,
                offset,
                core::ptr::null_mut(),
            )
        };
    }
    let Some(old) = request.old() else {
        return core::ptr::null_mut();
    };
    let size = old.usable().get();
    if newsize <= size
        && newsize >= (size - (size / 2))
        && (p as usize + offset_raw).is_multiple_of(alignment_raw)
    {
        return p;
    }

    let allocation_layout = if zero {
        match crate::core_types::LayoutRequest::aligned_at(
            newsize,
            alignment_raw,
            offset,
            crate::core_types::Zeroed::No,
        ) {
            Ok(layout) => layout,
            Err(_) => return core::ptr::null_mut(),
        }
    } else {
        layout
    };
    let theap_local = if MI_THEAP_INITASNULL && theap.is_null() {
        None
    } else {
        // SAFETY[AUDIT:aligned-realloc-copy-boundary]: aligned realloc copy preserves bounded old contents before freeing the validated old block.
        Some(unsafe {
            crate::core_types::ThreadHeapLocal::new_unchecked(
                crate::core_types::ThreadHeapPtr::new_unchecked(theap),
            )
        })
    };
    // SAFETY[AUDIT:aligned-realloc-copy-boundary]: aligned realloc copy preserves bounded old contents before freeing the validated old block.
    let newp = unsafe {
        mi_theap_malloc_zero_aligned_at_req(
            theap_local,
            allocation_layout,
            alignment,
            offset,
            core::ptr::null_mut(),
        )
    };
    if !newp.is_null() {
        let preserve = crate::core_types::PreserveBytes::new(
            crate::core_types::UsableSize::new(size),
            layout.requested(),
        );
        let preserved = preserve.get();
        if zero && newsize > preserved {
            let start = if preserved >= size_of::<isize>() {
                preserved - size_of::<isize>()
            } else {
                0
            };
            // SAFETY[AUDIT:aligned-realloc-copy-boundary]: aligned realloc copy preserves bounded old contents before freeing the validated old block.
            unsafe { _mi_memzero((newp as *mut u8).add(start).cast(), newsize - start) };
        }
        // SAFETY[AUDIT:aligned-realloc-copy-boundary]: aligned realloc copy preserves bounded old contents before freeing the validated old block.
        unsafe {
            _mi_memcpy_aligned(newp, p, preserve.get());
            crate::mimalloc::mi_free(p);
        }
    }
    newp
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_realloc_zero_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    zero: bool,
) -> *mut c_void {
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let zeroed = if zero {
        crate::core_types::Zeroed::Yes
    } else {
        crate::core_types::Zeroed::No
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        if zero {
            crate::core_types::LayoutRequest::zalloc(newsize)
        } else {
            crate::core_types::LayoutRequest::malloc(newsize)
        }
    } else {
        crate::core_types::LayoutRequest::aligned_at(newsize, alignment.get(), offset, zeroed)
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment, offset) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_realloc_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let Ok(layout) = crate::core_types::LayoutRequest::aligned_at(
        newsize,
        alignment.get(),
        offset,
        crate::core_types::Zeroed::No,
    ) else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment, offset) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_realloc_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        crate::core_types::LayoutRequest::malloc(newsize)
    } else {
        crate::core_types::LayoutRequest::aligned_at(
            newsize,
            alignment.get(),
            offset,
            crate::core_types::Zeroed::No,
        )
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment, offset) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_rezalloc_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    let Some(alignment_typed) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    if alignment_typed.get() <= size_of::<usize>() && offset == 0 {
        let Ok(layout) = crate::core_types::LayoutRequest::zalloc(newsize) else {
            return core::ptr::null_mut();
        };
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        return unsafe { crate::alloc::mi_theap_realloc_layout(theap, p, layout) };
    }
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let Ok(layout) = crate::core_types::LayoutRequest::aligned_at(
        newsize,
        alignment,
        offset,
        crate::core_types::Zeroed::Yes,
    ) else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-request-boundary]: aligned realloc request builders validate old user pointers and checked layout inputs.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment_typed, offset) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_rezalloc_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        crate::core_types::LayoutRequest::zalloc(newsize)
    } else {
        crate::core_types::LayoutRequest::aligned_at(
            newsize,
            alignment.get(),
            offset,
            crate::core_types::Zeroed::Yes,
        )
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment, offset) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_recalloc_aligned_at(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    let Some(request) = calloc_request_or_report(newcount, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment_typed) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    if alignment_typed.get() <= size_of::<usize>() && offset == 0 {
        let layout = request.layout();
        // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
        return unsafe { crate::alloc::mi_theap_realloc_layout(theap, p, layout) };
    }
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let Ok(layout) = crate::core_types::LayoutRequest::aligned_at(
        request.total().get(),
        alignment,
        offset,
        crate::core_types::Zeroed::Yes,
    ) else {
        return core::ptr::null_mut();
    };
    let realloc_request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
        let Some(realloc_request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        realloc_request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
    unsafe {
        mi_theap_recalloc_aligned_at_req(theap, request, realloc_request, alignment_typed, offset)
    }
}

unsafe fn mi_theap_recalloc_aligned_at_req(
    theap: *mut mi_theap_t,
    request: crate::core_types::CallocRequest,
    realloc_request: crate::core_types::ReallocRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    crate::internal::mi_debug_assert!(
        request.total().get() == realloc_request.new_layout().requested().get()
    );
    // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, realloc_request, alignment, offset) }
}

#[inline]
/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
pub unsafe fn mi_theap_recalloc_aligned(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    let Some(request) = calloc_request_or_report(newcount, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        Ok(request.layout())
    } else {
        crate::core_types::LayoutRequest::aligned_at(
            request.total().get(),
            alignment.get(),
            offset,
            crate::core_types::Zeroed::Yes,
        )
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let realloc_request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
        let Some(realloc_request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        realloc_request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-recalloc-product-boundary]: aligned recalloc paths use checked product requests and zeroing metadata.
    unsafe { mi_theap_recalloc_aligned_at_req(theap, request, realloc_request, alignment, offset) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realloc_aligned_at(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    unsafe { mi_theap_realloc_aligned_at(_mi_theap_default(), p, newsize, alignment, offset) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realloc_aligned(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    unsafe { mi_theap_realloc_aligned(_mi_theap_default(), p, newsize, alignment) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_rezalloc_aligned_at(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    unsafe { mi_theap_rezalloc_aligned_at(_mi_theap_default(), p, newsize, alignment, offset) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_rezalloc_aligned(
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    unsafe { mi_theap_rezalloc_aligned(_mi_theap_default(), p, newsize, alignment) }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc_aligned_at(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    unsafe {
        mi_theap_recalloc_aligned_at(_mi_theap_default(), p, newcount, size, alignment, offset)
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator, and `alignment` must be
/// valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc_aligned(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    unsafe { mi_theap_recalloc_aligned(_mi_theap_default(), p, newcount, size, alignment) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator, and `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc_aligned_at(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-global-export-boundary]: global aligned realloc exports forward raw C ABI inputs into checked realloc requests.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some((layout, alignment, offset)) =
        aligned_layout_or_report(newsize, alignment, offset, false)
    else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_heap_realloc_aligned_at_typed(heap, request, alignment, offset) }
}

/// # Safety
/// `heap` must reference a live heap owned by this allocator.
unsafe fn mi_heap_realloc_aligned_at_typed(
    heap: BorrowedHeap,
    request: crate::core_types::ReallocRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let theap = unsafe { _mi_heap_theap(heap.ptr().as_ptr()) };
    // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_theap_realloc_zero_aligned_at_req(theap, request, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator, and `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc_aligned(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        crate::core_types::LayoutRequest::malloc(newsize)
    } else {
        crate::core_types::LayoutRequest::aligned_at(
            newsize,
            alignment.get(),
            offset,
            crate::core_types::Zeroed::No,
        )
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-heap-realloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_heap_realloc_aligned_at_typed(heap, request, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator, and `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc_aligned_at(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some((layout, alignment, offset)) =
        aligned_layout_or_report(newsize, alignment, offset, true)
    else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_heap_realloc_aligned_at_typed(heap, request, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator, and `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc_aligned(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        crate::core_types::LayoutRequest::zalloc(newsize)
    } else {
        crate::core_types::LayoutRequest::aligned_at(
            newsize,
            alignment.get(),
            offset,
            crate::core_types::Zeroed::Yes,
        )
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
        let Some(request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-heap-rezalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_heap_realloc_aligned_at_typed(heap, request, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator, and `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc_aligned_at(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(request) = calloc_request_or_report(newcount, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment_typed) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(offset);
    let Ok(layout) = crate::core_types::LayoutRequest::aligned_at(
        request.total().get(),
        alignment,
        offset,
        crate::core_types::Zeroed::Yes,
    ) else {
        return core::ptr::null_mut();
    };
    let realloc_request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
        let Some(realloc_request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        realloc_request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe {
        mi_heap_recalloc_aligned_at_typed(heap, request, realloc_request, alignment_typed, offset)
    }
}

/// # Safety
/// `heap` must reference a live heap owned by this allocator.
unsafe fn mi_heap_recalloc_aligned_at_typed(
    heap: BorrowedHeap,
    request: crate::core_types::CallocRequest,
    realloc_request: crate::core_types::ReallocRequest,
    alignment: crate::core_types::Alignment,
    offset: crate::core_types::AlignmentOffset,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let theap = unsafe { _mi_heap_theap(heap.ptr().as_ptr()) };
    // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_theap_recalloc_aligned_at_req(theap, request, realloc_request, alignment, offset) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator, and `alignment` must be valid for aligned allocation.
// SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc_aligned(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    let Some(heap) = (unsafe { BorrowedHeap::from_allocation_raw(heap) }) else {
        return core::ptr::null_mut();
    };
    let Some(request) = calloc_request_or_report(newcount, size) else {
        return core::ptr::null_mut();
    };
    let Some(alignment) = crate::core_types::Alignment::new(alignment) else {
        return core::ptr::null_mut();
    };
    let offset = crate::core_types::AlignmentOffset::new(0);
    let layout = if alignment.get() <= size_of::<usize>() {
        Ok(request.layout())
    } else {
        crate::core_types::LayoutRequest::aligned_at(
            request.total().get(),
            alignment.get(),
            offset,
            crate::core_types::Zeroed::Yes,
        )
    };
    let Ok(layout) = layout else {
        return core::ptr::null_mut();
    };
    let realloc_request = if let Some(user) = crate::core_types::UserPtr::new(p) {
        // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
        let Some(realloc_request) = (unsafe {
            crate::alloc::mi_realloc_request_from_user(user, layout, core::ptr::null_mut())
        }) else {
            return core::ptr::null_mut();
        };
        realloc_request
    } else {
        crate::core_types::ReallocRequest::from_null(layout, layout.zeroed())
    };
    // SAFETY[AUDIT:aligned-realloc-heap-recalloc-boundary]: heap aligned realloc exports borrow heap handles before checked realloc dispatch.
    unsafe { mi_heap_recalloc_aligned_at_typed(heap, request, realloc_request, alignment, offset) }
}
