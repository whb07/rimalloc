//! Mechanical translation of the early `src/theap.c` helper slice.

// Retains translated thread-heap helper variants used by selected lifecycle
// paths and upstream parity checks.
#![allow(dead_code, non_camel_case_types)]

mod lists;

use core::{ffi::c_void, mem::size_of, ptr};

use crate::allocator::arena::_mi_arenas_collect;
use crate::allocator::options::_mi_error_message_fmt;
use crate::allocator::page::{
    _mi_page_abandon_internal, _mi_page_free_active_internal, _mi_page_free_collect,
    _mi_page_free_full_internal, _mi_page_is_valid, _mi_theap_collect_retired,
    mi_page_is_abandoned, mi_page_queue_is_valid_typed, mi_page_theap,
};
use crate::allocator::stats::{__mi_stat_decrease_mt, __mi_stat_increase_mt, _mi_stats_merge_into};
use crate::bits::{MI_INTPTR_BITS, mi_ctz};
use crate::core_types::{
    Bin, BlockVisitCallback, BlockVisitCallbackArg, BorrowedHeap, Errno, HeapPtr, PageBlockLayout,
    PagePtr, PageQueuePtr, ThreadHeapLocal, ThreadHeapPtr, ThreadHeapState, TldPtr,
};
use crate::internal::memid::mi_memid_t;
use crate::internal::{
    _mi_align_up, _mi_arenas_alloc, _mi_arenas_free, _mi_deferred_free, _mi_divide_up,
    _mi_is_heap_main, _mi_memcpy, _mi_meta_free, _mi_meta_zalloc, _mi_subproc,
    _mi_theap_guarded_init, _mi_theap_heap, _mi_theap_options_init, mi_page_all_free, mi_page_heap,
    mi_page_usable_block_size, mi_theap_is_initialized,
};
use crate::mimalloc::mi_free;
use crate::prim::{__mi_theap_empty, _mi_theap_default};
#[cfg(not(any(target_os = "macos", windows)))]
use crate::random::_mi_random_init;
use crate::random::{_mi_random_next, _mi_random_split};
use crate::types::{
    MI_ARENA_MIN_OBJ_SIZE, MI_BIN_COUNT, MI_BIN_FULL, MI_SMALL_PAGE_SIZE, mi_block_visit_fun,
    mi_heap_area_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_theap_t, mi_tld_t,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum mi_collect_t {
    MI_NORMAL = 0,
    MI_FORCE = 1,
    MI_ABANDON = 2,
}

pub const MI_MAX_BLOCKS: usize = MI_SMALL_PAGE_SIZE / core::mem::size_of::<*const c_void>();

/// # Safety
///
/// The callback receives allocator-owned thread-heap, queue, and current-page
/// pointers that are valid only for the synchronous invocation. `arg1` and
/// `arg2` must satisfy the callback's contract and remain valid through that
/// invocation. After the traversal has captured the current page's successor,
/// a callback may detach and terminally consume only that current page,
/// including its queue endpoints/count and the successor's predecessor link.
/// It must not invalidate the captured successor, change its forward link or
/// the remaining unvisited forward chain, or relink the current page into that
/// chain. The callback must not unwind across the C ABI or incompatibly reenter
/// page traversal.
pub type theap_page_visitor_fun = unsafe extern "C" fn(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool;

#[derive(Clone, Copy)]
struct TheapPageVisitCallback(theap_page_visitor_fun);

impl TheapPageVisitCallback {
    /// # Safety
    /// The visitor, opaque arguments, heap, queue, and current page must meet
    /// `theap_page_visitor_fun`'s synchronous callback contract. The raw
    /// pointers are not retained after this call.
    #[inline(always)]
    fn new(visitor: Option<theap_page_visitor_fun>) -> Option<Self> {
        visitor.map(Self)
    }

    #[inline(always)]
    unsafe fn invoke(
        self,
        theap: ThreadHeapPtr,
        queue: PageQueuePtr,
        page: PagePtr,
        state: TheapPageVisitState,
    ) -> bool {
        unsafe {
            (self.0)(
                theap.as_ptr(),
                queue.as_ptr(),
                page.as_ptr(),
                state.arg1,
                state.arg2,
            )
        }
    }
}

#[derive(Clone, Copy)]
struct TheapPageVisitState {
    visitor: Option<TheapPageVisitCallback>,
    arg1: *mut c_void,
    arg2: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct mi_theap_area_ex_s {
    pub area: mi_heap_area_t,
    pub page: *mut mi_page_t,
}

pub type mi_theap_area_ex_t = mi_theap_area_ex_s;

/// # Safety
///
/// The callback receives allocator-owned thread-heap and area pointers that
/// that are valid only for the synchronous invocation. `arg` must satisfy the
/// callback's contract and remain valid through that invocation. The callback
/// must not unwind across the C ABI.
pub type mi_theap_area_visit_fun = unsafe extern "C" fn(
    theap: *const mi_theap_t,
    area: *const mi_theap_area_ex_t,
    arg: *mut c_void,
) -> bool;

#[derive(Clone, Copy)]
struct TheapAreaVisitCallback(mi_theap_area_visit_fun);

impl TheapAreaVisitCallback {
    /// # Safety
    /// `vfun` must be nonnull, properly aligned, initialized, and readable as
    /// an `mi_theap_area_visit_fun` slot for this copy. The stored callback
    /// must have the declared ABI and remain valid for every later invocation.
    #[inline(always)]
    unsafe fn from_raw_slot(vfun: *mut c_void) -> Self {
        // SAFETY: the caller keeps this function-pointer slot live, aligned,
        // initialized, and readable for the exact typed copy.
        unsafe { Self(*vfun.cast::<mi_theap_area_visit_fun>()) }
    }

    /// # Safety
    /// `theap` must remain live and initialized, `area` must be nonnull,
    /// aligned, initialized callback storage valid for the synchronous call,
    /// and `arg` must satisfy the callback's opaque-argument contract. The
    /// callback must not retain `area`, incompatibly reenter the traversal, or
    /// unwind across its C ABI.
    #[inline(always)]
    unsafe fn invoke(
        self,
        theap: ThreadHeapPtr,
        area: *const mi_theap_area_ex_t,
        arg: *mut c_void,
    ) -> bool {
        // SAFETY: the caller supplies the live heap, synchronous initialized
        // area storage, opaque argument, and callback ABI/lifetime guarantees.
        unsafe { (self.0)(theap.as_ptr().cast_const(), area, arg) }
    }
}

#[repr(C)]
struct mi_visit_blocks_args_s {
    visit_blocks: bool,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
}

type mi_visit_blocks_args_t = mi_visit_blocks_args_s;

/// # Safety
///
/// `page` must have initialized geometry and count metadata that remain stable
/// during construction. The page metadata and block backing must remain live,
/// and its geometry must remain stable, for every use of the returned value or
/// any pointer carried in it. The value itself does not extend either lifetime
/// or grant access to the block backing.
pub(crate) unsafe fn heap_area_for_page(page: PagePtr) -> Option<mi_heap_area_t> {
    // SAFETY: the caller keeps all page geometry stable for this snapshot.
    let layout = unsafe { PageBlockLayout::from_current_page(page) }?;
    let page_area = layout.area();
    let block_size = layout.block_size();
    // SAFETY: the caller also proves initialized, stable page counts.
    let (capacity, used) = unsafe { (page.capacity(), page.used()) };
    let committed = block_size.get().checked_mul(capacity.as_usize())?;
    let full_block_size = crate::core_types::FullBlockSize::from_block_size(block_size);

    Some(mi_heap_area_t {
        blocks: page_area.start().as_ptr().cast(),
        reserved: page_area.byte_len(),
        committed,
        used: used.as_usize(),
        block_size: block_size.usable().get(),
        full_block_size: full_block_size.get(),
        reserved1: page.as_ptr().cast(),
    })
}

/// # Safety
///
/// `area` and `page` must describe the same valid allocator page, and `visitor`
/// and `arg` must satisfy the block callback contract for each live block. The
/// page must be either locally owned by its live initialized associated heap under
/// current-thread/exclusive local free-list mutation, or abandoned and eligible
/// for the atomic ownership claim. Its metadata, heap, encoded free-list keys,
/// counts, block backing, and geometry must remain live and stable through local
/// collection and free-map snapshot construction; protocol remote frees may
/// publish only through allocator atomics. The callback and opaque argument must
/// remain valid for every synchronous invocation and must not unwind. A visitor
/// may allocate and alter availability, but returning `true` promises it did not
/// free or invalidate this page's snapshotted allocation state, backing, block
/// geometry, later callback blocks, or shared `area` descriptor, and did not
/// incompatibly reenter and mutate this page.
#[inline(never)]
pub(crate) unsafe fn visit_page_blocks(
    area: &mi_heap_area_t,
    page_ptr: PagePtr,
    visitor: BlockVisitCallback,
    arg: BlockVisitCallbackArg,
) -> bool {
    let area = ptr::from_ref(area);
    let page = page_ptr.as_ptr();
    // SAFETY: the contract provides either a locally owned page or a claimable
    // abandoned page. This epoch classifies it and collects remote publication
    // before geometry/free-list traversal, preserving the original order.
    let mut abandoned_owner = unsafe {
        if mi_page_is_abandoned(page) {
            let abandoned = crate::core_types::assume_abandoned_page(page_ptr);
            let Ok(owner) = abandoned.try_claim_ownership() else {
                return false;
            };
            crate::allocator::page::collect_page_free_lists_abandoned(&owner, true);
            Some(owner)
        } else {
            crate::internal::mi_assert_internal!(!mi_page_theap(page).is_null());
            _mi_page_free_collect(page, true);
            None
        }
    };
    // SAFETY: local collection or the retained abandoned claim leaves the
    // initialized local free list empty before the available-list traversal.
    unsafe {
        crate::internal::mi_assert_internal!(page_ptr.local_free().is_empty());
    }
    let result = (|| {
        // SAFETY: local ownership or the abandoned claim keeps the initialized
        // used count live and stable for this snapshot.
        let used = unsafe { crate::allocator::page::mi_page_used_count(page) };
        if used.get() == 0 {
            return true;
        }

        // SAFETY: local traversal ownership or the abandoned claim keeps the
        // page, associated heap, counts, geometry, and backing stable while
        // this complete traversal snapshot is formed.
        let (block_layout, heap, ubsize, capacity) = unsafe {
            let Some(block_layout) = PageBlockLayout::from_current_page(page_ptr) else {
                crate::internal::mi_assert_internal!(false);
                return false;
            };
            let heap = HeapPtr::new_unchecked(mi_page_heap(page));
            let ubsize = mi_page_usable_block_size(page);
            let capacity = crate::allocator::page::mi_page_capacity_count(page);
            (block_layout, heap, ubsize, capacity)
        };
        let Some(block_range) = block_layout.nonempty_range(0, capacity.as_usize()) else {
            crate::internal::mi_assert_internal!(false);
            return false;
        };
        if capacity.get() == 1 {
            // SAFETY: the stable capacity/used snapshot and collected page
            // state establish the singleton list invariant; the callback
            // contract covers the sole live block invocation.
            return unsafe {
                crate::internal::mi_assert_internal!(used.get() == 1 && page_ptr.free().is_empty());
                visitor.invoke(
                    heap,
                    area,
                    block_range.first().as_ptr().cast(),
                    ubsize.get(),
                    arg,
                )
            };
        }

        if used.get() == capacity.get() {
            for block_index in 0..capacity.as_usize() {
                let Some(block) = block_range.block_at(block_index) else {
                    crate::internal::mi_assert_internal!(false);
                    return false;
                };
                // SAFETY: the range identifies this live allocated block and
                // the callback contract remains valid for this invocation.
                if !unsafe { visitor.invoke(heap, area, block.as_ptr().cast(), ubsize.get(), arg) }
                {
                    return false;
                }
            }
            return true;
        }

        let mut free_map = [0usize; MI_MAX_BLOCKS / MI_INTPTR_BITS];
        let bmapsize = _mi_divide_up(capacity.as_usize(), MI_INTPTR_BITS);
        if !capacity.as_usize().is_multiple_of(MI_INTPTR_BITS) {
            let shift = capacity.as_usize() % MI_INTPTR_BITS;
            free_map[bmapsize - 1] = usize::MAX << shift;
        }

        // SAFETY: local ownership or the abandoned claim keeps the encoded
        // available-list head and keys stable. Each decoded successor remains
        // raw until exact block-start membership is established.
        let free_count = unsafe {
            let mut free_count = 0usize;
            let mut free = page_ptr.free().as_raw();
            while !free.is_null() {
                free_count += 1;
                let Some(blockidx) = block_range.index_of(free) else {
                    crate::internal::mi_assert_internal!(false);
                    return false;
                };
                crate::internal::mi_assert_internal!(blockidx < MI_MAX_BLOCKS);
                let bitidx = blockidx / MI_INTPTR_BITS;
                let bit = blockidx - (bitidx * MI_INTPTR_BITS);
                free_map[bitidx] |= 1usize << bit;
                free = crate::internal::mi_block_next(page, free);
            }
            free_count
        };
        crate::internal::mi_assert_internal!(capacity.as_usize() == free_count + used.as_usize());

        for (map_index, mask) in free_map.iter().take(bmapsize).enumerate() {
            let first_block_index = map_index * MI_INTPTR_BITS;
            if *mask == 0 {
                for bit_index in 0..MI_INTPTR_BITS {
                    let Some(block) = block_range.block_at(first_block_index + bit_index) else {
                        crate::internal::mi_assert_internal!(false);
                        return false;
                    };
                    // SAFETY: this bit is absent from the validated free map,
                    // so the range identifies a live block for the callback.
                    if !unsafe {
                        visitor.invoke(heap, area, block.as_ptr().cast(), ubsize.get(), arg)
                    } {
                        return false;
                    }
                }
            } else {
                let mut used_mask = !*mask;
                while used_mask != 0 {
                    let bitidx = mi_ctz(used_mask);
                    let Some(block) = block_range.block_at(first_block_index + bitidx) else {
                        crate::internal::mi_assert_internal!(false);
                        return false;
                    };
                    // SAFETY: this set used bit identifies a live block within
                    // the validated page range for the callback invocation.
                    if !unsafe {
                        visitor.invoke(heap, area, block.as_ptr().cast(), ubsize.get(), arg)
                    } {
                        return false;
                    }
                    used_mask &= used_mask - 1;
                }
            }
        }
        true
    })();

    if let Some(owner) = abandoned_owner.take() {
        // SAFETY: the retained claim keeps the atomic ownership word and page
        // metadata live. Preserve the original collect-until-empty CAS retry
        // transaction before terminally releasing the claim.
        unsafe {
            let atomic = page_ptr.thread_free_cell();
            loop {
                let mut expected = atomic.load_relaxed();
                crate::internal::mi_assert_internal!(expected.is_owned());
                if expected.head().is_some() {
                    crate::allocator::page::collect_page_free_lists_abandoned(&owner, true);
                    continue;
                }
                if atomic.release_acq_rel(&mut expected) {
                    break;
                }
            }
        }
    }
    result
}

/// # Safety
/// `theap`, `pq`, and `page` must be nonnull live allocator metadata from one
/// initialized thread heap reached by its synchronous page traversal. The
/// current thread or an exclusive initialization/teardown owner must keep the
/// page association, counts, geometry, and backing stable through callback
/// return. `vfun` must be nonnull, aligned, initialized, readable
/// `mi_theap_area_visit_fun` storage that remains live for this invocation;
/// `arg` must satisfy that callback's contract. The callback must not retain
/// stack area storage, incompatibly reenter the traversal, or unwind.
unsafe extern "C" fn mi_theap_visit_areas_page(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    vfun: *mut c_void,
    arg: *mut c_void,
) -> bool {
    crate::internal::MI_UNUSED(&pq);
    // SAFETY: the page traversal supplies nonnull same-heap metadata and a
    // live initialized callback slot. The typed worker receives the complete
    // page/backing/callback lifetime and non-reentry contract unchanged.
    unsafe {
        let theap = ThreadHeapPtr::new_unchecked(theap);
        let page = PagePtr::new_unchecked(page);
        let visitor = TheapAreaVisitCallback::from_raw_slot(vfun);
        mi_theap_visit_areas_page_typed(theap, page, visitor, arg)
    }
}

/// Visit one page through the internal extended-area callback.
///
/// # Safety
/// `theap` must identify the live initialized owner of `page`. The current
/// thread or an exclusive initialization/teardown owner must keep the page's
/// association, counts, geometry, and backing stable through the synchronous
/// callback. `visitor` must remain valid with the declared ABI and `arg` must
/// satisfy its opaque-argument contract. During the callback it must not write
/// the stack-local extended area (`area` or `page`), incompatibly reenter this
/// traversal, or invalidate the page/backing/geometry. A `true` result must
/// preserve the allocation snapshot needed by any later block walk. It must
/// not retain the stack-local area or unwind across the C ABI.
unsafe fn mi_theap_visit_areas_page_typed(
    theap: ThreadHeapPtr,
    page: PagePtr,
    visitor: TheapAreaVisitCallback,
    arg: *mut c_void,
) -> bool {
    // SAFETY: the traversal contract keeps the page's initialized heap
    // association live and stable for this same-owner assertion.
    crate::internal::mi_assert_internal!(unsafe { mi_page_theap(page.as_ptr()) == theap.as_ptr() });
    // SAFETY: the caller keeps the page metadata, counts, geometry, and block
    // backing live and stable through construction and callback return.
    let Some(area) = (unsafe { heap_area_for_page(page) }) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };
    let xarea = mi_theap_area_ex_t {
        area,
        page: page.as_ptr(),
    };
    // SAFETY: `xarea` is initialized stack-local storage matching this live
    // page, while the callback and opaque argument remain valid synchronously.
    unsafe { visitor.invoke(theap, ptr::addr_of!(xarea), arg) }
}

/// # Safety
///
/// When non-null, `theap` must point to a live initialized thread heap whose
/// current thread (or exclusive initialization/teardown owner) may mutate local
/// page free lists and metadata, and whose page backing and geometry remain
/// valid through the synchronous traversal. When present, `visitor` and `arg`
/// must satisfy the area callback contract for that traversal, and the callback
/// must not unwind across the C ABI or incompatibly reenter the page traversal.
unsafe fn mi_theap_visit_areas(
    theap: *const mi_theap_t,
    visitor: Option<mi_theap_area_visit_fun>,
    arg: *mut c_void,
) -> bool {
    let Some(visitor) = visitor else {
        return false;
    };
    let mut visitor = visitor;
    let visitor_arg = ptr::addr_of_mut!(visitor).cast();
    let page_visitor = Some(mi_theap_visit_areas_page as theap_page_visitor_fun);

    // SAFETY: the caller supplies the live initialized heap and keeps the
    // opaque callback argument valid. The synchronous internal adapter does
    // not retain its stack-local `visitor_arg` slot.
    unsafe { mi_theap_visit_pages(theap.cast_mut(), page_visitor, true, visitor_arg, arg) }
}

/// # Safety
/// `theap` must be nonnull and identify the live initialized thread heap that
/// owns the page represented by `xarea`. `xarea` and `arg` must be nonnull,
/// aligned, initialized, respectively readable area and dispatch storage for
/// this synchronous invocation; neither may be concurrently mutated. Their
/// page, counts, geometry, backing, heap association, callback values, and
/// opaque argument must remain live through return. During the callback, it
/// must not directly or indirectly write the internal dispatch storage or
/// either field of the current extended area, incompatibly reenter traversal,
/// or invalidate the current page, its backing/geometry, or the later block
/// walk. A `true` result must preserve those values for that walk. It must not
/// retain stack-local storage or unwind across the C ABI.
#[inline(never)]
unsafe extern "C" fn mi_theap_area_visitor(
    theap: *const mi_theap_t,
    xarea: *const mi_theap_area_ex_t,
    arg: *mut c_void,
) -> bool {
    // SAFETY: the callback contract keeps `theap`, `xarea`, and `arg` aligned,
    // initialized, and live. Read only the fields needed for the first callback;
    // later traversal state is deliberately not read before it returns.
    let continue_visit = unsafe {
        let args = arg.cast::<mi_visit_blocks_args_t>();
        (*args).visitor.invoke(
            HeapPtr::new_unchecked(_mi_theap_heap(theap.cast_mut())),
            ptr::addr_of!((*xarea).area),
            ptr::null_mut(),
            (*xarea).area.block_size,
            (*args).arg,
        )
    };
    if !continue_visit {
        return false;
    }
    // SAFETY: a continuing callback preserves this initialized dispatch slot
    // and the exact original post-callback read of its `visit_blocks` flag.
    if unsafe { (*arg.cast::<mi_visit_blocks_args_t>()).visit_blocks } {
        // SAFETY: the callback contract and its true result preserve the live
        // matching area/page, callback, opaque argument, backing, and geometry.
        // These fields are intentionally reread only after the first callback.
        unsafe {
            let args = arg.cast::<mi_visit_blocks_args_t>();
            visit_page_blocks(
                &(*xarea).area,
                PagePtr::new_unchecked((*xarea).page),
                (*args).visitor,
                (*args).arg,
            )
        }
    } else {
        true
    }
}

/// # Safety
///
/// `theap`, when non-null, must point to a live initialized thread heap whose
/// current thread (or exclusive initialization/teardown owner) may mutate local
/// queues/free lists and metadata, and whose page backing and geometry remain
/// valid through the synchronous traversal. `visitor`, when present, and `arg`
/// must satisfy the block callback contract for every invocation and remain
/// valid until traversal returns. During invocation it must not write the
/// current area descriptor, unwind across the C ABI, or incompatibly reenter
/// page traversal. A callback that returns `true` may allocate and change
/// availability, but must not free or invalidate the current page's snapshotted
/// allocation state, backing, block geometry, or later callback blocks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_visit_blocks(
    theap: *const mi_theap_t,
    visit_blocks: bool,
    visitor: Option<mi_block_visit_fun>,
    arg: *mut c_void,
) -> bool {
    let Some(visitor) = visitor else {
        return false;
    };
    let mut args = mi_visit_blocks_args_t {
        visit_blocks,
        visitor: BlockVisitCallback::new(Some(visitor)),
        arg: BlockVisitCallbackArg::from_raw(arg),
    };
    let args_ptr = ptr::addr_of_mut!(args).cast::<c_void>();

    // SAFETY: the public contract supplies the live heap, callback, and
    // callback argument. The traversal is synchronous, so the stack-local
    // dispatch state at `args_ptr` remains live for every callback invocation.
    unsafe { mi_theap_visit_areas(theap, Some(mi_theap_area_visitor), args_ptr) }
}

/// # Safety
///
/// When non-null, `theap` must point to a live initialized thread heap whose
/// current thread (or exclusive initialization/teardown owner) may mutate local
/// queues/free lists and metadata, and whose queue storage, page backing, and
/// page geometry remain valid through the synchronous traversal. When present,
/// `fn_page`, `arg1`, and `arg2` must satisfy the page callback contract for
/// every invocation and remain valid until traversal returns. The callback must
/// not unwind across the C ABI or incompatibly reenter page traversal. After a
/// successor has been captured, it may detach and terminally consume only the
/// current page, while preserving the successor and the remaining unvisited
/// forward chain. A null `theap` returns `false` without invoking the callback.
pub unsafe fn mi_theap_visit_pages(
    theap: *mut mi_theap_t,
    fn_page: Option<theap_page_visitor_fun>,
    include_full: bool,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return false;
    };
    let state = TheapPageVisitState {
        visitor: TheapPageVisitCallback::new(fn_page),
        arg1,
        arg2,
    };

    // SAFETY: the caller supplies the live initialized heap and keeps `arg1`
    // and `arg2` valid under the callback contract. The typed traversal does
    // not itself retain the operation-local callback state.
    unsafe { mi_theap_visit_pages_typed(theap, include_full, state) }
}

