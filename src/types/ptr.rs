//! DT-2 pointer domain types (zero-cost newtypes over NonNull).
//! All types are #[repr(transparent)], Copy, Clone, PartialEq, Eq, etc.
//! They do not implement Deref or DerefMut.

use super::atomic::{AtomicUsizeField, MemoryOrder};
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
use core::sync::atomic::AtomicUsize;

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
            /// `ptr` must be non-null. This constructor does not establish
            /// pointee validity, initialization, liveness, or ownership.
            #[inline(always)]
            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self {
                // SAFETY: the caller proves `ptr` is non-null; constructing the
                // wrapper neither dereferences it nor creates a reference.
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
    #[inline(always)]
    pub(crate) unsafe fn block_size(self) -> BlockSize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized page metadata live and
        // readable, so its block-size field may be copied without aliasing it.
        let block_size = unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) };
        crate::internal::mi_assert_internal!(block_size > 0);
        // SAFETY: asserted above; initialized pages keep block_size non-zero.
        unsafe { BlockSize::new_unchecked(block_size) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata, or to fresh
    /// metadata at a boundary where a zero block-size sentinel is valid.
    #[inline(always)]
    pub(crate) unsafe fn block_size_raw(self) -> usize {
        let raw = self.as_ptr();
        // SAFETY: the caller permits reading the live page's initialized
        // block-size field, including its fresh-page zero sentinel.
        unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    #[inline(always)]
    pub(crate) unsafe fn used(self) -> UsedBlockCount {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the page metadata live and grants readable
        // access to its initialized `used` field for this copy.
        UsedBlockCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).used)) })
    }

    /// # Safety
    /// The page backing allocation must remain live, aligned, and writable for
    /// the `used` field; fresh metadata initialization is allowed. The caller
    /// must have exclusive access to that field, and `used` must not exceed the
    /// current initialized, stable capacity.
    #[inline(always)]
    pub(crate) unsafe fn set_used(self, used: UsedBlockCount) {
        let raw = self.as_ptr();
        // SAFETY: the caller grants exclusive access to the in-bounds `used`
        // field in live page backing; the coupled capacity bound was proven.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).used), used.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata satisfying
    /// `used <= capacity <= reserved`.
    #[inline(always)]
    pub(crate) unsafe fn capacity(self) -> PageCapacity {
        let raw = self.as_ptr();
        // SAFETY: the caller's initialized-page contract permits a stable
        // snapshot of the coupled used/capacity/reserved fields.
        let used = unsafe { self.used() };
        // SAFETY: the same stable page snapshot permits reading `reserved`.
        let reserved = unsafe { self.reserved() };
        // SAFETY: `raw` remains live and readable throughout this snapshot;
        // the caller also guarantees the initialized capacity invariant.
        let capacity = unsafe { ptr::read(ptr::addr_of!((*raw).capacity)) };
        crate::internal::mi_assert_internal!(used.get() <= capacity);
        crate::internal::mi_assert_internal!(capacity <= reserved.get());
        PageCapacity::new(used, capacity, reserved)
            // SAFETY: the caller guarantees these page counts satisfy the
            // invariant; this fallback preserves release behavior if assertions vanish.
            .unwrap_or_else(|| unsafe { PageCapacity::new_unchecked(capacity) })
    }

    /// # Safety
    /// The page backing allocation must remain live, aligned, and writable for
    /// the `capacity` field; fresh metadata initialization is allowed. The
    /// caller must have exclusive access to that field. The page's `used` and
    /// `reserved` fields must already be initialized and stable, and this value
    /// must satisfy those current counts; a value from another page is insufficient.
    #[inline(always)]
    pub(crate) unsafe fn set_capacity(self, capacity: PageCapacity) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns the in-bounds capacity field in
        // live page backing, and validated it against the initialized counts.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).capacity), capacity.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    #[inline(always)]
    pub(crate) unsafe fn reserved(self) -> ReservedBlockCount {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized page live and readable
        // while its reserved-block count is copied.
        ReservedBlockCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).reserved)) })
    }

    /// # Safety
    /// The page backing allocation must remain live, aligned, and writable for
    /// the `reserved` field; fresh metadata initialization is allowed. The
    /// caller must have exclusive access to that field, and the current capacity
    /// must already be initialized, stable, and no greater than `reserved`.
    #[inline(always)]
    pub(crate) unsafe fn set_reserved(self, reserved: ReservedBlockCount) {
        let raw = self.as_ptr();
        // SAFETY: the caller grants exclusive access to the in-bounds reserved
        // field in live page backing and proved the initialized capacity bound.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).reserved), reserved.get()) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    #[inline(always)]
    pub(crate) unsafe fn free(self) -> AvailableFreeList {
        let raw = self.as_ptr();
        // SAFETY: the caller owns or otherwise stabilizes the live page's
        // initialized available-list head for this raw-pointer snapshot.
        unsafe { AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free))) }
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    #[inline(always)]
    pub(crate) unsafe fn free_raw(self) -> *mut mi_block_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the page and its available-list head live
        // and stable while the raw head value is copied.
        unsafe { ptr::read(ptr::addr_of!((*raw).free)) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_free(self, free: AvailableFreeList) {
        let raw = self.as_ptr();
        let head = free.as_raw();
        // SAFETY: the caller exclusively owns publication of the live page's
        // available-list head, and `free` supplies its initialized value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).free), head) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata.
    #[inline(always)]
    pub(crate) unsafe fn local_free(self) -> LocalFreeList {
        let raw = self.as_ptr();
        // SAFETY: the caller has local ownership of the live page and keeps
        // its initialized local-list head stable for this snapshot.
        unsafe { LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free))) }
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_local_free(self, local_free: LocalFreeList) {
        let raw = self.as_ptr();
        let head = local_free.as_raw();
        // SAFETY: the caller has exclusive local ownership of this page field,
        // and `local_free` is the initialized head to publish.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).local_free), head) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata whose queue
    /// links may be inspected by the page-queue protocol.
    #[inline(always)]
    pub(crate) unsafe fn queue_next(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        // SAFETY: the queue protocol keeps this live page linked and its
        // initialized `next` field stable during the read.
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).next)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata whose queue links
    /// are owned by the current page-queue mutation. A non-null `next` must be
    /// suitable for that caller-owned queue transition.
    #[inline(always)]
    pub(crate) unsafe fn set_queue_next(self, next: Option<PagePtr>) {
        let raw = self.as_ptr();
        let next = next.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: this live page participates in the caller-owned queue
        // transition; the destination field is exclusive and `next` is not dereferenced.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).next), next) };
    }

    /// # Safety
    /// The page pointer must refer to initialized page metadata whose queue
    /// links may be inspected by the page-queue protocol.
    #[inline(always)]
    pub(crate) unsafe fn queue_prev(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        // SAFETY: the queue protocol keeps this live page linked and its
        // initialized `prev` field stable during the read.
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).prev)) })
    }

    /// # Safety
    /// The page pointer must refer to writable page metadata whose queue links
    /// are owned by the current page-queue mutation. A non-null `prev` must be
    /// suitable for that caller-owned queue transition.
    #[inline(always)]
    pub(crate) unsafe fn set_queue_prev(self, prev: Option<PagePtr>) {
        let raw = self.as_ptr();
        let prev = prev.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: this live page participates in the caller-owned queue
        // transition; the destination field is exclusive and `prev` is not dereferenced.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).prev), prev) };
    }

    /// # Safety
    /// The page pointer and aligned `xthread_id` field must remain live and
    /// initialized during projection. Borrowing the returned field additionally
    /// requires live atomic-compatible storage for the whole borrow.
    #[inline(always)]
    pub(crate) unsafe fn thread_id_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the page allocation live; projecting the
        // properly aligned `xthread_id` field creates only an atomic raw view.
        unsafe { ptr::addr_of!((*raw).xthread_id).cast() }
    }

    /// # Safety
    /// The page pointer and aligned `xthread_free` field must remain live and
    /// initialized during projection. Borrowing the returned field additionally
    /// requires live atomic-compatible storage for the whole borrow.
    #[inline(always)]
    pub(crate) unsafe fn thread_free_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the page allocation live; projecting the
        // properly aligned `xthread_free` field creates only an atomic raw view.
        unsafe { ptr::addr_of!((*raw).xthread_free).cast() }
    }

    /// # Safety
    /// When encoding or tracking is enabled, the page must remain live and its
    /// initialized `keys` field must be stable and readable for this snapshot.
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
            // SAFETY: the caller keeps the initialized page live and its
            // encoding keys stable and readable for this two-word snapshot.
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
    /// When encoding or tracking is enabled, the page backing allocation must
    /// remain live, aligned, and writable for the `keys` field; fresh metadata
    /// initialization is allowed. The caller must exclusively initialize or
    /// replace that field before any encoded-list operation can observe it.
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
            // SAFETY: the caller exclusively owns the in-bounds keys field in
            // live page backing; `keys` is its complete initialized value.
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
    #[inline(always)]
    pub(crate) unsafe fn start(self) -> PageStart {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized page metadata live and its
        // non-null block-area start stable for this read and wrapper creation.
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
    /// The block pointer must refer to a writable block link word.
    #[inline(always)]
    pub(crate) unsafe fn write_link(self, link: BlockLink) {
        // SAFETY: the caller grants writable access to this block's link word;
        // `link` carries the initialized representation to store.
        unsafe { link.write_to(self) };
    }

    /// # Safety
    /// `self` must be a readable, properly aligned block link under the active
    /// tracking state. `page` must be its live owning page, with initialized
    /// stable encoding keys. The caller must stabilize the free-list link and
    /// page metadata against concurrent mutation or destruction for the call.
    #[inline(always)]
    pub(crate) unsafe fn next(self, page: PagePtr) -> Option<BlockPtr> {
        // SAFETY: the caller proves the block link is live for tracking and
        // readable; this transition exposes exactly one link word.
        unsafe {
            crate::allocator::tracking::mi_track_mem_defined(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        // SAFETY: the tracking transition above makes this exact initialized
        // link word readable for the duration of the raw word snapshot.
        let link = unsafe { EncodedPtr::read_word_from_raw(self.as_ptr()) };
        let next = if crate::types::MI_ENCODE_FREELIST != 0 {
            // SAFETY: the caller supplied the owning live page; its stable keys
            // decode this block's initialized encoded link in the same domain.
            unsafe { link.decode_block(page, page.keys()) }
        } else {
            BlockPtr::new(link.as_unencoded_raw())
        };
        // SAFETY: decoding copied the link word; the caller's tracking contract
        // now permits returning exactly that live block word to no-access state.
        unsafe {
            crate::allocator::tracking::mi_track_mem_noaccess(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        next
    }

    /// # Safety
    /// `self` must be a writable, properly aligned block link under the active
    /// tracking state. `page` must be its live owning page with initialized,
    /// stable encoding keys, and `next` must be null or a block in that same
    /// free-list domain. The caller must exclusively own this link mutation and
    /// keep the page and participating blocks live throughout the operation.
    #[inline(always)]
    pub(crate) unsafe fn set_next(self, page: PagePtr, next: Option<BlockPtr>) {
        // SAFETY: the caller grants writable access to this live block link;
        // tracking must expose that exact word before it is initialized.
        unsafe {
            crate::allocator::tracking::mi_track_mem_undefined(
                self.as_ptr().cast(),
                core::mem::size_of::<mi_block_t>(),
            )
        };
        let link = if crate::types::MI_ENCODE_FREELIST != 0 {
            // SAFETY: `page` is the live owner of this block; its initialized,
            // stable keys encode `next` in the same free-list domain.
            unsafe { EncodedPtr::encode_block(next, page, page.keys()) }
        } else {
            EncodedPtr::from_unencoded_raw(
                next.map(BlockPtr::as_ptr)
                    .unwrap_or(ptr::null_mut())
                    .cast_const(),
            )
        };
        // SAFETY: the caller owns this block link mutation and the encoding
        // branch constructed a link for the same owning page domain.
        unsafe { self.write_link(BlockLink::FreeList(link)) };
        // SAFETY: the link write is complete and the caller's tracking protocol
        // requires returning exactly this block link word to no-access state.
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
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized heap live and its `subproc`
        // association stable and readable for this pointer-value copy.
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The heap pointer must refer to initialized heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps(self) -> Option<ThreadHeapPtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized heap live and stabilizes its
        // thread-heap list head while this pointer value is copied.
        ThreadHeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).theaps)) })
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_heaps(self, theaps: Option<ThreadHeapPtr>) {
        let raw = self.as_ptr();
        let theaps = theaps.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the caller exclusively owns this live heap's list-head
        // transition and supplies a live head or the null empty-list value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theaps), theaps) };
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose lock field is valid.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the heap allocation live and in bounds;
        // projecting its aligned initialized lock field creates no reference.
        unsafe { ptr::addr_of_mut!((*raw).theaps_lock) }
    }

    /// # Safety
    /// The heap pointer must refer to writable heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns this live heap policy update;
        // `node` supplies the initialized NUMA value for the field.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose lock field is valid.
    #[inline(always)]
    pub(crate) unsafe fn arena_pages_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the heap allocation live and in bounds; its
        // aligned embedded arena-pages lock therefore has a non-null field address.
        unsafe { ptr::addr_of_mut!((*raw).arena_pages_lock) }
    }

    /// # Safety
    /// The heap pointer must refer to heap metadata whose stats field is valid.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the heap allocation live and in bounds;
        // projecting its initialized stats field creates only a raw pointer.
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
    #[inline(always)]
    pub(crate) unsafe fn set_tld(self, tld: Option<TldPtr>) {
        let raw = self.as_ptr();
        let tld = tld.map(TldPtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the caller exclusively owns initialization or reassociation
        // of this live thread heap's TLD field and supplies a live TLD or null.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).tld), tld) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose
    /// refcount is valid for atomic access.
    #[inline(always)]
    pub(crate) unsafe fn refcount_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the thread heap live; the aligned `refcount`
        // field is initialized and all concurrent access follows its atomic protocol.
        unsafe { ptr::addr_of!((*raw).refcount).cast() }
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_heap(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the caller exclusively owns this live thread heap's heap
        // association transition and supplies a live heap or null sentinel.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heap), heap) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to initialized thread-heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn heartbeat(self) -> Heartbeat {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized thread heap live and grants
        // stable readable access to its heartbeat field for this copy.
        Heartbeat::new(unsafe { ptr::read(ptr::addr_of!((*raw).heartbeat)) })
    }

    /// # Safety
    /// The thread-heap pointer must refer to writable thread-heap metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_heartbeat(self, heartbeat: Heartbeat) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns this live thread heap's heartbeat
        // update and supplies an initialized typed value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heartbeat), heartbeat.get()) };
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose page
    /// queue at `bin` is initialized.
    #[inline(always)]
    pub(crate) unsafe fn page_queue(self, bin: Bin) -> PageQueuePtr {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the thread heap and complete queue array live;
        // `Bin` bounds the in-array field projection, whose address is non-null.
        unsafe { PageQueuePtr::new_unchecked(ptr::addr_of_mut!((*raw).pages[bin.as_usize()])) }
    }

    /// # Safety
    /// The thread-heap pointer must refer to initialized thread-heap metadata.
    /// Every direct-cache slot must contain either the non-null empty-page
    /// sentinel or a live page pointer.
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
    /// The thread-heap pointer and its full direct-cache array must remain live,
    /// initialized, and exclusively writable. `start <= end`; both `DirectBin`
    /// values bound an inclusive cache range. `page` must be the initialized
    /// empty-page sentinel or a live page suitable for every slot in that range.
    #[inline(always)]
    pub(crate) unsafe fn fill_direct_pages(self, start: DirectBin, end: DirectBin, page: PagePtr) {
        let raw = self.as_ptr();
        let start = start.as_usize();
        let end = end.as_usize();
        crate::internal::mi_assert_internal!(start <= end);
        let page = page.as_ptr();
        // SAFETY: typed bins bound the inclusive initialized cache range; the
        // caller exclusively owns every projected slot and supplies a non-null value.
        unsafe {
            let pages = ptr::addr_of_mut!((*raw).pages_free_direct).cast::<*mut mi_page_t>();
            core::slice::from_raw_parts_mut(pages.add(start), end - start + 1).fill(page);
        }
    }

    /// # Safety
    /// The thread-heap pointer must refer to thread-heap metadata whose stats
    /// field is valid.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the thread heap allocation live and in bounds;
        // projecting its initialized stats field creates only a raw pointer.
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}
define_ptr_wrapper!(TldPtr, mi_tld_t, "Non-null thread-local data");

impl TldPtr {
    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata. `thread_id`
    /// must be `None` only for the main-thread sentinel state.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_id(self, thread_id: Option<ThreadId>) {
        let raw = self.as_ptr();
        let thread_id = thread_id.map(ThreadId::get).unwrap_or(0);
        // SAFETY: the caller exclusively owns this live TLD identity update;
        // the typed ID or main-thread zero is the field's initialized state.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).thread_id), thread_id) };
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_seq(self, seq: ThreadSeq) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns this live TLD initialization or
        // update and supplies an initialized thread-sequence value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).thread_seq), seq.get()) };
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_numa_node(self, node: NumaNode) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns this live TLD's NUMA update and
        // supplies the initialized typed node value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).numa_node), node.get()) };
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized TLD live and stabilizes its
        // subprocess association while this raw pointer value is copied.
        SubprocPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).subproc)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>) {
        let raw = self.as_ptr();
        let subproc = subproc.map(SubprocPtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the caller exclusively owns this live TLD association update
        // and supplies a live subprocess pointer or the null main sentinel.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).subproc), subproc) };
    }

    /// # Safety
    /// The TLD pointer must refer to thread-local metadata whose memid field is valid.
    #[inline(always)]
    pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the TLD allocation live and in bounds;
        // projecting its initialized memid field creates only a raw pointer.
        unsafe { ptr::addr_of_mut!((*raw).memid) }
    }

    /// # Safety
    /// The TLD pointer must refer to initialized thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps(self) -> Option<ThreadHeapPtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized TLD live and stabilizes its
        // thread-heap list head while this raw pointer value is copied.
        ThreadHeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).theaps)) })
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_thread_heaps(self, theaps: Option<ThreadHeapPtr>) {
        let raw = self.as_ptr();
        let theaps = theaps.map(ThreadHeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the caller exclusively owns this live TLD list-head update
        // and supplies a live thread heap or the null empty-list value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).theaps), theaps) };
    }

    /// # Safety
    /// The TLD pointer must refer to thread-local metadata whose lock field is valid.
    #[inline(always)]
    pub(crate) unsafe fn thread_heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the TLD allocation live and in bounds;
        // projecting its aligned initialized lock field creates no reference.
        unsafe { ptr::addr_of_mut!((*raw).theaps_lock) }
    }

    /// # Safety
    /// The TLD pointer must refer to writable thread-local metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_is_in_threadpool(self, is_in_threadpool: bool) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns this live TLD state transition;
        // the boolean is a fully initialized representation for the field.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).is_in_threadpool), is_in_threadpool) };
    }
}
define_ptr_wrapper!(ArenaPtr, mi_arena_t, "Non-null raw arena metadata");

