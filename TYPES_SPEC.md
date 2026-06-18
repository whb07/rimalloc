Below is a data-types-only refinement for the PRD. It is scoped to the repository’s actual shape: the crate currently exposes all modules publicly from lib.rs, including types, internal, page, free, heap, arena, atomic, and mimalloc; it conditionally uses no_std for optimized Linux non-test builds.  The manifest confirms Rust 2024, rlib/cdylib, the current feature flags, panic-abort, fat LTO, and codegen-units = 1.  The current type layer still has C aliases such as mi_threadid_t = usize, mi_page_flags_t = usize, mi_thread_free_t = usize, mi_encoded_t = usize, and raw mi_page_t/mi_block_t/mi_heap_t aliases.  The main page metadata struct currently contains raw ownership, free-list, queue, heap, thread-heap, size, page-start, and memid fields directly.

Refined PRD section: Data-type model for type-safe, zero-cost mechanical_mimalloc

1. Data-type objective

The modernization must introduce an internal Rust data-type layer that makes allocator invariants explicit while preserving the raw C ABI and current fast-path code shape.

The goal is not to hide allocator reality behind high-level abstractions. The goal is to make invalid internal calls unrepresentable: a local-free function should not accept a foreign page; an abandoned-page reclaim path should not accept an arbitrary page pointer; a full queue should not accept a non-full page; a memid union arm should not be readable unless the discriminator proves that arm is active; and encoded freelist logic should not be open-coded at arbitrary call sites.

All new internal data types must be zero-cost by default:

#[repr(transparent)]
pub(crate) struct PagePtr(core::ptr::NonNull<raw::mi_page_t>);

A type is acceptable for hot paths only if it compiles to the same pointer, integer, or branch structure as the current mechanical code. The allocation fast path currently loads page.free, falls back when null, stores the next block into page.free, increments used, and returns the block.  The free fast path currently computes an XOR between the current thread id and the page’s tagged thread id, then branches local-fast versus slow path.  The new data types must preserve those shapes.

2. Layering rule for data types

2.1 raw module: exact C-compatible representation

raw owns the current #[repr(C)] structs, C names, C callbacks, and exported ABI-compatible typedefs.

The following stay raw and layout-compatible:

pub type mi_page_t = mi_page_s;
pub type mi_block_t = mi_block_s;
pub type mi_heap_t = mi_heap_s;
pub type mi_theap_t = mi_theap_s;
pub type mi_arena_t = mi_arena_s;
pub type mi_subproc_t = mi_subproc_s;
pub type mi_tld_t = mi_tld_s;
pub type mi_memid_t = mi_memid_s;

raw is allowed to expose C-shaped fields because those fields are the compatibility substrate. Internal modules must stop taking these raw fields as semantic inputs once typed equivalents exist.

Required rule:

// Allowed only in raw/ffi/compat modules:
*mut raw::mi_page_t
*mut raw::mi_block_t
usize // as raw flags, bins, sizes, encoded pointers, thread-free words

2.2 core_types module: semantic newtypes

core_types owns all new internal data types. It should be split by domain:

src/core_types/
  mod.rs
  scalar.rs        // sizes, counts, alignments, bins, indices
  ptr.rs           // NonNull pointer wrappers
  layout.rs        // allocation/reallocation requests
  page.rs          // page state, ownership, flags, page queues
  block.rs         // block pointers, block tags, free-list links
  freelist.rs      // available/local/thread/encoded freelists
  heap.rs          // heap/thread-heap/tld/subproc wrappers
  arena.rs         // arena/slice/page-table wrappers
  memid.rs         // memid discriminator + union-safe views
  atomic.rs        // typed atomic field wrappers
  callbacks.rs     // typed callback wrappers

core_types may use raw pointers internally for field access, but it must not expose raw C aliases as its public API. It must also avoid Deref and DerefMut for allocator metadata, because producing Rust references from aliasable allocator metadata is often harder to prove than using raw pointer field access directly.

impl PagePtr {
    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *mut raw::mi_page_t {
        self.0.as_ptr()
    }
    // No Deref/DerefMut.
}

3. Scalar domain types

The current implementation uses usize for many distinct concepts: requested sizes, block sizes, usable sizes, bins, flags, arena indices, slice counts, encoded pointers, and thread-free words. These must become separate transparent types.

3.1 Required scalar type table

Concept	Internal type	Representation	Zero allowed?	Invariant
User-requested allocation size	RequestedSize	usize	Yes	Raw C API size; may be zero
Effective allocator request	EffectiveSize	usize	No in Rust layout paths	After mimalloc zero-size normalization
Padded request	PaddedSize	NonZeroUsize	No	requested + MI_PADDING_SIZE, checked
Usable size	UsableSize	usize	Yes	Size visible to user, excluding padding
Page block size	BlockSize	NonZeroUsize	No	Raw page block_size, includes padding
Full block size	FullBlockSize	NonZeroUsize	No	Block size including allocator metadata/padding
Good size	GoodSize	usize	Yes	Result of mi_good_size
Alignment	Alignment	NonZeroUsize	No	Power of two
Optional C alignment	MaybeAlignment	usize	Yes	Zero means “default/no explicit alignment”
Aligned-at offset	AlignmentOffset	usize	Yes	Must not overflow with size/alignment math
Word size count	WordCount	usize	Yes	ceil(size / size_of::<usize>())
Bin index	Bin	u16	No for bin 0 if invalid	< MI_BIN_COUNT
Small direct bin index	DirectBin	u16	Depends on index	< MI_PAGES_DIRECT
Stats bin	StatsBin	u16	Yes	<= MI_BIN_HUGE
Page queue count	PageQueueCount	usize	Yes	Mirrors raw queue count
Page capacity	PageCapacity	u16	Yes during init	used <= capacity <= reserved
Used block count	UsedBlockCount	u16	Yes	<= capacity
Reserved block count	ReservedBlockCount	u16	Normally nonzero after page init	>= capacity
Retire countdown	RetireExpire	u8	Yes	Zero means not retired
Slice index	SliceIndex	u32 or usize wrapper	Yes	< arena.slice_count
Slice count	SliceCount	NonZeroU32 internally	No for real arena/meta ids	Fits raw u32 memid fields
Arena index	ArenaIndex	u16 or u32	Yes	< MI_MAX_ARENAS
Numa node	NumaNode	i32	Yes	-1 or valid OS node
Thread id	ThreadId	NonZeroUsize	No	Low MI_PAGE_FLAG_MASK bits are zero
Thread sequence	ThreadSeq	usize	Yes	Monotonic best-effort id
Heap sequence	HeapSeq	usize	Yes	Mirrors raw heap sequence
Subproc sequence	SubprocSeq	usize	Yes	Mirrors raw subproc sequence
Heartbeat	Heartbeat	u64	Yes	Thread-heap heartbeat counter
OS page size	OsPageSize	NonZeroUsize	No	Power of two where required
Committed bytes	CommittedBytes	usize	Yes	Memory committed for page/arena
Reserved bytes	ReservedBytes	usize	Yes	Virtual/reserved memory
Page-map index	PageMapIndex	usize	Yes	< MI_PAGE_MAP_COUNT
Page-map sub-index	PageMapSubIndex	usize	Yes	< MI_PAGE_MAP_SUB_COUNT
Encoded pointer word	EncodedWord	usize	Yes	Only interpreted with matching keys
Canary	PaddingCanary	u32	Yes	Encoded from page/block/keys
Padding delta	PaddingDelta	u32	Yes	<= usable_block_size
Errno value	Errno	c_int	Yes	C errno-compatible