/// Traverse the selected page queues of one thread heap.
///
/// # Safety
/// `theap` must identify a live initialized locally owned thread heap under
/// current-thread ownership or exclusive initialization/teardown access. Its
/// page count and complete fixed-bin queue array must be initialized and
/// readable for the initial snapshots, and every queue storage location must
/// remain live throughout traversal. Every page, link, metadata object,
/// geometry value, and backing allocation must be live and initialized when it
/// is read, and each visited chain must be finite.
/// Protocol remote-free publication may continue only through allocator
/// atomics. If present, `state.visitor` must have the declared ABI, and it and
/// both opaque arguments must remain valid for every synchronous invocation and
/// must not unwind or incompatibly reenter traversal. After `next` is captured,
/// the callback may detach and terminally consume the current page, including
/// updating the owning queue's links/endpoints/count, the captured successor's
/// predecessor link, and the heap page count. It must not relink the current
/// page into the unvisited traversal, invalidate the captured successor, alter
/// that successor's forward link or the remaining forward chain, or invalidate
/// metadata/backing needed by later callbacks before traversal resumes. A
/// `false` callback result stops traversal immediately without advancing to the
/// captured successor.
unsafe fn mi_theap_visit_pages_typed(
    theap: ThreadHeapPtr,
    include_full: bool,
    state: TheapPageVisitState,
) -> bool {
    let theap_ptr = theap.as_ptr();
    // SAFETY: the contract keeps this initialized heap's page-count field live
    // and readable for the initial empty check. Callbacks have not run yet.
    if unsafe { (*theap_ptr).page_count == 0 } {
        return false;
    }
    let mut count = 0usize;
    // SAFETY: preserve the original second page-count read before traversal;
    // the field is still live and callbacks have not yet run.
    let total = unsafe { (*theap_ptr).page_count };
    let max_bin = if include_full {
        MI_BIN_FULL
    } else {
        MI_BIN_FULL - 1
    };

    for i in 0..=max_bin {
        // SAFETY: the safe loop bounds keep `i` within the selected fixed-bin range. The caller
        // keeps the complete queue array live and the selected queue head initialized until read.
        let (queue, mut page) = unsafe {
            let bin = Bin::new_unchecked(i as u16);
            let queue = theap.page_queue(bin);
            let page = queue.first();
            (queue, page)
        };
        while let Some(current) = page {
            let current_ptr = current.as_ptr();
            // SAFETY: the current page remains live with an initialized stable forward link until
            // this read. Capturing it before owner validation and callback invocation preserves the
            // original removal-safe traversal order.
            let next = unsafe { current.queue_next() };
            // SAFETY: the current page remains live with its initialized owner association stable
            // before the callback. Keeping the read in the assertion preserves assertion-only
            // evaluation after the successor snapshot.
            crate::internal::mi_assert_internal!(
                unsafe { mi_page_theap(current_ptr) } == theap_ptr
            );
            if crate::internal::MI_DEBUG > 1 {
                count += 1;
            }
            if let Some(visitor) = state.visitor
                // SAFETY: `next` was captured first. The callback, opaque arguments, current page,
                // queue, and heap remain live for this synchronous invocation and the contract
                // permits only current-page detach/consumption while preserving the successor.
                && !unsafe { visitor.invoke(theap, queue, current, state) }
            {
                return false;
            }
            page = next;
        }
    }

    crate::internal::mi_assert_internal!(
        !include_full || crate::internal::MI_DEBUG <= 1 || count == total
    );
    true
}

