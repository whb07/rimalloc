//! Core allocation / free paths + libc overrides

use core::ffi::{c_char, c_int, c_long, c_void};
use core::mem::size_of;
use core::{
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::core_types::{BorrowedHeap, Errno, PagePtr, ThreadHeapLocal, ThreadHeapPtr, UserPtr};
use crate::free::_mi_usable_size_typed;
use crate::internal::{
    __mi_stat_adjust_decrease, __mi_stat_counter_increase, __mi_stat_increase, _mi_align_up,
    _mi_is_aligned, _mi_memcpy, _mi_memzero, _mi_memzero_aligned, _mi_os_page_size_typed,
    _mi_ptr_page, _mi_strlen, _mi_strnlen, _mi_theap_get_free_small_page_typed, _mi_theap_heap,
    _mi_thread_id, MI_GUARDED, MI_LARGE_MAX_OBJ_SIZE, MI_PADDING, MI_PADDING_CHECK, mi_likely,
    mi_page_block_size, mi_page_heap, mi_page_keys, mi_page_set_has_interior_pointers,
    mi_page_slice_start, mi_page_start, mi_page_usable_block_size, mi_theap_is_initialized,
    mi_theap_malloc_use_guarded, mi_unlikely,
};
use crate::mimalloc::{mi_free, mi_malloc_aligned};
use crate::options::{_mi_error_message_fmt, mi_option_is_enabled, mi_option_t};
use crate::page::{
    _mi_malloc_generic_typed, _mi_theap_or_default_typed, MI_MAX_ALLOC_SIZE, mi_bin_from_usable,
    mi_good_size_typed,
};
use crate::prim::{_mi_heap_theap, _mi_theap_default, MI_THEAP_INITASNULL};
use crate::track::{
    MI_TRACK_ENABLED, mi_track_align, mi_track_malloc, mi_track_mem_defined, mi_track_mem_undefined,
};
use crate::types::{
    ENOMEM, MI_DEBUG_UNINIT, MI_MAX_ALIGN_SIZE, MI_PADDING_SIZE, MI_SMALL_SIZE_MAX, MI_STAT,
    mi_block_t, mi_heap_t, mi_page_t, mi_theap_t,
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
    unsafe {
        unsafe extern "C" {
            fn __errno_location() -> *mut c_int;
        }
        *__errno_location() = err;
    }
}

#[cfg(target_vendor = "apple")]
unsafe fn mi_set_errno(err: c_int) {
    unsafe {
        unsafe extern "C" {
            fn __error() -> *mut c_int;
        }
        *__error() = err;
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
unsafe fn mi_set_errno(_err: c_int) {}

unsafe fn mi_theap_strdup_typed(theap: ThreadHeapLocal, s: *const c_char) -> *mut c_char {
    unsafe {
        if s.is_null() {
            return ptr::null_mut();
        }
        let len = _mi_strlen(s);
        if len > MI_MAX_ALLOC_SIZE - 1 {
            return ptr::null_mut();
        }
        let t = mi_theap_malloc(theap.ptr().as_ptr(), len + 1).cast::<c_char>();
        if t.is_null() {
            return ptr::null_mut();
        }
        _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
        *t.add(len) = 0;
        t
    }
}

unsafe fn mi_theap_strdup(theap: *mut mi_theap_t, s: *const c_char) -> *mut c_char {
    unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_strdup_typed(ThreadHeapLocal::new_unchecked(theap), s)
    }
}

unsafe fn mi_theap_strndup_typed(
    theap: ThreadHeapLocal,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    unsafe {
        if s.is_null() {
            return ptr::null_mut();
        }
        let len = _mi_strnlen(s, n);
        if len > MI_MAX_ALLOC_SIZE - 1 {
            return ptr::null_mut();
        }
        let t = mi_theap_malloc(theap.ptr().as_ptr(), len + 1).cast::<c_char>();
        if t.is_null() {
            return ptr::null_mut();
        }
        _mi_memcpy(t.cast::<c_void>(), s.cast::<c_void>(), len);
        *t.add(len) = 0;
        t
    }
}

unsafe fn mi_theap_strndup(theap: *mut mi_theap_t, s: *const c_char, n: usize) -> *mut c_char {
    unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_strndup_typed(ThreadHeapLocal::new_unchecked(theap), s, n)
    }
}

unsafe fn mi_malloc_generic_fallback_from_padded(
    theap: *mut mi_theap_t,
    padded: crate::core_types::PaddedSize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let zeroed = if zero {
            crate::core_types::Zeroed::Yes
        } else {
            crate::core_types::Zeroed::No
        };
        let layout = crate::core_types::LayoutRequest::from_padded_unchecked(padded, zeroed);
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        _mi_malloc_generic_typed(
            theap,
            layout,
            crate::core_types::MallocGenericFlags::new(
                zeroed,
                crate::core_types::MaybeHugeAlignment::none(),
            ),
            usable,
        )
    }
}