3.2 Scalar type rules

Every scalar type must implement only operations that preserve its invariant.

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BlockSize(core::num::NonZeroUsize);
impl BlockSize {
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(size: usize) -> Self {
        Self(core::num::NonZeroUsize::new_unchecked(size))
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
    #[inline(always)]
    pub(crate) fn usable(self) -> UsableSize {
        UsableSize(self.get() - raw::MI_PADDING_SIZE)
    }
}

Arithmetic that can overflow must return an option or domain-specific error:

impl RequestedSize {
    #[inline]
    pub(crate) fn checked_padded(self) -> Result<PaddedSize, AllocSizeError> {
        self.0
            .checked_add(raw::MI_PADDING_SIZE)
            .and_then(core::num::NonZeroUsize::new)
            .map(PaddedSize)
            .ok_or(AllocSizeError::Overflow)
    }
}

No new internal function should take bare usize for a known domain concept after that domain type exists. FFI functions and compatibility shims are exempt.

4. Pointer domain types

4.1 Required pointer wrappers

Raw pointer	Internal type	Nullable form	Invariant
*mut mi_page_t	PagePtr	Option<PagePtr>	Non-null, aligned raw page metadata
*const mi_page_t	PageConstPtr or PagePtr with read-only methods	Option<PageConstPtr>	Non-null raw page metadata
*mut mi_block_t	BlockPtr	Option<BlockPtr>	Non-null block pointer, block-aligned for its page
*mut mi_heap_t	HeapPtr	Option<HeapPtr>	Non-null raw heap metadata
*mut mi_theap_t	ThreadHeapPtr	Option<ThreadHeapPtr>	Non-null raw thread heap metadata
*mut mi_tld_t	TldPtr	Option<TldPtr>	Non-null thread-local data
*mut mi_arena_t	ArenaPtr	Option<ArenaPtr>	Non-null raw arena metadata
*mut mi_subproc_t	SubprocPtr	Option<SubprocPtr>	Non-null raw subproc metadata
*mut mi_page_queue_t	PageQueuePtr	Option<PageQueuePtr>	Non-null page queue
*mut c_void user allocation	UserPtr	Option<UserPtr>	Non-null pointer returned to/from user
Interior user pointer	InteriorUserPtr	Option<InteriorUserPtr>	User pointer may not equal block start
Page start pointer	PageStart	Never null after initialized page	Start of page block area
Slice start pointer	SliceStart	Never null after initialized page	Aligned arena slice start
Raw OS allocation pointer	OsMemPtr	Option<OsMemPtr>	OS allocation base

4.2 Pointer wrapper requirements

Pointer wrappers must be Copy, Clone, and #[repr(transparent)]. They must not implement Deref or DerefMut.

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PagePtr(NonNull<raw::mi_page_t>);
impl PagePtr {
    #[inline(always)]
    pub(crate) fn new(ptr: *mut raw::mi_page_t) -> Option<Self> {
        NonNull::new(ptr).map(Self)
    }
    /// # Safety
    /// `ptr` must be non-null, properly aligned, and point to live page metadata.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: *mut raw::mi_page_t) -> Self {
        Self(NonNull::new_unchecked(ptr))
    }
    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *mut raw::mi_page_t {
        self.0.as_ptr()
    }
}

Field access methods should use addr_of!, addr_of_mut!, read, and write rather than creating broad &mut raw::mi_page_t references.

impl PagePtr {
    #[inline(always)]
    pub(crate) fn block_size(self) -> BlockSize {
        // SAFETY: PagePtr proves non-null page metadata. The field is read by value.
        let size = unsafe { core::ptr::addr_of!((*self.as_ptr()).block_size).read() };
        // SAFETY: initialized pages have non-zero block_size; callers that may see uninit pages
        // must use Page<Uninit> and may not call this method.
        unsafe { BlockSize::new_unchecked(size) }
    }
}

5. Allocation layout types

5.1 LayoutRequest

Allocator-facing size and alignment must be represented by a validated request object instead of independent usize parameters.

#[derive(Clone, Copy)]
pub(crate) struct LayoutRequest {
    requested: RequestedSize,
    effective: EffectiveSize,
    padded: PaddedSize,
    alignment: MaybeAlignment,
    zeroed: Zeroed,
}

Required constructors:

impl LayoutRequest {
    pub(crate) fn malloc(size: usize) -> Result<Self, AllocSizeError>;
    pub(crate) fn zalloc(size: usize) -> Result<Self, AllocSizeError>;
    pub(crate) fn aligned(size: usize, alignment: usize) -> Result<Self, LayoutError>;
    pub(crate) fn aligned_at(
        size: usize,
        alignment: usize,
        offset: usize,
        zeroed: Zeroed,
    ) -> Result<Self, LayoutError>;
}