/// # Safety
///
/// `theap`, `pq`, and `page` must be valid allocator objects from the same
/// thread heap.
pub unsafe extern "C" fn mi_theap_page_is_valid(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg1: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    unsafe {
        crate::internal::MI_UNUSED(&arg1);
        crate::internal::MI_UNUSED(&arg2);
        crate::internal::MI_UNUSED(&pq);
        let theap = ThreadHeapPtr::new_unchecked(theap);
        let page = PagePtr::new_unchecked(page);
        mi_theap_page_is_valid_typed(theap, page)
    }
}

unsafe fn mi_theap_page_is_valid_typed(theap: ThreadHeapPtr, page: PagePtr) -> bool {
    unsafe {
        let page = page.as_ptr();
        crate::internal::mi_assert_internal!(mi_page_theap(page) == theap.as_ptr());
        crate::internal::mi_assert_expensive!(_mi_page_is_valid(page));
        true
    }
}

/// # Safety
///
/// `theap` must be the current-thread-local owner of the live initialized
/// `page`, and `pq` must be that page's compatible normal, huge, or full queue.
/// The caller must exclusively own local mutation of the heap TLD, queue links,
/// page free lists, capacity/countdown fields, and stats; protocol remote-free
/// publication may continue only through allocator atomics. Page metadata,
/// geometry, backing, arena/MemId, heap/TLD, and queue storage must remain live
/// and stable; the page must not be retired before collection. `arg_collect`
/// must be nonnull, aligned, readable initialized `mi_collect_t` storage. Arena,
/// purge, commit, tracking, and diagnostic callbacks and opaque arguments must
/// remain synchronously valid, exclude incompatible reentry, and not unwind.
pub unsafe extern "C" fn mi_theap_page_collect(
    theap: *mut mi_theap_t,
    pq: *mut mi_page_queue_t,
    page: *mut mi_page_t,
    arg_collect: *mut c_void,
    arg2: *mut c_void,
) -> bool {
    unsafe {
        crate::internal::MI_UNUSED(&arg2);
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        let queue = PageQueuePtr::new_unchecked(pq);
        let page = PagePtr::new_unchecked(page);
        let collect = *arg_collect.cast::<mi_collect_t>();
        mi_theap_page_collect_typed(theap, queue, page, collect)
    }
}

