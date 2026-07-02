//! DT-2 pointer domain types (zero-cost newtypes over NonNull).
//! All types are #[repr(transparent)], Copy, Clone, PartialEq, Eq, etc.
//! They do not implement Deref or DerefMut.

use core::ffi::c_void;
use core::ptr::{self, NonNull};
use core::sync::atomic::AtomicPtr;

use super::atomic::{
    AtomicI64Field, AtomicPageThreadId, AtomicPtrField, AtomicThreadFree, AtomicThreadHeapRefcount,
    AtomicUsizeField, MemoryOrder, MiLockPtr,
};
use super::constants::{
    mi_arena_pages_t, mi_arena_t, mi_block_t, mi_heap_t, mi_page_queue_t, mi_page_t, mi_subproc_t,
    mi_theap_t, mi_thread_local_t, mi_tld_t,
};
use super::freelist_wrappers::{AvailableFreeList, BlockLink, EncodedPtr, LocalFreeList, PageKeys};
use super::page::ThreadId;
use super::scalar::{
    ArenaCount, ArenaIndex, Bin, BlockSize, DirectBin, HeapCount, Heartbeat, NumaNode,
    PageCapacity, PageQueueCount, ReservedBlockCount, SliceCount, SubprocSeq, ThreadCount,
    ThreadSeq, UsedBlockCount,
};

use crate::atomic::mi_lock_t;
use crate::bitmap::{mi_bbitmap_t, mi_bitmap_t};
use crate::internal::mi_memid_t;
use crate::stats::mi_stats_t;

