//! Core allocation / free paths + libc overrides

use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::ptr;

use crate::allocator::free::_mi_usable_size_typed;
use crate::allocator::init::_mi_theap_empty_wrong;
use crate::allocator::options::{_mi_error_message_fmt, mi_option_is_enabled, mi_option_t};
use crate::allocator::page::{
    _mi_malloc_generic_typed, _mi_theap_or_default_typed, MI_MAX_ALLOC_SIZE, mi_bin_from_usable,
    mi_good_size_typed,
};
use crate::allocator::tracking::{
    MI_TRACK_ENABLED, mi_track_align, mi_track_malloc, mi_track_mem_defined, mi_track_mem_undefined,
};
use crate::core_types::{BorrowedHeap, Errno, ThreadHeapLocal, ThreadHeapPtr, UserPtr};
use crate::internal::{
    __mi_stat_adjust_decrease, __mi_stat_counter_increase, __mi_stat_increase, _mi_align_up,
    _mi_is_aligned, _mi_memcpy, _mi_memzero, _mi_memzero_aligned, _mi_os_page_size_typed,
    _mi_ptr_page, _mi_theap_get_free_small_page_typed, _mi_theap_heap, _mi_thread_id, MI_GUARDED,
    MI_LARGE_MAX_OBJ_SIZE, mi_likely, mi_page_block_size, mi_page_heap, mi_page_keys,
    mi_page_set_has_interior_pointers, mi_page_slice_start, mi_page_start,
    mi_page_usable_block_size, mi_theap_is_initialized, mi_theap_malloc_use_guarded, mi_unlikely,
};
use crate::mimalloc::mi_free;
use crate::prim::{_mi_heap_theap, _mi_theap_default, MI_THEAP_INITASNULL};
use crate::types::{
    MI_DEBUG_UNINIT, MI_MAX_ALIGN_SIZE, MI_PADDING, MI_PADDING_CHECK, MI_PADDING_SIZE,
    MI_SMALL_SIZE_MAX, MI_STAT, mi_block_t, mi_heap_t, mi_theap_t,
};

mod cxx_new;
mod public_malloc;
mod string_path;
mod zeroed;
#[allow(unused_imports)]
pub use cxx_new::{
    mi_heap_alloc_new, mi_heap_alloc_new_n, mi_new, mi_new_aligned, mi_new_aligned_nothrow,
    mi_new_n, mi_new_nothrow, mi_new_realloc, mi_new_reallocn,
};
#[allow(unused_imports)]
pub(crate) use cxx_new::{mi_theap_alloc_new, mi_theap_try_new};
#[allow(unused_imports)]
pub use public_malloc::{
    mi_heap_malloc, mi_heap_malloc_small, mi_heap_zalloc_small, mi_malloc, mi_malloc_small,
    mi_theap_malloc, mi_theap_malloc_small, mi_theap_zalloc_small, mi_zalloc_small,
};
#[cfg(all(unix, not(target_os = "wasi")))]
#[allow(unused_imports)]
pub use string_path::{mi_heap_realpath, mi_realpath, mi_theap_realpath};
#[allow(unused_imports)]
pub use string_path::{mi_heap_strdup, mi_heap_strndup, mi_strdup, mi_strndup};
#[allow(unused_imports)]
pub use zeroed::{
    mi_calloc, mi_heap_calloc, mi_heap_zalloc, mi_theap_calloc, mi_theap_zalloc, mi_zalloc,
};

pub const _DEFAULT_SOURCE: () = ();