/// Collect one page reached through its owning thread-heap queue.
///
/// # Safety
/// `theap` must be the current-thread-local owner of the live initialized
/// `page`, and `queue` must be that page's compatible owning normal, huge, or
/// full queue. The caller must exclusively own local mutation of the heap TLD,
/// queue links, page free lists, capacity/countdown fields, and stats; protocol
/// remote-free publication may continue only through allocator atomics. Page
/// metadata, geometry, backing, arena membership, MemId, heap/TLD, and queue
/// storage must remain live until the page is either retained or consumed by a
/// terminal free/abandon transition. A page satisfying the terminal release
/// predicate must already have been transitioned out of retired state by the
/// preceding retired-page collection; a retained retired page has a nonzero
/// countdown. The page's used count, retirement countdown, and full
/// classification must remain stable between each read and the corresponding
/// consuming transition. Arena, purge, commit, tracking, and diagnostic
/// callbacks and opaque arguments must remain valid for synchronous invocation,
/// exclude incompatible reentry that mutates this heap/page/queue transaction,
/// and must not unwind. A terminal branch consumes the sole local-page
/// capability and neither this function nor its caller may access the page
/// afterward.
unsafe fn mi_theap_page_collect_typed(
    theap: ThreadHeapLocal,
    queue: PageQueuePtr,
    page: PagePtr,
    collect: mi_collect_t,
) -> bool {
    // SAFETY: the live local heap/page/queue relation and exclusive mutation
    // epoch permit validation, construction of the sole local capability, and
    // collection of the remote list into local page state in this exact order.
    let local_page = unsafe {
        crate::internal::mi_assert_internal!(mi_theap_page_is_valid_typed(theap.ptr(), page));
        let local_page = crate::core_types::assume_local_page(page);
        _mi_page_free_collect(
            local_page.page().as_ptr(),
            collect as i32 >= mi_collect_t::MI_FORCE as i32,
        );
        local_page
    };
    let page = local_page.page().as_ptr();

    // SAFETY: collection has completed and the caller-owned page metadata and
    // used/free-list state remain stable for this release predicate.
    if unsafe { mi_page_all_free(page) } {
        let force = collect as i32 >= mi_collect_t::MI_FORCE as i32;
        // Preserve the original short circuit: forced collection does not read
        // the retirement countdown.
        let release = force || {
            // SAFETY: the page remains live and exclusively locally owned,
            // so its initialized retirement countdown is stable here.
            unsafe { crate::allocator::page::mi_page_retire_expire(page).is_zero() }
        };
        if release {
            // SAFETY: the live locally owned page retains its initialized stable full flag until
            // the corresponding capability transition below consumes `local_page`.
            let is_full = unsafe { crate::allocator::page::mi_page_is_in_full(page) };
            if is_full {
                // SAFETY: the stable full classification and exact owning queue refine the sole
                // local-page capability without duplicating it.
                let full = unsafe { local_page.into_full_after_classification() };
                let Ok(all_free) = full.try_mark_all_free() else {
                    crate::internal::mi_assert_internal!(false);
                    return true;
                };
                // SAFETY: the repeated zero-used refinement and stable full classification prove
                // the exact all-free/full queue capability consumed by terminal release. No page
                // access follows this call.
                unsafe {
                    let full_queue = crate::core_types::FullPageQueue::new_unchecked(queue);
                    let _ = _mi_page_free_full_internal(all_free, full_queue);
                }
            } else {
                // SAFETY: the stable non-full, non-retired classification and exact owning queue
                // refine the sole local-page capability without duplicating it.
                let active = unsafe { local_page.into_active_after_classification() };
                let Ok(all_free) = active.try_mark_all_free() else {
                    crate::internal::mi_assert_internal!(false);
                    return true;
                };
                // SAFETY: the repeated zero-used refinement and stable active classification prove
                // the exact all-free/source queue capability consumed by terminal release. No page
                // access follows this call.
                unsafe {
                    let normal_queue = crate::core_types::NormalPageQueue::new_unchecked(queue);
                    let _ = _mi_page_free_active_internal(all_free, normal_queue);
                }
            }
        }
    } else if collect == mi_collect_t::MI_ABANDON {
        // SAFETY: the retained non-all-free page remains live and locally owned with its initialized
        // full flag stable until the corresponding capability transition consumes `local_page`.
        let is_full = unsafe { crate::allocator::page::mi_page_is_in_full(page) };
        let page_for_abandon = if is_full {
            // SAFETY: the stable full classification and exact owning queue refine the sole local
            // capability before packaging it for the abandon transition.
            unsafe { local_page.into_full_after_classification() }.into_abandon_candidate()
        } else {
            // SAFETY: the stable active/non-full classification and exact owning queue refine the
            // sole local capability before packaging it for the abandon transition.
            unsafe { local_page.into_active_after_classification() }.into_abandon_candidate()
        };
        // SAFETY: the consumed active/full capability and exact owning queue remain live under the
        // exclusive local epoch. Abandon may retain or terminally release the page; neither this
        // function nor its caller accesses the consumed page afterward.
        unsafe {
            let _ = _mi_page_abandon_internal(page_for_abandon, queue);
        }
    }

    true
}

/// # Safety
///
/// `theap` must point to a live initialized thread heap. The caller must own
/// its local queue mutation, either through current-thread local ownership or
/// exclusive initialization/teardown access. Queue links, page backing, and
/// page geometry must remain valid and must not be concurrently destroyed or
/// relinked during validation. Protocol-compliant remote atomic frees remain
/// permitted.
pub unsafe fn mi_theap_is_valid(theap: *mut mi_theap_t) -> bool {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        crate::internal::mi_assert_internal!(false);
        return false;
    };

    // SAFETY: the caller supplies the live initialized heap, owns its local
    // queue mutation, and keeps queue links, page backing, and geometry stable
    // against destruction or relinking while typed validation traverses them.
    unsafe { mi_theap_is_valid_typed(theap) }
}

/// # Safety
///
/// `theap` must identify a live initialized thread heap. The caller must own
/// its local queue mutation through current-thread local ownership or exclusive
/// initialization/teardown access. Its page count, complete fixed-bin queue
/// table, queue endpoints/counts, and every linked page, adjacent link, owner
/// association, classification, geometry, metadata, and backing inspected by
/// validation must remain live, initialized, readable, and stable. Each queue
/// chain must be finite, and every nonnull link encountered must resolve to
/// live page metadata. Concurrent local relinking, non-protocol metadata
/// mutation, and page or heap teardown are excluded;
/// protocol-compliant remote atomic frees remain permitted. Allocator
/// diagnostics must remain valid, must not unwind, and must not incompatibly
/// reenter page, queue, or heap mutation.
unsafe fn mi_theap_is_valid_typed(theap: ThreadHeapPtr) -> bool {
    let state = TheapPageVisitState {
        visitor: TheapPageVisitCallback::new(Some(mi_theap_page_is_valid)),
        arg1: core::ptr::null_mut(),
        arg2: core::ptr::null_mut(),
    };

    // SAFETY: the contract keeps the live heap, complete queue table, every linked page, geometry,
    // and backing stable while the fixed internal callback synchronously checks page ownership.
    // Its null opaque arguments are unused and it is not retained. Protocol remote frees do not
    // mutate the topology inspected by this traversal.
    unsafe {
        mi_theap_visit_pages_typed(theap, true, state);
    }

    // SAFETY: the caller holds current-thread local ownership or the exclusive lifecycle access
    // that permits projecting the same live heap for read-only queue validation.
    let theap_wrapper = unsafe { ThreadHeapLocal::new_unchecked(theap) };
    for bin_index in 0..MI_BIN_COUNT {
        // SAFETY: the loop bounds prove `bin_index` is a valid ascending fixed-bin index. The
        // contract keeps that exact queue and every reachable page live and stable while the
        // synchronous validator reads its topology, classification, ownership, and count.
        let is_valid = unsafe {
            let bin = Bin::new_unchecked(bin_index as u16);
            let queue = theap.page_queue(bin);
            mi_page_queue_is_valid_typed(theap_wrapper, queue)
        };
        crate::internal::mi_assert_internal!(is_valid);
    }
    true
}

/// # Safety
///
/// `theap` must identify a live initialized thread heap whose associated heap
/// remains live and stable for the call. The caller must have exclusive access
/// to the thread heap's source stats because the merge resets them. The backing
/// heap's destination stats must be live, correctly aligned, atomic-compatible
/// storage, with all concurrent access to its fields performed atomically.
unsafe fn mi_theap_merge_stats_typed(theap: ThreadHeapPtr) {
    // SAFETY: the caller keeps the initialized thread heap and its associated
    // backing heap live and stable, exclusively owns the source stats reset,
    // and guarantees atomic-compatible access to the destination heap stats.
    unsafe {
        let theap_raw = theap.as_ptr();
        crate::internal::mi_assert_internal!(mi_theap_is_initialized(theap_raw));
        let heap = HeapPtr::new_unchecked(_mi_theap_heap(theap_raw).cast::<mi_heap_t>());
        _mi_stats_merge_into(heap.stats(), theap.stats());
    }
}