pub(crate) unsafe fn _mi_page_malloc_zero_typed(
    theap: *mut mi_theap_t,
    available: crate::core_types::AvailablePage,
    padded: crate::core_types::PaddedSize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let page_ptr = available.page();
        let page = page_ptr.as_ptr();
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
        crate::mi_assert_internal!(!block.is_null() && _mi_ptr_page(block.cast()) == page);
        crate::core_types::UsableOut::new(usable).write_if_present(mi_page_usable_block_size(page));

        let (allocated, _) = available.pop_free_block();
        let block = allocated.block().as_ptr();
        crate::mi_assert_internal!(
            (*page).free.is_null() || _mi_ptr_page((*page).free.cast()) == page
        );
        crate::mi_assert_internal!(
            (*page).block_size < MI_MAX_ALIGN_SIZE
                || _mi_is_aligned(block.cast(), MI_MAX_ALIGN_SIZE)
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
                __mi_stat_counter_increase(
                    ptr::addr_of_mut!((*theap).stats.malloc_normal_count),
                    1,
                );
                let bin = mi_bin_from_usable(usable_size).as_usize();
                __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.malloc_bins[bin]), 1);
                __mi_stat_increase(
                    ptr::addr_of_mut!((*theap).stats.malloc_requested),
                    size - MI_PADDING_SIZE,
                );
            }
        }

        if !zero {
            crate::core_types::BlockTag::write_aligned_to(
                crate::core_types::BlockPtr::new_unchecked(block),
            );
            if cfg!(debug_assertions) && MI_TRACK_ENABLED == 0 {
                memset_nonhuge(
                    crate::core_types::PagePtr::new_unchecked(page),
                    crate::core_types::UserPtr::new_unchecked(block.cast()),
                    usable_size,
                );
            }
        } else if !(*page).free_is_zero {
            _mi_memzero_aligned(block.cast(), bsize);
        } else {
            crate::core_types::BlockTag::write_aligned_to(
                crate::core_types::BlockPtr::new_unchecked(block),
            );
            mi_track_mem_defined(block.cast(), bsize);
        }

        if MI_PADDING != 0 {
            let block_ptr = crate::core_types::BlockPtr::new_unchecked(block);
            let padding = crate::core_types::PaddingPtr::from_block_usable(block_ptr, usable_size);
            let padding_ptr = padding.as_ptr();
            let delta =
                (padding_ptr as usize).wrapping_sub(block as usize) - (size - MI_PADDING_SIZE);
            if cfg!(debug_assertions) {
                crate::mi_assert_internal!(
                    delta <= bsize && bsize >= (size - MI_PADDING_SIZE + delta)
                );
            }
            mi_track_mem_defined(padding_ptr.cast(), crate::core_types::PaddingPtr::SIZE);
            let keys = crate::core_types::PageKeys(mi_page_keys(page));
            padding.write(crate::core_types::Padding::new(
                crate::core_types::PaddingCanary::from_page_block_keys(
                    crate::core_types::PagePtr::new_unchecked(page),
                    block_ptr,
                    keys,
                ),
                crate::core_types::PaddingDelta::for_usable(delta, usable_size).unwrap_or_else(
                    || {
                        crate::mi_assert_internal!(false);
                        crate::core_types::PaddingDelta::new_unchecked(delta as u32)
                    },
                ),
            ));
            if MI_PADDING_CHECK != 0 && !crate::page::mi_page_is_huge(page) {
                let fill = padding_ptr.cast::<u8>().sub(delta);
                let maxpad = delta.min(MI_MAX_ALIGN_SIZE);
                for i in 0..maxpad {
                    *fill.add(i) = crate::types::MI_DEBUG_PADDING;
                }
            }
        }

        block.cast()
    }
}

#[inline]
unsafe fn memset_nonhuge(
    page: PagePtr,
    block: UserPtr,
    usable_size: crate::core_types::UsableSize,
) {
    unsafe {
        if !crate::page::mi_page_is_huge(page.as_ptr()) {
            ptr::write_bytes(block.as_ptr(), MI_DEBUG_UNINIT, usable_size.get());
        }
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_page_malloc_zero(
    theap: *mut mi_theap_t,
    page: *mut mi_page_t,
    size: usize,
    zero: bool,
) -> *mut c_void {
    unsafe {
        let padded = crate::core_types::PaddedSize::new_unchecked(size);
        let active = crate::core_types::ActivePage::new_unchecked(
            crate::core_types::PagePtr::new_unchecked(page),
        );
        let Some(available) = active.available() else {
            return mi_malloc_generic_fallback_from_padded(theap, padded, zero, ptr::null_mut());
        };
        _mi_page_malloc_zero_typed(theap, available, padded, zero, ptr::null_mut())
    }
}

unsafe fn mi_theap_malloc_small_zero_nonnull(
    theap: ThreadHeapLocal,
    size: crate::core_types::RequestedSize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let local_theap = theap;
        let theap = local_theap.ptr().as_ptr();
        let mut size = size.get();
        crate::mi_assert!(!theap.is_null());
        crate::mi_assert!(size <= MI_SMALL_SIZE_MAX);
        if cfg!(debug_assertions) {
            let tid = _mi_thread_id();
            crate::mi_assert!((*(*theap).tld).thread_id == 0 || (*(*theap).tld).thread_id == tid);
        }
        let requested_size = size;
        if mi_unlikely(size == 0) {
            size = crate::core_types::RequestedSize::MIN_ZERO_SIZE;
        }
        if crate::internal::MI_GUARDED != 0
            && mi_unlikely(mi_theap_malloc_use_guarded(
                local_theap,
                crate::core_types::EffectiveSize::new_unchecked(size),
            ))
        {
            return _mi_theap_malloc_guarded(theap, size, zero);
        }

        // Compute the padded size once via the typed `PaddedSize` boundary so both
        // the page lookup and the page-malloc use the same value, instead of
        // open-coding `size + MI_PADDING_SIZE` twice. Uses the branchless unchecked
        // path: `size <= MI_SMALL_SIZE_MAX` (≤ 1024, guarded upstream) and is
        // normalized to non-zero above, so the sum cannot overflow or yield zero.
        let padded = crate::core_types::RequestedSize::new(size).padded_unchecked();
        let page = _mi_theap_get_free_small_page_typed(local_theap, padded);
        let active = crate::core_types::ActivePage::new_unchecked(
            crate::core_types::PagePtr::new_unchecked(page),
        );
        let p = if let Some(available) = active.available() {
            _mi_page_malloc_zero_typed(theap, available, padded, zero, usable)
        } else {
            mi_malloc_generic_fallback_from_padded(theap, padded, zero, usable)
        };
        mi_track_malloc(p, requested_size, zero);

        if cfg!(debug_assertions) && !p.is_null() && zero && requested_size != 0 {
            crate::mi_assert_expensive!(crate::internal::mi_mem_is_zero(p, requested_size));
        }
        p
    }
}

unsafe fn mi_theap_malloc_generic(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let raw_theap = theap.ptr().as_ptr();
        let size = layout.requested().get();
        let zero = flags.zeroed() == crate::core_types::Zeroed::Yes;
        crate::mi_assert!(
            (*(*raw_theap).tld).thread_id == 0 || (*(*raw_theap).tld).thread_id == _mi_thread_id()
        );

        if MI_GUARDED != 0
            && flags.huge_alignment().get() == 0
            && mi_unlikely(mi_theap_malloc_use_guarded(theap, layout.effective()))
        {
            return _mi_theap_malloc_guarded(raw_theap, size, zero);
        }

        let p = _mi_malloc_generic_typed(theap.ptr(), layout, flags, usable);
        mi_track_malloc(p, size, zero);
        if cfg!(debug_assertions) && !p.is_null() && zero {
            crate::mi_assert_expensive!(crate::internal::mi_mem_is_zero(p, size));
        }
        p
    }
}