Required invariants:

* requested preserves the C API input.
* effective encodes mimalloc zero-size behavior.
* padded is checked effective + MI_PADDING_SIZE.
* alignment is either absent/default or a non-zero power-of-two.
* aligned-at calculations must prove that ptr + offset can satisfy alignment without overflow.
* calloc and realloc must not reuse LayoutRequest in ways that confuse old and new sizes.

5.2 CallocRequest

pub(crate) struct CallocRequest {
    count: ItemCount,
    item_size: RequestedSize,
    total: RequestedSize,
    layout: LayoutRequest,
}

CallocRequest::new(count, size) must be the only internal path that multiplies allocation count by item size. The current implementation has overflow helpers in internal.rs; these should become constructors for CallocRequest and AllocProduct.

5.3 ReallocRequest

pub(crate) struct ReallocRequest {
    ptr: Option<UserPtr>,
    old: OldAllocation,
    new_layout: LayoutRequest,
    zero_new_tail: Zeroed,
}

Required related types:

pub(crate) struct OldAllocation {
    ptr: UserPtr,
    usable: UsableSize,
    page: PagePtr,
    block: BlockPtr,
}
pub(crate) struct PreserveBytes(usize);

Rules:

* OldAllocation may only be constructed after pointer-to-page validation.
* PreserveBytes is computed as min(old.usable, new.requested) using checked/known-safe values.
* ReallocRequest must distinguish realloc(null, size) from resizing an existing allocation.

5.4 MallocGenericFlags

The current generic allocation path combines zeroing and huge-alignment into a single usize where low-bit constraints matter; mi_theap_malloc_generic asserts (huge_alignment & 1) == 0.  This must become a transparent typed flag object.

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MallocGenericFlags(usize);
impl MallocGenericFlags {
    pub(crate) fn new(zeroed: Zeroed, huge_alignment: MaybeHugeAlignment) -> Self;
    pub(crate) fn zeroed(self) -> Zeroed;
    pub(crate) fn huge_alignment(self) -> MaybeHugeAlignment;
}

Invariant: alignment bits must never collide with the zeroing bit.

6. Page identity, flags, and ownership types

The raw page currently stores xthread_id, free, used, capacity, reserved, retire_expire, local_free, xthread_free, block_size, page_start, optional keys, theap, heap, intrusive queue links, committed size, and memid directly.  These fields encode multiple state machines and must be separated into typed views.

6.1 PageFlags

Current flags are low bits in xthread_id: MI_PAGE_IN_FULL_QUEUE, MI_PAGE_HAS_INTERIOR_POINTERS, and MI_PAGE_FLAG_MASK.

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageFlags(usize);
impl PageFlags {
    pub(crate) const EMPTY: Self;
    pub(crate) const IN_FULL_QUEUE: Self;
    pub(crate) const HAS_INTERIOR_POINTERS: Self;
    #[inline(always)]
    pub(crate) const fn bits(self) -> usize;
    #[inline(always)]
    pub(crate) const fn contains(self, flag: Self) -> bool;
    #[inline(always)]
    pub(crate) const fn from_low_bits(bits: usize) -> Self;
}

Invariant: PageFlags only contains bits inside MI_PAGE_FLAG_MASK.

6.2 ThreadId and PageThreadId

ThreadId represents a real owning thread id. It must be non-zero and must have the flag bits clear.

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThreadId(NonZeroUsize);

PageThreadId wraps the raw tagged page field:

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct PageThreadId(usize);
pub(crate) enum PageOwnerId {
    Thread(ThreadId),
    Abandoned,
    AbandonedMapped,
}

Required methods:

impl PageThreadId {
    pub(crate) fn owner(self) -> PageOwnerId;
    pub(crate) fn flags(self) -> PageFlags;
    pub(crate) fn is_abandoned(self) -> bool;
    pub(crate) fn is_abandoned_mapped(self) -> bool;
    pub(crate) fn with_flags(self, flags: PageFlags) -> Self;
    pub(crate) fn with_owner(self, owner: PageOwnerId) -> Self;
}

The current mi_page_set_theap method writes either an abandoned sentinel or the thread id from the thread heap’s TLD, while preserving the low page-flag bits.  That behavior must move into PageThreadId::with_owner.

6.3 Fast-path free classification

The free-path fast classification must preserve the current XOR structure.

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct XThreadDelta(usize);
pub(crate) enum FreePathKind {
    LocalFast,
    LocalSlowFlags,
    RemotePlain,
    RemoteSlowFlags,
}

Required branch-equivalent API:

impl PageThreadId {
    #[inline(always)]
    pub(crate) fn xor_current(self, current: ThreadId) -> XThreadDelta {
        XThreadDelta(current.get() ^ self.0)
    }
}
impl XThreadDelta {
    #[inline(always)]
    pub(crate) fn is_local_fast(self) -> bool {
        self.0 == 0
    }
    #[inline(always)]
    pub(crate) fn needs_local_slow(self) -> bool {
        self.0 <= raw::MI_PAGE_FLAG_MASK
    }
    #[inline(always)]
    pub(crate) fn is_remote_plain(self) -> bool {
        (self.0 & raw::MI_PAGE_FLAG_MASK) == 0
    }
}

The hot call site should remain structurally equivalent to:

let delta = page.thread_id().xor_current(current_thread);
if delta.is_local_fast() {
    // local fast free
} else {
    // cold slow path
}

The typed ownership enum may be created only in cold code if doing so would add branches or code size to the fast path.

6.4 Page ownership wrappers

#[repr(transparent)]
pub(crate) struct LocalPage(PagePtr);
#[repr(transparent)]
pub(crate) struct ForeignPage(PagePtr);
#[repr(transparent)]
pub(crate) struct AbandonedPage(PagePtr);
#[repr(transparent)]
pub(crate) struct AbandonedMappedPage(PagePtr);
#[repr(transparent)]
pub(crate) struct OwnedAbandonedPage(PagePtr);

Required constructors:

impl PagePtr {
    pub(crate) fn classify_for_free(self, current: ThreadId) -> PageFreeOwner;
    pub(crate) fn try_local(self, current: ThreadId) -> Result<LocalPage, PagePtr>;
    pub(crate) fn try_abandoned(self) -> Result<AbandonedPage, PagePtr>;
}
pub(crate) enum PageFreeOwner {
    Local(LocalPage),
    Foreign(ForeignPage),
    Abandoned(AbandonedPage),
}

Required call restrictions:

fn free_block_local(page: LocalPage, block: BlockForFree, mode: LocalFreeMode);
fn free_block_remote(page: ForeignPage, block: BlockForFree);
fn try_reclaim_abandoned(page: OwnedAbandonedPage, target: ThreadHeapPtr) -> ReclaimResult;

The current mi_free_block_local mutates local_free, decrements used, retires all-free pages, or unfulls pages when needed.  It must only accept LocalPage. The current remote free path pushes onto xthread_free with a CAS loop and handles abandoned-page collection when ownership changes.  It must only accept ForeignPage or AbandonedPage.

7. Page lifecycle state machine

7.1 Page typestate core

Use typestate for page lifecycle transitions where it improves correctness without hurting hot-path codegen. Use transparent wrappers for hot-path classifications where typestate would create bloat.

pub(crate) struct Page<S> {
    ptr: PagePtr,
    _state: PhantomData<S>,
}
pub(crate) enum Uninit {}
pub(crate) enum Active {}
pub(crate) enum Full {}
pub(crate) enum Retired {}
pub(crate) enum Abandoned {}
pub(crate) enum AbandonedMapped {}
pub(crate) enum Destroyed {}

All state wrappers are #[repr(transparent)] if they contain only PagePtr; if the PhantomData prevents transparent representation, use tuple newtypes for hot states:

#[repr(transparent)]
pub(crate) struct ActivePage(PagePtr);
#[repr(transparent)]
pub(crate) struct FullPage(PagePtr);
#[repr(transparent)]
pub(crate) struct RetiredPage(PagePtr);

7.2 Required page states

State	Raw field interpretation	Valid typed operations
UninitPage	Metadata exists but allocation lists and ownership are not ready	initialize(theap)
ActivePage	Owned by a thread heap, in a normal page queue, may allocate/free	malloc_from_free, free_local, to_full, retire, abandon
AvailablePage	Active and free != null	pop_free_block
FullPage	Active and in full queue; full flag set	unfull, free_local, abandon
RetiredPage	Active, all-free, retire countdown nonzero	tick_retire, free_to_arena, reactivate_if_used
AbandonedPage	No active thread heap owner; abandoned sentinel thread id	claim_ownership, try_free, try_reclaim, try_reabandon_to_mapped
OwnedAbandonedPage	Abandoned page with xthread-free ownership bit claimed	collect_remote_frees, reclaim, unown_from_free
AbandonedMappedPage	Abandoned and marked mapped	unmap_or_reclaim
DestroyedPage	Returned to arena/OS; metadata not usable	No methods except raw debug tombstone checks

7.3 Required page transitions

Transition	Method	Source type	Target type
Fresh arena page initialized	initialize	Page<Uninit> + ThreadHeapLocal	ActivePage
Fast allocation possible	available	ActivePage	Option<AvailablePage>
Pop free block	pop_free_block	AvailablePage	(AllocatedBlock, ActivePage)
Page becomes full	to_full	ActivePage + NormalPageQueue	FullPage
Page leaves full queue	unfull	FullPage + FullPageQueue	ActivePage
All blocks free	mark_all_free	ActivePage	AllFreePage<Active>
Retire all-free page	retire	AllFreePage<Active>	RetiredPage
Retired countdown expires	expire	RetiredPage	DestroyedPage
Page abandoned	abandon	ActivePage or FullPage	AbandonedPage
Abandoned page ownership claimed	claim_ownership	AbandonedPage	Option<OwnedAbandonedPage>
Abandoned page reclaimed	reclaim	OwnedAbandonedPage + ThreadHeapLocal	ActivePage
Abandoned page reabandoned as mapped	try_reabandon_to_mapped	OwnedAbandonedPage	AbandonedMappedPage or unchanged
Page freed to arena	free_to_arena	AllFreePage<_>	DestroyedPage

The current _mi_page_init establishes a page’s heap, thread heap, keys, page area, ownership, empty free lists, zero used, owned xthread_free, and then extends the free list.  That should become the only constructor from UninitPage to ActivePage.

The current reclaim path asserts that the page is owned and abandoned, then sets the thread heap, collects frees, finds the queue, and pushes at the end.  That should become the only transition from OwnedAbandonedPage to ActivePage.

8. Block and free-list types

8.1 Block pointer family

#[repr(transparent)]
pub(crate) struct BlockPtr(NonNull<raw::mi_block_t>);
#[repr(transparent)]
pub(crate) struct AllocatedBlock(BlockPtr);
#[repr(transparent)]
pub(crate) struct FreeBlock(BlockPtr);
#[repr(transparent)]
pub(crate) struct GuardedBlock(BlockPtr);
pub(crate) struct BlockForFree {
    block: BlockPtr,
    was_guarded: WasGuarded,
}

Rules:

* AllocatedBlock is a block currently owned by user code.
* FreeBlock is a block already in one of the allocator’s free lists.
* GuardedBlock is a block whose raw next tag represents guarded allocation metadata.
* BlockForFree is produced only after pointer validation, optional unalignment, and guarded-block unprotection.

8.2 Encoded pointer and block-link types

The current freelist encoding uses mi_encoded_t = usize; decoding and encoding depend on page keys, and corrupted decoded pointers are checked against page membership.

Required types:

#[repr(transparent)]
pub(crate) struct EncodedWord(usize);
#[derive(Clone, Copy)]
pub(crate) struct PageKeys([usize; 2]);
#[repr(transparent)]
pub(crate) struct EncodedPtr<T> {
    word: EncodedWord,
    _ty: PhantomData<*mut T>,
}

Required methods:

impl EncodedPtr<raw::mi_block_t> {
    pub(crate) fn decode_block(
        self,
        null_context: PagePtr,
        keys: PageKeys,
    ) -> Option<BlockPtr>;
    pub(crate) fn encode_block(
        block: Option<BlockPtr>,
        null_context: PagePtr,
        keys: PageKeys,
    ) -> Self;
}