/// # Safety
///
/// `theap` must identify a live initialized thread heap under current-thread
/// local ownership or exclusive teardown ownership. Its TLD, backing heap,
/// page queues, page metadata, and page backing must remain live and stable for
/// the call, with no concurrent destroy or queue relinking. Protocol remote
/// frees may publish through the allocator's atomic remote-free lists. The
/// caller must exclusively own the thread heap's local queue/free-list,
/// retirement, arena collection, and source-stats reset operations; its TLD
/// pointer and associated heap must remain stable. Any registered deferred-free,
/// arena purge/commit, tracking, diagnostic, or page-collection callback and
/// opaque argument must remain valid for synchronous invocation on this thread,
/// tolerate allocator reentry, preserve the thread heap, TLD, page topology,
/// arena state, and stats needed by all later phases, and must not unwind.
unsafe fn mi_theap_collect_ex_typed(theap: ThreadHeapLocal, mut collect: mi_collect_t) {
    let theap_ptr = theap.ptr();
    let theap_raw = theap_ptr.as_ptr();

    // SAFETY: the caller keeps the initialized heap, its queue topology, pages,
    // geometry, and backing live and owns local mutation during this preflight.
    unsafe {
        crate::internal::mi_assert_expensive!(mi_theap_is_valid(theap_raw));
    }

    let force = collect as i32 >= mi_collect_t::MI_FORCE as i32;
    // SAFETY: the local/exclusive lifecycle epoch and callback contract permit
    // deferred processing followed by retired-page collection. The deferred
    // callback is synchronous and must preserve the topology consumed by the
    // immediately following retirement phase.
    unsafe {
        _mi_deferred_free(theap_raw, force);
        _mi_theap_collect_retired(theap_raw, force);
    }

    let state = TheapPageVisitState {
        visitor: TheapPageVisitCallback::new(Some(mi_theap_page_collect)),
        arg1: core::ptr::addr_of_mut!(collect).cast(),
        arg2: core::ptr::null_mut(),
    };
    // SAFETY: retired collection has completed. The caller still exclusively
    // owns the live queues/pages, while the synchronous adapter and stack-local
    // collect argument remain valid for every page callback and are not retained.
    unsafe {
        mi_theap_visit_pages_typed(theap_ptr, collect != mi_collect_t::MI_NORMAL, state);
    }

    // SAFETY: page traversal has returned and its callbacks preserve the live
    // heap/TLD, arena registry, and stats. Preserve the original post-traversal
    // TLD read, then arena collection followed by destructive stats merge.
    unsafe {
        _mi_arenas_collect(
            collect == mi_collect_t::MI_FORCE,
            collect as i32 >= mi_collect_t::MI_FORCE as i32,
            (*theap_raw).tld,
        );
        mi_theap_merge_stats_typed(theap_ptr);
    }
}

/// # Safety
///
/// `theap` must be null or identify a live initialized thread heap under
/// current-thread local ownership or exclusive teardown ownership. Its TLD,
/// backing heap, queues, pages, and registered deferred-free callback state
/// must satisfy `mi_theap_collect_ex_typed` for the call. No concurrent destroy
/// or queue relinking is permitted; protocol remote frees remain allowed.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_collect_abandon(theap: *mut mi_theap_t) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the caller keeps this thread heap live and stable while its
    // initialized state is read under local or exclusive teardown ownership.
    if !unsafe { mi_theap_is_initialized(theap.as_ptr()) } {
        return;
    }
    // SAFETY: the caller's ownership proves the initialized thread heap is
    // local for this operation; the collection core's lifecycle and callback
    // requirements hold through abandonment.
    unsafe {
        mi_theap_collect_ex_typed(
            ThreadHeapLocal::new_unchecked(theap),
            mi_collect_t::MI_ABANDON,
        )
    };
}

/// # Safety
///
/// `theap` must be null or identify a live initialized thread heap under
/// current-thread local ownership or exclusive teardown ownership. Its TLD,
/// backing heap, queues, pages, and registered deferred-free callback state
/// must satisfy `mi_theap_collect_ex_typed` for the call. No concurrent destroy
/// or queue relinking is permitted; protocol remote frees remain allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_collect(theap: *mut mi_theap_t, force: bool) {
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return;
    };
    // SAFETY: the caller keeps this thread heap live and stable while its
    // initialized state is read under local or exclusive teardown ownership.
    if !unsafe { mi_theap_is_initialized(theap.as_ptr()) } {
        return;
    }
    let collect = if force {
        mi_collect_t::MI_FORCE
    } else {
        mi_collect_t::MI_NORMAL
    };
    // SAFETY: the caller's ownership proves the initialized thread heap is
    // local for this operation, and the collection core's lifecycle and
    // callback requirements hold for the selected mode.
    unsafe { mi_theap_collect_ex_typed(ThreadHeapLocal::new_unchecked(theap), collect) };
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_collect(force: bool) {
    unsafe {
        mi_theap_collect(_mi_theap_default(), force);
    }
}

/// # Safety
///
/// `heap` must be null or point to a heap managed by this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_heap_collect(heap: *mut mi_heap_t, force: bool) {
    unsafe {
        mi_heap_collect_typed(BorrowedHeap::from_lookup_raw(heap), force);
    }
}

/// # Safety
///
/// `heap` must be `None` or identify a live allocator heap whose associated
/// thread heap is under current-thread local ownership or exclusive teardown
/// ownership. The associated thread heap's TLD, backing heap, queues, pages,
/// and registered deferred-free callback state must remain valid as required
/// by `mi_theap_collect_ex_typed`. No concurrent destroy or queue relinking is
/// permitted; protocol remote frees remain allowed.
#[inline]
unsafe fn mi_heap_collect_typed(heap: Option<BorrowedHeap>, force: bool) {
    // SAFETY: the caller keeps the optional heap live while its associated
    // thread heap is resolved.
    let Some(theap) =
        (unsafe { crate::allocator::heap::mi_heap_theap_typed(heap.map(BorrowedHeap::ptr)) })
    else {
        return;
    };
    // SAFETY: the caller keeps the associated thread heap live and stable
    // while its initialized state is read.
    if !unsafe { mi_theap_is_initialized(theap.as_ptr()) } {
        return;
    }
    let collect = if force {
        mi_collect_t::MI_FORCE
    } else {
        mi_collect_t::MI_NORMAL
    };
    // SAFETY: the caller's ownership proves the initialized associated thread
    // heap is local for this operation, and the collection core's lifecycle
    // and callback requirements hold for the selected mode.
    unsafe { mi_theap_collect_ex_typed(ThreadHeapLocal::new_unchecked(theap), collect) };
}

/// # Safety
///
/// Must be called only after allocator process/thread initialization has made
/// the default thread heap accessible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_get_default() -> *mut mi_theap_t {
    unsafe {
        let mut theap = _mi_theap_default();
        if !mi_theap_is_initialized(theap) {
            crate::allocator::init::mi_thread_init();
            theap = _mi_theap_default();
            crate::internal::mi_assert_internal!(mi_theap_is_initialized(theap));
        }
        theap
    }
}

/// # Safety
///
/// `theap`, when non-null, must point to a valid initialized thread heap whose
/// lifetime covers subsequent default-thread-heap use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_theap_set_default(theap: *mut mi_theap_t) -> *mut mi_theap_t {
    unsafe {
        let old = mi_theap_get_default();
        if !theap.is_null() {
            let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
            mi_theap_set_default_typed(theap);
        }
        old
    }
}

unsafe fn mi_theap_set_default_typed(theap: ThreadHeapLocal) {
    unsafe {
        crate::allocator::init::_mi_theap_default_set(theap.ptr().as_ptr());
    }
}