macro_rules! define_ptr_wrapper {
    ($name:ident, $raw_type:ty, $doc:expr) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub(crate) struct $name(NonNull<$raw_type>);

        impl $name {
            #[inline(always)]
            pub(crate) fn new(ptr: *mut $raw_type) -> Option<Self> {
                NonNull::new(ptr).map(Self)
            }

            /// # Safety
            /// `ptr` must be non-null and point to valid metadata or data according to the type's invariants.
            // SAFETY[AUDIT:typed-pointer-constructor-boundary]: caller-proven metadata access stays behind transparent NonNull wrappers.
            #[inline(always)]
            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self {
                Self(unsafe { NonNull::new_unchecked(ptr) })
            }

            #[inline(always)]
            pub(crate) fn as_ptr(self) -> *mut $raw_type {
                self.0.as_ptr()
            }
        }
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

#[inline(always)]
fn checked_non_null_byte_add(ptr: NonNull<u8>, offset: usize) -> Option<NonNull<u8>> {
    let raw = ptr.as_ptr();
    ptr_exposed_addr(raw).checked_add(offset)?;
    NonNull::new(raw.wrapping_add(offset))
}

#[inline(always)]
fn checked_non_null_byte_sub(ptr: NonNull<u8>, offset: usize) -> Option<NonNull<u8>> {
    let raw = ptr.as_ptr();
    ptr_exposed_addr(raw).checked_sub(offset)?;
    NonNull::new(raw.wrapping_sub(offset))
}

define_ptr_wrapper!(PagePtr, mi_page_t, "Non-null, aligned raw page metadata");

impl PagePtr {
    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn block_size(self) -> BlockSize {
        let raw = self.as_ptr();
        let block_size = unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) };
        crate::internal::mi_assert_internal!(block_size > 0);
        // SAFETY: asserted above; initialized pages keep block_size non-zero.
        // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe { BlockSize::new_unchecked(block_size) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_block_size(self, block_size: BlockSize) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).block_size), block_size.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn used(self) -> UsedBlockCount {
        let raw = self.as_ptr();
        UsedBlockCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).used)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_used(self, used: UsedBlockCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).used), used.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata satisfying
    /// `used <= capacity <= reserved`.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn capacity(self) -> PageCapacity {
        let raw = self.as_ptr();
        let used = unsafe { self.used() };
        let reserved = unsafe { self.reserved() };
        // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        let capacity = unsafe { ptr::read(ptr::addr_of!((*raw).capacity)) };
        crate::internal::mi_assert_internal!(used.get() <= capacity);
        crate::internal::mi_assert_internal!(capacity <= reserved.get());
        PageCapacity::new(used, capacity, reserved)
            // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            .unwrap_or_else(|| unsafe { PageCapacity::new_unchecked(capacity) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_capacity(self, capacity: PageCapacity) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).capacity), capacity.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn reserved(self) -> ReservedBlockCount {
        let raw = self.as_ptr();
        ReservedBlockCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).reserved)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_reserved(self, reserved: ReservedBlockCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).reserved), reserved.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-free-list-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn free(self) -> AvailableFreeList {
        let raw = self.as_ptr();
        unsafe { AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free))) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-free-list-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_free(self, free: AvailableFreeList) {
        let raw = self.as_ptr();
        let head = free.as_raw();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).free), head) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-free-list-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn local_free(self) -> LocalFreeList {
        let raw = self.as_ptr();
        unsafe { LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free))) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-free-list-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_local_free(self, local_free: LocalFreeList) {
        let raw = self.as_ptr();
        let head = local_free.as_raw();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).local_free), head) };
    }

    /// # Safety
    /// The page pointer must refer to page metadata whose `xthread_id` field can
    /// be accessed through the allocator's atomic field protocol.
    // SAFETY[AUDIT:typed-page-atomic-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_id_cell(self) -> AtomicPageThreadId {
        let raw = self.as_ptr();
        AtomicPageThreadId(AtomicUsizeField(unsafe {
            ptr::addr_of_mut!((*raw).xthread_id)
        }))
    }

    /// # Safety
    /// The page pointer must refer to page metadata whose `xthread_free` field can
    /// be accessed through the allocator's atomic field protocol.
    // SAFETY[AUDIT:typed-page-atomic-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_free_cell(self) -> AtomicThreadFree {
        let raw = self.as_ptr();
        AtomicThreadFree(AtomicUsizeField(unsafe {
            ptr::addr_of_mut!((*raw).xthread_free)
        }))
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-atomic-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
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
            // SAFETY[AUDIT:typed-page-atomic-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            PageKeys(unsafe { ptr::read(ptr::addr_of!((*raw).keys)) })
        }

        #[cfg(not(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        )))]
        {
            let _ = self;
            PageKeys::disabled()
        }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    // SAFETY[AUDIT:typed-page-atomic-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
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
            // SAFETY[AUDIT:typed-page-atomic-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            unsafe { ptr::write(ptr::addr_of_mut!((*raw).keys), keys.0) };
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
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn start(self) -> PageStart {
        let raw = self.as_ptr();
        unsafe { PageStart::new_unchecked(ptr::read(ptr::addr_of!((*raw).page_start))) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn byte_size(self) -> usize {
        unsafe { self.block_size().get() * self.reserved().as_usize() }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    // SAFETY[AUDIT:typed-page-scalar-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn contains_user(self, user: UserPtr) -> bool {
        let start = ptr_exposed_addr(unsafe { self.start() }.as_ptr());
        let end = start.wrapping_add(unsafe { self.byte_size() });
        let user = ptr_exposed_addr(user.as_ptr());
        start <= user && user < end
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
    // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn encoded_link(self) -> EncodedPtr<mi_block_t> {
        unsafe { EncodedPtr::read_word_from_raw(self.as_ptr()) }
    }

    /// # Safety
    /// The block pointer must refer to a writable block link word.
    // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn write_link(self, link: BlockLink) {
        unsafe { link.write_to(self) };
    }

    /// # Safety
    /// `page` must be the page that owns this block link.
    // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn next(self, page: PagePtr) -> Option<BlockPtr> {
        unsafe {
            crate::track::mi_track_mem_defined(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        let link = unsafe { self.encoded_link() };
        let next = if crate::types::MI_ENCODE_FREELIST != 0 {
            unsafe { link.decode_block(page, page.keys()) }
        } else {
            BlockPtr::new(link.as_unencoded_raw())
        };
        // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe {
            crate::track::mi_track_mem_noaccess(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        next
    }

    /// # Safety
    /// `page` must be the page that owns this block link, and `next` must be
    /// null or a block in the same encoded free-list domain.
    // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_next(self, page: PagePtr, next: Option<BlockPtr>) {
        unsafe {
            crate::track::mi_track_mem_undefined(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        let link = if crate::types::MI_ENCODE_FREELIST != 0 {
            // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            unsafe { EncodedPtr::encode_block(next, page, page.keys()) }
        } else {
            EncodedPtr::from_unencoded_raw(
                next.map(BlockPtr::as_ptr)
                    .unwrap_or(ptr::null_mut())
                    .cast_const(),
            )
        };
        // SAFETY[AUDIT:typed-block-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe { self.write_link(BlockLink::FreeList(link)) };
        unsafe {
            crate::track::mi_track_mem_noaccess(
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
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>) {
        let raw = self.as_ptr();
        let subproc = subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).subproc), subproc) };
    }

    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_local_key(self) -> mi_thread_local_t {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).theap)) }
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_local_key(self, key: mi_thread_local_t) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theap), key) };
    }

    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps(self) -> Option<ThreadHeapPtr> {
        let raw = self.as_ptr();
        ThreadHeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).theaps)) })
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_heaps(self, theaps: Option<ThreadHeapPtr>) {
        let raw = self.as_ptr();
        let theaps = theaps.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theaps), theaps) };
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose lock field is valid.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).theaps_lock) }
    }

    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn exclusive_arena(self) -> Option<ArenaPtr> {
        let raw = self.as_ptr();
        ArenaPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).exclusive_arena)) })
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_exclusive_arena(self, arena: Option<ArenaPtr>) {
        let raw = self.as_ptr();
        let arena = arena.map(ArenaPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).exclusive_arena), arena) };
    }

    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn numa_node(self) -> NumaNode {
        let raw = self.as_ptr();
        NumaNode::new(unsafe { ptr::read(ptr::addr_of!((*raw).numa_node)) })
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose arena page cell at
    /// `index` is valid for atomic access.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_pages_cell(self, index: ArenaIndex) -> *mut AtomicPtr<c_void> {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).arena_pages[index.get() as usize]) }
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose lock field is valid.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_pages_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).arena_pages_lock) }
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose stats field is valid.
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
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
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_tld(self, tld: Option<TldPtr>) {
        let raw = self.as_ptr();
        let tld = tld.map(TldPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).tld), tld) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose heap
    /// field can be accessed through the allocator's atomic field protocol.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heap_cell(self) -> AtomicPtrField<mi_heap_t> {
        let raw = self.as_ptr();
        AtomicPtrField(unsafe { ptr::addr_of_mut!((*raw).heap) })
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose
    /// refcount is valid for atomic access.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount {
        let raw = self.as_ptr();
        AtomicThreadHeapRefcount(AtomicUsizeField(unsafe {
            ptr::addr_of_mut!((*raw).refcount)
        }))
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heap(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heap), heap) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to initialized thread-heap metadata.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heartbeat(self) -> Heartbeat {
        let raw = self.as_ptr();
        Heartbeat::new(unsafe { ptr::read(ptr::addr_of!((*raw).heartbeat)) })
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heartbeat(self, heartbeat: Heartbeat) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heartbeat), heartbeat.get()) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose page
    /// queue at `bin` is initialized.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn page_queue(self, bin: Bin) -> PageQueuePtr {
        let raw = self.as_ptr();
        unsafe { PageQueuePtr::new_unchecked(ptr::addr_of_mut!((*raw).pages[bin.as_usize()])) }
    }

    /// # Safety
    /// The thread-heap pointer must refer to initialized thread-heap metadata.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn direct_page(self, bin: DirectBin) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe {
            ptr::read(ptr::addr_of!((*raw).pages_free_direct[bin.get() as usize]))
        })
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_direct_page(self, bin: DirectBin, page: Option<PagePtr>) {
        let raw = self.as_ptr();
        let page = page.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe {
            ptr::write(
                ptr::addr_of_mut!((*raw).pages_free_direct[bin.get() as usize]),
                page,
            )
        };
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata, and
    /// `start <= end`.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn fill_direct_pages(
        self,
        start: DirectBin,
        end: DirectBin,
        page: Option<PagePtr>,
    ) {
        let raw = self.as_ptr();
        let start = start.as_usize();
        let end = end.as_usize();
        crate::internal::mi_assert_internal!(start <= end);
        let page = page.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe {
            let pages = ptr::addr_of_mut!((*raw).pages_free_direct).cast::<*mut mi_page_t>();
            core::slice::from_raw_parts_mut(pages.add(start), end - start + 1).fill(page);
        }
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose stats
    /// field is valid.
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}
define_ptr_wrapper!(TldPtr, mi_tld_t, "Non-null thread-local data");