No call site outside core_types::block or core_types::freelist may directly read or write mi_block_s.next.

8.3 Free-list type family

#[repr(transparent)]
pub(crate) struct AvailableFreeList(Option<BlockPtr>);
#[repr(transparent)]
pub(crate) struct LocalFreeList(Option<BlockPtr>);
#[repr(transparent)]
pub(crate) struct RemoteFreeList(Option<BlockPtr>);
#[repr(transparent)]
pub(crate) struct EncodedFreeList(Option<BlockPtr>);

Recommended zero-cost implementation: store Option<BlockPtr>. Rust’s Option<NonNull<T>> has the same size as a raw pointer on supported targets due to the null-pointer niche.

Required operations:

impl AvailableFreeList {
    pub(crate) fn pop(self, page: AvailablePage) -> (AllocatedBlock, AvailableFreeList);
    pub(crate) fn push(self, page: LocalPage, block: FreeBlock) -> Self;
}
impl LocalFreeList {
    pub(crate) fn collect_into_available(
        self,
        page: LocalPage,
        available: AvailableFreeList,
        force: ForceCollect,
    ) -> (AvailableFreeList, LocalFreeList);
}
impl RemoteFreeList {
    pub(crate) fn collect_into_local(
        self,
        page: OwnedAbandonedPage,
        local: LocalFreeList,
    ) -> LocalFreeList;
}

8.4 Thread-free word

The current xthread_free field packs a block pointer and an ownership bit; helper functions mask the low bit for block, test ownership, and construct the word.

Required type:

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct ThreadFreeWord(usize);
pub(crate) enum ThreadFreeOwner {
    Owned,
    Unowned,
}
pub(crate) struct ThreadFreeSnapshot {
    head: Option<BlockPtr>,
    owner: ThreadFreeOwner,
}

Required methods:

impl ThreadFreeWord {
    pub(crate) const OWNED_BIT: usize = 1;
    #[inline(always)]
    pub(crate) fn head(self) -> Option<BlockPtr>;
    #[inline(always)]
    pub(crate) fn is_owned(self) -> bool;
    #[inline(always)]
    pub(crate) fn from_parts(head: Option<BlockPtr>, owned: ThreadFreeOwner) -> Self;
}

Invariant: all BlockPtr values stored in ThreadFreeWord must have low bit zero. This must be checked in debug constructors and documented as a page/block alignment invariant.

8.5 Atomic thread-free cell

#[repr(transparent)]
pub(crate) struct ThreadFreeCell(*mut usize);

Required API:

impl ThreadFreeCell {
    pub(crate) fn load_relaxed(self) -> ThreadFreeWord;
    pub(crate) fn compare_exchange_push(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord>;
    pub(crate) fn claim_ownership(self) -> bool;
    pub(crate) fn release_ownership(self, expected_head: Option<BlockPtr>) -> bool;
}

The remote free CAS loop must call this typed cell rather than directly using mi_atomic_load_explicit, mi_atomic_cas_weak_acq_rel, and raw address casts.

9. Page queue types

Raw mi_page_queue_t contains first, last, count, and block_size.  It currently relies on procedural checks to ensure full pages are in the full queue, huge pages are in the huge queue, and normal pages are in the matching size queue.

9.1 Queue kind types

pub(crate) enum NormalQueue {}
pub(crate) enum HugeQueue {}
pub(crate) enum FullQueue {}
#[repr(transparent)]
pub(crate) struct PageQueue<K> {
    ptr: PageQueuePtr,
    _kind: PhantomData<K>,
}

If PhantomData prevents repr(transparent), use concrete newtypes:

#[repr(transparent)]
pub(crate) struct NormalPageQueue(PageQueuePtr);
#[repr(transparent)]
pub(crate) struct HugePageQueue(PageQueuePtr);
#[repr(transparent)]
pub(crate) struct FullPageQueue(PageQueuePtr);

9.2 Queue membership witnesses

pub(crate) struct QueuedPage<K> {
    page: PagePtr,
    queue: PageQueue<K>,
}
pub(crate) struct UnqueuedPage<S> {
    page: Page<S>,
}

Required rules:

* NormalPageQueue may contain only active non-full non-huge pages whose block_size equals queue block_size.
* FullPageQueue may contain only FullPage.
* HugePageQueue may contain only huge/singleton pages.
* QueuedPage<FullQueue> is the only type accepted by unfull.
* QueuedPage<NormalQueue> is the only type accepted by normal dequeue/free-to-arena paths.

The current _mi_page_unfull temporarily clears and restores the full flag to find the original queue and move the page.  The typed API should encode this as FullPage::unfull(full_queue, target_queue) and keep the raw flag manipulation private.

10. Heap, thread-heap, TLD, and subproc types

10.1 Heap pointer family

#[repr(transparent)]
pub(crate) struct HeapPtr(NonNull<raw::mi_heap_t>);
#[repr(transparent)]
pub(crate) struct MainHeap(HeapPtr);
#[repr(transparent)]
pub(crate) struct OwnedHeap(HeapPtr);
#[repr(transparent)]
pub(crate) struct BorrowedHeap(HeapPtr);

Required rules:

* MainHeap represents the global main heap and must never be deleted through mi_heap_delete.
* OwnedHeap represents a heap created by heap APIs and eligible for delete/destroy.
* BorrowedHeap represents a heap pointer observed from an allocation or raw C input.
* Option<HeapPtr> represents nullable raw C heap inputs.

The raw heap currently stores subproc links, intrusive heap links, thread-heap pointer, optional exclusive arena, abandoned counts, OS abandoned pages, arena page table, locks, and stats.  Those should be exposed internally through typed accessors, not public raw field access.

10.2 Thread heap and TLD states

#[repr(transparent)]
pub(crate) struct ThreadHeapPtr(NonNull<raw::mi_theap_t>);
#[repr(transparent)]
pub(crate) struct TldPtr(NonNull<raw::mi_tld_t>);
#[repr(transparent)]
pub(crate) struct ThreadHeapLocal(ThreadHeapPtr);
#[repr(transparent)]
pub(crate) struct ThreadHeapBorrowed(ThreadHeapPtr);
pub(crate) enum ThreadHeapState {
    Uninitialized,
    Initialized,
    Main,
    Deleted,
}

Required operations:

impl ThreadHeapPtr {
    pub(crate) fn try_local(self, current: ThreadId) -> Result<ThreadHeapLocal, ThreadHeapPtr>;
    pub(crate) fn heap(self) -> Option<HeapPtr>;
    pub(crate) fn tld(self) -> Option<TldPtr>;
}

The raw thread heap stores TLD, heap, refcount, heartbeat, random state, page counters, full-retain/reclaim/abandon flags, guarded settings under guarded, direct free pages, page queues, memid, and stats.  Internal accessors must expose these as typed values.

10.3 Subproc types

#[repr(transparent)]
pub(crate) struct SubprocPtr(NonNull<raw::mi_subproc_t>);
#[repr(transparent)]
pub(crate) struct MainSubproc(SubprocPtr);
#[repr(transparent)]
pub(crate) struct BorrowedSubproc(SubprocPtr);

Required typed fields:

* SubprocSeq
* ArenaCount
* ThreadCount
* HeapCount
* ArenaIndex
* MiLockGuard
* MemId

Raw intrusive links remain raw-private.

11. Arena and slice types

11.1 Arena pointer family

#[repr(transparent)]
pub(crate) struct ArenaPtr(NonNull<raw::mi_arena_t>);
#[repr(transparent)]
pub(crate) struct SharedArena(ArenaPtr);
#[repr(transparent)]
pub(crate) struct ExclusiveArena(ArenaPtr);
#[repr(transparent)]
pub(crate) struct ParentArena(ArenaPtr);

Required invariants:

* ExclusiveArena may only be constructed if raw is_exclusive is true.
* SharedArena may only be constructed if raw is_exclusive is false.
* ParentArena is a non-owning arena relationship.
* ArenaIndex < MI_MAX_ARENAS.
* SliceIndex + SliceCount <= arena.slice_count.

The raw arena stores memid, subproc, arena index, slice count, info slices, NUMA node, exclusivity, purge/commit metadata, parent, bitmaps, meta pages, and page tables.

11.2 Slice and arena allocation types

#[repr(transparent)]
pub(crate) struct SliceIndex(u32);
#[repr(transparent)]
pub(crate) struct SliceCount(NonZeroU32);
pub(crate) struct SliceRange {
    start: SliceIndex,
    count: SliceCount,
}
pub(crate) struct ArenaAllocation {
    arena: ArenaPtr,
    range: SliceRange,
    memid: MemId<ArenaMem>,
}

Required constructors:

impl SliceRange {
    pub(crate) fn new(start: usize, count: usize, arena: ArenaPtr) -> Result<Self, SliceError>;
}

Rules:

* Conversion to raw u32 must be checked.
* Arena-backed MemId must use SliceRange.
* No direct as u32 truncation outside this module.

11.3 Arena page-table types

#[repr(transparent)]
pub(crate) struct ArenaPageTablePtr(NonNull<raw::mi_arena_pages_t>);
#[repr(transparent)]
pub(crate) struct AbandonedCountCell(*const core::sync::atomic::AtomicUsize);
#[repr(transparent)]
pub(crate) struct ArenaPageCell(*const core::sync::atomic::AtomicPtr<c_void>);

Required rules:

* Abandoned count indexing uses Bin, not usize.
* Arena page table indexing uses ArenaIndex, not usize.
* A page registered in the page map must produce a RegisteredPageRange witness before pointer-to-page lookup can return a trusted PagePtr.

12. MemId data types

The current memid model uses a #[repr(i32)] mi_memkind_t, a union mi_memid_mem_t, and a mi_memid_t struct containing union plus discriminator and flags.  There is already at least one helper that checks memkind before reading an arena union arm.  This must become a complete typed model.

12.1 Raw-compatible memkind

C-like enum values that may cross ABI boundaries must not be modeled internally as exhaustive Rust enums unless all values are controlled. Use an integer newtype for raw representation.

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemKindCode(i32);

Known constants:

impl MemKindCode {
    pub(crate) const NONE: Self;
    pub(crate) const EXTERNAL: Self;
    pub(crate) const STATIC: Self;
    pub(crate) const META: Self;
    pub(crate) const OS: Self;
    pub(crate) const OS_HUGE: Self;
    pub(crate) const OS_REMAP: Self;
    pub(crate) const ARENA: Self;
    pub(crate) const HEAP_MAIN: Self;
}

Typed interpretation:

pub(crate) enum MemKind {
    None,
    External,
    Static,
    Meta,
    Os,
    OsHuge,
    OsRemap,
    Arena,
    HeapMain,
    Unknown(MemKindCode),
}

12.2 Typed memid views

pub(crate) struct MemId<K = AnyMem> {
    raw: raw::mi_memid_t,
    _kind: PhantomData<K>,
}
pub(crate) enum AnyMem {}
pub(crate) enum NoneMem {}
pub(crate) enum ExternalMem {}
pub(crate) enum StaticMem {}
pub(crate) enum MetaMem {}
pub(crate) enum OsMem {}
pub(crate) enum ArenaMem {}
pub(crate) enum HeapMainMem {}

View enum:

pub(crate) enum MemIdView {
    None(MemId<NoneMem>),
    External(MemId<ExternalMem>),
    Static(MemId<StaticMem>),
    Meta(MetaMemId),
    Os(OsMemId),
    Arena(ArenaMemId),
    HeapMain(MemId<HeapMainMem>),
    Unknown(MemKindCode),
}

Typed field structs:

pub(crate) struct OsMemId {
    base: Option<OsMemPtr>,
    size: ReservedBytes,
    initially_committed: InitiallyCommitted,
    initially_zero: InitiallyZero,
    pinned: Pinned,
    kind: OsMemKind,
}
pub(crate) enum OsMemKind {
    Normal,
    Huge,
    Remap,
}
pub(crate) struct ArenaMemId {
    arena: ArenaPtr,
    range: SliceRange,
    initially_committed: InitiallyCommitted,
    initially_zero: InitiallyZero,
    pinned: Pinned,
}
pub(crate) struct MetaMemId {
    meta_page: NonNull<c_void>,
    block_index: BlockIndex,
    block_count: NonZeroU32,
}

12.3 MemId construction rules

All raw memid constructors must be replaced internally:

impl MemId<NoneMem> {
    pub(crate) const fn none() -> Self;
}
impl MemId<OsMem> {
    pub(crate) fn os(
        base: Option<OsMemPtr>,
        size: ReservedBytes,
        committed: InitiallyCommitted,
        zero: InitiallyZero,
        large: Pinned,
    ) -> Self;
}
impl MemId<ArenaMem> {
    pub(crate) fn arena(arena: ArenaPtr, range: SliceRange) -> Self;
}
impl MemId<MetaMem> {
    pub(crate) fn meta(
        meta_page: NonNull<c_void>,
        block_index: BlockIndex,
        block_count: NonZeroU32,
    ) -> Self;
}

Acceptance criteria:

* No direct memid.mem.os, memid.mem.arena, or memid.mem.meta reads outside core_types::memid.
* MemId::view() is the only way to inspect an arbitrary raw memid.
* Every union read is guarded by a memkind check.
* Unknown(MemKindCode) must be handled without UB.

13. Atomic data types

The current atomic compatibility layer maps mi_memory_order_e to Rust Ordering, uses raw pointer-to-atomic casts, and exposes a C-style memory-order enum.   It also centralizes pointer-to-atomic reference creation in helpers such as atomic_usize.

13.1 Memory order type

Internal APIs must use core::sync::atomic::Ordering. C-compatible mi_memory_order_e stays only in raw/atomic_compat.

pub(crate) type MemoryOrder = core::sync::atomic::Ordering;

13.2 Atomic field wrappers

#[repr(transparent)]
pub(crate) struct AtomicUsizeField(*mut usize);
#[repr(transparent)]
pub(crate) struct AtomicPageThreadId(AtomicUsizeField);
#[repr(transparent)]
pub(crate) struct AtomicThreadFree(AtomicUsizeField);
#[repr(transparent)]
pub(crate) struct AtomicAbandonedCount(*const AtomicUsize);
#[repr(transparent)]
pub(crate) struct AtomicArenaPage(*const AtomicPtr<c_void>);

Required methods:

impl AtomicPageThreadId {
    pub(crate) fn load_relaxed(self) -> PageThreadId;
    pub(crate) fn set_flags(self, flags: PageFlags, set: bool) -> PageThreadId;
    pub(crate) fn set_owner_release(self, owner: PageOwnerId);
}
impl AtomicThreadFree {
    pub(crate) fn load_relaxed(self) -> ThreadFreeWord;
    pub(crate) fn push_remote_acq_rel(self, block: BlockPtr) -> RemotePushOutcome;
    pub(crate) fn collect_acq_rel(self) -> Option<RemoteFreeList>;
    pub(crate) fn claim_acq_rel(self) -> bool;
}

Pointer-to-atomic casts are allowed only inside AtomicUsizeField. The wrapper must document alignment, aliasing, and atomic access requirements once.

13.3 Lock and once types

The current mi_atomic_guard_scope_t and mi_atomic_once_scope_t already use RAII-style Drop to release guards.  These should become internal Rust-named wrappers:

pub(crate) struct AtomicGuard<'a> {
    field: AtomicUsizeField,
    _borrow: PhantomData<&'a ()>,
}
pub(crate) struct OnceGuard<'a> {
    once: OnceStatePtr,
    _borrow: PhantomData<&'a ()>,
}
#[repr(transparent)]
pub(crate) struct MiLockPtr(NonNull<raw::mi_lock_t>);
pub(crate) struct MiLockGuard<'a> {
    lock: MiLockPtr,
    _borrow: PhantomData<&'a ()>,
}