/// Initialize and publish newly allocated thread-heap metadata.
///
/// # Safety
/// `theap` must identify live, aligned, writable storage for one `mi_theap_t`
/// whose `memid` describes that exact backing. `heap` and `tld` must identify
/// live initialized owners for the new thread heap. The caller must exclusively
/// own initialization of `theap` and the relevant heap/TLD list publication;
/// concurrent lookup, relink, teardown, or use before publication is excluded.
/// The template, subprocess stats, allocator options, random state, guarded
/// configuration, and list locks must remain initialized and live. Registered
/// diagnostic or guarded callbacks and opaque arguments must be valid for any
/// synchronous invocation, exclude incompatible reentry, and must not unwind.
#[inline]
unsafe fn _mi_theap_init_typed(theap: ThreadHeapPtr, heap: HeapPtr, tld: TldPtr) {
    let theap_raw = theap.as_ptr();
    let tld_raw = tld.as_ptr();
    // SAFETY: the caller exclusively owns this exact writable allocation. Save
    // its backing MemId, copy the initialized empty template, restore the MemId,
    // and establish the initial owner/refcount fields in the original order.
    unsafe {
        let memid = (*theap_raw).memid;
        _mi_memcpy(
            theap_raw.cast(),
            __mi_theap_empty().cast_const().cast(),
            size_of::<mi_theap_t>(),
        );
        (*theap_raw).memid = memid;
        theap.refcount_cell().store_seq_cst(1);
        theap.set_tld(Some(tld));
        theap.set_heap(Some(heap));
        crate::internal::mi_assert_internal!(
            (*theap_raw).stats.size == size_of::<crate::allocator::stats::mi_stats_t>()
        );
    }

    // SAFETY: the newly initialized metadata and live TLD are still exclusively
    // owned. Options are initialized before applying the thread-pool override.
    unsafe {
        // SAFETY: `theap` is live, initialized, current-thread local, and still
        // unpublished; its policy fields are exclusively writable, and option
        // storage plus any diagnostic callback/argument remain valid and
        // non-unwinding for this synchronous snapshot.
        _mi_theap_options_init(theap_raw);
        if (*tld_raw).is_in_threadpool {
            let configured = crate::allocator::policy::ThreadHeapPolicy::from_fields(
                (*theap_raw).allow_page_reclaim,
                (*theap_raw).allow_page_abandon,
                (*theap_raw).page_full_retain,
            );
            let adjusted =
                crate::allocator::policy::ActivePolicy::adjust_for_thread_pool(configured);
            (*theap_raw).page_full_retain = adjusted.page_full_retain().get();
        }
    }

    // SAFETY: the caller owns TLD-list publication, and `theap` has initialized
    // clear template links. Preserve publication before random-state setup and
    // use the returned former head only while both metadata objects stay live.
    let tld_theaps_head = unsafe { lists::push_tld_front(tld, theap) };

    // SAFETY: after TLD publication the caller still excludes concurrent use.
    // The old head, if any, and subprocess/configuration state stay live while
    // random, cookie, guarded state, and stats are initialized in exact order.
    unsafe {
        if let Some(tld_theaps_head) = tld_theaps_head {
            _mi_random_split(
                ptr::addr_of_mut!((*tld_theaps_head.as_ptr()).random),
                ptr::addr_of_mut!((*theap_raw).random),
            );
        } else {
            #[cfg(any(target_os = "macos", windows))]
            crate::internal::_mi_random_init_weak(ptr::addr_of_mut!((*theap_raw).random));
            #[cfg(not(any(target_os = "macos", windows)))]
            _mi_random_init(ptr::addr_of_mut!((*theap_raw).random));
        }
        (*theap_raw).cookie = _mi_random_next(ptr::addr_of_mut!((*theap_raw).random)) | 1;
        // SAFETY: the initialized current-thread heap remains exclusively owned
        // during publication setup; guarded fields/random state are mutable,
        // option storage and diagnostics are valid and non-unwinding, and
        // incompatible reentry is excluded.
        _mi_theap_guarded_init(theap_raw);
        __mi_stat_increase_mt(ptr::addr_of_mut!((*_mi_subproc()).stats.theaps), 1);
    }

    // SAFETY: initialization and accounting are complete; the caller owns the
    // live heap list and `theap` has clear heap-list links for final publication.
    unsafe { lists::push_heap_front(heap, theap) };
}

/// # Safety
///
/// `heap` and `tld` must point to live initialized allocator metadata objects
/// used to own the new thread heap. Their allocation policy, exclusive-arena
/// selection, thread sequence, NUMA node, list heads/locks, options, random
/// state, subprocess association/stats, and guarded configuration must remain
/// stable through allocation and publication. The caller must exclusively own
/// the relevant heap/TLD lifecycle against concurrent teardown or relink.
/// Allocation, commit, purge, guarded, tracking, and diagnostic callbacks and
/// opaque arguments must remain valid for synchronous invocation, exclude
/// incompatible reentry, and must not unwind. On failure nothing is published;
/// on success the returned pointer is the newly initialized, linked thread heap.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_create(
    heap: *mut mi_heap_t,
    tld: *mut mi_tld_t,
) -> *mut mi_theap_t {
    crate::internal::mi_assert_internal!(!tld.is_null());
    crate::internal::mi_assert_internal!(!heap.is_null());

    let Some(heap) = HeapPtr::new(heap) else {
        return ptr::null_mut();
    };
    let Some(tld) = TldPtr::new(tld) else {
        return ptr::null_mut();
    };

    // SAFETY: the raw contract supplies the live heap/TLD owners, stable policy
    // and allocation state, exclusive lifecycle epoch, and callback obligations
    // required by the typed allocation-and-publication transaction.
    unsafe { _mi_theap_create_typed(heap, tld) }
        .map(ThreadHeapPtr::as_ptr)
        .unwrap_or(ptr::null_mut())
}

/// Allocate, initialize, and publish one thread heap.
///
/// # Safety
/// `heap` and `tld` must be live initialized matching owners under an exclusive
/// create/publication epoch. Their allocation policy, exclusive-arena field,
/// thread sequence, NUMA node, list heads/locks, options, random state,
/// subprocess association/stats, and guarded configuration must remain stable.
/// No concurrent teardown, relink, or premature use of the new metadata is
/// permitted. Allocation, commit, purge, guarded, tracking, and diagnostic
/// callbacks and opaque arguments must remain valid for synchronous invocation,
/// exclude incompatible reentry, and must not unwind. Failure publishes no
/// thread heap; success returns the initialized heap linked into both owners.
#[inline]
unsafe fn _mi_theap_create_typed(heap: HeapPtr, tld: TldPtr) -> Option<ThreadHeapPtr> {
    let heap_raw = heap.as_ptr();
    let tld_raw = tld.as_ptr();
    let mut memid = mi_memid_t::default();
    // SAFETY: the caller keeps the initialized heap and its exclusive-arena
    // selection field live and stable for both policy reads in this allocation.
    let use_meta = unsafe { (*heap_raw).exclusive_arena.is_null() };
    let theap = if use_meta {
        // SAFETY: the live metadata allocator and callbacks accept the writable
        // MemId out-slot and return zeroed backing for the requested object.
        unsafe {
            _mi_meta_zalloc(size_of::<mi_theap_t>(), ptr::addr_of_mut!(memid)).cast::<mi_theap_t>()
        }
    } else {
        let size = _mi_align_up(size_of::<mi_theap_t>(), MI_ARENA_MIN_OBJ_SIZE);
        // SAFETY: the contract keeps heap/TLD allocation inputs, arena registry,
        // callbacks, and MemId out-slot valid. Preserve the second arena-field
        // read and the TLD sequence/NUMA reads at the allocation call.
        let theap = unsafe {
            _mi_arenas_alloc(
                heap_raw,
                size,
                true,
                true,
                (*heap_raw).exclusive_arena,
                (*tld_raw).thread_seq,
                crate::core_types::NumaNode::new((*tld_raw).numa_node).get(),
                ptr::addr_of_mut!(memid),
            )
            .cast::<mi_theap_t>()
        };
        if !theap.is_null() {
            let allocated_size = match crate::core_types::MemId::view(memid) {
                crate::core_types::MemIdView::Arena(a) => {
                    a.range().count_usize() * crate::allocator::arena::MI_ARENA_SLICE_SIZE
                }
                crate::core_types::MemIdView::Os(os) => os.size.get(),
                crate::core_types::MemIdView::External(external) => external.size.get(),
                _ => 0,
            };
            crate::internal::mi_assert_internal!(allocated_size >= size);
        }
        theap
    };

    if theap.is_null() {
        _mi_error_message_fmt(
            Errno::ENOMEM,
            format_args!("unable to allocate theap meta-data\n"),
        );
        return None;
    }

    // SAFETY: successful allocation returned live aligned writable backing and
    // matching MemId. Publish the MemId before consuming the uninitialized
    // storage through the complete initialization/list transaction.
    unsafe {
        (*theap).memid = memid;
        let theap = ThreadHeapPtr::new_unchecked(theap);
        _mi_theap_init_typed(theap, heap, tld);
        Some(theap)
    }
}

/// # Safety
///
/// `theap` must point to a valid initialized thread heap.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_random_next(theap: *mut mi_theap_t) -> usize {
    unsafe {
        let theap = ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(theap));
        _mi_theap_random_next_typed(theap)
    }
}

#[inline(always)]
unsafe fn _mi_theap_random_next_typed(theap: ThreadHeapLocal) -> usize {
    unsafe {
        let theap = theap.ptr().as_ptr();
        _mi_random_next(ptr::addr_of_mut!((*theap).random))
    }
}

#[inline]
fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool {
    match crate::core_types::MemId::view(memid) {
        crate::core_types::MemIdView::None
        | crate::core_types::MemIdView::External(_)
        | crate::core_types::MemIdView::Static => false,
        crate::core_types::MemIdView::Unknown(code) => {
            code.0 > crate::core_types::MemKindCode::STATIC.0
        }
        _ => true,
    }
}

