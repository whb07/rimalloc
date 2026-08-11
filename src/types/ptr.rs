//! DT-2 pointer domain types (zero-cost newtypes over NonNull).
//! All types are #[repr(transparent)], Copy, Clone, PartialEq, Eq, etc.
//! They do not implement Deref or DerefMut.

use super::atomic::{
    AtomicPageThreadId, AtomicThreadFree, AtomicThreadHeapRefcount, AtomicUsizeField, MemoryOrder,
    MiLockPtr,
};
use super::constants::{
    mi_arena_t, mi_block_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_subproc_t, mi_theap_t,
    mi_tld_t,
};
use super::freelist_wrappers::{AvailableFreeList, BlockLink, EncodedPtr, LocalFreeList, PageKeys};
use super::page::ThreadId;
use super::scalar::{
    ArenaCount, ArenaIndex, Bin, BlockSize, DirectBin, Heartbeat, NumaNode, PageCapacity,
    PageQueueCount, ReservedBlockCount, SliceCount, ThreadSeq, UsedBlockCount,
};
use core::ffi::c_void;
use core::ptr::{self, NonNull};

use crate::allocator::stats::mi_stats_t;
use crate::atomic::mi_lock_t;
use crate::internal::memid::mi_memid_t;

macro_rules! define_ptr_wrapper {
    (@base $name:ident, $raw_type:ty, $doc:expr) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub(crate) struct $name(NonNull<$raw_type>);

        impl $name {
            #[inline(always)]
            pub(crate) fn as_ptr(self) -> *mut $raw_type {
                self.0.as_ptr()
            }
        }
    };
    (@new $name:ident, $raw_type:ty) => {
        impl $name {
            #[inline(always)]
            pub(crate) fn new(ptr: *mut $raw_type) -> Option<Self> {
                NonNull::new(ptr).map(Self)
            }
        }
    };
    (@unchecked $name:ident, $raw_type:ty) => {
        impl $name {
            /// # Safety
            /// `ptr` must be non-null and point to valid metadata or data according to the type's invariants.
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            #[inline(always)]
            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self {
                Self(unsafe { NonNull::new_unchecked(ptr) })
            }

        }
    };
    ($name:ident, $raw_type:ty, $doc:expr) => {
        define_ptr_wrapper!(@base $name, $raw_type, $doc);
        define_ptr_wrapper!(@new $name, $raw_type);
        define_ptr_wrapper!(@unchecked $name, $raw_type);
    };
    ($name:ident, $raw_type:ty, $doc:expr, no_new) => {
        define_ptr_wrapper!(@base $name, $raw_type, $doc);
        define_ptr_wrapper!(@unchecked $name, $raw_type);
    };
    ($name:ident, $raw_type:ty, $doc:expr, no_unchecked) => {
        define_ptr_wrapper!(@base $name, $raw_type, $doc);
        define_ptr_wrapper!(@new $name, $raw_type);
    };
}

#[inline(always)]
pub(crate) fn ptr_exposed_addr<T>(ptr: *const T) -> usize {
    #[cfg(miri)]
    {
        ptr.addr()
    }

    #[cfg(not(miri))]
    {
        ptr as usize
    }
}

#[inline(always)]
pub(crate) fn ptr_addr_diff<T, U>(hi: *const T, lo: *const U) -> usize {
    ptr_exposed_addr(hi).wrapping_sub(ptr_exposed_addr(lo))
}

#[inline(always)]
pub(crate) fn ptr_is_aligned<T>(ptr: *const T, alignment: usize) -> bool {
    alignment == 0 || ptr_exposed_addr(ptr).is_multiple_of(alignment)
}

#[inline(always)]
pub(crate) fn ptr_align_up<T>(ptr: *const T, alignment: usize) -> *mut c_void {
    #[cfg(miri)]
    {
        ptr.cast::<u8>()
            .map_addr(|addr| crate::internal::_mi_align_up(addr, alignment))
            .cast_mut()
            .cast::<c_void>()
    }

    #[cfg(not(miri))]
    {
        crate::internal::_mi_align_up(ptr_exposed_addr(ptr), alignment) as *mut c_void
    }
}