14. Guarded allocation and padding types

14.1 Guarded block tags

The current internal constants use MI_BLOCK_TAG_ALIGNED = 0 and MI_BLOCK_TAG_GUARDED = !MI_BLOCK_TAG_ALIGNED.  The raw block’s next field is therefore overloaded as either a freelist link or a guarded tag. That must become typed.

#[repr(transparent)]
pub(crate) struct BlockTag(usize);
impl BlockTag {
    pub(crate) const ALIGNED: Self;
    pub(crate) const GUARDED: Self;
}
pub(crate) enum BlockLink {
    FreeList(EncodedPtr<raw::mi_block_t>),
    Guarded(BlockTag),
}

14.2 Padding metadata

Raw padding is currently:

#[repr(C)]
pub struct mi_padding_s {
    pub canary: u32,
    pub delta: u32,
}

Typed wrapper:

#[repr(transparent)]
pub(crate) struct PaddingPtr(NonNull<raw::mi_padding_t>);
#[derive(Clone, Copy)]
pub(crate) struct Padding {
    canary: PaddingCanary,
    delta: PaddingDelta,
}

Required rules:

* PaddingDelta <= UsableSize.
* PaddingCanary is generated only by PageKeys.
* Padding read/write is centralized in core_types::block::padding.
* Guarded blocks skip ordinary padding verification where current behavior does so.

15. User pointer validation types

15.1 Pointer lookup result

pub(crate) enum PointerLookup {
    Null,
    Valid {
        user: UserPtr,
        page: PagePtr,
        location: PointerLocation,
    },
    Invalid {
        user: Option<UserPtr>,
        reason: PointerInvalidReason,
    },
}
pub(crate) enum PointerLocation {
    BlockStart(BlockPtr),
    Interior {
        block: BlockPtr,
        offset: BlockOffset,
    },
    Guarded {
        block: GuardedBlock,
        offset: BlockOffset,
    },
}

15.2 Free preparation

pub(crate) struct ValidatedFree {
    user: UserPtr,
    page: PagePtr,
    block: BlockForFree,
}

Only ValidatedFree can enter the typed free path. This prevents code from accidentally freeing an interior pointer as though it were a block start.

Required flow:

C pointer
  -> PointerLookup
  -> ValidatedFree
  -> classify page ownership
  -> LocalPage/F​​oreignPage/AbandonedPage
  -> free_block_local/free_block_remote

16. Feature-gated data types

The raw page has optional keys: [usize; 2] under secure, debug assertions, track-valgrind, or track-asan.  This must be hidden behind a stable typed API.