/// Release thread-heap metadata after its final reference is consumed.
///
/// # Safety
/// `theap` must identify live allocator-owned metadata whose refcount has just
/// transitioned to zero. `raw_memid` and `memid` must be the same stable MemId
/// snapshot and describe the exact live backing of `theap`; `size` must be the
/// original thread-heap metadata size. No other access, refcount operation,
/// TLS reference, or teardown may race or occur after this call. Subprocess
/// stats and release callbacks/opaque arguments must remain valid for their
/// synchronous use, exclude incompatible reentry, and not unwind. Each branch
/// may release the backing, so the caller must perform no later access through
/// any copy of `theap`.
#[inline(always)]
unsafe fn mi_theap_free_mem_typed(
    theap: ThreadHeapPtr,
    raw_memid: mi_memid_t,
    memid: crate::core_types::MemIdView,
    size: crate::core_types::ReservedBytes,
) {
    let raw = theap.as_ptr();

    // SAFETY: the final-reference transition owns this heap-count decrement;
    // the live subprocess stats field supports its atomic update. This remains
    // ordered before every terminal backing-release branch.
    unsafe {
        __mi_stat_decrease_mt(ptr::addr_of_mut!((*_mi_subproc()).stats.theaps), 1);
    }

    match memid {
        crate::core_types::MemIdView::HeapMain => {
            // SAFETY: the matching HeapMain MemId identifies the main-heap
            // allocation at `raw`. The assertion reads metadata before the
            // terminal free; no thread-heap field is accessed afterward.
            unsafe {
                crate::internal::mi_assert_internal!(_mi_is_heap_main(
                    _mi_theap_heap(raw).cast_const()
                ));
                mi_free(raw.cast());
            }
        }
        crate::core_types::MemIdView::Meta(_) => {
            // SAFETY: the matching Meta MemId and original metadata size name
            // the exact live meta allocation. This terminal call consumes its
            // free transition and no thread-heap field is accessed afterward.
            unsafe { _mi_meta_free(raw.cast(), size.get(), raw_memid) };
        }
        crate::core_types::MemIdView::Arena(_) => {
            let alloc_size = crate::core_types::ReservedBytes::new(_mi_align_up(
                size.get(),
                MI_ARENA_MIN_OBJ_SIZE,
            ));
            // SAFETY: arena allocation rounded the metadata request to this
            // minimum object size; the matching MemId names that exact live
            // range. This terminal call performs the sole backing release.
            unsafe { _mi_arenas_free(raw.cast(), alloc_size.get(), raw_memid) };
        }
        _ => {
            // SAFETY: the matching non-special MemId and original metadata
            // size name the exact live backing. This terminal call performs
            // the sole release and no thread-heap field is accessed afterward.
            unsafe { _mi_arenas_free(raw.cast(), size.get(), raw_memid) };
        }
    }
}

/// Add one live reference to refcounted thread-heap metadata.
///
/// # Safety
/// `theap` must remain live through the AcqRel increment, with a stable MemId
/// and refcount. The caller must already own a live reference or otherwise
/// exclude the final decrement and backing release until this increment lands.
#[inline(always)]
pub(crate) unsafe fn _mi_theap_incref_typed(theap: ThreadHeapPtr) {
    // SAFETY: the caller keeps the MemId and atomic refcount live and excludes
    // the terminal release while this conditional AcqRel increment executes.
    unsafe {
        if mi_memid_is_refcounted((*theap.as_ptr()).memid) {
            theap.refcount_cell().incref_acq_rel();
        }
    }
}

/// Drop one live reference, releasing backing on the final decrement.
///
/// # Safety
/// `theap` must identify live allocator-owned metadata with a stable MemId and
/// one reference owned by the caller. Its refcount, subprocess stats, backing,
/// and release callbacks/opaque arguments must remain valid through the AcqRel
/// decrement and possible terminal free. Concurrent users may act only through
/// valid references. The caller must not access `theap` after this call because
/// it may free the backing.
#[inline(always)]
pub(crate) unsafe fn mi_theap_decref_inline(theap: ThreadHeapPtr) {
    // SAFETY: the owned live reference keeps the initialized MemId field
    // readable and stable through this single raw snapshot.
    let raw_memid = unsafe { ptr::read(ptr::addr_of!((*theap.as_ptr()).memid)) };
    if !mi_memid_is_refcounted(raw_memid) {
        return;
    }

    // SAFETY: the caller owns one live reference, the refcount field remains
    // live for atomic access, and every concurrent user participates through
    // the same reference protocol. AcqRel publishes prior reference use to the
    // thread that observes the terminal transition.
    let previous = unsafe { theap.refcount_cell().decref_acq_rel() };
    if previous != 1 {
        return;
    }

    let memid = crate::core_types::MemId::view(raw_memid);
    let size = crate::core_types::ReservedBytes::new(size_of::<mi_theap_t>());
    // SAFETY: `previous == 1` exclusively owns the terminal release. The MemId
    // is the pre-decrement stable snapshot and `size` is the exact metadata
    // allocation request. Do not access `theap` after this call.
    unsafe { mi_theap_free_mem_typed(theap, raw_memid, memid, size) };
}

/// # Safety
///
/// `theap` must point to live allocator-owned thread-heap metadata whose heap
/// field may be atomically detached by this call. For an initialized heap, its
/// associated heap, TLD, list links, source and destination stats, refcount,
/// MemId, and backing must remain live through the terminal decref. The caller
/// must own the teardown/unlink epoch: no concurrent heap-field detach, list
/// relink, stats reset, TLD clear, or non-refcounted metadata access may occur.
/// If an acquire flag is `true`, this call acquires/releases that list lock; if
/// it is `false`, the caller must already hold the matching heap or TLD lock.
/// Existing references may finish only through the refcount protocol. Release,
/// arena, tracking, and diagnostic callbacks and opaque arguments must remain
/// valid for synchronous invocation, exclude incompatible reentry, and must not
/// unwind. `true` means this call detached and unlinked the heap; `false` means
/// it was uninitialized, deleted, or already detached.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_theap_free(
    theap: *mut mi_theap_t,
    acquire_heap_theaps_lock: bool,
    acquire_tld_theaps_lock: bool,
) -> bool {
    crate::internal::mi_assert!(!theap.is_null());
    let Some(theap) = ThreadHeapPtr::new(theap) else {
        return true;
    };
    // SAFETY: the raw boundary contract keeps all lifecycle metadata and locks
    // live, grants the exclusive detach/unlink epoch, defines each lock flag's
    // polarity, and retains backing validity through terminal decref.
    unsafe { _mi_theap_free_typed(theap, acquire_heap_theaps_lock, acquire_tld_theaps_lock) }
}