#[inline(always)]
pub(crate) fn ptr_align_down<T>(ptr: *const T, alignment: usize) -> *mut c_void {
    #[cfg(miri)]
    {
        ptr.cast::<u8>()
            .map_addr(|addr| crate::internal::_mi_align_down(addr, alignment))
            .cast_mut()
            .cast::<c_void>()
    }

    #[cfg(not(miri))]
    {
        crate::internal::_mi_align_down(ptr_exposed_addr(ptr), alignment) as *mut c_void
    }
}

define_ptr_wrapper!(PagePtr, mi_page_t, "Non-null, aligned raw page metadata");

impl PagePtr {
    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn block_size(self) -> BlockSize {
        let raw = self.as_ptr();
        let block_size = unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) };
        crate::internal::mi_assert_internal!(block_size > 0);
        // SAFETY: asserted above; initialized pages keep block_size non-zero.
        // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe { BlockSize::new_unchecked(block_size) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata, or to fresh
    /// metadata at a boundary where a zero block-size sentinel is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn block_size_raw(self) -> usize {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn used(self) -> UsedBlockCount {
        let raw = self.as_ptr();
        UsedBlockCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).used)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_used(self, used: UsedBlockCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).used), used.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata satisfying
    /// `used <= capacity <= reserved`.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn capacity(self) -> PageCapacity {
        let raw = self.as_ptr();
        let used = unsafe { self.used() };
        let reserved = unsafe { self.reserved() };
        // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
        let capacity = unsafe { ptr::read(ptr::addr_of!((*raw).capacity)) };
        crate::internal::mi_assert_internal!(used.get() <= capacity);
        crate::internal::mi_assert_internal!(capacity <= reserved.get());
        PageCapacity::new(used, capacity, reserved)
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            .unwrap_or_else(|| unsafe { PageCapacity::new_unchecked(capacity) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_capacity(self, capacity: PageCapacity) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).capacity), capacity.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn reserved(self) -> ReservedBlockCount {
        let raw = self.as_ptr();
        ReservedBlockCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).reserved)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_reserved(self, reserved: ReservedBlockCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).reserved), reserved.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn free(self) -> AvailableFreeList {
        let raw = self.as_ptr();
        unsafe { AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free))) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn free_raw(self) -> *mut mi_block_t {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).free)) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_free(self, free: AvailableFreeList) {
        let raw = self.as_ptr();
        let head = free.as_raw();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).free), head) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn local_free(self) -> LocalFreeList {
        let raw = self.as_ptr();
        unsafe { LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free))) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_local_free(self, local_free: LocalFreeList) {
        let raw = self.as_ptr();
        let head = local_free.as_raw();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).local_free), head) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata whose queue
    /// links may be inspected by the page-queue protocol.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn queue_next(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).next)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata whose queue links
    /// are owned by the current page-queue mutation.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_queue_next(self, next: Option<PagePtr>) {
        let raw = self.as_ptr();
        let next = next.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).next), next) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata whose queue
    /// links may be inspected by the page-queue protocol.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn queue_prev(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).prev)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata whose queue links
    /// are owned by the current page-queue mutation.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_queue_prev(self, prev: Option<PagePtr>) {
        let raw = self.as_ptr();
        let prev = prev.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).prev), prev) };
    }

    /// # Safety
    /// The page pointer must refer to page metadata whose `xthread_id` field can
    /// be accessed through the allocator's atomic field protocol.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_id_cell(self) -> AtomicPageThreadId {
        let raw = self.as_ptr();
        AtomicPageThreadId::from_field(AtomicUsizeField::from_raw(unsafe {
            ptr::addr_of_mut!((*raw).xthread_id)
        }))
    }

    /// # Safety
    /// The page pointer must refer to page metadata whose `xthread_free` field can
    /// be accessed through the allocator's atomic field protocol.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_free_cell(self) -> AtomicThreadFree {
        let raw = self.as_ptr();
        AtomicThreadFree::from_field(AtomicUsizeField::from_raw(unsafe {
            ptr::addr_of_mut!((*raw).xthread_free)
        }))
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn keys(self) -> PageKeys {
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        {
            let raw = self.as_ptr();
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            PageKeys::from_raw(unsafe { ptr::read(ptr::addr_of!((*raw).keys)) })
        }

        #[cfg(not(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        )))]
        {
            let _ = self;
            PageKeys::from_raw([0, 0])
        }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_keys(self, keys: PageKeys) {
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        {
            let raw = self.as_ptr();
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            unsafe { ptr::write(ptr::addr_of_mut!((*raw).keys), *keys.as_raw()) };
        }

        #[cfg(not(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        )))]
        {
            let _ = (self, keys);
        }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata with a non-null
    /// block-area start.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn start(self) -> PageStart {
        let raw = self.as_ptr();
        unsafe { PageStart::new_unchecked(ptr::read(ptr::addr_of!((*raw).page_start))) }
    }

    /// # Safety
    /// The page pointer must refer to live initialized page metadata whose
    /// block-area range is valid for address comparisons.
    #[inline(always)]
    pub(crate) unsafe fn area(self) -> Option<PageArea> {
        // SAFETY: the caller proves the page metadata and its non-null block
        // start are initialized for this read-only range projection.
        unsafe { PageArea::from_page(self) }
    }
}