impl ArenaPtr {
    /// # Safety
    /// The arena pointer must refer to writable arena metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_index(self, index: ArenaIndex) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns initialization of this live
        // arena's identity and supplies the typed published arena index.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).arena_idx), index.get() as usize) };
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata with non-zero
    /// slice count. The count must remain stable and be representable as the
    /// allocator's `u32`-backed `SliceCount` for the duration of the read.
    #[inline(always)]
    pub(crate) unsafe fn slice_count(self) -> SliceCount {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized arena live and its non-zero
        // slice count stable and readable for this snapshot.
        let count = unsafe { ptr::read(ptr::addr_of!((*raw).slice_count)) };
        crate::internal::mi_assert_internal!(count != 0);
        SliceCount::from_usize(count).unwrap_or_else(|| {
            crate::internal::mi_assert_internal!(false);
            // One preserves release behavior after an impossible invalid count.
            SliceCount::ONE
        })
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    #[inline(always)]
    pub(crate) unsafe fn is_exclusive(self) -> bool {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized arena live and its policy
        // fields stable while the exclusive flag is copied.
        unsafe { ptr::read(ptr::addr_of!((*raw).is_exclusive)) }
    }

    /// # Safety
    /// The arena pointer must refer to initialized arena metadata.
    #[inline(always)]
    pub(crate) unsafe fn parent(self) -> Option<ArenaPtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized arena live and stabilizes
        // its parent association while the raw pointer value is copied.
        ArenaPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).parent)) })
    }
}
define_ptr_wrapper!(SubprocPtr, mi_subproc_t, "Non-null raw subproc metadata");