impl TldPtr {
    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_id(self) -> Option<ThreadId> {
        let raw = self.as_ptr();
        let thread_id = unsafe { ptr::read(ptr::addr_of!((*raw).thread_id)) };
        if thread_id == 0 {
            None
        } else {
            // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            Some(unsafe { ThreadId::new_unchecked(thread_id) })
        }
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata. `thread_id`
    /// must be `None` only for the main-thread sentinel state.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_id(self, thread_id: Option<ThreadId>) {
        let raw = self.as_ptr();
        let thread_id = thread_id.map(ThreadId::get).unwrap_or(0);
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).thread_id), thread_id) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_seq(self) -> ThreadSeq {
        let raw = self.as_ptr();
        ThreadSeq::new(unsafe { ptr::read(ptr::addr_of!((*raw).thread_seq)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_seq(self, seq: ThreadSeq) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).thread_seq), seq.get()) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn numa_node(self) -> NumaNode {
        let raw = self.as_ptr();
        NumaNode::new(unsafe { ptr::read(ptr::addr_of!((*raw).numa_node)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>) {
        let raw = self.as_ptr();
        let subproc = subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).subproc), subproc) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata whose
    /// subproc, if present, points to initialized subproc metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> Option<*mut mi_stats_t> {
        unsafe { self.subproc().map(|subproc| subproc.stats()) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata whose
    /// subproc, if present, points to initialized subproc metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_count_cell(self) -> Option<AtomicUsizeField> {
        unsafe { self.subproc().map(|subproc| subproc.arena_count_cell()) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata whose
    /// subproc, if present, points to initialized subproc metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena(self, index: ArenaIndex) -> Option<ArenaPtr> {
        unsafe { self.subproc().and_then(|subproc| subproc.arena(index)) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata whose
    /// subproc, if present, points to writable subproc metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_arena(self, index: ArenaIndex, arena: Option<ArenaPtr>) {
        if let Some(subproc) = unsafe { self.subproc() } {
            unsafe { subproc.set_arena(index, arena) };
        }
    }

    /// # Safety
    /// The TLD pointer must refer to thread-local metadata whose memid field is valid.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).memid) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps(self) -> Option<ThreadHeapPtr> {
        let raw = self.as_ptr();
        ThreadHeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).theaps)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_heaps(self, theaps: Option<ThreadHeapPtr>) {
        let raw = self.as_ptr();
        let theaps = theaps.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theaps), theaps) };
    }

    /// # Safety
    /// The TLD pointer must refer to thread-local metadata whose lock field is valid.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).theaps_lock) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn recurse(self) -> bool {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).recurse)) }
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_recurse(self, recurse: bool) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).recurse), recurse) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn is_in_threadpool(self) -> bool {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).is_in_threadpool)) }
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_is_in_threadpool(self, is_in_threadpool: bool) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).is_in_threadpool), is_in_threadpool) };
    }
}
define_ptr_wrapper!(ArenaPtr, mi_arena_t, "Non-null raw arena metadata");