define_ptr_wrapper!(
    BlockPtr,
    mi_block_t,
    "Non-null block pointer, block-aligned for its page"
);

impl BlockPtr {
    /// # Safety
    /// The block pointer must refer to a block whose free-list link word is
    /// accessible under the current tracking state.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn encoded_link(self) -> EncodedPtr<mi_block_t> {
        unsafe { EncodedPtr::read_word_from_raw(self.as_ptr()) }
    }

    /// # Safety
    /// The block pointer must refer to a writable block link word.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn write_link(self, link: BlockLink) {
        unsafe { link.write_to(self) };
    }

    /// # Safety
    /// `page` must be the page that owns this block link.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn next(self, page: PagePtr) -> Option<BlockPtr> {
        unsafe {
            crate::allocator::tracking::mi_track_mem_defined(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
        let link = unsafe { self.encoded_link() };
        let next = if crate::types::MI_ENCODE_FREELIST != 0 {
            unsafe { link.decode_block(page, page.keys()) }
        } else {
            BlockPtr::new(link.as_unencoded_raw())
        };
        // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe {
            crate::allocator::tracking::mi_track_mem_noaccess(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        next
    }

    /// # Safety
    /// `page` must be the page that owns this block link, and `next` must be
    /// null or a block in the same encoded free-list domain.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_next(self, page: PagePtr, next: Option<BlockPtr>) {
        unsafe {
            crate::allocator::tracking::mi_track_mem_undefined(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        let link = if crate::types::MI_ENCODE_FREELIST != 0 {
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            unsafe { EncodedPtr::encode_block(next, page, page.keys()) }
        } else {
            EncodedPtr::from_unencoded_raw(
                next.map(BlockPtr::as_ptr)
                    .unwrap_or(ptr::null_mut())
                    .cast_const(),
            )
        };
        // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe { self.write_link(BlockLink::FreeList(link)) };
        unsafe {
            crate::allocator::tracking::mi_track_mem_noaccess(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
    }
}
define_ptr_wrapper!(HeapPtr, mi_heap_t, "Non-null raw heap metadata");

impl HeapPtr {
    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps(self) -> Option<ThreadHeapPtr> {
        let raw = self.as_ptr();
        ThreadHeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).theaps)) })
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_heaps(self, theaps: Option<ThreadHeapPtr>) {
        let raw = self.as_ptr();
        let theaps = theaps.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theaps), theaps) };
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose lock field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).theaps_lock) }
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose lock field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_pages_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).arena_pages_lock) }
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose stats field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}