impl SubprocPtr {
    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    #[inline(always)]
    pub(crate) unsafe fn arena_count_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live; its aligned arena count
        // is initialized and all concurrent access uses the atomic protocol.
        unsafe { ptr::addr_of!((*raw).arena_count) }
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    #[inline(always)]
    pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount {
        // SAFETY: the caller's live atomic-field contract remains in force for
        // this load, including exclusive use of the atomic access protocol.
        let count = AtomicUsizeField::from_cell(unsafe { &*self.arena_count_field() });
        ArenaCount::new(count.load(order))
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose lock field is valid.
    #[inline(always)]
    pub(crate) unsafe fn arena_reserve_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live and in bounds; its
        // aligned embedded reserve lock therefore has a non-null field address.
        unsafe { ptr::addr_of_mut!((*raw).arena_reserve_lock) }
    }

    /// # Safety
    /// The subproc pointer must refer to initialized subproc metadata.
    #[inline(always)]
    pub(crate) unsafe fn heaps(self) -> Option<HeapPtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized subprocess live and its heap
        // list head stable while this raw pointer value is copied.
        HeapPtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).heaps)) })
    }

    /// # Safety
    /// The subproc pointer must refer to writable subproc metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_heaps(self, heap: Option<HeapPtr>) {
        let raw = self.as_ptr();
        let heap = heap.map(HeapPtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the caller exclusively owns subprocess heap-list relinking
        // and supplies a live heap head or the null empty-list value.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).heaps), heap) };
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose lock field is valid.
    #[inline(always)]
    pub(crate) unsafe fn heaps_lock(self) -> *mut mi_lock_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live and in bounds; its
        // aligned embedded heaps lock therefore has a non-null field address.
        unsafe { ptr::addr_of_mut!((*raw).heaps_lock) }
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    #[inline(always)]
    pub(crate) unsafe fn thread_count_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live; its aligned thread count
        // is initialized and all concurrent access uses the atomic protocol.
        unsafe { ptr::addr_of!((*raw).thread_count) }
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    #[inline(always)]
    pub(crate) unsafe fn thread_total_count_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live; its aligned total-thread
        // count is initialized and all concurrent access uses atomics.
        unsafe { ptr::addr_of!((*raw).thread_total_count) }
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose count field is valid.
    #[inline(always)]
    pub(crate) unsafe fn heap_count_field(self) -> *const AtomicUsize {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live; its aligned heap count
        // is initialized and all concurrent access uses the atomic protocol.
        unsafe { ptr::addr_of!((*raw).heap_count) }
    }

    /// # Safety
    /// The subproc pointer must refer to subproc metadata whose stats field is valid.
    #[inline(always)]
    pub(crate) unsafe fn stats(self) -> *mut mi_stats_t {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the subprocess live and in bounds; projecting
        // its initialized stats field creates only a raw pointer.
        unsafe { ptr::addr_of_mut!((*raw).stats) }
    }
}
define_ptr_wrapper!(PageQueuePtr, mi_page_queue_t, "Non-null page queue", no_new);