#[derive(Clone, Copy)]
pub(crate) struct PageKeys([usize; 2]);
impl PagePtr {
    pub(crate) fn keys(self) -> PageKeys;
    pub(crate) fn set_keys(self, keys: PageKeys);
}

When keys are not compiled in, PageKeys::disabled() returns [0, 0] and methods compile away.

Guarded feature types:

#[cfg(feature = "guarded")]
pub(crate) struct GuardedConfig {
    size_min: RequestedSize,
    size_max: RequestedSize,
    sample_rate: NonZeroUsize,
    sample_count: usize,
}
pub(crate) enum GuardedDecision {
    UseGuarded,
    UseNormal,
}

Tracking feature types:

pub(crate) enum TrackMode {
    Disabled,
    Valgrind,
    Asan,
}

The mode should be const/cfg resolved; it must not add runtime branching to hot paths beyond what already exists.

17. “Illegal states impossible” API restrictions

The following signatures must become impossible in new internal code:

fn mi_free_block_local(page: *mut mi_page_t, block: *mut mi_block_t, ...);
fn mi_free_block_mt(page: *mut mi_page_t, block: *mut mi_block_t, ...);
fn _mi_theap_page_reclaim(theap: *mut mi_theap_t, page: *mut mi_page_t);
fn _mi_arenas_page_free(page: *mut mi_page_t, theap: *mut mi_theap_t);
fn mi_page_queue_enqueue_from(full: *mut mi_page_queue_t, pq: *mut mi_page_queue_t, page: *mut mi_page_t);

They should become typed equivalents:

fn free_block_local(page: LocalPage, block: BlockForFree, mode: LocalFreeMode);
fn free_block_remote(page: ForeignPage, block: BlockForFree);
fn reclaim_page(
    page: OwnedAbandonedPage,
    target: ThreadHeapLocal,
) -> ActivePage;
fn free_all_free_page_to_arena(
    page: AllFreePage<Active>,
    owner: ThreadHeapLocal,
) -> DestroyedPage;
fn move_to_full_queue(
    page: ActivePage,
    from: NormalPageQueue,
    to: FullPageQueue,
) -> FullPage;

18. Data-type acceptance criteria

This PRD phase is complete for data types when all of the following are true:

1. core_types exists and owns all internal semantic wrappers.
2. raw remains the only place where C names and raw ABI layout are mandatory.
3. No new internal API accepts bare usize for size, alignment, bin, flags, counts, encoded pointers, arena indices, slice counts, or thread-free words once the matching domain type exists.
4. No new internal API accepts *mut mi_page_t, *mut mi_block_t, *mut mi_heap_t, *mut mi_theap_t, or *mut mi_arena_t outside raw/ffi/compat modules once pointer wrappers exist.
5. mi_free_block_local or its replacement accepts only LocalPage.
6. Remote free accepts only ForeignPage or AbandonedPage.
7. Reclaim accepts only OwnedAbandonedPage.
8. Full-queue operations accept only FullPage and FullPageQueue.
9. Normal queue operations accept only active non-full pages and matching NormalPageQueue.
10. Memid union reads occur only through MemId::view() or typed MemId<K> constructors.
11. xthread_id bit operations occur only through PageThreadId, PageFlags, or AtomicPageThreadId.
12. xthread_free bit operations occur only through ThreadFreeWord or AtomicThreadFree.
13. mi_block_s.next is read/written only through BlockLink, EncodedPtr, or free-list wrappers.
14. Pointer-to-atomic casts occur only through typed atomic field wrappers.
15. Pointer wrappers do not implement Deref or DerefMut.
16. All hot wrappers are #[repr(transparent)], Copy, and inlineable.
17. Assembly checks show no added branches or memory traffic in mi_malloc, mi_malloc_small, mi_free, and _mi_theap_malloc_zero_ex.
18. Layout tests prove raw #[repr(C)] structs retain size, alignment, and field offsets across secure, guarded, track-valgrind, and track-asan.

19. Revised first data-type PR sequence

1. PR DT-1: core_types::scalar
    Add RequestedSize, EffectiveSize, PaddedSize, UsableSize, BlockSize, Alignment, MaybeAlignment, Bin, DirectBin, SliceIndex, SliceCount, ArenaIndex, and checked constructors.
2. PR DT-2: core_types::ptr
    Add PagePtr, BlockPtr, HeapPtr, ThreadHeapPtr, TldPtr, ArenaPtr, SubprocPtr, PageQueuePtr, UserPtr, and OsMemPtr. Do not migrate hot paths yet.
3. PR DT-3: LayoutRequest
    Add LayoutRequest, CallocRequest, ReallocRequest, MallocGenericFlags, and conversion shims from C API inputs.
4. PR DT-4: PageFlags and PageThreadId
    Centralize MI_PAGE_FLAG_MASK, MI_PAGE_IN_FULL_QUEUE, MI_PAGE_HAS_INTERIOR_POINTERS, abandoned, and abandoned-mapped logic.
5. PR DT-5: ThreadFreeWord
    Centralize mi_tf_block, mi_tf_is_owned, and mi_tf_create behind ThreadFreeWord.
6. PR DT-6: pointer wrappers in non-hot helpers
    Migrate validation, page lookup, and debug-only helpers first.
7. PR DT-7: MemId typed view
    Add MemKindCode, MemKind, MemId<K>, MemIdView, OsMemId, ArenaMemId, and MetaMemId. Remove direct union reads outside memid.rs.
8. PR DT-8: free-list wrappers
    Add AvailableFreeList, LocalFreeList, RemoteFreeList, EncodedPtr, PageKeys, and BlockLink.
9. PR DT-9: page ownership wrappers
    Add LocalPage, ForeignPage, AbandonedPage, OwnedAbandonedPage, and migrate mi_free_block_local, mi_free_block_mt, and abandoned reclaim signatures.
10. PR DT-10: page queue and lifecycle states
    Add ActivePage, AvailablePage, FullPage, RetiredPage, AbandonedMappedPage, NormalPageQueue, FullPageQueue, and HugePageQueue.

This gives the modernization a strict data-type foundation before broader unsafe/lint/style cleanup.