define_ptr_wrapper!(
    ThreadHeapPtr,
    mi_theap_t,
    "Non-null raw thread heap metadata"
);

impl ThreadHeapPtr {
    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_tld(self, tld: Option<TldPtr>) {
        let raw = self.as_ptr();
        let tld = tld.map(TldPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).tld), tld) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose
    /// refcount is valid for atomic access.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount {
        let raw = self.as_ptr();
        AtomicThreadHeapRefcount::from_field(AtomicUsizeField::from_raw(unsafe {
            ptr::addr_of_mut!((*raw).refcount)
        }))
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heap(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heap), heap) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to initialized thread-heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heartbeat(self) -> Heartbeat {
        let raw = self.as_ptr();
        Heartbeat::new(unsafe { ptr::read(ptr::addr_of!((*raw).heartbeat)) })
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heartbeat(self, heartbeat: Heartbeat) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heartbeat), heartbeat.get()) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose page
    /// queue at `bin` is initialized.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn page_queue(self, bin: Bin) -> PageQueuePtr {
        let raw = self.as_ptr();
        unsafe { PageQueuePtr::new_unchecked(ptr::addr_of_mut!((*raw).pages[bin.as_usize()])) }
    }

    /// # Safety
    /// The thread-heap pointer must refer to initialized thread-heap metadata.
    /// Every direct-cache slot must contain either the non-null empty-page
    /// sentinel or a live page pointer.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn direct_page(self, bin: DirectBin) -> PagePtr {
        let raw = self.as_ptr();
        // SAFETY: the initialized direct cache never contains null slots.
        unsafe {
            PagePtr::new_unchecked(ptr::read(ptr::addr_of!(
                (*raw).pages_free_direct[bin.get() as usize]
            )))
        }
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata, and
    /// `start <= end`. `page` must be the initialized empty-page sentinel or a
    /// live page pointer suitable for every direct-cache slot in the range.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn fill_direct_pages(self, start: DirectBin, end: DirectBin, page: PagePtr) {
        let raw = self.as_ptr();
        let start = start.as_usize();
        let end = end.as_usize();
        crate::internal::mi_assert_internal!(start <= end);
        let page = page.as_ptr();
        // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe {
            let pages = ptr::addr_of_mut!((*raw).pages_free_direct).cast::<*mut mi_page_t>();
            core::slice::from_raw_parts_mut(pages.add(start), end - start + 1).fill(page);
        }
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose stats
    /// field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}
define_ptr_wrapper!(TldPtr, mi_tld_t, "Non-null thread-local data");

impl TldPtr {
    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata. `thread_id`
    /// must be `None` only for the main-thread sentinel state.
    // SAFETY: TLD thread identity fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_id(self, thread_id: Option<ThreadId>) {
        let raw = self.as_ptr();
        let thread_id = thread_id.map(ThreadId::get).unwrap_or(0);
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).thread_id), thread_id) };
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY: TLD thread identity fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_seq(self, seq: ThreadSeq) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).thread_seq), seq.get()) };
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY: TLD thread identity fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY: TLD subproc, stats, and arena forwarding stays behind optional typed witnesses.
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY: TLD subproc, stats, and arena forwarding stays behind optional typed witnesses.
    #[inline(always)]
    pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>) {
        let raw = self.as_ptr();
        let subproc = subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).subproc), subproc) };
    }

    /// # Safety
    /// The TLD pointer must refer to thread-local metadata whose memid field is valid.
    // SAFETY: TLD memid, thread-heap, recursion, and thread-pool fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).memid) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY: TLD memid, thread-heap, recursion, and thread-pool fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps(self) -> Option<ThreadHeapPtr> {
        let raw = self.as_ptr();
        ThreadHeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).theaps)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY: TLD memid, thread-heap, recursion, and thread-pool fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_heaps(self, theaps: Option<ThreadHeapPtr>) {
        let raw = self.as_ptr();
        let theaps = theaps.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theaps), theaps) };
    }

    /// # Safety
    /// The TLD pointer must refer to thread-local metadata whose lock field is valid.
    // SAFETY: TLD memid, thread-heap, recursion, and thread-pool fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).theaps_lock) }
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY: TLD memid, thread-heap, recursion, and thread-pool fields stay behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_is_in_threadpool(self, is_in_threadpool: bool) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).is_in_threadpool), is_in_threadpool) };
    }
}
define_ptr_wrapper!(ArenaPtr, mi_arena_t, "Non-null raw arena metadata");