/// # Safety
/// `local_theap` must be the live current-thread heap that owns `available`; its
/// local queues, free list, direct cache, and statistics must be exclusively
/// mutable for this operation, while protocol atomic remote-free publication is
/// allowed. `available` must describe a live initialized page whose free-list
/// head, keys, and `free_is_zero` state are valid. `padded` must be the logical
/// requested size plus exactly `MI_PADDING_SIZE`, and fit that page's block size.
/// The popped block's full usable range and padding tail must be writable within
/// the live page backing. If nonnull, `usable` must be aligned and uniquely
/// writable for one `usize` throughout the call.
pub(crate) unsafe fn _mi_page_malloc_zero_typed(
    local_theap: ThreadHeapLocal,
    available: crate::core_types::AvailablePage,
    padded: crate::core_types::PaddedSize,
    zeroed: crate::core_types::Zeroed,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller couples the available page to the current-thread heap,
    // keeps its free list, keys, backing, and local stats exclusively accessible,
    // and supplies a fitting padded size plus a valid optional output slot for
    // this coherent pop, accounting, initialization, and padding epoch.
    unsafe {
        let theap = local_theap.ptr().as_ptr();
        let page_ptr = available.page();
        let page = page_ptr.as_ptr();
        let size = padded.get();
        let block_size = page_ptr.block_size_raw();
        if block_size != 0 {
            crate::internal::mi_assert_internal!(crate::internal::mi_page_block_size(page) >= size);
            crate::internal::mi_assert_internal!(_mi_is_aligned(
                mi_page_slice_start(page).cast(),
                crate::allocator::page::MI_OS_PAGE_SIZE
            ));
            crate::internal::mi_assert_internal!(_mi_ptr_page(mi_page_start(page).cast()) == page);
        }

        let block = page_ptr.free().as_raw();
        crate::internal::mi_assert_internal!(
            !block.is_null() && _mi_ptr_page(block.cast()) == page
        );
        crate::core_types::UsableOut::new(usable).write_if_present(mi_page_usable_block_size(page));

        let (allocated, _) = available.pop_free_block();
        let block = allocated.into_ptr().as_ptr();
        let free = page_ptr.free().as_raw();
        crate::internal::mi_assert_internal!(free.is_null() || _mi_ptr_page(free.cast()) == page);
        crate::internal::mi_assert_internal!(
            block_size < MI_MAX_ALIGN_SIZE || _mi_is_aligned(block.cast(), MI_MAX_ALIGN_SIZE)
        );

        if cfg!(debug_assertions) && (*page).free_is_zero && size > size_of::<mi_block_t>() {
            crate::internal::mi_assert_expensive!(crate::internal::mi_mem_is_zero(
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

        if zeroed == crate::core_types::Zeroed::No {
            crate::core_types::BlockTag::write_aligned_to(
                crate::core_types::BlockPtr::new_unchecked(block),
            );
            if cfg!(debug_assertions)
                && MI_TRACK_ENABLED == 0
                && !crate::allocator::page::mi_page_is_huge(page)
            {
                // SAFETY: the popped non-huge block is exclusively writable for
                // its full usable size, which remains within the live page backing;
                // tracking marked it undefined immediately above.
                ptr::write_bytes(block.cast::<u8>(), MI_DEBUG_UNINIT, usable_size.get());
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
                crate::internal::mi_assert_internal!(
                    delta <= bsize && bsize >= (size - MI_PADDING_SIZE + delta)
                );
            }
            mi_track_mem_defined(padding_ptr.cast(), crate::core_types::PaddingPtr::SIZE);
            let keys = crate::core_types::PageKeys::from_raw(mi_page_keys(page));
            padding.write(crate::core_types::Padding::new(
                crate::core_types::PaddingCanary::from_page_block_keys(
                    crate::core_types::PagePtr::new_unchecked(page),
                    block_ptr,
                    keys,
                ),
                crate::core_types::PaddingDelta::for_usable(delta, usable_size).unwrap_or_else(
                    || {
                        crate::internal::mi_assert_internal!(false);
                        crate::core_types::PaddingDelta::new_unchecked(delta as u32)
                    },
                ),
            ));
            if MI_PADDING_CHECK != 0 && !crate::allocator::page::mi_page_is_huge(page) {
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

/// # Safety
/// `theap` must be a live current-thread heap whose TLD, page queues, and small
/// page metadata remain initialized for this allocation. The caller must hold
/// exclusive current-thread mutation rights over its direct-page cache, local
/// page queues and free lists, and local statistics; protocol atomic remote-free
/// publication is permitted. `size` must not exceed `MI_SMALL_SIZE_MAX`. If
/// nonnull, `usable` must be aligned and uniquely writable for one `usize` for
/// the duration of the call.
unsafe fn mi_theap_malloc_small_zero_typed(
    theap: ThreadHeapLocal,
    size: crate::core_types::RequestedSize,
    zeroed: crate::core_types::Zeroed,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the current-thread heap, its TLD, queues, and
    // selected page metadata live and stable; the bounded requested size yields
    // a fitting padded size, and `usable` satisfies its optional out-pointer
    // contract throughout the allocation and tracking epoch.
    unsafe {
        let local_theap = theap;
        let theap = local_theap.ptr().as_ptr();
        let mut size = size.get();
        crate::internal::mi_assert!(!theap.is_null());
        crate::internal::mi_assert!(size <= MI_SMALL_SIZE_MAX);
        if cfg!(debug_assertions) {
            let tid = _mi_thread_id();
            crate::internal::mi_assert!(
                (*(*theap).tld).thread_id == 0 || (*(*theap).tld).thread_id == tid
            );
        }
        let requested_size = size;
        if mi_unlikely(size == 0) {
            size = crate::core_types::RequestedSize::MIN_ZERO_SIZE;
        }
        let zero = zeroed.as_bool();
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
        let available = _mi_theap_get_free_small_page_typed(local_theap, padded);
        let p = if let Some(available) = available {
            _mi_page_malloc_zero_typed(local_theap, available, padded, zeroed, usable)
        } else {
            // SAFETY: `padded` was derived immediately above from the normalized
            // logical request and includes exactly the allocator padding size.
            let layout = crate::core_types::LayoutRequest::from_padded_unchecked(padded, zeroed);
            _mi_malloc_generic_typed(
                local_theap.ptr(),
                layout,
                crate::core_types::MallocGenericFlags::new(
                    zeroed,
                    crate::core_types::MaybeHugeAlignment::none(),
                ),
                usable,
            )
        };
        mi_track_malloc(p, requested_size, zero);

        if cfg!(debug_assertions) && !p.is_null() && zero && requested_size != 0 {
            crate::internal::mi_assert_expensive!(crate::internal::mi_mem_is_zero(
                p,
                requested_size
            ));
        }
        p
    }
}

/// # Safety
/// `theap` must identify a live current-thread heap. The caller must have
/// exclusive mutation rights over its local TLD state, direct cache, page queues,
/// free lists, and statistics; protocol atomic remote-free publication is
/// permitted. `layout` and `flags` must agree on zeroing, and any nonzero huge
/// alignment in `flags` must be valid for the request. If nonnull, `usable` must
/// be aligned and uniquely writable for one `usize` throughout the call.
unsafe fn mi_theap_malloc_generic(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    flags: crate::core_types::MallocGenericFlags,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the current-thread heap's local allocation and
    // collection state exclusively mutable, supplies matching layout/flags, and
    // keeps the optional output writable through guarded/generic dispatch and
    // allocation tracking.
    unsafe {
        let raw_theap = theap.ptr().as_ptr();
        let size = layout.requested().get();
        let zero = flags.zeroed() == crate::core_types::Zeroed::Yes;
        crate::internal::mi_assert!(
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
            crate::internal::mi_assert_expensive!(crate::internal::mi_mem_is_zero(p, size));
        }
        p
    }
}

#[inline(always)]
fn no_alignment() -> crate::core_types::MaybeAlignment {
    // SAFETY: zero is the defined sentinel for an ordinary allocation with no
    // requested optional alignment.
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
    let layout = match crate::core_types::Zeroed::from_bool(zero) {
        crate::core_types::Zeroed::No => crate::core_types::LayoutRequest::malloc(size),
        crate::core_types::Zeroed::Yes => crate::core_types::LayoutRequest::zalloc(size),
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

/// # Safety
/// Unless lazy null-thread-heap initialization is enabled and `theap` is null,
/// `theap` must identify the current thread's live initialized heap. Its TLD,
/// queues, and page metadata must remain live without teardown. The caller must
/// have exclusive mutation rights over its direct-page cache, local page queues
/// and free lists, and local statistics; protocol atomic remote-free publication
/// is permitted. `size` must not exceed `MI_SMALL_SIZE_MAX`. If nonnull, `usable`
/// must be aligned and uniquely writable for one `usize` for the duration of the
/// call.
unsafe fn mi_theap_malloc_small_zero(
    theap: *mut mi_theap_t,
    size: crate::core_types::RequestedSize,
    zeroed: crate::core_types::Zeroed,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller supplies the current-thread heap epoch and optional
    // writable output; lazy null initialization is completed before either the
    // bounded small-page path or generic allocation path uses heap metadata.
    unsafe {
        if MI_THEAP_INITASNULL && !theap.is_null() {
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_small_zero_typed(
                ThreadHeapLocal::new_unchecked(theap),
                size,
                zeroed,
                usable,
            )
        } else if MI_THEAP_INITASNULL {
            let layout = match zeroed {
                crate::core_types::Zeroed::No => {
                    crate::core_types::LayoutRequest::malloc(size.get())
                }
                crate::core_types::Zeroed::Yes => {
                    crate::core_types::LayoutRequest::zalloc(size.get())
                }
            };
            let Ok(layout) = layout else {
                _mi_error_message_fmt(
                    Errno::EOVERFLOW,
                    format_args!("allocation request is too large ({} bytes)\n", size.get()),
                );
                return ptr::null_mut();
            };
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            let theap = ThreadHeapLocal::new_unchecked(theap);
            if size.get() <= MI_SMALL_SIZE_MAX {
                mi_theap_malloc_small_zero_typed(theap, size, zeroed, usable)
            } else {
                let flags = crate::core_types::MallocGenericFlags::new(
                    zeroed,
                    crate::core_types::MaybeHugeAlignment::new(0),
                );
                mi_theap_malloc_generic(theap, layout, flags, usable)
            }
        } else {
            let Some(theap) = _mi_theap_or_default_typed(theap) else {
                return ptr::null_mut();
            };
            mi_theap_malloc_small_zero_typed(
                ThreadHeapLocal::new_unchecked(theap),
                size,
                zeroed,
                usable,
            )
        }
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread. Its
/// thread-heap association must remain stable without concurrent reassignment or
/// teardown for the returned local witness's use.
unsafe fn mi_heap_borrowed_theap_local(heap: BorrowedHeap) -> Option<ThreadHeapLocal> {
    // SAFETY: the caller keeps the heap and association live and current-thread
    // owned while the raw thread heap is projected and classified.
    unsafe {
        let theap = _mi_heap_theap(heap.ptr().as_ptr());
        if ptr::eq(theap, ptr::addr_of!(_mi_theap_empty_wrong).cast_mut()) {
            return None;
        }
        ThreadHeapPtr::new(theap).map(|theap| ThreadHeapLocal::new_unchecked(theap))
    }
}

/// # Safety
/// `theap` may be null or an allocator default/lazy sentinel supported by the
/// current configuration. Otherwise it must identify a live initialized
/// current-thread heap whose local allocation state is exclusively mutable for
/// this call; protocol atomic remote-free publication is permitted. If nonnull,
/// `usable` must be aligned and uniquely writable for one `usize` throughout the
/// call. `huge_alignment` may be arbitrary and is validated before allocation.
#[inline(always)]
pub(crate) unsafe extern "C" fn _mi_theap_malloc_zero_ex(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    huge_alignment: usize,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps any non-sentinel heap live, current-thread owned,
    // and locally exclusive, and keeps the optional output writable. The
    // debug-only evenness assertion preserves the existing internal diagnostic;
    // the typed conversion rejects invalid scalar alignments before dispatch.
    unsafe {
        crate::internal::mi_assert!((huge_alignment & 1) == 0);
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

/// # Safety
/// `theap` must identify a live current-thread heap. The caller must have
/// exclusive mutation rights over its local TLD state, direct cache, page queues,
/// free lists, and statistics; protocol atomic remote-free publication is
/// permitted. For a small request `huge_alignment` must be zero; for a generic
/// request, any nonzero value must be a validated alignment for that request. If
/// nonnull, `usable` must be aligned and uniquely writable for one `usize`
/// throughout the call.
unsafe fn _mi_theap_malloc_zero_ex_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    huge_alignment: crate::core_types::MaybeAlignment,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the current-thread local allocation epoch
    // exclusive, supplies an alignment valid for the selected small or generic
    // branch, and keeps the optional result slot writable through dispatch.
    unsafe {
        let requested = layout.requested();
        let size = requested.get();
        if size <= MI_SMALL_SIZE_MAX {
            crate::internal::mi_assert_internal!(huge_alignment.get() == 0);
            mi_theap_malloc_small_zero_typed(theap, requested, layout.zeroed(), usable)
        } else {
            let flags = crate::core_types::MallocGenericFlags::new(
                layout.zeroed(),
                crate::core_types::MaybeHugeAlignment::new(huge_alignment.get()),
            );
            mi_theap_malloc_generic(theap, layout, flags, usable)
        }
    }
}

/// # Safety
/// `theap` may be null or an allocator default/lazy sentinel supported by the
/// current configuration. Otherwise it must identify a live initialized
/// current-thread heap whose local allocation state is exclusively mutable for
/// this call; protocol atomic remote-free publication is permitted. If nonnull,
/// `usable` must be aligned and uniquely writable for one `usize` throughout the
/// call.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_malloc_zero(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps any non-sentinel heap live, current-thread owned,
    // and locally exclusive, and keeps the optional output writable through
    // lazy/default resolution, layout validation, and unaligned dispatch.
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

/// # Safety
/// `theap` must be a live current-thread heap with exclusive mutation rights
/// over its local allocation state; protocol atomic remote-free publication is
/// permitted. If nonnull, `usable` must be aligned and uniquely writable for one
/// `usize` throughout the call.
unsafe fn _mi_theap_malloc_zero_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the local heap epoch exclusive and the optional
    // result slot writable; the zero alignment selects ordinary unaligned
    // small-or-generic allocation for the validated layout.
    unsafe { _mi_theap_malloc_zero_ex_typed(theap, layout, no_alignment(), usable) }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// retain a stable association to its local thread heap. The caller must have
/// exclusive mutation rights over local allocation state during the call;
/// protocol atomic remote-free publication is permitted.
unsafe fn mi_heap_malloc_typed(
    heap: BorrowedHeap,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    // SAFETY: the caller keeps the heap live, current-thread owned, and locally
    // exclusive while its thread heap is projected and used for allocation.
    unsafe {
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        _mi_theap_malloc_zero_typed(theap, layout, ptr::null_mut())
    }
}

fn product_size_or_report(
    count: crate::core_types::ItemCount,
    size: crate::core_types::RequestedSize,
) -> Option<crate::core_types::RequestedSize> {
    let count = count.get();
    let size = size.get();
    let total = if count == 1 {
        size
    } else if let Some(total) = count.checked_mul(size) {
        total
    } else {
        _mi_error_message_fmt(
            Errno::EOVERFLOW,
            format_args!("allocation request is too large ({count} * {size} bytes)\n"),
        );
        return None;
    };
    Some(crate::core_types::RequestedSize::new(total))
}

pub(crate) fn calloc_request_or_report(
    count: usize,
    size: usize,
) -> Option<crate::core_types::CallocRequest> {
    let count = crate::core_types::ItemCount::new(count);
    let size = crate::core_types::RequestedSize::new(size);
    let total = product_size_or_report(count, size)?;
    crate::core_types::CallocRequest::from_checked_total(total.get()).ok()
}

fn alloc_product_or_report(count: usize, size: usize) -> Option<crate::core_types::AllocProduct> {
    let count = crate::core_types::ItemCount::new(count);
    let size = crate::core_types::RequestedSize::new(size);
    let total = product_size_or_report(count, size)?;
    crate::core_types::AllocProduct::from_checked_total(total.get()).ok()
}

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. If nonnull, `usable` must be
/// aligned and uniquely writable for one `usize` throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc_small(size: usize, usable: *mut usize) -> *mut c_void {
    // SAFETY: the caller-provided bound/output contract and allocator lifecycle
    // keep the default heap's local allocation state exclusively owned and the
    // optional result slot writable through the small allocation.
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::No,
            usable,
        )
    }
}

/// # Safety
/// `size` must not exceed `MI_SMALL_SIZE_MAX`. If nonnull, `usable` must be
/// aligned and uniquely writable for one `usize` throughout the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_uzalloc_small(size: usize, usable: *mut usize) -> *mut c_void {
    // SAFETY: the caller-provided bound/output contract and allocator lifecycle
    // keep the default heap's local allocation state exclusively owned and the
    // optional result slot writable through the zeroing small allocation.
    unsafe {
        mi_theap_malloc_small_zero(
            _mi_theap_default(),
            crate::core_types::RequestedSize::new(size),
            crate::core_types::Zeroed::Yes,
            usable,
        )
    }
}

/// # Safety
/// `theap` may be null only when lazy initialization is enabled; otherwise it
/// must identify a live initialized current-thread heap. The caller must have
/// exclusive mutation rights over its local allocation state, while protocol
/// atomic remote-free publication is permitted. If nonnull, `usable` must be
/// aligned and uniquely writable for one `usize` throughout the call.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_theap_umalloc(
    theap: *mut mi_theap_t,
    size: usize,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the raw current-thread heap and optional output
    // live through lazy handling, layout validation, and typed allocation; the
    // terminal dispatch writes the usable size at most once.
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

/// # Safety
/// `theap` must identify a live current-thread heap. The caller must have
/// exclusive mutation rights over its local allocation state; protocol atomic
/// remote-free publication is permitted. If nonnull, `usable` must be aligned
/// and uniquely writable for one `usize` throughout the call.
unsafe fn mi_theap_umalloc_typed(
    theap: ThreadHeapLocal,
    layout: crate::core_types::LayoutRequest,
    usable: *mut usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the current-thread local allocation state
    // exclusive and the optional output writable; `layout` is already validated
    // and the outlined dispatch retains those facts for the synchronous call.
    unsafe { _mi_theap_malloc_zero_typed(theap, layout, usable) }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
// SAFETY: usable-size allocation exports preserve nullable usable out-pointers and typed layout routing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_umalloc(size: usize, usable: *mut usize) -> *mut c_void {
    unsafe { mi_theap_umalloc(_mi_theap_default(), size, usable) }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
// SAFETY: usable-size allocation exports preserve nullable usable out-pointers and typed layout routing.
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
pub unsafe extern "C" fn mi_uzalloc(size: usize, usable: *mut usize) -> *mut c_void {
    unsafe { _mi_theap_malloc_zero_ex(_mi_theap_default(), size, true, 0, usable) }
}

/// # Safety
/// If `usable` is non-null it must be writable for one `usize`.
// SAFETY: usable-size allocation exports preserve nullable usable out-pointers and typed layout routing.
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

// SAFETY: usable-size allocation exports preserve nullable usable out-pointers and typed layout routing.
#[unsafe(no_mangle)]
pub extern "C" fn mi_malloc_good_size(size: usize) -> usize {
    mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()
}

/// A nonnull result is allocator-owned and may be released with `mi_free`.
#[unsafe(no_mangle)]
pub extern "C" fn mi_mallocn(count: usize, size: usize) -> *mut c_void {
    if let Some(req) = alloc_product_or_report(count, size) {
        // SAFETY: allocator lifecycle keeps the current thread's default heap
        // live and its local allocation state exclusively owned while this
        // synchronous checked-product allocation runs.
        unsafe {
            let Some(theap) = _mi_theap_or_default_typed(_mi_theap_default()) else {
                return core::ptr::null_mut();
            };
            _mi_theap_malloc_zero_typed(
                ThreadHeapLocal::new_unchecked(theap),
                req.layout(),
                ptr::null_mut(),
            )
        }
    } else {
        core::ptr::null_mut()
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown. The caller must have exclusive
/// mutation rights over its local allocation state; protocol atomic remote-free
/// publication is permitted. A nonnull result is allocator-owned and may be
/// released with `mi_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_mallocn(
    heap: *mut mi_heap_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the heap live and current-thread owned through
    // raw classification, checked-product validation, and allocation dispatch.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Some(req) = alloc_product_or_report(count, size) else {
            return core::ptr::null_mut();
        };
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        _mi_theap_malloc_zero_typed(theap, req.layout(), ptr::null_mut())
    }
}

/// # Safety
/// `p` must be null or identify an allocation previously returned by this
/// allocator, not yet freed, and valid for this operation. Its page and
/// allocation metadata must remain stable for this call, with no concurrent
/// free or reallocation of `p`.
// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
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
    // SAFETY: the caller supplies a live compatible allocator allocation;
    // its stable metadata permits deriving the owning page from `p`.
    let page = unsafe { _mi_ptr_page(p) };
    if page.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `page` was derived from this live allocation, and the caller
    // keeps their matching allocation metadata stable for the operation.
    let size = unsafe { _mi_usable_size_typed(p, page) };
    if newsize > size.get() {
        return ptr::null_mut();
    }
    p
}

/// # Safety
/// `p` must identify a live nonnull allocation returned by this allocator. Its
/// page and block metadata must remain stable, with no concurrent free or
/// realloc, for as long as the returned witness is used. `msg` must point to a
/// live NUL-terminated diagnostic string for this call.
unsafe fn mi_realloc_old_allocation(
    p: *mut c_void,
    msg: *const c_char,
) -> Option<crate::core_types::OldAllocation> {
    // SAFETY: the caller keeps the nonnull allocation and its metadata stable;
    // `msg` remains a readable NUL-terminated diagnostic string while pointer
    // validation derives the matching page and block location.
    unsafe {
        match crate::allocator::free::mi_validate_pointer(p, msg) {
            crate::core_types::PointerLookup::Valid(valid) => {
                let (user, page, location) = valid.into_parts();
                let usable =
                    crate::allocator::free::mi_usable_size_from_validated_location(page, location);
                // SAFETY: `PointerLookup::Valid` proves `user` belongs to `page` and
                // supplies the block start derived by the validation boundary.
                Some(crate::core_types::OldAllocation::from_validated_parts(
                    user, usable, page,
                ))
            }
            crate::core_types::PointerLookup::Null | crate::core_types::PointerLookup::Invalid => {
                None
            }
        }
    }
}

/// # Safety
/// `theap` may be null only where the supported lazy/default heap protocol accepts it; otherwise
/// it must resolve to a live initialized heap owned by the current thread, with exclusive local
/// allocation mutation and no concurrent teardown or non-protocol mutation. Protocol atomic
/// remote-free publication is permitted. `p` must be null or identify a live allocator allocation
/// whose page and block metadata remain stable, without concurrent free or realloc, until this
/// operation returns or consumes it. The internally constructed request does not extend those
/// lifetimes. Each nonnull `usable_pre` and `usable_post` must be aligned and writable for one
/// `usize` when accessed and must not overlap allocator metadata. They may mutually alias or point
/// within the old user allocation while that storage remains live at the access.
///
/// Invalid layout or request validation returns null without consuming an old allocation. Heap
/// localization or allocation failure also leaves it live. An in-place success returns the
/// still-live old allocation; a moved success frees the old allocation inside realloc and returns
/// the new allocation. Layout failure leaves both outputs untouched. Request-validation failure
/// clears only a nonnull `usable_pre`; after a valid request, heap-localization failure leaves both
/// outputs untouched. All output writes precede a moved-success release of old user storage.
pub(crate) unsafe extern "C" fn _mi_theap_realloc_zero(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
    zero: bool,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    let Some(layout) = malloc_layout_or_report(newsize, zero) else {
        return ptr::null_mut();
    };
    // SAFETY: the caller keeps the optional old allocation metadata stable through disposition.
    // The optional pre-size output is writable for one usize at this validation access and does
    // not overlap allocator metadata.
    if let Some(req) = unsafe { mi_realloc_request_from_ptr(p, layout, usable_pre) } {
        // SAFETY: the raw heap satisfies the supported lazy/default or live current-thread
        // protocol, with exclusive local mutation. The request's old allocation remains live and
        // stable through dispatch, and both optional outputs are valid at each access before any
        // moved-success free.
        unsafe { mi_theap_realloc_request_raw(theap, req, usable_pre, usable_post) }
    } else {
        ptr::null_mut()
    }
}

/// # Safety
/// `theap` must identify a live current-thread heap whose local allocation state
/// is exclusively mutable for this call; protocol atomic remote-free publication
/// is permitted. If `old` is present, it must identify a live allocator
/// allocation whose page and block metadata remain stable with no concurrent free
/// or realloc until this call returns or frees it. The Copy witness does not
/// extend those lifetimes. `zero_new_tail` must agree with `layout`. Each nonnull
/// usable-size output must be valid, aligned, and writable for one `usize` when
/// accessed, and must not overlap allocator metadata. The outputs may alias each
/// other or lie within the live old allocation's user storage because every
/// output write occurs before a moved-success release of that storage.
///
/// On allocation failure, an existing old allocation remains live. On in-place
/// success it remains the returned allocation. On moved success this call copies
/// the preserved prefix and frees the old allocation before returning the new one.
#[inline(always)]
unsafe fn mi_theap_realloc_validated_core(
    theap: ThreadHeapLocal,
    old: Option<crate::core_types::OldAllocation>,
    layout: crate::core_types::LayoutRequest,
    zero_new_tail: crate::core_types::Zeroed,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    let p = old
        .map(|old| old.ptr().as_ptr())
        .unwrap_or(core::ptr::null_mut());
    let new_requested = layout.requested();
    let newsize = new_requested.get();

    // SAFETY: the caller keeps any old allocation and its page metadata live and
    // stable. Both optional outputs are valid when written; these writes precede
    // every possible old-user-storage release. The current-thread heap remains
    // exclusively available for the same-heap in-place decision.
    unsafe {
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
            crate::internal::mi_assert_internal!(!p.is_null());
            if !usable_post.is_null() {
                *usable_post = mi_page_usable_block_size(page).get();
            }
            return p;
        }
    }

    // SAFETY: the caller keeps the current-thread heap's local allocation state
    // exclusive, `layout` is validated, and `usable_post` is valid when accessed.
    let newp = unsafe { mi_theap_umalloc_typed(theap, layout, usable_post) };
    if mi_likely(!newp.is_null()) {
        // SAFETY: `newp` is the newly allocated writable block. The caller keeps
        // any old allocation readable and stable until this transaction finishes;
        // the validated sizes bound tail initialization and the preserved copy.
        // Tracking is restored before the terminal old-allocation free.
        unsafe {
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
                    crate::internal::mi_assert_internal!(false);
                    return ptr::null_mut();
                };
                let copysize = preserve.get();
                mi_track_mem_defined(p, copysize);
                _mi_memcpy(newp, p, copysize);
                mi_free(p);
            }
        }
    }
    newp
}

/// # Safety
/// If `p` is nonnull, it must identify a live allocation returned by this allocator. Its page and
/// block metadata must remain stable, without concurrent free or realloc, for as long as the
/// returned `OldAllocation` is used; copying that witness does not extend this lifetime. If
/// nonnull, `usable_pre` must be aligned and writable for one `usize` when accessed and must not
/// overlap allocator metadata. It may point within the old user allocation if that storage is
/// still live at the access.
unsafe fn mi_realloc_old_from_ptr(
    p: *mut c_void,
    usable_pre: *mut usize,
) -> Option<Option<crate::core_types::OldAllocation>> {
    let Some(_user) = crate::core_types::UserPtr::new(p) else {
        return Some(None);
    };
    // SAFETY: the caller keeps this nonnull allocator allocation and its page/block metadata
    // stable through use of the returned witness. The optional failure output is writable at the
    // access and does not overlap allocator metadata.
    unsafe {
        let Some(old) = mi_realloc_old_allocation(p, c"mi_realloc".as_ptr()) else {
            if !usable_pre.is_null() {
                *usable_pre = 0;
            }
            return None;
        };
        Some(Some(old))
    }
}

/// # Safety
/// If `p` is nonnull, it must identify a live allocation returned by this allocator. Its page and
/// block metadata must remain stable, without concurrent free or realloc, for as long as the
/// returned request is used; the returned request does not extend this lifetime. If nonnull,
/// `usable_pre` must be aligned and writable for one `usize` when accessed and must not overlap
/// allocator metadata. It may point within the old user allocation if that storage is still live
/// at the access.
unsafe fn mi_realloc_request_from_ptr(
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
    usable_pre: *mut usize,
) -> Option<crate::core_types::ReallocRequest> {
    // SAFETY: the caller supplies the allocator allocation lifetime and metadata stability needed
    // by the returned request, and any failure output is valid at its access.
    let old = unsafe { mi_realloc_old_from_ptr(p, usable_pre)? };
    match old {
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

/// # Safety
/// `user` must identify a live allocation returned by this allocator. Its page and block metadata
/// must remain stable, without concurrent free or realloc, for as long as the returned request is
/// used; the returned request does not extend this lifetime. If nonnull, `usable_pre` must be
/// aligned and writable for one `usize` when accessed and must not overlap allocator metadata. It
/// may point within the user allocation if that storage is still live at the access.
pub(crate) unsafe fn mi_realloc_request_from_user(
    user: crate::core_types::UserPtr,
    layout: crate::core_types::LayoutRequest,
    usable_pre: *mut usize,
) -> Option<crate::core_types::ReallocRequest> {
    let p = user.as_ptr();
    // SAFETY: the caller keeps the typed nonnull user allocation and its page/block metadata live
    // and stable through use of the returned request. The optional failure output is writable at
    // the access and does not overlap allocator metadata.
    let old = unsafe {
        let Some(old) = mi_realloc_old_allocation(p, c"mi_realloc".as_ptr()) else {
            if !usable_pre.is_null() {
                *usable_pre = 0;
            }
            return None;
        };
        old
    };
    Some(crate::core_types::ReallocRequest::from_existing(
        old,
        layout,
        layout.zeroed(),
    ))
}

/// # Safety
/// `theap` must be null only when lazy thread-heap initialization is enabled, or otherwise valid.
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY: thread-heap realloc entrypoints validate old allocations and typed layout requests before core dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_realloc(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    // SAFETY: thread-heap realloc entrypoints validate old allocations and typed layout requests before core dispatch.
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
    // SAFETY: thread-heap realloc entrypoints validate old allocations and typed layout requests before core dispatch.
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
    // SAFETY: thread-heap realloc entrypoints validate old allocations and typed layout requests before core dispatch.
    unsafe { mi_theap_realloc_request_raw(theap, req, ptr::null_mut(), ptr::null_mut()) }
}

#[inline(always)]
pub(crate) unsafe fn mi_theap_realloc_layout(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    // SAFETY: thread-heap realloc entrypoints validate old allocations and typed layout requests before core dispatch.
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

unsafe fn mi_theap_realloc_request_raw(
    theap: *mut mi_theap_t,
    req: crate::core_types::ReallocRequest,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    // SAFETY: thread-heap realloc entrypoints validate old allocations and typed layout requests before core dispatch.
    unsafe {
        let Some(theap) = _mi_theap_or_default_typed(theap) else {
            return ptr::null_mut();
        };
        mi_theap_realloc_validated_core(
            ThreadHeapLocal::new_unchecked(theap),
            req.old(),
            req.new_layout(),
            req.zero_new_tail(),
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
    crate::internal::mi_debug_assert!(
        product.layout().requested().get() == req.new_layout().requested().get()
    );
    // SAFETY: reallocn paths validate checked products and preserve raw pointer ownership semantics.
    unsafe { mi_theap_realloc_typed(theap, req) }
}

unsafe fn mi_theap_reallocn_req_raw(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    req: crate::core_types::AllocProduct,
) -> *mut c_void {
    // SAFETY: reallocn paths validate checked products and preserve raw pointer ownership semantics.
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

/// # Safety
/// `theap` must remain a live current-thread local heap through the call, with exclusive mutation
/// of its local allocation state; protocol atomic remote-free publication is permitted. If `req`
/// contains an old allocation, that allocation and its page and block metadata must remain live
/// and stable, without concurrent free or realloc, until this call returns or consumes it. The
/// request does not extend those lifetimes. An in-place success returns the still-live old
/// allocation. A moved success frees the old allocation inside typed realloc and returns the new
/// allocation. Core allocation failure leaves the old allocation live, and this helper then frees
/// a nonnull old allocation exactly once.
unsafe fn mi_theap_reallocf_req(
    theap: ThreadHeapLocal,
    req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    let p = req.ptr().map(|ptr| ptr.as_ptr()).unwrap_or(ptr::null_mut());
    // SAFETY: the caller supplies current-thread exclusive local heap access and keeps any old
    // allocation metadata stable through this typed realloc disposition.
    let newp = unsafe { mi_theap_realloc_typed(theap, req) };
    if newp.is_null() && !p.is_null() {
        // SAFETY: a failed typed realloc leaves the old allocation live; this is the reallocf
        // operation's single terminal free of that allocation.
        unsafe { mi_free(p) };
    }
    newp
}

/// # Safety
/// `theap` may be null only where the supported default/lazy heap protocol accepts it; otherwise
/// it must resolve to a live initialized heap owned by the current thread, with exclusive local
/// allocation mutation and no concurrent teardown or non-protocol mutation. Protocol atomic
/// remote-free publication is permitted. `p` must be null or identify a live allocator allocation
/// whose page and block metadata remain stable, without concurrent free or realloc, until this
/// operation returns or consumes it. The internally constructed request does not extend those
/// lifetimes. Request-validation failure and ordinary heap-localization failure return null while
/// leaving `p` live. In either the lazy or localized path, an in-place success returns the
/// still-live old allocation, while a moved success frees the old allocation inside typed realloc
/// and returns the new allocation. Lazy-dispatch or typed allocation failure frees nonnull `p`
/// exactly once.
unsafe fn mi_theap_reallocf_req_raw(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    layout: crate::core_types::LayoutRequest,
) -> *mut c_void {
    // SAFETY: the caller keeps the optional old allocation and its metadata live and stable while
    // this request is validated; the null output pointer requires no writable storage.
    let Some(req) = (unsafe { mi_realloc_request_from_ptr(p, layout, ptr::null_mut()) }) else {
        return ptr::null_mut();
    };
    if theap.is_null() && MI_THEAP_INITASNULL {
        // SAFETY: the supported lazy-null heap protocol dispatches the validated request. Failure
        // leaves a nonnull old allocation live, and this epoch then performs its single final free.
        unsafe {
            let newp = mi_theap_realloc_request_raw(theap, req, ptr::null_mut(), ptr::null_mut());
            if newp.is_null() && !p.is_null() {
                mi_free(p);
            }
            return newp;
        }
    }
    // SAFETY: a nonnull/default heap accepted by this path remains live and current-thread local;
    // localization failure returns without consuming the old allocation.
    let Some(theap) = (unsafe { _mi_theap_or_default_typed(theap) }) else {
        return ptr::null_mut();
    };
    // SAFETY: successful localization establishes the live current-thread local heap witness. The
    // caller keeps its local state exclusive and the request's old allocation stable through the
    // typed reallocf operation, which performs the sole failure free.
    unsafe { mi_theap_reallocf_req(ThreadHeapLocal::new_unchecked(theap), req) }
}

unsafe fn mi_theap_recalloc(
    theap: ThreadHeapLocal,
    req: crate::core_types::CallocRequest,
    realloc_req: crate::core_types::ReallocRequest,
) -> *mut c_void {
    // SAFETY: rezalloc and recalloc paths preserve zero-tail and checked calloc request semantics.
    unsafe {
        crate::internal::mi_debug_assert!(
            req.layout().requested().get() == realloc_req.new_layout().requested().get()
        );
        mi_theap_realloc_typed(theap, realloc_req)
    }
}

unsafe fn mi_theap_recalloc_raw(
    theap: *mut mi_theap_t,
    p: *mut c_void,
    req: crate::core_types::CallocRequest,
) -> *mut c_void {
    // SAFETY: rezalloc and recalloc paths preserve zero-tail and checked calloc request semantics.
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
        mi_theap_recalloc(ThreadHeapLocal::new_unchecked(theap), req, realloc_req)
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY: public realloc ABI entrypoints preserve nullable pointer semantics and heap dispatch validation.
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
// SAFETY: reallocn paths validate checked products and preserve raw pointer ownership semantics.
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
// SAFETY: public realloc ABI entrypoints preserve nullable pointer semantics and heap dispatch validation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_urealloc(
    p: *mut c_void,
    newsize: usize,
    usable_pre: *mut usize,
    usable_post: *mut usize,
) -> *mut c_void {
    // SAFETY: public realloc ABI entrypoints preserve nullable pointer semantics and heap dispatch validation.
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
// SAFETY: reallocf paths preserve free-on-failure behavior after typed request validation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe {
        if let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) {
            mi_theap_reallocf_req_raw(_mi_theap_default(), p, layout)
        } else {
            if !p.is_null() {
                crate::allocator::free::mi_free(p);
            }
            core::ptr::null_mut()
        }
    }
}

/// # Safety
/// `p` must be null or a pointer previously returned by this allocator.
// SAFETY: rezalloc and recalloc paths preserve zero-tail and checked calloc request semantics.
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
// SAFETY: rezalloc and recalloc paths preserve zero-tail and checked calloc request semantics.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_recalloc(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe {
        let Some(req) = calloc_request_or_report(count, size) else {
            return ptr::null_mut();
        };
        mi_theap_recalloc_raw(_mi_theap_default(), p, req)
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown or non-protocol mutation. `p` must
/// be null or a live allocation returned by this allocator; its page and block
/// metadata must remain stable with no concurrent free or realloc. On success,
/// the old allocation is consumed; on failure it remains live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_realloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the raw heap and optional old allocation live
    // through classification, current-thread localization, and realloc dispatch.
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
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_realloc_typed(theap, req)
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown or non-protocol mutation. `p` must
/// be null or a live allocation returned by this allocator; its page and block
/// metadata must remain stable with no concurrent free or realloc. On successful
/// reallocation the old allocation is consumed; on failure it remains live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_reallocn(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the raw heap and optional old allocation live
    // through checked-product validation, localization, and realloc dispatch.
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
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_reallocn_req(theap, product, req)
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown or non-protocol mutation. `p` must
/// be null or a live allocation returned by this allocator; its page and block
/// metadata must remain stable with no concurrent free or realloc. Unlike normal
/// realloc, invalid new-size layout frees a nonnull old allocation after heap
/// validation; after a valid request, allocation failure also frees it. Heap
/// validation failure leaves the old allocation untouched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_reallocf(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the raw heap and optional old allocation live
    // through classification and dispatch, and permits the documented
    // free-on-failure transition.
    unsafe {
        let Some(heap) = BorrowedHeap::from_allocation_raw(heap) else {
            return core::ptr::null_mut();
        };
        let Ok(layout) = crate::core_types::LayoutRequest::malloc(newsize) else {
            if !p.is_null() {
                crate::allocator::free::mi_free(p);
            }
            return core::ptr::null_mut();
        };
        let Some(req) = mi_realloc_request_from_ptr(p, layout, ptr::null_mut()) else {
            return core::ptr::null_mut();
        };
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return core::ptr::null_mut();
        };
        mi_theap_reallocf_req(theap, req)
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown. The caller must have exclusive
/// mutation rights over its local allocation state; protocol atomic remote-free
/// publication is permitted. `p` must be null or a live allocation returned by
/// this allocator; its page and block metadata must remain stable with no
/// concurrent free or realloc. On success, the old allocation is consumed and
/// newly added bytes are zero; on failure it remains live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_rezalloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the heap's local state exclusive and the optional
    // old allocation live through zeroing-layout validation and localization;
    // the typed request carries zero-new-tail into realloc dispatch.
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
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_realloc_typed(theap, req)
    }
}

/// # Safety
/// `heap` must identify a live initialized heap owned by the current thread and
/// remain stable without concurrent teardown or non-protocol mutation. `p` must
/// be null or a live allocation returned by this allocator; its page and block
/// metadata must remain stable with no concurrent free or realloc. On successful
/// reallocation the old allocation is consumed and newly added bytes are zero;
/// on failure it remains live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_recalloc(
    heap: *mut mi_heap_t,
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY: the caller keeps the raw heap and optional old allocation live
    // through checked calloc validation, localization, and realloc dispatch.
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
        let Some(theap) = mi_heap_borrowed_theap_local(heap) else {
            return ptr::null_mut();
        };
        mi_theap_recalloc(theap, req, realloc_req)
    }
}

unsafe fn mi_block_ptr_set_guarded(
    block: crate::core_types::GuardedBlock,
    obj_size: crate::core_types::UsableSize,
) -> Option<UserPtr> {
    // SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
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
        crate::internal::mi_assert_internal!(
            block_size >= obj_size + os_page_size + size_of::<mi_block_t>()
        );
        if block_size < obj_size + os_page_size + size_of::<mi_block_t>() {
            mi_free(block.cast());
            return None;
        }

        let guard_page = block.cast::<u8>().add(block_size - os_page_size);
        crate::internal::mi_assert_internal!(_mi_is_aligned(block.cast(), os_page_size));
        crate::internal::mi_assert_internal!(_mi_is_aligned(guard_page.cast(), os_page_size));
        if !(*page).memid.is_pinned && _mi_is_aligned(guard_page.cast(), os_page_size) {
            let ok = crate::os::_mi_os_protect(guard_page.cast(), os_page_size);
            if mi_unlikely(!ok) {
                crate::allocator::options::_mi_warning_message_fmt(format_args!(
                    "failed to set a guard page behind an object (object {block:p} of size {block_size})\n"
                ));
            }
        } else {
            crate::allocator::options::_mi_warning_message_fmt(format_args!(
                "unable to set a guard page behind an object due to pinned memory (large OS pages?) (object {block:p} of size {block_size})\n"
            ));
        }

        let mut offset = block_size - os_page_size - obj_size;
        crate::internal::mi_assert_internal!(offset > size_of::<mi_block_t>());
        if offset > crate::types::MI_PAGE_MAX_OVERALLOC_ALIGN {
            offset = crate::types::MI_PAGE_MAX_OVERALLOC_ALIGN;
        }
        let p = block.cast::<u8>().add(offset).cast::<c_void>();
        crate::internal::mi_assert_internal!(p.cast::<u8>() == guard_page.sub(obj_size));
        mi_track_align(block.cast(), p, offset, obj_size);
        mi_track_mem_defined(block.cast(), size_of::<mi_block_t>());
        Some(UserPtr::new_unchecked(p))
    }
}

// SAFETY: allocation raw pointers, layouts, padding, tracking, ABI, and fixtures preserve mimalloc ownership and hot-path invariants.
pub(crate) unsafe extern "C" fn _mi_theap_malloc_guarded(
    theap: *mut mi_theap_t,
    size: usize,
    zero: bool,
) -> *mut c_void {
    // SAFETY: guarded allocation receives the
    // raw thread-heap pointer and allocation request through the C ABI shim.
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