#[inline(always)]
fn no_alignment() -> crate::core_types::MaybeAlignment {
    unsafe { crate::core_types::MaybeAlignment::new_unchecked(0) }
}

fn maybe_alignment_or_report(alignment: usize) -> Option<crate::core_types::MaybeAlignment> {
    let Some(alignment) = crate::core_types::MaybeAlignment::new(alignment) else {
        _mi_error_message_fmt(
            Errno::EOVERFLOW,
            format_args!(
                "allocation alignment must be zero or a power-of-two (alignment {alignment})\n"
            ),
        );
        return None;
    };
    Some(alignment)
}

fn alignment_or_report(size: usize, alignment: usize) -> Option<crate::core_types::Alignment> {
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
    Some(alignment)
}

fn malloc_layout_or_report(size: usize, zero: bool) -> Option<crate::core_types::LayoutRequest> {
    let layout = if zero {
        crate::core_types::LayoutRequest::zalloc(size)
    } else {
        crate::core_types::LayoutRequest::malloc(size)
    };
    match layout {
        Ok(layout) => Some(layout),
        Err(_) => {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!("allocation request is too large ({size} bytes)\n"),
            );
            None
        }
    }
}

unsafe fn mi_theap_malloc_zero_req(
    theap: *mut mi_theap_t,
    layout: crate::core_types::LayoutRequest,
    huge_alignment: crate::core_types::MaybeAlignment,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let requested = layout.requested();
        let size = requested.get();
        let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;
        if size <= MI_SMALL_SIZE_MAX {
            crate::mi_assert_internal!(huge_alignment.get() == 0);
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_small_zero_nonnull(
                ThreadHeapLocal::new_unchecked(theap),
                requested,
                zero,
                usable,
            )
        } else {
            let flags = crate::core_types::MallocGenericFlags::new(
                layout.zeroed(),
                crate::core_types::MaybeHugeAlignment::new(huge_alignment.get()),
            );
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_generic(ThreadHeapLocal::new_unchecked(theap), layout, flags, usable)
        }
    }
}

unsafe fn mi_theap_malloc_small_zero(
    theap: *mut mi_theap_t,
    size: crate::core_types::RequestedSize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        if MI_THEAP_INITASNULL && !theap.is_null() {
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_small_zero_typed(
                ThreadHeapLocal::new_unchecked(theap),
                size,
                zero,
                usable,
            )
        } else if MI_THEAP_INITASNULL {
            mi_theap_malloc_zero_nonnull_lazy(theap, size.get(), zero, no_alignment(), usable)
        } else {
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_small_zero_typed(
                ThreadHeapLocal::new_unchecked(theap),
                size,
                zero,
                usable,
            )
        }
    }
}

unsafe fn mi_theap_malloc_small_zero_typed(
    theap: ThreadHeapLocal,
    size: crate::core_types::RequestedSize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    unsafe { mi_theap_malloc_small_zero_nonnull(theap, size, zero, usable) }
}

unsafe fn mi_heap_borrowed_theap_local(heap: BorrowedHeap) -> Option<ThreadHeapLocal> {
    unsafe {
        ThreadHeapPtr::new(_mi_heap_theap(heap.ptr().as_ptr()))
            .map(|theap| ThreadHeapLocal::new_unchecked(theap))
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_small(size: usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            false,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            theap,
            crate::core_types::RequestedSize::new(size),
            false,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_malloc_small_typed(heap, size)
    }
}

unsafe fn mi_heap_malloc_small_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_nonnull(theap, size, false, ptr::null_mut())
    }
}

unsafe fn mi_theap_malloc_zero_nonnull_lazy(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    huge_alignment: crate::core_types::MaybeAlignment,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        if let Some(layout) = malloc_layout_or_report(size, zero) {
            mi_theap_malloc_zero_req(theap, layout, huge_alignment, usable)
        } else {
            ptr::null_mut()
        }
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
    unsafe {
        crate::mi_assert!((huge_alignment & 1) == 0);
        let Some(huge_alignment) = maybe_alignment_or_report(huge_alignment) else {
            return ptr::null_mut();
        };
        let Some(layout) = malloc_layout_or_report(size, zero) else {
            return ptr::null_mut();
        };
        if !theap.is_null() || !MI_THEAP_INITASNULL {
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            _mi_theap_malloc_zero_ex_typed(
                ThreadHeapLocal::new_unchecked(theap),
                layout,
                huge_alignment,
                usable,
            )
        } else {
            let flags = crate::core_types::MallocGenericFlags::new(
                layout.zeroed(),
                crate::core_types::MaybeHugeAlignment::new(huge_alignment.get()),
            );
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_generic(ThreadHeapLocal::new_unchecked(theap), layout, flags, usable)
        }
    }
}

unsafe fn _mi_theap_malloc_zero_ex_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    huge_alignment: crate::core_types::MaybeAlignment,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        let requested = layout.requested();
        let size = requested.get();
        let zero = layout.zeroed() == crate::core_types::Zeroed::Yes;
        if size <= MI_SMALL_SIZE_MAX {
            crate::mi_assert_internal!(huge_alignment.get() == 0);
            mi_theap_malloc_small_zero_nonnull(theap, requested, zero, usable)
        } else {
            let flags = crate::core_types::MallocGenericFlags::new(
                layout.zeroed(),
                crate::core_types::MaybeHugeAlignment::new(huge_alignment.get()),
            );
            mi_theap_malloc_generic(theap, layout, flags, usable)
        }
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_malloc_zero(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return _mi_theap_malloc_zero_ex(theap, size, zero, 0, usable);
        }
        let Some(layout) = malloc_layout_or_report(size, zero) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        _mi_theap_malloc_zero_typed(ThreadHeapLocal::new_unchecked(theap), layout, usable)
    }
}