impl ArenaPtr {
    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_index(self, index: ArenaIndex) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).arena_idx), index.get() as usize) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata with non-zero
    /// slice count.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn slice_count(self) -> SliceCount {
        let raw = self.as_ptr();
        let count = unsafe { ptr::read(ptr::addr_of!((*raw).slice_count)) };
        crate::internal::mi_assert_internal!(count != 0);
        SliceCount::from_usize(count).unwrap_or_else(|| {
            crate::internal::mi_assert_internal!(false);
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            unsafe { SliceCount::new_unchecked(1) }
        })
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn is_exclusive(self) -> bool {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).is_exclusive)) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn parent(self) -> Option<ArenaPtr> {
        let raw = self.as_ptr();
        ArenaPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).parent)) })
    }
}
define_ptr_wrapper!(SubprocPtr, mi_subproc_t, "Non-null raw subproc metadata");

impl SubprocPtr {
    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField::from_raw(unsafe { ptr::addr_of_mut!((*raw).arena_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount {
        ArenaCount::new(unsafe { self.arena_count_cell().load(order) })
    }

    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena(self, index: ArenaIndex) -> Option<ArenaPtr> {
        let raw = self.as_ptr();
        ArenaPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).arenas[index.get() as usize])) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose lock field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_reserve_lock(self) -> MiLockPtr {
        let raw = self.as_ptr();
        MiLockPtr::from_nonnull(unsafe {
            NonNull::new_unchecked(ptr::addr_of_mut!((*raw).arena_reserve_lock))
        })
    }

    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heaps(self) -> Option<HeapPtr> {
        let raw = self.as_ptr();
        HeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).heaps)) })
    }

    /// # Safety
    /// The subproc pointer must refer to writable subproc metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heaps(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heaps), heap) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose lock field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heaps_lock(self) -> MiLockPtr {
        let raw = self.as_ptr();
        MiLockPtr::from_nonnull(unsafe {
            NonNull::new_unchecked(ptr::addr_of_mut!((*raw).heaps_lock))
        })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField::from_raw(unsafe {
            ptr::addr_of_mut!((*raw).thread_count).cast::<usize>()
        })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_total_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField::from_raw(unsafe {
            ptr::addr_of_mut!((*raw).thread_total_count).cast::<usize>()
        })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heap_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField::from_raw(unsafe { ptr::addr_of_mut!((*raw).heap_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose stats field is valid.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}
define_ptr_wrapper!(PageQueuePtr, mi_page_queue_t, "Non-null page queue", no_new);

impl PageQueuePtr {
    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn first(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).first)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_first(self, first: Option<PagePtr>) {
        let raw = self.as_ptr();
        let first = first.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).first), first) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn last(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).last)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_last(self, last: Option<PagePtr>) {
        let raw = self.as_ptr();
        let last = last.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).last), last) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn count(self) -> PageQueueCount {
        let raw = self.as_ptr();
        PageQueueCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).count)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_count(self, count: PageQueueCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).count), count.get()) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn block_size(self) -> Option<BlockSize> {
        let raw = self.as_ptr();
        let block_size = unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) };
        if block_size == 0 {
            None
        } else {
            // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
            Some(unsafe { BlockSize::new_unchecked(block_size) })
        }
    }
}