impl PageQueuePtr {
    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn first(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized queue live and its first-page
        // link stable under queue ownership while the pointer value is copied.
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).first)) })
    }

    /// # Safety
    /// The page-queue pointer must remain live and the caller must exclusively
    /// own its head transition. A non-null `first` must be suitable for that
    /// caller-owned queue transition.
    #[inline(always)]
    pub(crate) unsafe fn set_first(self, first: Option<PagePtr>) {
        let raw = self.as_ptr();
        let first = first.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the live queue participates in the caller-owned transition;
        // its head field is exclusive and `first` is not dereferenced here.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).first), first) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn last(self) -> Option<PagePtr> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized queue live and its last-page
        // link stable under queue ownership while the pointer value is copied.
        PagePtr::new(unsafe { ptr::read(ptr::addr_of!((*raw).last)) })
    }

    /// # Safety
    /// The page-queue pointer must remain live and the caller must exclusively
    /// own its tail transition. A non-null `last` must be suitable for that
    /// caller-owned queue transition.
    #[inline(always)]
    pub(crate) unsafe fn set_last(self, last: Option<PagePtr>) {
        let raw = self.as_ptr();
        let last = last.map(PagePtr::as_ptr).unwrap_or(ptr::null_mut());
        // SAFETY: the live queue participates in the caller-owned transition;
        // its tail field is exclusive and `last` is not dereferenced here.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).last), last) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn count(self) -> PageQueueCount {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized queue live and its count
        // stable under queue ownership while the value is copied.
        PageQueueCount::new(unsafe { ptr::read(ptr::addr_of!((*raw).count)) })
    }

    /// # Safety
    /// The page-queue pointer must refer to writable page-queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn set_count(self, count: PageQueueCount) {
        let raw = self.as_ptr();
        // SAFETY: the caller exclusively owns this live queue count update and
        // supplies a typed value consistent with its link transition.
        unsafe { ptr::write(ptr::addr_of_mut!((*raw).count), count.get()) };
    }

    /// # Safety
    /// The page-queue pointer must refer to initialized page-queue metadata.
    #[inline(always)]
    pub(crate) unsafe fn block_size(self) -> Option<BlockSize> {
        let raw = self.as_ptr();
        // SAFETY: the caller keeps the initialized queue live and its block-size
        // class stable and readable, including the zero unused-queue sentinel.
        let block_size = unsafe { ptr::read(ptr::addr_of!((*raw).block_size)) };
        if block_size == 0 {
            None
        } else {
            // SAFETY: the branch excludes zero, which is the sole `BlockSize`
            // validity requirement for this initialized queue field.
            Some(unsafe { BlockSize::new_unchecked(block_size) })
        }
    }
}