impl ArenaPtr {
    /// # Safety
    /// The arena pointer must refer to arena metadata whose memid field is valid.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).memid) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>) {
        let raw = self.as_ptr();
        let subproc = subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).subproc), subproc) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata with an arena
    /// index less than `MI_MAX_ARENAS`.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn index(self) -> ArenaIndex {
        let raw = self.as_ptr();
        let index = unsafe { ptr::read(ptr::addr_of!((*raw).arena_idx)) };
        crate::internal::mi_assert_internal!(index < crate::types::MI_MAX_ARENAS);
        // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
        unsafe { ArenaIndex::new_unchecked(index as u16) }
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_index(self, index: ArenaIndex) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).arena_idx), index.get() as usize) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata with non-zero
    /// slice count.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn slice_count(self) -> SliceCount {
        let raw = self.as_ptr();
        let count = unsafe { ptr::read(ptr::addr_of!((*raw).slice_count)) };
        crate::internal::mi_assert_internal!(count != 0);
        SliceCount::from_usize(count).unwrap_or_else(|| {
            crate::internal::mi_assert_internal!(false);
            // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            unsafe { SliceCount::new_unchecked(1) }
        })
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_slice_count(self, count: SliceCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).slice_count), count.get() as usize) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn numa_node(self) -> NumaNode {
        let raw = self.as_ptr();
        NumaNode::new(unsafe { ptr::read(ptr::addr_of!((*raw).numa_node)) })
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn is_exclusive(self) -> bool {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).is_exclusive)) }
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_is_exclusive(self, is_exclusive: bool) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).is_exclusive), is_exclusive) };
    }

    /// # Safety
    /// The arena pointer must refer to arena metadata whose purge field is valid.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn purge_expire_cell(self) -> AtomicI64Field {
        let raw = self.as_ptr();
        AtomicI64Field(unsafe { ptr::addr_of_mut!((*raw).purge_expire).cast::<i64>() })
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn parent(self) -> Option<ArenaPtr> {
        let raw = self.as_ptr();
        ArenaPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).parent)) })
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_parent(self, parent: Option<ArenaPtr>) {
        let raw = self.as_ptr();
        let parent = parent.map(ArenaPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).parent), parent) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn slices_free(self) -> *mut mi_bbitmap_t {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).slices_free)) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn slices_committed(self) -> *mut mi_bitmap_t {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).slices_committed)) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn slices_dirty(self) -> *mut mi_bitmap_t {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).slices_dirty)) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn slices_purge(self) -> *mut mi_bitmap_t {
        let raw = self.as_ptr();
        unsafe { ptr::read(ptr::addr_of!((*raw).slices_purge)) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn pages_meta(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).pages_meta)) })
    }

    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_pages_meta(self, pages: Option<PagePtr>) {
        let raw = self.as_ptr();
        let pages = pages.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).pages_meta), pages) };
    }

    /// # Safety
    /// The arena pointer must refer to arena metadata whose page table field is valid.
    // SAFETY[AUDIT:typed-arena-linkage-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn pages_main(self) -> *mut mi_arena_pages_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).pages_main) }
    }
}
define_ptr_wrapper!(SubprocPtr, mi_subproc_t, "Non-null raw subproc metadata");