define_ptr_wrapper!(UserPtr, c_void, "Non-null pointer returned to/from user");
define_ptr_wrapper!(
    InteriorUserPtr,
    c_void,
    "User pointer may not equal block start",
    no_unchecked
);

impl InteriorUserPtr {
    #[inline(always)]
    pub(crate) fn as_u8_ptr(self) -> *mut u8 {
        self.as_ptr().cast::<u8>()
    }
}

define_ptr_wrapper!(PageStart, u8, "Start of page block area", no_new);
define_ptr_wrapper!(SliceStart, u8, "Aligned arena slice start", no_new);
define_ptr_wrapper!(OsMemPtr, c_void, "OS allocation base");

/// Geometry for a non-null start and a non-overflowing half-open exposed-address interval.
///
/// This value supports address arithmetic only. It does not prove backing
/// allocation coverage, provenance, ownership, or liveness, and it creates no
/// slice or reference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageArea {
    start: PageStart,
    byte_len: usize,
}

impl PageArea {
    #[inline(always)]
    fn from_start_and_len(start: PageStart, byte_len: usize) -> Option<Self> {
        ptr_exposed_addr(start.as_ptr()).checked_add(byte_len)?;
        Some(Self { start, byte_len })
    }

    /// # Safety
    /// `page` must refer to live initialized page metadata whose non-null
    /// block-area start and reserved block count describe its current range.
    #[inline(always)]
    unsafe fn from_page(page: PagePtr) -> Option<Self> {
        // SAFETY: the caller proves the page metadata and block-area start are
        // initialized; typed accessors preserve their scalar invariants.
        let start = unsafe { page.start() };
        // SAFETY: the caller proves initialized page sizing metadata.
        let byte_len = unsafe {
            page.block_size()
                .get()
                .checked_mul(page.reserved().as_usize())?
        };
        Self::from_start_and_len(start, byte_len)
    }

    #[inline(always)]
    pub(crate) fn start(self) -> PageStart {
        self.start
    }

    #[inline(always)]
    pub(crate) fn byte_len(self) -> usize {
        self.byte_len
    }

    #[inline(always)]
    pub(crate) fn start_address(self) -> usize {
        ptr_exposed_addr(self.start.as_ptr())
    }

    #[inline(always)]
    pub(crate) fn end_address(self) -> usize {
        // Construction proves this addition cannot overflow.
        self.start_address() + self.byte_len
    }

    #[inline(always)]
    pub(crate) fn offset_of<T>(self, ptr: *const T) -> Option<usize> {
        self.contains(ptr)
            .then(|| ptr_exposed_addr(ptr) - self.start_address())
    }

    #[inline(always)]
    pub(crate) fn contains<T>(self, ptr: *const T) -> bool {
        let address = ptr_exposed_addr(ptr);
        self.start_address() <= address && address < self.end_address()
    }
}

/// Non-owning snapshot of a page's fixed-size block geometry.
///
/// This value validates the reserved block interval and can derive raw block
/// pointers within it. It does not extend the backing allocation lifetime or
/// prove ownership, initialization of block contents, or permission to access
/// a returned pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageBlockLayout {
    area: PageArea,
    block_size: BlockSize,
    reserved: ReservedBlockCount,
}

/// Validated non-empty block-index interval within a [`PageBlockLayout`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageBlockRange {
    first: BlockPtr,
    block_size: BlockSize,
    block_count: usize,
}