define_ptr_wrapper!(UserPtr, c_void, "Non-null pointer returned to/from user");

impl From<BlockPtr> for UserPtr {
    #[inline(always)]
    fn from(block: BlockPtr) -> Self {
        Self(block.0.cast())
    }
}
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
    pub(crate) unsafe fn from_current_page(page: PagePtr) -> Option<Self> {
        // SAFETY: the caller proves all three page geometry fields are
        // initialized and stable for this snapshot.
        let (start, block_size, reserved) =
            unsafe { (page.start(), page.block_size(), page.reserved()) };
        Self::from_parts(start, block_size, reserved)
    }

    /// # Safety
    /// `page` must refer to live initialized page metadata whose non-null block
    /// start, block size, and reserved count describe the current backing
    /// allocation. Concurrent mutation or teardown must be excluded while the
    /// snapshot and pointers derived from it are used.
    #[inline(always)]
    pub(crate) unsafe fn from_page(page: PagePtr, expected: BlockSize) -> Option<Self> {
        // SAFETY: the caller provides the live, stable page geometry required
        // by the delegated snapshot constructor.
        let layout = unsafe { Self::from_current_page(page) }?;
        if layout.block_size != expected {
            return None;
        }
        Some(layout)
    }

    #[inline(always)]
    pub(crate) fn area(self) -> PageArea {
        self.area
    }

    #[inline(always)]
    pub(crate) fn block_size(self) -> BlockSize {
        self.block_size
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
    /// Tests exposed-address geometry only; this does not prove provenance,
    /// liveness, initialization, or permission to access `block`.
    #[inline(always)]
    pub(crate) fn contains(self, block: BlockPtr) -> bool {
        self.index_of(block.as_ptr()).is_some()
    }

    /// Returns the block's range-relative index when it is an exact block
    /// start inside this exposed-address geometry. This does not prove
    /// provenance, liveness, initialization, or access permission.
    #[inline(always)]
    pub(crate) fn index_of(self, block: *const mi_block_t) -> Option<usize> {
        let byte_offset =
            ptr_exposed_addr(block).checked_sub(ptr_exposed_addr(self.first.as_ptr()))?;
        if !byte_offset.is_multiple_of(self.block_size.get()) {
            return None;
        }
        let index = byte_offset / self.block_size.get();
        (index < self.block_count).then_some(index)
    }

    #[inline(always)]
    pub(crate) fn block_at(self, index: usize) -> Option<BlockPtr> {
        (index < self.block_count).then(|| {
            // SAFETY: the branch bounds `index` to this validated range.
            unsafe { self.block_at_offset_unchecked(index) }
        })
    }

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
    /// `ptr` must be non-null. This constructor does not establish page
    /// validity, initialization, liveness, ownership, or readable access.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: *const mi_page_t) -> Self {
        // SAFETY: the caller proves only the invariant required by `NonNull`:
        // `ptr` is non-null. Casting away constness does not grant mutation.
        Self(unsafe { NonNull::new_unchecked(ptr as *mut mi_page_t) })
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *const mi_page_t {
        self.0.as_ptr() as *const mi_page_t
    }
}

#[cfg(test)]
mod tests;