impl SubprocPtr {
    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    // SAFETY[AUDIT:typed-subproc-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn sequence(self) -> SubprocSeq {
        let raw = self.as_ptr();
        SubprocSeq::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc_seq)) })
    }

    /// # Safety
    /// The subproc pointer must refer to writable subproc metadata.
    // SAFETY[AUDIT:typed-subproc-identity-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_sequence(self, sequence: SubprocSeq) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).subproc_seq), sequence.get()) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField(unsafe { ptr::addr_of_mut!((*raw).arena_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount {
        ArenaCount::new(unsafe { self.arena_count_cell().load(order) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder) {
        unsafe { self.arena_count_cell().store(count.get(), order) };
    }

    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena(self, index: ArenaIndex) -> Option<ArenaPtr> {
        let raw = self.as_ptr();
        ArenaPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).arenas[index.get() as usize])) })
    }

    /// # Safety
    /// The subproc pointer must refer to writable subproc metadata.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_arena(self, index: ArenaIndex, arena: Option<ArenaPtr>) {
        let raw = self.as_ptr();
        let arena = arena.map(ArenaPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe {
            ptr::write(
                ptr::addr_of_mut!((*raw).arenas[index.get() as usize]),
                arena,
            )
        };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose lock field is valid.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn arena_reserve_lock(self) -> MiLockPtr {
        let raw = self.as_ptr();
        MiLockPtr(unsafe { NonNull::new_unchecked(ptr::addr_of_mut!((*raw).arena_reserve_lock)) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose purge field is valid.
    // SAFETY[AUDIT:typed-subproc-arena-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn purge_expire_cell(self) -> AtomicI64Field {
        let raw = self.as_ptr();
        AtomicI64Field(unsafe { ptr::addr_of_mut!((*raw).purge_expire).cast::<i64>() })
    }

    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    // SAFETY[AUDIT:typed-subproc-heap-link-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn main_heap(self) -> Option<HeapPtr> {
        let raw = self.as_ptr();
        HeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).heap_main)) })
    }

    /// # Safety
    /// The subproc pointer must refer to writable subproc metadata.
    // SAFETY[AUDIT:typed-subproc-heap-link-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_main_heap(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heap_main), heap) };
    }

    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    // SAFETY[AUDIT:typed-subproc-heap-link-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heaps(self) -> Option<HeapPtr> {
        let raw = self.as_ptr();
        HeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).heaps)) })
    }

    /// # Safety
    /// The subproc pointer must refer to writable subproc metadata.
    // SAFETY[AUDIT:typed-subproc-heap-link-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heaps(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heaps), heap) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose lock field is valid.
    // SAFETY[AUDIT:typed-subproc-heap-link-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heaps_lock(self) -> MiLockPtr {
        let raw = self.as_ptr();
        MiLockPtr(unsafe { NonNull::new_unchecked(ptr::addr_of_mut!((*raw).heaps_lock)) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField(unsafe { ptr::addr_of_mut!((*raw).thread_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_count(self, order: MemoryOrder) -> ThreadCount {
        ThreadCount::new(unsafe { self.thread_count_cell().load(order) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_count(self, count: ThreadCount, order: MemoryOrder) {
        unsafe { self.thread_count_cell().store(count.get(), order) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_total_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField(unsafe { ptr::addr_of_mut!((*raw).thread_total_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn thread_total_count(self, order: MemoryOrder) -> ThreadCount {
        ThreadCount::new(unsafe { self.thread_total_count_cell().load(order) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_total_count(self, count: ThreadCount, order: MemoryOrder) {
        unsafe { self.thread_total_count_cell().store(count.get(), order) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heap_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField(unsafe { ptr::addr_of_mut!((*raw).heap_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount {
        HeapCount::new(unsafe { self.heap_count_cell().load(order) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heap_count(self, count: HeapCount, order: MemoryOrder) {
        unsafe { self.heap_count_cell().store(count.get(), order) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heap_total_count_cell(self) -> AtomicUsizeField {
        let raw = self.as_ptr();
        AtomicUsizeField(unsafe { ptr::addr_of_mut!((*raw).heap_total_count).cast::<usize>() })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn heap_total_count(self, order: MemoryOrder) -> HeapCount {
        HeapCount::new(unsafe { self.heap_total_count_cell().load(order) })
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    // SAFETY[AUDIT:typed-subproc-count-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_heap_total_count(self, count: HeapCount, order: MemoryOrder) {
        unsafe { self.heap_total_count_cell().store(count.get(), order) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose memid field is valid.
    // SAFETY[AUDIT:typed-subproc-metadata-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).memid) }
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose stats field is valid.
    // SAFETY[AUDIT:typed-subproc-metadata-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}
define_ptr_wrapper!(PageQueuePtr, mi_page_queue_t, "Non-null page queue");

impl PageQueuePtr {
    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn first(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).first)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_first(self, first: Option<PagePtr>) {
        let raw = self.as_ptr();
        let first = first.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).first), first) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn last(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).last)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_last(self, last: Option<PagePtr>) {
        let raw = self.as_ptr();
        let last = last.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).last), last) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn count(self) -> PageQueueCount {
        let raw = self.as_ptr();
        PageQueueCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).count)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_count(self, count: PageQueueCount) {
        let raw = self.as_ptr();
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).count), count.get()) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn block_size(self) -> Option<BlockSize> {
        let raw = self.as_ptr();
        let block_size = unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) };
        if block_size == 0 {
            None
        } else {
            // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
            Some(unsafe { BlockSize::new_unchecked(block_size) })
        }
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn set_block_size(self, block_size: Option<BlockSize>) {
        let raw = self.as_ptr();
        let block_size = block_size.map(BlockSize::get).unwrap_or(0);
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).block_size), block_size) };
    }
}

define_ptr_wrapper!(UserPtr, c_void, "Non-null pointer returned to/from user");
define_ptr_wrapper!(
    InteriorUserPtr,
    c_void,
    "User pointer may not equal block start"
);

impl UserPtr {
    #[inline(always)]
    pub(crate) fn from_non_null(ptr: NonNull<c_void>) -> Self {
        Self(ptr)
    }

    #[inline(always)]
    pub(crate) fn as_non_null(self) -> NonNull<c_void> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn as_const_ptr(self) -> *const c_void {
        self.as_ptr().cast_const()
    }

    #[inline(always)]
    pub(crate) fn as_u8_ptr(self) -> *mut u8 {
        self.as_ptr().cast::<u8>()
    }

    #[inline(always)]
    pub(crate) fn into_interior(self) -> InteriorUserPtr {
        InteriorUserPtr(self.0)
    }

    #[inline(always)]
    pub(crate) fn option_as_ptr(user: Option<Self>) -> *mut c_void {
        user.map(Self::as_ptr).unwrap_or(ptr::null_mut())
    }
}

impl InteriorUserPtr {
    #[inline(always)]
    pub(crate) fn from_non_null(ptr: NonNull<c_void>) -> Self {
        Self(ptr)
    }

    #[inline(always)]
    pub(crate) fn as_non_null(self) -> NonNull<c_void> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn as_const_ptr(self) -> *const c_void {
        self.as_ptr().cast_const()
    }

    #[inline(always)]
    pub(crate) fn as_u8_ptr(self) -> *mut u8 {
        self.as_ptr().cast::<u8>()
    }

    #[inline(always)]
    pub(crate) fn as_user_ptr(self) -> UserPtr {
        UserPtr(self.0)
    }

    #[inline(always)]
    pub(crate) fn option_as_ptr(interior: Option<Self>) -> *mut c_void {
        interior.map(Self::as_ptr).unwrap_or(ptr::null_mut())
    }
}

define_ptr_wrapper!(PageStart, u8, "Start of page block area");
define_ptr_wrapper!(SliceStart, u8, "Aligned arena slice start");
define_ptr_wrapper!(OsMemPtr, c_void, "OS allocation base");

impl PageStart {
    #[inline(always)]
    pub(crate) fn as_non_null(self) -> NonNull<u8> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn checked_byte_add(self, offset: usize) -> Option<NonNull<u8>> {
        checked_non_null_byte_add(self.0, offset)
    }

    #[inline(always)]
    pub(crate) fn checked_byte_sub(self, offset: usize) -> Option<NonNull<u8>> {
        checked_non_null_byte_sub(self.0, offset)
    }
}

impl SliceStart {
    #[inline(always)]
    pub(crate) fn as_non_null(self) -> NonNull<u8> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn checked_byte_add(self, offset: usize) -> Option<NonNull<u8>> {
        checked_non_null_byte_add(self.0, offset)
    }

    #[inline(always)]
    pub(crate) fn checked_byte_sub(self, offset: usize) -> Option<NonNull<u8>> {
        checked_non_null_byte_sub(self.0, offset)
    }
}

impl OsMemPtr {
    #[inline(always)]
    pub(crate) fn from_non_null(ptr: NonNull<c_void>) -> Self {
        Self(ptr)
    }

    #[inline(always)]
    pub(crate) fn as_non_null(self) -> NonNull<c_void> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn as_u8_non_null(self) -> NonNull<u8> {
        self.0.cast::<u8>()
    }

    #[inline(always)]
    pub(crate) fn checked_byte_add(self, offset: usize) -> Option<Self> {
        checked_non_null_byte_add(self.as_u8_non_null(), offset)
            .map(|ptr| Self(ptr.cast::<c_void>()))
    }

    #[inline(always)]
    pub(crate) fn checked_byte_sub(self, offset: usize) -> Option<Self> {
        checked_non_null_byte_sub(self.as_u8_non_null(), offset)
            .map(|ptr| Self(ptr.cast::<c_void>()))
    }
}

/// Non-null raw page metadata (read-only view).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageConstPtr(NonNull<mi_page_t>);

impl PageConstPtr {
    #[inline(always)]
    pub(crate) fn new(ptr: *const mi_page_t) -> Option<Self> {
        NonNull::new(ptr as *mut mi_page_t).map(Self)
    }

    /// # Safety
    /// `ptr` must be non-null and point to valid page metadata.
    // SAFETY[AUDIT:typed-pointer-constructor-boundary]: caller-proven metadata access stays behind transparent NonNull wrappers.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: *const mi_page_t) -> Self {
        Self(unsafe { NonNull::new_unchecked(ptr as *mut mi_page_t) })
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *const mi_page_t {
        self.0.as_ptr() as *const mi_page_t
    }
}

pub(super) fn __lint_touch() {
    let _ = checked_non_null_byte_add as fn(NonNull<u8>, usize) -> Option<NonNull<u8>>;
    let _ = checked_non_null_byte_sub as fn(NonNull<u8>, usize) -> Option<NonNull<u8>>;

    // SAFETY[AUDIT:typed-page-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = PagePtr::set_block_size as unsafe fn(PagePtr, BlockSize);
    let _ = BlockPtr::encoded_link as unsafe fn(BlockPtr) -> EncodedPtr<mi_block_t>;
    let _ = BlockPtr::next as unsafe fn(BlockPtr, PagePtr) -> Option<BlockPtr>;

    let _ = HeapPtr::subproc as unsafe fn(HeapPtr) -> Option<SubprocPtr>;
    let _ = HeapPtr::set_subproc as unsafe fn(HeapPtr, Option<SubprocPtr>);
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = HeapPtr::thread_local_key as unsafe fn(HeapPtr) -> mi_thread_local_t;
    let _ = HeapPtr::set_thread_local_key as unsafe fn(HeapPtr, mi_thread_local_t);
    let _ = HeapPtr::thread_heaps as unsafe fn(HeapPtr) -> Option<ThreadHeapPtr>;
    let _ = HeapPtr::set_thread_heaps as unsafe fn(HeapPtr, Option<ThreadHeapPtr>);
    let _ = HeapPtr::thread_heaps_lock as unsafe fn(HeapPtr) -> *mut mi_lock_t;
    let _ = HeapPtr::exclusive_arena as unsafe fn(HeapPtr) -> Option<ArenaPtr>;
    // SAFETY[AUDIT:typed-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = HeapPtr::set_exclusive_arena as unsafe fn(HeapPtr, Option<ArenaPtr>);
    let _ = HeapPtr::numa_node as unsafe fn(HeapPtr) -> NumaNode;
    let _ = HeapPtr::set_numa_node as unsafe fn(HeapPtr, NumaNode);
    let _ = HeapPtr::arena_pages_cell as unsafe fn(HeapPtr, ArenaIndex) -> *mut AtomicPtr<c_void>;
    let _ = HeapPtr::arena_pages_lock as unsafe fn(HeapPtr) -> *mut mi_lock_t;
    let _ = HeapPtr::stats as unsafe fn(HeapPtr) -> *mut mi_stats_t;

    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = ThreadHeapPtr::heap_cell as unsafe fn(ThreadHeapPtr) -> AtomicPtrField<mi_heap_t>;
    let _ = ThreadHeapPtr::refcount_cell as unsafe fn(ThreadHeapPtr) -> AtomicThreadHeapRefcount;
    let _ = ThreadHeapPtr::set_heap as unsafe fn(ThreadHeapPtr, Option<HeapPtr>);
    let _ = ThreadHeapPtr::heartbeat as unsafe fn(ThreadHeapPtr) -> Heartbeat;
    let _ = ThreadHeapPtr::set_heartbeat as unsafe fn(ThreadHeapPtr, Heartbeat);
    let _ = ThreadHeapPtr::page_queue as unsafe fn(ThreadHeapPtr, Bin) -> PageQueuePtr;
    // SAFETY[AUDIT:typed-thread-heap-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = ThreadHeapPtr::direct_page as unsafe fn(ThreadHeapPtr, DirectBin) -> Option<PagePtr>;
    let _ = ThreadHeapPtr::set_direct_page as unsafe fn(ThreadHeapPtr, DirectBin, Option<PagePtr>);
    let _ = ThreadHeapPtr::stats as unsafe fn(ThreadHeapPtr) -> *mut mi_stats_t;

    let _ = TldPtr::thread_id as unsafe fn(TldPtr) -> Option<ThreadId>;
    let _ = TldPtr::set_thread_id as unsafe fn(TldPtr, Option<ThreadId>);
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = TldPtr::thread_seq as unsafe fn(TldPtr) -> ThreadSeq;
    let _ = TldPtr::set_thread_seq as unsafe fn(TldPtr, ThreadSeq);
    let _ = TldPtr::numa_node as unsafe fn(TldPtr) -> NumaNode;
    let _ = TldPtr::set_numa_node as unsafe fn(TldPtr, NumaNode);
    let _ = TldPtr::subproc as unsafe fn(TldPtr) -> Option<SubprocPtr>;
    let _ = TldPtr::set_subproc as unsafe fn(TldPtr, Option<SubprocPtr>);
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = TldPtr::stats as unsafe fn(TldPtr) -> Option<*mut mi_stats_t>;
    let _ = TldPtr::arena_count_cell as unsafe fn(TldPtr) -> Option<AtomicUsizeField>;
    let _ = TldPtr::arena as unsafe fn(TldPtr, ArenaIndex) -> Option<ArenaPtr>;
    let _ = TldPtr::set_arena as unsafe fn(TldPtr, ArenaIndex, Option<ArenaPtr>);
    let _ = TldPtr::memid_ptr as unsafe fn(TldPtr) -> *mut mi_memid_t;
    let _ = TldPtr::thread_heaps as unsafe fn(TldPtr) -> Option<ThreadHeapPtr>;
    // SAFETY[AUDIT:typed-tld-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = TldPtr::set_thread_heaps as unsafe fn(TldPtr, Option<ThreadHeapPtr>);
    let _ = TldPtr::thread_heaps_lock as unsafe fn(TldPtr) -> *mut mi_lock_t;
    let _ = TldPtr::recurse as unsafe fn(TldPtr) -> bool;
    let _ = TldPtr::set_recurse as unsafe fn(TldPtr, bool);
    let _ = TldPtr::is_in_threadpool as unsafe fn(TldPtr) -> bool;
    let _ = TldPtr::set_is_in_threadpool as unsafe fn(TldPtr, bool);

    // SAFETY[AUDIT:typed-arena-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = ArenaPtr::memid_ptr as unsafe fn(ArenaPtr) -> *mut mi_memid_t;
    let _ = ArenaPtr::subproc as unsafe fn(ArenaPtr) -> Option<SubprocPtr>;
    let _ = ArenaPtr::set_subproc as unsafe fn(ArenaPtr, Option<SubprocPtr>);
    let _ = ArenaPtr::index as unsafe fn(ArenaPtr) -> ArenaIndex;
    let _ = ArenaPtr::set_index as unsafe fn(ArenaPtr, ArenaIndex);
    let _ = ArenaPtr::slice_count as unsafe fn(ArenaPtr) -> SliceCount;
    // SAFETY[AUDIT:typed-arena-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = ArenaPtr::set_slice_count as unsafe fn(ArenaPtr, SliceCount);
    let _ = ArenaPtr::numa_node as unsafe fn(ArenaPtr) -> NumaNode;
    let _ = ArenaPtr::set_numa_node as unsafe fn(ArenaPtr, NumaNode);
    let _ = ArenaPtr::is_exclusive as unsafe fn(ArenaPtr) -> bool;
    let _ = ArenaPtr::set_is_exclusive as unsafe fn(ArenaPtr, bool);
    let _ = ArenaPtr::purge_expire_cell as unsafe fn(ArenaPtr) -> AtomicI64Field;
    // SAFETY[AUDIT:typed-arena-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = ArenaPtr::parent as unsafe fn(ArenaPtr) -> Option<ArenaPtr>;
    let _ = ArenaPtr::set_parent as unsafe fn(ArenaPtr, Option<ArenaPtr>);
    let _ = ArenaPtr::slices_free as unsafe fn(ArenaPtr) -> *mut mi_bbitmap_t;
    let _ = ArenaPtr::slices_committed as unsafe fn(ArenaPtr) -> *mut mi_bitmap_t;
    let _ = ArenaPtr::slices_dirty as unsafe fn(ArenaPtr) -> *mut mi_bitmap_t;
    let _ = ArenaPtr::slices_purge as unsafe fn(ArenaPtr) -> *mut mi_bitmap_t;
    // SAFETY[AUDIT:typed-arena-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = ArenaPtr::pages_meta as unsafe fn(ArenaPtr) -> Option<PagePtr>;
    let _ = ArenaPtr::set_pages_meta as unsafe fn(ArenaPtr, Option<PagePtr>);
    let _ = ArenaPtr::pages_main as unsafe fn(ArenaPtr) -> *mut mi_arena_pages_t;

    let _ = SubprocPtr::sequence as unsafe fn(SubprocPtr) -> SubprocSeq;
    let _ = SubprocPtr::set_sequence as unsafe fn(SubprocPtr, SubprocSeq);
    // SAFETY[AUDIT:typed-subproc-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = SubprocPtr::set_arena_count as unsafe fn(SubprocPtr, ArenaCount, MemoryOrder);
    let _ = SubprocPtr::purge_expire_cell as unsafe fn(SubprocPtr) -> AtomicI64Field;
    let _ = SubprocPtr::main_heap as unsafe fn(SubprocPtr) -> Option<HeapPtr>;
    let _ = SubprocPtr::set_main_heap as unsafe fn(SubprocPtr, Option<HeapPtr>);
    let _ = SubprocPtr::heaps as unsafe fn(SubprocPtr) -> Option<HeapPtr>;
    let _ = SubprocPtr::set_heaps as unsafe fn(SubprocPtr, Option<HeapPtr>);
    // SAFETY[AUDIT:typed-subproc-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = SubprocPtr::heaps_lock as unsafe fn(SubprocPtr) -> MiLockPtr;
    let _ = SubprocPtr::thread_count as unsafe fn(SubprocPtr, MemoryOrder) -> ThreadCount;
    let _ = SubprocPtr::set_thread_count as unsafe fn(SubprocPtr, ThreadCount, MemoryOrder);
    let _ = SubprocPtr::thread_total_count as unsafe fn(SubprocPtr, MemoryOrder) -> ThreadCount;
    let _ = SubprocPtr::set_thread_total_count as unsafe fn(SubprocPtr, ThreadCount, MemoryOrder);
    let _ = SubprocPtr::heap_count_cell as unsafe fn(SubprocPtr) -> AtomicUsizeField;
    // SAFETY[AUDIT:typed-subproc-fixture-touch-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = SubprocPtr::heap_count as unsafe fn(SubprocPtr, MemoryOrder) -> HeapCount;
    let _ = SubprocPtr::set_heap_count as unsafe fn(SubprocPtr, HeapCount, MemoryOrder);
    let _ = SubprocPtr::heap_total_count_cell as unsafe fn(SubprocPtr) -> AtomicUsizeField;
    let _ = SubprocPtr::heap_total_count as unsafe fn(SubprocPtr, MemoryOrder) -> HeapCount;
    let _ = SubprocPtr::set_heap_total_count as unsafe fn(SubprocPtr, HeapCount, MemoryOrder);
    let _ = SubprocPtr::memid_ptr as unsafe fn(SubprocPtr) -> *mut mi_memid_t;

    let _ = PageQueuePtr::new as fn(*mut mi_page_queue_t) -> Option<PageQueuePtr>;
    // SAFETY[AUDIT:typed-page-queue-accessors]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = PageQueuePtr::set_block_size as unsafe fn(PageQueuePtr, Option<BlockSize>);

    let _ = UserPtr::from_non_null as fn(NonNull<c_void>) -> UserPtr;
    let _ = UserPtr::as_non_null as fn(UserPtr) -> NonNull<c_void>;
    let _ = UserPtr::as_const_ptr as fn(UserPtr) -> *const c_void;
    let _ = UserPtr::as_u8_ptr as fn(UserPtr) -> *mut u8;
    let _ = UserPtr::into_interior as fn(UserPtr) -> InteriorUserPtr;
    let _ = UserPtr::option_as_ptr as fn(Option<UserPtr>) -> *mut c_void;
    let _ = InteriorUserPtr::from_non_null as fn(NonNull<c_void>) -> InteriorUserPtr;
    // SAFETY[AUDIT:typed-pointer-constructor-boundary]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = InteriorUserPtr::new_unchecked as unsafe fn(*mut c_void) -> InteriorUserPtr;
    let _ = InteriorUserPtr::as_non_null as fn(InteriorUserPtr) -> NonNull<c_void>;
    let _ = InteriorUserPtr::as_const_ptr as fn(InteriorUserPtr) -> *const c_void;
    let _ = InteriorUserPtr::as_u8_ptr as fn(InteriorUserPtr) -> *mut u8;
    let _ = InteriorUserPtr::as_user_ptr as fn(InteriorUserPtr) -> UserPtr;
    let _ = InteriorUserPtr::option_as_ptr as fn(Option<InteriorUserPtr>) -> *mut c_void;

    let _ = PageStart::new as fn(*mut u8) -> Option<PageStart>;
    // SAFETY[AUDIT:typed-pointer-constructor-boundary]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = PageStart::new_unchecked as unsafe fn(*mut u8) -> PageStart;
    let _ = PageStart::as_ptr as fn(PageStart) -> *mut u8;
    let _ = PageStart::as_non_null as fn(PageStart) -> NonNull<u8>;
    let _ = PageStart::checked_byte_add as fn(PageStart, usize) -> Option<NonNull<u8>>;
    let _ = PageStart::checked_byte_sub as fn(PageStart, usize) -> Option<NonNull<u8>>;
    let _ = SliceStart::new as fn(*mut u8) -> Option<SliceStart>;
    // SAFETY[AUDIT:typed-pointer-constructor-boundary]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = SliceStart::new_unchecked as unsafe fn(*mut u8) -> SliceStart;
    let _ = SliceStart::as_ptr as fn(SliceStart) -> *mut u8;
    let _ = SliceStart::as_non_null as fn(SliceStart) -> NonNull<u8>;
    let _ = SliceStart::checked_byte_add as fn(SliceStart, usize) -> Option<NonNull<u8>>;
    let _ = SliceStart::checked_byte_sub as fn(SliceStart, usize) -> Option<NonNull<u8>>;
    let _ = OsMemPtr::from_non_null as fn(NonNull<c_void>) -> OsMemPtr;
    let _ = OsMemPtr::as_non_null as fn(OsMemPtr) -> NonNull<c_void>;
    let _ = OsMemPtr::as_u8_non_null as fn(OsMemPtr) -> NonNull<u8>;
    let _ = OsMemPtr::checked_byte_add as fn(OsMemPtr, usize) -> Option<OsMemPtr>;
    let _ = OsMemPtr::checked_byte_sub as fn(OsMemPtr, usize) -> Option<OsMemPtr>;
    let _ = PageConstPtr::new as fn(*const mi_page_t) -> Option<PageConstPtr>;
    // SAFETY[AUDIT:typed-pointer-constructor-boundary]: caller-proven metadata access stays behind transparent NonNull wrappers.
    let _ = PageConstPtr::new_unchecked as unsafe fn(*const mi_page_t) -> PageConstPtr;
    let _ = PageConstPtr::as_ptr as fn(PageConstPtr) -> *const mi_page_t;
}

#[cfg(test)]
mod tests;