unsafe fn _mi_theap_malloc_zero_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    usable: *mut usize,
) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero_ex_typed(theap, layout, no_alignment(), usable) }
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

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return _mi_theap_malloc_zero(theap, size, false, ptr::null_mut());
        }
        let Some(layout) = malloc_layout_or_report(size, false) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_typed(ThreadHeapLocal::new_unchecked(theap), layout)
    }
}

unsafe fn mi_theap_malloc_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero_typed(theap, layout, ptr::null_mut()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_malloc(size: usize) -> *mut c_void {
    unsafe { mi_theap_malloc(_mi_theap_default(), size) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(size) else {
            return core::ptr::null_mut();
        };
        mi_heap_malloc_typed(heap, layout)
    }
}

unsafe fn mi_heap_malloc_typed(
    heap: BorrowedHeap,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_typed(theap, layout)
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc_small(size: usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            true,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return mi_theap_malloc_small_zero(
                theap,
                crate::core_types::RequestedSize::new(size),
                true,
                ptr::null_mut(),
            );
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_typed(
            ThreadHeapLocal::new_unchecked(theap),
            crate::core_types::RequestedSize::new(size),
            true,
            ptr::null_mut(),
        )
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_zalloc_small_typed(heap, size)
    }
}

unsafe fn mi_heap_zalloc_small_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_malloc_small_zero_nonnull(theap, size, true, ptr::null_mut())
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_zalloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return _mi_theap_malloc_zero(theap, size, true, ptr::null_mut());
        }
        let Some(layout) = malloc_layout_or_report(size, true) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_zalloc_typed(ThreadHeapLocal::new_unchecked(theap), layout)
    }
}

unsafe fn mi_theap_zalloc_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero_typed(theap, layout, ptr::null_mut()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_zalloc(size: usize) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero(_mi_theap_default(), size, true, ptr::null_mut()) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_zalloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Some(layout) = malloc_layout_or_report(size, true) else {
            return ptr::null_mut();
        };
        mi_heap_zalloc_typed(heap, layout)
    }
}

unsafe fn mi_heap_zalloc_typed(
    heap: BorrowedHeap,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_zalloc_typed(theap, layout)
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_calloc(
    theap: *mut mi_theap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return match calloc_request_or_report(count, size) {
                Some(req) => mi_theap_calloc_req_lazy(theap, req),
                None => ptr::null_mut(),
            };
        }
        let Some(req) = calloc_request_or_report(count, size) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_calloc_typed(ThreadHeapLocal::new_unchecked(theap), req)
    }
}

unsafe fn mi_theap_calloc_typed(
    theap: ThreadHeapLocal,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    unsafe { mi_theap_calloc_req(theap, req) }
}

unsafe fn mi_theap_calloc_req(
    theap: ThreadHeapLocal,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    unsafe { mi_theap_zalloc_typed(theap, req.layout()) }
}

unsafe fn mi_theap_calloc_req_lazy(
    theap: *mut mi_theap_t,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    unsafe { mi_theap_zalloc(theap, req.layout().requested().get()) }
}

fn product_size_or_report(count: usize, size: usize) -> Option<usize> {
    let mut total = 0;
    if unsafe { crate::internal::mi_count_size_overflow(count, size, &mut total) } {
        return None;
    }
    Some(total)
}

pub(crate) fn calloc_request_or_report(
    count: usize,
    size: usize,
) -> Option<crate::core_types::CallocRequest> {
    let _ = product_size_or_report(count, size)?;
    crate::core_types::CallocRequest::new(count, size).ok()
}

fn alloc_product_or_report(count: usize, size: usize) -> Option<crate::core_types::AllocProduct> {
    let _ = product_size_or_report(count, size)?;
    crate::core_types::AllocProduct::new(count, size).ok()
}

/// # Safety
/// Returned memory must be released through this allocator.
// Retained as an exported C ABI allocation symbol; release cdylib builds may
// reference it only from external dynamic callers.
#[allow(dead_code)]
#[cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe {
        if let Some(req) = calloc_request_or_report(count, size) {
            let Some(theap) = _mi_theap_or_default_typed(_mi_theap_default()) else {
                return core::ptr::null_mut();
            };
            mi_theap_calloc_req(ThreadHeapLocal::new_unchecked(theap), req)
        } else {
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_calloc(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Some(req) = calloc_request_or_report(count, size) else {
            return core::ptr::null_mut();
        };
        mi_heap_calloc_typed(heap, req)
    }
}

unsafe fn mi_heap_calloc_typed(
    heap: BorrowedHeap,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_calloc_req(theap, req)
    }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc_small(size: usize, usable: *mut usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            false,
            usable,
        )
    }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc_small(size: usize, usable: *mut usize) -> *mut c_void {
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            true,
            usable,
        )
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// If `usable` is non-null it must be writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_umalloc(
    theap: *mut mi_theap_t,
    size: usize,
    usable: *mut usize,
) -> *mut c_void {
    unsafe {
        if theap.is_null() && MI_THEAP_INITASNULL {
            return _mi_theap_malloc_zero_ex(theap, size, false, 0, usable);
        }
        let Some(layout) = malloc_layout_or_report(size, false) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_umalloc_typed(ThreadHeapLocal::new_unchecked(theap), layout, usable)
    }
}