/// Atomically detach and unlink one initialized thread heap.
///
/// # Safety
/// `theap` must identify live allocator-owned thread-heap metadata. Its heap
/// field, associated heap and TLD, list links, source/destination stats,
/// refcount, MemId, and backing must remain live until terminal decref. The
/// caller must exclude concurrent detach, list relink, stats reset, TLD clear,
/// teardown, and non-refcounted metadata access. A `true` acquire flag means
/// this function must acquire/release that list lock; a `false` flag means the
/// caller already holds it. Existing users may finish only through refcounting.
/// Release, arena, tracking, and diagnostic callbacks and opaque arguments must
/// remain valid for synchronous invocation, exclude incompatible reentry, and
/// must not unwind. After terminal decref no field of `theap` may be accessed.
#[inline]
unsafe fn _mi_theap_free_typed(
    theap: ThreadHeapPtr,
    acquire_heap_theaps_lock: bool,
    acquire_tld_theaps_lock: bool,
) -> bool {
    // SAFETY: the caller keeps metadata live for state classification and owns
    // the lifecycle epoch against concurrent detach or state mutation.
    let theap = unsafe {
        match theap.state() {
            ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap) => {
                theap.ptr().as_ptr()
            }
            ThreadHeapState::Uninitialized | ThreadHeapState::Deleted => return false,
        }
    };

    // SAFETY: the heap pointer field is live atomic-compatible storage. The
    // AcqRel exchange is the unique detach claim; null reports a prior detach.
    let heap = unsafe {
        crate::core_types::AtomicPtrField::from_raw(ptr::addr_of_mut!((*theap).heap))
            .exchange_acq_rel(ptr::null_mut())
    };
    if heap.is_null() {
        return false;
    }

    // SAFETY: successful detach exclusively owns this source stats reset, and
    // the associated heap's atomic destination stats remain live for the merge.
    unsafe {
        _mi_stats_merge_into(
            ptr::addr_of_mut!((*heap).stats),
            ptr::addr_of_mut!((*theap).stats),
        );
    }

    // SAFETY: the detach claim and lock-flag contract grant the heap-list and
    // TLD-list unlink epochs. Snapshot the live TLD before clearing it, then
    // preserve heap unlink -> TLD unlink -> TLD clear exactly.
    let theap_ptr = unsafe { ThreadHeapPtr::new_unchecked(theap) };
    unsafe {
        let heap = HeapPtr::new_unchecked(heap);
        let tld = TldPtr::new_unchecked((*theap).tld);
        lists::remove_from_heap(heap, theap_ptr, acquire_heap_theaps_lock);
        lists::remove_from_tld(tld, theap_ptr, acquire_tld_theaps_lock);

        theap_ptr.set_tld(None);
    }

    // SAFETY: all metadata reads and list mutations are complete. Consume the
    // final reference under the backing/MemId release contract; do not access
    // `theap` or `theap_ptr` after this call.
    unsafe {
        mi_theap_decref_inline(theap_ptr);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::mi_block_t;
    use core::mem::{MaybeUninit, align_of};

    struct PageTraversalFixture {
        seen_pages: [*mut mi_page_t; 3],
        seen_queues: [*mut mi_page_queue_t; 3],
        len: usize,
        stop_after: Option<usize>,
        detach_first: bool,
    }

    impl PageTraversalFixture {
        fn new(stop_after: Option<usize>, detach_first: bool) -> Self {
            Self {
                seen_pages: [ptr::null_mut(); 3],
                seen_queues: [ptr::null_mut(); 3],
                len: 0,
                stop_after,
                detach_first,
            }
        }
    }

    /// # Safety
    /// `theap`, `pq`, and `page` must be the live stack-local traversal fixture,
    /// and `arg1` must point exclusively to its live `PageTraversalFixture`.
    /// When `detach_first` is set, the first page must have a live successor in
    /// the supplied queue and the fixture must exclusively own all affected
    /// heap, queue, and link fields. The callback is synchronous and must not
    /// retain pointers for dereference after it returns or unwind; it records
    /// page and queue addresses only as non-dereferenced identities for the
    /// test assertions.
    unsafe extern "C" fn record_page_traversal(
        theap: *mut mi_theap_t,
        pq: *mut mi_page_queue_t,
        page: *mut mi_page_t,
        arg1: *mut c_void,
        _arg2: *mut c_void,
    ) -> bool {
        // SAFETY: the test call satisfies the callback contract and keeps all
        // stack-local metadata exclusively live through this invocation.
        unsafe {
            let fixture = &mut *arg1.cast::<PageTraversalFixture>();
            let index = fixture.len;
            if index >= fixture.seen_pages.len() {
                return false;
            }
            fixture.seen_pages[index] = page;
            fixture.seen_queues[index] = pq;
            fixture.len += 1;

            if fixture.detach_first && index == 0 {
                let queue = PageQueuePtr::new_unchecked(pq);
                let current = PagePtr::new_unchecked(page);
                let Some(successor) = current.queue_next() else {
                    return false;
                };
                let queue_count = queue.count().get();
                let page_count = (*theap).page_count;
                if queue_count == 0 || page_count == 0 {
                    return false;
                }
                queue.set_first(Some(successor));
                successor.set_queue_prev(None);
                current.set_queue_next(None);
                current.set_queue_prev(None);
                queue.set_count(crate::core_types::PageQueueCount::new(queue_count - 1));
                (*theap).page_count = page_count - 1;
            }

            match fixture.stop_after {
                Some(stop_after) => fixture.len < stop_after,
                None => true,
            }
        }
    }

    #[test]
    fn visit_dispatch_rejects_null_heap_and_missing_callbacks() {
        // SAFETY: all three entry points explicitly accept null heaps and
        // absent callbacks without dereferencing either raw argument.
        unsafe {
            assert!(!mi_theap_visit_areas(ptr::null(), None, ptr::null_mut()));
            assert!(!mi_theap_visit_blocks(
                ptr::null(),
                false,
                None,
                ptr::null_mut()
            ));
            assert!(!mi_theap_visit_pages(
                ptr::null_mut(),
                None,
                false,
                ptr::null_mut(),
                ptr::null_mut()
            ));
        }
    }

    #[test]
    fn typed_page_traversal_preserves_order_short_circuit_and_saved_successor() {
        let mut theap = MaybeUninit::<mi_theap_t>::zeroed();
        let mut first_page = MaybeUninit::<mi_page_t>::zeroed();
        let mut second_page = MaybeUninit::<mi_page_t>::zeroed();
        let mut full_page = MaybeUninit::<mi_page_t>::zeroed();

        // SAFETY: the stack-local heap receives the complete initialized queue
        // table and every linked page remains live with stable ownership. All
        // topology mutation is exclusive. The final traversal's callback
        // detaches only the current head after the worker has captured its
        // successor, preserving that successor and the remaining topology.
        unsafe {
            let theap_raw = theap.as_mut_ptr();
            (*theap_raw).pages = crate::allocator::init::MI_PAGE_QUEUES_EMPTY();
            (*theap_raw).page_count = 3;
            let theap = ThreadHeapPtr::new_unchecked(theap_raw);
            let block_size = crate::core_types::BlockSize::new_unchecked(64);
            let normal_queue =
                theap.page_queue(crate::allocator::page::mi_bin_from_block_size(block_size));
            let full_queue = theap.page_queue(Bin::new_unchecked(MI_BIN_FULL as u16));
            let thread_id = crate::core_types::ThreadId::new(0x1000)
                .expect("fixture thread id must not overlap page flag bits");
            let active_owner = crate::core_types::PageThreadId::from_parts(
                crate::core_types::PageOwnerId::Thread(thread_id),
                crate::core_types::PageFlags::new(0).expect("zero page flags"),
            )
            .raw();
            let full_owner = crate::core_types::PageThreadId::from_parts(
                crate::core_types::PageOwnerId::Thread(thread_id),
                crate::core_types::PageFlags::IN_FULL_QUEUE,
            )
            .raw();

            let first_raw = first_page.as_mut_ptr();
            let second_raw = second_page.as_mut_ptr();
            let full_raw = full_page.as_mut_ptr();
            for page in [first_raw, second_raw] {
                (*page).theap = theap_raw;
                (*page).block_size = block_size.get();
                (*page).xthread_id = active_owner;
            }
            (*full_raw).theap = theap_raw;
            (*full_raw).block_size = block_size.get();
            (*full_raw).xthread_id = full_owner;

            let first = PagePtr::new_unchecked(first_raw);
            let second = PagePtr::new_unchecked(second_raw);
            let full = PagePtr::new_unchecked(full_raw);
            first.set_queue_prev(None);
            first.set_queue_next(Some(second));
            second.set_queue_prev(Some(first));
            second.set_queue_next(None);
            full.set_queue_prev(None);
            full.set_queue_next(None);
            normal_queue.set_first(Some(first));
            normal_queue.set_last(Some(second));
            normal_queue.set_count(crate::core_types::PageQueueCount::new(2));
            full_queue.set_first(Some(full));
            full_queue.set_last(Some(full));
            full_queue.set_count(crate::core_types::PageQueueCount::new(1));

            let mut normal_only = PageTraversalFixture::new(None, false);
            let normal_state = TheapPageVisitState {
                visitor: TheapPageVisitCallback::new(Some(record_page_traversal)),
                arg1: ptr::addr_of_mut!(normal_only).cast(),
                arg2: ptr::null_mut(),
            };
            assert!(mi_theap_visit_pages_typed(theap, false, normal_state));
            assert_eq!(normal_only.len, 2);
            assert_eq!(&normal_only.seen_pages[..2], &[first_raw, second_raw]);
            assert_eq!(
                &normal_only.seen_queues[..2],
                &[normal_queue.as_ptr(), normal_queue.as_ptr()]
            );

            let mut with_full = PageTraversalFixture::new(None, false);
            let full_state = TheapPageVisitState {
                visitor: TheapPageVisitCallback::new(Some(record_page_traversal)),
                arg1: ptr::addr_of_mut!(with_full).cast(),
                arg2: ptr::null_mut(),
            };
            assert!(mi_theap_visit_pages_typed(theap, true, full_state));
            assert_eq!(with_full.len, 3);
            assert_eq!(with_full.seen_pages, [first_raw, second_raw, full_raw]);

            let mut stopped = PageTraversalFixture::new(Some(2), false);
            let stopped_state = TheapPageVisitState {
                visitor: TheapPageVisitCallback::new(Some(record_page_traversal)),
                arg1: ptr::addr_of_mut!(stopped).cast(),
                arg2: ptr::null_mut(),
            };
            assert!(!mi_theap_visit_pages_typed(theap, true, stopped_state));
            assert_eq!(stopped.len, 2);
            assert_eq!(&stopped.seen_pages[..2], &[first_raw, second_raw]);

            let mut detached = PageTraversalFixture::new(None, true);
            let detached_state = TheapPageVisitState {
                visitor: TheapPageVisitCallback::new(Some(record_page_traversal)),
                arg1: ptr::addr_of_mut!(detached).cast(),
                arg2: ptr::null_mut(),
            };
            assert!(mi_theap_visit_pages_typed(theap, true, detached_state));
            assert_eq!(detached.seen_pages, [first_raw, second_raw, full_raw]);
            assert_eq!(normal_queue.first(), Some(second));
            assert_eq!(normal_queue.last(), Some(second));
            assert_eq!(normal_queue.count().get(), 1);
            assert_eq!((*theap_raw).page_count, 2);
        }
    }

    #[test]
    fn heap_area_for_page_preserves_all_callback_fields() {
        let mut blocks = [MaybeUninit::<mi_block_t>::uninit(); 8];
        let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
        let full_block_size = 2 * size_of::<mi_block_t>();

        // SAFETY: the fixture initializes every page geometry/count field read
        // by the projection, and its complete block backing remains live.
        unsafe {
            let raw = raw_page.as_mut_ptr();
            ptr::addr_of_mut!((*raw).page_start).write(blocks.as_mut_ptr().cast());
            ptr::addr_of_mut!((*raw).block_size).write(full_block_size);
            ptr::addr_of_mut!((*raw).reserved).write(4);
            ptr::addr_of_mut!((*raw).capacity).write(2);
            ptr::addr_of_mut!((*raw).used).write(1);
            let page = PagePtr::new_unchecked(raw);
            let area = heap_area_for_page(page).expect("valid page projects a heap area");

            assert_eq!(area.blocks, blocks.as_mut_ptr().cast());
            assert_eq!(area.reserved, 4 * full_block_size);
            assert_eq!(area.committed, 2 * full_block_size);
            assert_eq!(area.used, 1);
            assert_eq!(
                area.block_size,
                full_block_size - crate::types::MI_PADDING_SIZE
            );
            assert_eq!(area.full_block_size, full_block_size);
            assert_eq!(area.reserved1, raw.cast());
        }
    }

    #[test]
    fn heap_area_for_page_rejects_overflowing_geometry() {
        let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
        let alignment = align_of::<mi_block_t>();
        let overflowing_block_size = usize::MAX - (usize::MAX % alignment);

        // SAFETY: PageBlockLayout is geometry-only and rejects the overflowing
        // reserved-byte product before deriving or accessing block storage.
        unsafe {
            let raw = raw_page.as_mut_ptr();
            ptr::addr_of_mut!((*raw).page_start)
                .write(ptr::without_provenance_mut::<u8>(alignment));
            ptr::addr_of_mut!((*raw).block_size).write(overflowing_block_size);
            ptr::addr_of_mut!((*raw).reserved).write(2);
            ptr::addr_of_mut!((*raw).capacity).write(0);
            ptr::addr_of_mut!((*raw).used).write(0);
            assert!(heap_area_for_page(PagePtr::new_unchecked(raw)).is_none());
        }
    }
}