impl PageBlockLayout {
    #[inline(always)]
    fn from_parts(
        start: PageStart,
        block_size: BlockSize,
        reserved: ReservedBlockCount,
    ) -> Option<Self> {
        let alignment = core::mem::align_of::<mi_block_t>();
        if !ptr_is_aligned(start.as_ptr(), alignment) || !block_size.get().is_multiple_of(alignment)
        {
            return None;
        }
        let byte_len = block_size.get().checked_mul(reserved.as_usize())?;
        let area = PageArea::from_start_and_len(start, byte_len)?;
        Some(Self {
            area,
            block_size,
            reserved,
        })
    }

    /// # Safety
    /// `page` must refer to live initialized page metadata whose non-null block
    /// start, block size, and reserved count describe the current backing
    /// allocation. Concurrent mutation or teardown must be excluded while the
    /// snapshot and pointers derived from it are used.
    #[inline(always)]
    pub(crate) unsafe fn from_page(page: PagePtr, expected: BlockSize) -> Option<Self> {
        // SAFETY: the caller proves all three page geometry fields are
        // initialized and stable for this snapshot.
        let (start, block_size, reserved) =
            unsafe { (page.start(), page.block_size(), page.reserved()) };
        if block_size != expected {
            return None;
        }
        Self::from_parts(start, block_size, reserved)
    }

    #[inline(always)]
    pub(crate) fn area(self) -> PageArea {
        self.area
    }

    #[inline(always)]
    fn block_at_validated(self, index: usize) -> BlockPtr {
        let byte_offset = index * self.block_size.get();
        let address = self.area.start_address() + byte_offset;
        let block = self
            .area
            .start()
            .as_ptr()
            .map_addr(|_| address)
            .cast::<mi_block_t>();
        // SAFETY: construction validated the full non-overflowing reserved
        // interval, and `index` is inside it.
        unsafe { BlockPtr::new_unchecked(block) }
    }

    #[inline(always)]
    pub(crate) fn nonempty_range(
        self,
        first_index: usize,
        block_count: usize,
    ) -> Option<PageBlockRange> {
        if block_count == 0 {
            return None;
        }
        let end = first_index.checked_add(block_count)?;
        if end > self.reserved.as_usize() {
            return None;
        }
        Some(PageBlockRange {
            first: self.block_at_validated(first_index),
            block_size: self.block_size,
            block_count,
        })
    }
}

impl PageBlockRange {
    #[inline(always)]
    pub(crate) fn block_size(self) -> BlockSize {
        self.block_size
    }

    #[inline(always)]
    pub(crate) fn block_count(self) -> usize {
        self.block_count
    }

    #[inline(always)]
    pub(crate) fn first(self) -> BlockPtr {
        self.first
    }

    #[inline(always)]
    pub(crate) fn last(self) -> BlockPtr {
        // SAFETY: every constructed range is non-empty.
        unsafe { self.block_at_offset_unchecked(self.block_count - 1) }
    }

    /// # Safety
    /// `offset` must be strictly less than this range's block count.
    #[inline(always)]
    unsafe fn block_at_offset_unchecked(self, offset: usize) -> BlockPtr {
        let byte_offset = offset * self.block_size.get();
        let address = ptr_exposed_addr(self.first.as_ptr()) + byte_offset;
        let block = self
            .first
            .as_ptr()
            .cast::<u8>()
            .map_addr(|_| address)
            .cast::<mi_block_t>();
        // SAFETY: the caller bounds the offset; range construction proved the
        // full interval, alignment, and non-overflowing address arithmetic.
        unsafe { BlockPtr::new_unchecked(block) }
    }
}

/// Non-null raw page metadata (read-only view).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageConstPtr(NonNull<mi_page_t>);

impl PageConstPtr {
    /// # Safety
    /// `ptr` must be non-null and point to valid page metadata.
    // SAFETY: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: *const mi_page_t) -> Self {
        Self(unsafe { NonNull::new_unchecked(ptr as *mut mi_page_t) })
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *const mi_page_t {
        self.0.as_ptr() as *const mi_page_t
    }
}

#[cfg(test)]
mod tests;