unsafe fn mi_theap_umalloc_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    usable: *mut usize,
) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero_typed(theap, layout, usable) }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc(size: usize, usable: *mut usize) -> *mut c_void {
    unsafe { mi_theap_umalloc(_mi_theap_default(), size, usable) }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc(size: usize, usable: *mut usize) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero_ex(_mi_theap_default(), size, true, 0, usable) }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_ucalloc(count: usize, size: usize, usable: *mut usize) -> *mut c_void {
    unsafe {
        if let Some(req) = calloc_request_or_report(count, size) {
            mi_uzalloc(req.layout().requested().get(), usable)
        } else {
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mi_malloc_good_size(size: usize) -> usize {
    mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()
}

unsafe fn mi_theap_mallocn_req(
    theap: ThreadHeapLocal,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    unsafe { mi_theap_malloc_typed(theap, req.layout()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_mallocn(count: usize, size: usize) -> *mut c_void {
    unsafe {
        if let Some(req) = alloc_product_or_report(count, size) {
            let Some(theap) = _mi_theap_or_default_typed(_mi_theap_default()) else {
                return core::ptr::null_mut();
            };
            mi_theap_mallocn_req(ThreadHeapLocal::new_unchecked(theap), req)
        } else {
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_mallocn(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Some(req) = alloc_product_or_report(count, size) else {
            return core::ptr::null_mut();
        };
        mi_heap_mallocn_typed(heap, req)
    }
}

unsafe fn mi_heap_mallocn_typed(
    heap: BorrowedHeap,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_mallocn_req(theap, req)
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_expand(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
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
}

unsafe fn mi_validate_ptr_page(
    p: *const c_void,
    msg: *const c_char,
) -> crate::core_types::PointerLookup {
    unsafe { crate::free::mi_validate_pointer(p.cast_mut(), msg) }
}

unsafe fn mi_realloc_old_allocation(
    p: *mut c_void,
    msg: *const c_char,
) -> Option<crate::core_types::OldAllocation> {
    unsafe {
        match mi_validate_ptr_page(p, msg) {
            crate::core_types::PointerLookup::Valid(valid) => {
                let user = valid.user();
                let page = valid.page();
                let location = valid.location();
                let block = location.block();
                let usable = crate::free::mi_usable_size_from_validated_location(page, location);
                // SAFETY: `PointerLookup::Valid` proves `user` belongs to `page` and
                // supplies the block start derived by the validation boundary.
                Some(crate::core_types::OldAllocation::from_validated_parts(
                    user, usable, page, block,
                ))
            }
            crate::core_types::PointerLookup::Null
            | crate::core_types::PointerLookup::Invalid { .. } => None,
        }
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
    unsafe {
        let Some(layout) = malloc_layout_or_report(newsize, zero) else {
            return ptr::null_mut();
        };
        if let Some(req) = mi_realloc_request_from_ptr(p, layout, usable_pre) {
            mi_theap_realloc_request_raw(theap, req, usable_pre, usable_post)
        } else {
            ptr::null_mut()
        }
    }
}

#[inline(always)]
unsafe fn mi_theap_realloc_validated_core(
    theap: ThreadHeapLocal,
    old: Option<crate::core_types::OldAllocation>,
    layout: crate::core_types::LayoutRequest,
    zero_new_tail: crate::core_types::Zeroed,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    unsafe {
        let p = old
            .map(|old| old.ptr().as_ptr())
            .unwrap_or(core::ptr::null_mut());
        let new_requested = layout.requested();
        let newsize = new_requested.get();
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
                && mi_page_heap(page) == _mi_theap_heap(theap.ptr().as_ptr()),
        ) {
            crate::mi_assert_internal!(!p.is_null());
            if !usable_post.is_null() {
                *usable_post = mi_page_usable_block_size(page).get();
            }
            return p;
        }

        let newp = mi_theap_umalloc_typed(theap, layout, usable_post);
        if mi_likely(!newp.is_null()) {
            let preserve =
                old.map(|old| crate::core_types::PreserveBytes::new(old.usable(), new_requested));
            let preserved = preserve.map_or(0, |preserve| preserve.get());
            if zero_new_tail == crate::core_types::Zeroed::Yes && newsize > preserved {
                let start = if preserved >= size_of::<isize>() {
                    preserved - size_of::<isize>()
                } else {
                    0
                };
                _mi_memzero(newp.cast::<u8>().add(start).cast(), newsize - start);
            } else if newsize == 0 {
                *newp.cast::<u8>() = 0;
            }
            if mi_likely(!p.is_null()) {
                let Some(preserve) = preserve else {
                    crate::mi_assert_internal!(false);
                    return ptr::null_mut();
                };
                let copysize = preserve.get();
                mi_track_mem_defined(p, copysize);
                _mi_memcpy(newp, p, copysize);
                mi_free(p);
            }
        }
        newp
    }
}

unsafe fn mi_realloc_old_from_ptr(
    p: *mut c_void,
    usable_pre: *mut usize,
) -> Option<Option<crate::core_types::OldAllocation>> {
    unsafe {
        let Some(_user) = crate::core_types::UserPtr::new(p) else {
            return Some(None);
        };
        let Some(old) = mi_realloc_old_allocation(p, c"mi_realloc".as_ptr()) else {
            if !usable_pre.is_null() {
                *usable_pre = 0;
            }
            return None;
        };
        Some(Some(old))
    }
}

unsafe fn mi_realloc_request_from_ptr(
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
    usable_pre: *mut usize,
) -> Option<crate::core_types::ReallocRequest> {
    unsafe {
        match mi_realloc_old_from_ptr(p, usable_pre)? {
            Some(old) => Some(crate::core_types::ReallocRequest::from_existing(
                old,
                layout,
                layout.zeroed(),
            )),
            None => Some(crate::core_types::ReallocRequest::from_null(
                layout,
                layout.zeroed(),
            )),
        }
    }
}

pub(crate) unsafe fn mi_realloc_request_from_user(
    user: crate::core_types::UserPtr,
    layout: crate::core_types::LayoutRequest,
    usable_pre: *mut usize,
) -> Option<crate::core_types::ReallocRequest> {
    unsafe {
        let p = user.as_ptr();
        let Some(old) = mi_realloc_old_allocation(p, c"mi_realloc".as_ptr()) else {
            if !usable_pre.is_null() {
                *usable_pre = 0;
            }
            return None;
        };
        Some(crate::core_types::ReallocRequest::from_existing(
            old,
            layout,
            layout.zeroed(),
        ))
    }
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_realloc(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    unsafe {
        let Some(layout) = malloc_layout_or_report(newsize, false) else {
            return ptr::null_mut();
        };
        let Some(old) = mi_realloc_old_from_ptr(p, ptr::null_mut()) else {
            return ptr::null_mut();
        };
        if theap.is_null() && MI_THEAP_INITASNULL {
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            return mi_theap_realloc_validated_core(
                ThreadHeapLocal::new_unchecked(theap),
                old,
                layout,
                layout.zeroed(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_realloc_validated_core(
            ThreadHeapLocal::new_unchecked(theap),
            old,
            layout,
            layout.zeroed(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    }
}

unsafe fn mi_theap_realloc_typed(
    theap: ThreadHeapLocal,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        mi_theap_realloc_validated_core(
            theap,
            req.old(),
            req.new_layout(),
            req.zero_new_tail(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    }
}

pub(crate) unsafe fn mi_theap_realloc_req(
    theap: *mut mi_theap_t,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe { mi_theap_realloc_request_raw(theap, req, ptr::null_mut(), ptr::null_mut()) }
}

#[inline(always)]
pub(crate) unsafe fn mi_theap_realloc_layout(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    unsafe {
        let Some(old) = mi_realloc_old_from_ptr(p, ptr::null_mut()) else {
            return ptr::null_mut();
        };
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_realloc_validated_core(
            ThreadHeapLocal::new_unchecked(theap),
            old,
            layout,
            layout.zeroed(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    }
}

unsafe fn mi_theap_realloc_request(
    theap: ThreadHeapLocal,
    req: crate::core_types::ReallocRequest,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    unsafe {
        mi_theap_realloc_validated_core(
            theap,
            req.old(),
            req.new_layout(),
            req.zero_new_tail(),
            usable_pre,
            usable_post,
        )
    }
}

unsafe fn mi_theap_realloc_request_raw(
    theap: *mut mi_theap_t,
    req: crate::core_types::ReallocRequest,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_realloc_request(
            ThreadHeapLocal::new_unchecked(theap),
            req,
            usable_pre,
            usable_post,
        )
    }
}

unsafe fn mi_theap_reallocn_req(
    theap: ThreadHeapLocal,
    product: crate::core_types::AllocProduct,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    crate::mi_debug_assert!(
        product.layout().requested().get() == req.new_layout().requested().get()
    );
    unsafe { mi_theap_realloc_typed(theap, req) }
}

unsafe fn mi_theap_reallocn_req_raw(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    unsafe {
        let Some(realloc_req) = mi_realloc_request_from_ptr(p, req.layout(), ptr::null_mut())
        else {
            return ptr::null_mut();
        };
        if theap.is_null() && MI_THEAP_INITASNULL {
            return mi_theap_realloc_request_raw(
                theap,
                realloc_req,
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_reallocn_req(ThreadHeapLocal::new_unchecked(theap), req, realloc_req)
    }
}

unsafe fn mi_theap_reallocf_req(
    theap: ThreadHeapLocal,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        let p = req.ptr().map(|ptr| ptr.as_ptr()).unwrap_or(ptr::null_mut());
        let newp = mi_theap_realloc_typed(theap, req);
        if newp.is_null() && !p.is_null() {
            mi_free(p);
        }
        newp
    }
}

unsafe fn mi_theap_reallocf_req_raw(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    unsafe {
        let Some(req) = mi_realloc_request_from_ptr(p, layout, ptr::null_mut()) else {
            return ptr::null_mut();
        };
        if theap.is_null() && MI_THEAP_INITASNULL {
            let newp = mi_theap_realloc_request_raw(theap, req, ptr::null_mut(), ptr::null_mut());
            if newp.is_null() && !p.is_null() {
                mi_free(p);
            }
            return newp;
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_reallocf_req(ThreadHeapLocal::new_unchecked(theap), req)
    }
}

unsafe fn mi_theap_rezalloc(
    theap: ThreadHeapLocal,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe { mi_theap_realloc_typed(theap, req) }
}

unsafe fn mi_theap_recalloc(
    theap: ThreadHeapLocal,
    req: crate::core_types::CallocRequest,
    realloc_req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe { mi_theap_recalloc_req(theap, req, realloc_req) }
}

unsafe fn mi_theap_recalloc_req(
    theap: ThreadHeapLocal,
    req: crate::core_types::CallocRequest,
    realloc_req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        crate::mi_debug_assert!(
            req.layout().requested().get() == realloc_req.new_layout().requested().get()
        );
        mi_theap_realloc_typed(theap, realloc_req)
    }
}

unsafe fn mi_theap_recalloc_raw(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let Some(req) = calloc_request_or_report(count, size) else {
            return ptr::null_mut();
        };
        let Some(realloc_req) = mi_realloc_request_from_ptr(p, req.layout(), ptr::null_mut())
        else {
            return ptr::null_mut();
        };
        if theap.is_null() && MI_THEAP_INITASNULL {
            return mi_theap_realloc_request_raw(
                theap,
                realloc_req,
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_recalloc(ThreadHeapLocal::new_unchecked(theap), req, realloc_req)
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
            mi_theap_realloc_layout(_mi_theap_default(), p, layout)
        } else {
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocn(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe {
        if let Some(req) = alloc_product_or_report(count, size) {
            mi_theap_reallocn_req_raw(_mi_theap_default(), p, req)
        } else {
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator. Non-null usable outputs
/// must be writable for one `usize` each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_urealloc(
    p: *mut c_void,
    newsize: usize,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    unsafe {
        _mi_theap_realloc_zero(
            _mi_theap_default(),
            p,
            newsize,
            false,
            usable_pre,
            usable_post,
        )
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
            mi_theap_reallocf_req_raw(_mi_theap_default(), p, layout)
        } else {
            if !p.is_null() {
                crate::free::mi_free(p);
            }
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_rezalloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        if let Ok(layout) = crate::core_types::LayoutRequest::zalloc(newsize) {
            mi_theap_realloc_layout(_mi_theap_default(), p, layout)
        } else {
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { mi_theap_recalloc_raw(_mi_theap_default(), p, count, size) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) else {
            return core::ptr::null_mut();
        };
        let Some(req) = mi_realloc_request_from_ptr(p, layout, ptr::null_mut()) else {
            return core::ptr::null_mut();
        };
        mi_heap_realloc_typed(heap, req)
    }
}

unsafe fn mi_heap_realloc_typed(
    heap: BorrowedHeap,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_realloc_typed(theap, req)
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_reallocn(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Some(product) = alloc_product_or_report(count, size) else {
            return ptr::null_mut();
        };
        let Some(req) = mi_realloc_request_from_ptr(p, product.layout(), ptr::null_mut()) else {
            return ptr::null_mut();
        };
        mi_heap_reallocn_typed(heap, product, req)
    }
}

unsafe fn mi_heap_reallocn_typed(
    heap: BorrowedHeap,
    product: crate::core_types::AllocProduct,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_reallocn_req(theap, product, req)
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_reallocf(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) else {
            if !p.is_null() {
                crate::free::mi_free(p);
            }
            return core::ptr::null_mut();
        };
        let Some(req) = mi_realloc_request_from_ptr(p, layout, ptr::null_mut()) else {
            return core::ptr::null_mut();
        };
        mi_heap_reallocf_typed(heap, req)
    }
}

unsafe fn mi_heap_reallocf_typed(
    heap: BorrowedHeap,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_reallocf_req(theap, req)
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Ok(layout) = crate::core_types::LayoutRequest::zalloc(newsize) else {
            return ptr::null_mut();
        };
        let Some(req) = mi_realloc_request_from_ptr(p, layout, ptr::null_mut()) else {
            return ptr::null_mut();
        };
        mi_heap_rezalloc_typed(heap, req)
    }
}

unsafe fn mi_heap_rezalloc_typed(
    heap: BorrowedHeap,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_rezalloc(theap, req)
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. `p` must be null or a pointer previously returned by this
/// allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let Some(req) = calloc_request_or_report(count, size) else {
            return ptr::null_mut();
        };
        let Some(realloc_req) = mi_realloc_request_from_ptr(p, req.layout(), ptr::null_mut())
        else {
            return ptr::null_mut();
        };
        mi_heap_recalloc_typed(heap, req, realloc_req)
    }
}

unsafe fn mi_heap_recalloc_typed(
    heap: BorrowedHeap,
    req: crate::core_types::CallocRequest,
    realloc_req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_recalloc(theap, req, realloc_req)
    }
}

/// # Safety
/// `s` must be null or point to a valid NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strdup(s: *const c_char) -> *mut c_char {
    unsafe { mi_theap_strdup(_mi_theap_default(), s) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `s` must be null or point to a valid NUL-terminated C
/// string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strdup(heap: *mut mi_heap_t, s: *const c_char) -> *mut c_char {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_strdup_typed(heap, s)
    }
}

unsafe fn mi_heap_strdup_typed(heap: BorrowedHeap, s: *const c_char) -> *mut c_char {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_strdup_typed(theap, s)
    }
}

/// # Safety
/// `s` must be null or readable for at most `n` bytes up to a NUL terminator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char {
    unsafe { mi_theap_strndup(_mi_theap_default(), s, n) }
}

/// # Safety
/// `heap` must be a valid heap pointer. `s` must be null or readable for at most `n` bytes up to a
/// NUL terminator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_strndup(
    heap: *mut mi_heap_t,
    s: *const c_char,
    n: usize,
) -> *mut c_char {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_strndup_typed(heap, s, n)
    }
}

unsafe fn mi_heap_strndup_typed(heap: BorrowedHeap, s: *const c_char, n: usize) -> *mut c_char {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_strndup_typed(theap, s, n)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_path_max() -> usize {
    unsafe {
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
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_theap_realpath_typed(
    theap: ThreadHeapLocal,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    unsafe {
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
        let result = mi_theap_strndup_typed(theap, rname, n);
        mi_free(buf.cast::<c_void>());
        result
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must point to a valid NUL-terminated path. If `resolved_name` is non-null it must be
/// writable as required by `realpath`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_realpath(
    theap: *mut mi_theap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    unsafe {
        if !resolved_name.is_null() {
            return realpath(fname, resolved_name);
        }
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_realpath_typed(ThreadHeapLocal::new_unchecked(theap), fname, resolved_name)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `heap` must be a valid heap pointer. `fname` must point to a valid NUL-terminated path. If
/// `resolved_name` is non-null it must be writable as required by `realpath`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realpath(
    heap: *mut mi_heap_t,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        mi_heap_realpath_typed(heap, fname, resolved_name)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_heap_realpath_typed(
    heap: BorrowedHeap,
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_realpath_typed(theap, fname, resolved_name)
    }
}

#[cfg(all(unix, not(target_os = "wasi")))]
/// # Safety
/// `fname` must point to a valid NUL-terminated path. If `resolved_name` is non-null it must be
/// writable as required by `realpath`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_realpath(
    fname: *const c_char,
    resolved_name: *mut c_char,
) -> *mut c_char {
    unsafe { mi_theap_realpath(_mi_theap_default(), fname, resolved_name) }
}

type StdNewHandler = Option<unsafe extern "C" fn()>;

#[cfg(all(unix, not(target_os = "wasi")))]
unsafe fn mi_get_new_handler() -> StdNewHandler {
    unsafe {
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
    unsafe {
        let h = mi_get_new_handler();
        if let Some(h) = h {
            h();
            true
        } else {
            _mi_error_message_fmt(Errno::ENOMEM, format_args!("out of memory in 'new'"));
            if !nothrow {
                abort();
            }
            false
        }
    }
}

#[inline(never)]
pub(crate) unsafe extern "C" fn mi_theap_try_new(
    theap: *mut mi_theap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    unsafe {
        let mut p = mi_theap_malloc(theap, size);
        while p.is_null() && mi_try_new_handler(nothrow) {
            p = mi_theap_malloc(theap, size);
        }
        p
    }
}

#[inline(never)]
unsafe extern "C" fn mi_try_new(size: usize, nothrow: bool) -> *mut c_void {
    unsafe { mi_theap_try_new(_mi_theap_default(), size, nothrow) }
}

#[inline(never)]
unsafe extern "C" fn mi_heap_try_new(
    heap: *mut mi_heap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_try_new_typed(heap, size, nothrow)
    }
}

unsafe fn mi_heap_try_new_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
    nothrow: bool,
) -> *mut c_void {
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_try_new(theap.ptr().as_ptr(), size.get(), nothrow)
    }
}

pub(crate) unsafe extern "C" fn mi_theap_alloc_new(
    theap: *mut mi_theap_t,
    size: usize,
) -> *mut c_void {
    unsafe {
        let p = mi_theap_malloc(theap, size);
        if mi_unlikely(p.is_null()) {
            mi_theap_try_new(theap, size, false)
        } else {
            p
        }
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new(size: usize) -> *mut c_void {
    unsafe { mi_theap_alloc_new(_mi_theap_default(), size) }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new(heap: *mut mi_heap_t, size: usize) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        let size = crate::core_types::RequestedSize::new(size);
        mi_heap_alloc_new_typed(heap, size)
    }
}

unsafe fn mi_heap_alloc_new_typed(
    heap: BorrowedHeap,
    size: crate::core_types::RequestedSize,
) -> *mut c_void {
    unsafe {
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(size.get()) else {
            return mi_heap_try_new_typed(heap, size, false);
        };
        let p = mi_heap_malloc_typed(heap, layout);
        if mi_unlikely(p.is_null()) {
            mi_heap_try_new_typed(heap, size, false)
        } else {
            p
        }
    }
}

unsafe fn mi_theap_alloc_new_n(
    theap: *mut mi_theap_t,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    unsafe { mi_theap_alloc_new(theap, req.total().get()) }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_n(count: usize, size: usize) -> *mut c_void {
    unsafe {
        match alloc_product_or_report(count, size) {
            Some(req) => mi_theap_alloc_new_n(_mi_theap_default(), req),
            None => {
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

/// # Safety
/// `heap` must be a valid heap pointer. Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_alloc_new_n(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return ptr::null_mut();
        };
        match alloc_product_or_report(count, size) {
            Some(req) => mi_heap_alloc_new_n_typed(heap, req),
            None => {
                mi_try_new_handler(false);
                ptr::null_mut()
            }
        }
    }
}

unsafe fn mi_heap_alloc_new_n_typed(
    heap: BorrowedHeap,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    unsafe {
        let p = mi_heap_malloc_typed(heap, req.layout());
        if mi_unlikely(p.is_null()) {
            mi_heap_try_new_typed(heap, req.total(), false)
        } else {
            p
        }
    }
}

/// # Safety
/// Returned memory must be released through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_nothrow(size: usize) -> *mut c_void {
    unsafe {
        let p = mi_malloc(size);
        if mi_unlikely(p.is_null()) {
            mi_try_new(size, true)
        } else {
            p
        }
    }
}

/// # Safety
/// `alignment` must be valid for the aligned allocation API. Returned memory must be released
/// through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_aligned(size: usize, alignment: usize) -> *mut c_void {
    unsafe {
        let Some(alignment) = alignment_or_report(size, alignment) else {
            return ptr::null_mut();
        };
        mi_new_aligned_typed(size, alignment)
    }
}

unsafe fn mi_new_aligned_typed(
    size: usize,
    alignment: crate::core_types::Alignment,
) -> *mut c_void {
    unsafe {
        let mut p: *mut c_void;
        loop {
            p = mi_malloc_aligned(size, alignment.get());
            if !p.is_null() || !mi_try_new_handler(false) {
                return p;
            }
        }
    }
}

/// # Safety
/// `alignment` must be valid for the aligned allocation API. Returned memory must be released
/// through this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_aligned_nothrow(size: usize, alignment: usize) -> *mut c_void {
    unsafe {
        let Some(alignment) = alignment_or_report(size, alignment) else {
            return ptr::null_mut();
        };
        mi_new_aligned_nothrow_typed(size, alignment)
    }
}

unsafe fn mi_new_aligned_nothrow_typed(
    size: usize,
    alignment: crate::core_types::Alignment,
) -> *mut c_void {
    unsafe {
        let mut p: *mut c_void;
        loop {
            p = mi_malloc_aligned(size, alignment.get());
            if !p.is_null() || !mi_try_new_handler(true) {
                return p;
            }
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        let mut q: *mut c_void;
        loop {
            q = mi_realloc(p, newsize);
            if !q.is_null() || !mi_try_new_handler(false) {
                return q;
            }
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_new_reallocn(
    p: *mut c_void,
    newcount: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        match alloc_product_or_report(newcount, size) {
            Some(req) => mi_new_realloc(p, req.total().get()),
            None => {
                mi_try_new_handler(false);
                ptr::null_mut()
            }
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

unsafe fn mi_block_ptr_set_guarded(
    block: crate::core_types::GuardedBlock,
    obj_size: crate::core_types::UsableSize,
) -> Option<UserPtr> {
    unsafe {
        let block = block.block().as_ptr();
        let obj_size = obj_size.get();
        if MI_GUARDED == 0 {
            let _ = obj_size;
            return Some(UserPtr::new_unchecked(block.cast()));
        }
        let page = _mi_ptr_page(block.cast());
        mi_page_set_has_interior_pointers(crate::core_types::PagePtr::new_unchecked(page), true);

        let block_size = mi_page_block_size(page);
        let os_page_size = _mi_os_page_size_typed().get();
        crate::mi_assert_internal!(block_size >= obj_size + os_page_size + size_of::<mi_block_t>());
        if block_size < obj_size + os_page_size + size_of::<mi_block_t>() {
            mi_free(block.cast());
            return None;
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
        Some(UserPtr::new_unchecked(p))
    }
}

pub(crate) unsafe extern "C" fn _mi_theap_malloc_guarded(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
) -> *mut c_void {
    unsafe {
        if MI_GUARDED == 0 {
            let _ = mi_block_ptr_set_guarded
                as unsafe fn(
                    crate::core_types::GuardedBlock,
                    crate::core_types::UsableSize,
                ) -> Option<UserPtr>;
            return _mi_theap_malloc_zero(theap, size, zero, ptr::null_mut());
        }
        if mi_unlikely(size >= MI_MAX_ALLOC_SIZE - MI_PADDING_SIZE) {
            _mi_error_message_fmt(
                Errno::EOVERFLOW,
                format_args!("(guarded) allocation request is too large ({size} bytes)\n"),
            );
            return ptr::null_mut();
        }

        let os_page_size = _mi_os_page_size_typed().get();
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
        let Some(theap_typed) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        let block = _mi_malloc_generic_typed(
            theap_typed,
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
        let block_ptr = crate::core_types::BlockPtr::new_unchecked(block);
        crate::core_types::BlockTag::write_guarded_to(block_ptr);
        let Some(user) = mi_block_ptr_set_guarded(
            crate::core_types::GuardedBlock::after_guarded_detection(block_ptr),
            crate::core_types::UsableSize::new(obj_size),
        ) else {
            return ptr::null_mut();
        };
        let p = user.as_ptr();
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
                __mi_stat_adjust_decrease(
                    ptr::addr_of_mut!((*theap).stats.malloc_requested),
                    req_size,
                );
                __mi_stat_increase(ptr::addr_of_mut!((*theap).stats.malloc_requested), size);
            }
        }
        p
    }
}
