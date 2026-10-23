//! Scalar domain wrappers for allocator sizes, counts, indexes, and bins.
//!
//! These crate-private newtypes are zero-cost layout boundaries: checked
//! constructors validate raw inputs, while unchecked constructors are reserved
//! for call sites that have already proved the scalar domain.

use core::{
    ffi::{c_int, c_long},
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
};

/// Domain-specific error for allocation size overflows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AllocSizeError {
    Overflow,
}

/// POSIX-style errno value used by internal error reporting.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct Errno(c_int);

impl Errno {
    pub(crate) const EAGAIN: Self = Self(crate::types::EAGAIN);
    pub(crate) const ENOMEM: Self = Self(crate::types::ENOMEM);
    pub(crate) const EFAULT: Self = Self(crate::types::EFAULT);
    pub(crate) const EINVAL: Self = Self(crate::types::EINVAL);
    pub(crate) const EOVERFLOW: Self = Self(crate::types::EOVERFLOW);

    #[inline(always)]
    pub(crate) const fn new(err: c_int) -> Self {
        Self(err)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> c_int {
        self.0
    }
}

/// User-requested allocation size (may be zero).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct RequestedSize(usize);

impl RequestedSize {
    pub(crate) const MIN_ZERO_SIZE: usize = core::mem::size_of::<usize>();

    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
    #[inline(always)]
    pub(crate) const fn effective_size(self) -> usize {
        if self.0 == 0 {
            Self::MIN_ZERO_SIZE
        } else {
            self.0
        }
    }
    #[inline]
    pub(crate) fn checked_padded(self) -> Result<PaddedSize, AllocSizeError> {
        self.0
            .checked_add(crate::types::MI_PADDING_SIZE)
            .and_then(NonZeroUsize::new)
            .map(PaddedSize)
            .ok_or(AllocSizeError::Overflow)
    }
}

/// A request bounded by the small-allocation limit; zero retains its tracking meaning.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SmallRequestedSize(RequestedSize);

impl SmallRequestedSize {
    pub(crate) const ZERO: Self = Self(RequestedSize::new(0));

    #[inline(always)]
    pub(crate) const fn new(size: RequestedSize) -> Option<Self> {
        if size.get() <= crate::types::MI_SMALL_SIZE_MAX {
            Some(Self(size))
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) const fn requested(self) -> RequestedSize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn normalized(self) -> NormalizedSmallSize {
        // SAFETY: zero becomes one word; every other request is positive and
        // the constructor bounds both cases by MI_SMALL_SIZE_MAX.
        NormalizedSmallSize(unsafe { NonZeroUsize::new_unchecked(self.0.effective_size()) })
    }
}

/// A positive small request, bounded so adding allocator padding cannot overflow.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct NormalizedSmallSize(NonZeroUsize);

const _: () = {
    assert!(RequestedSize::MIN_ZERO_SIZE <= crate::types::MI_SMALL_SIZE_MAX);
    assert!(crate::types::MI_SMALL_SIZE_MAX <= usize::MAX - crate::types::MI_PADDING_SIZE);
};

impl NormalizedSmallSize {
    #[inline(always)]
    pub(crate) const fn effective(self) -> EffectiveSize {
        EffectiveSize(self.0)
    }

    #[inline(always)]
    pub(crate) const fn padded(self) -> PaddedSize {
        // SAFETY: normalization supplies positivity; the small bound and the
        // compile-time padding bound above exclude overflow.
        PaddedSize(unsafe {
            NonZeroUsize::new_unchecked(self.0.get() + crate::types::MI_PADDING_SIZE)
        })
    }
}

/// Effective size after mimalloc zero-size normalization (never zero in Rust paths).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct EffectiveSize(NonZeroUsize);

impl EffectiveSize {
    /// # Safety
    /// `size` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(size: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(size) })
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
}

/// Padded request (requested + MI_PADDING_SIZE). Never zero.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PaddedSize(NonZeroUsize);

impl PaddedSize {
    /// # Safety
    /// `size` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(size: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(size) })
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
}

/// Usable size visible to the user (excludes padding).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct UsableSize(usize);

impl UsableSize {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
    #[inline]
    pub(crate) fn checked_sub(self, sub: usize) -> Result<Self, AllocSizeError> {
        self.0
            .checked_sub(sub)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
}

/// Nullable `usize*` out-parameter used by `mi_u*` allocation/free APIs.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct UsableOut(*mut usize);

impl UsableOut {
    #[inline(always)]
    pub(crate) const fn new(out: *mut usize) -> Self {
        Self(out)
    }

    /// # Safety
    /// If non-null, the wrapped pointer must be valid for a `usize` write.
    #[inline(always)]
    pub(crate) unsafe fn write_if_present(self, usable: UsableSize) {
        if !self.0.is_null() {
            unsafe { *self.0 = usable.get() };
        }
    }
}

/// Raw page block size (includes padding). Never zero.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct BlockSize(NonZeroUsize);

impl BlockSize {
    #[inline(always)]
    pub(crate) const fn from_padded(size: PaddedSize) -> Self {
        Self(size.0)
    }

    /// # Safety
    /// `size` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(size: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(size) })
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
    /// Subtract padding to get the user-visible usable size.
    ///
    /// Uses `wrapping_sub` (branchless in release) rather than `checked_sub` to
    /// honor the spec's zero-added-branches mandate for the free path (spec §1
    /// invariant 17). Correctness relies on the invariant below; a release build
    /// cannot detect a violation, so this is only sound when the caller upholds it.
    ///
    /// # Safety (invariant)
    ///
    /// For any initialized page, `mi_page_block_size` returns a value that already
    /// includes `MI_PADDING_SIZE` (padding is reserved when the page is sized), so
    /// `block_size >= MI_PADDING_SIZE` always holds. The `debug_assert!` catches
    /// corruption in debug builds. If this invariant cannot be guaranteed at a
    /// call site, the caller must validate before constructing the `BlockSize`.
    #[inline(always)]
    pub(crate) fn usable(self) -> UsableSize {
        crate::internal::mi_debug_assert!(
            self.get() >= crate::types::MI_PADDING_SIZE,
            "Block size must be at least as large as padding size"
        );
        UsableSize(self.get().wrapping_sub(crate::types::MI_PADDING_SIZE))
    }
}

/// Alignment requirement (power of two, non-zero).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct Alignment(NonZeroUsize);

impl Alignment {
    /// Safely creates an `Alignment`, verifying it is a non-zero power of two.
    #[inline(always)]
    pub(crate) fn new(align: usize) -> Option<Self> {
        if align.is_power_of_two() {
            Some(unsafe { Self::new_unchecked(align) })
        } else {
            None
        }
    }
    /// # Safety
    /// `align` must be a non-zero power of two.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(align: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(align) })
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
}

/// Optional alignment (0 means “default / no explicit alignment”).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct MaybeAlignment(usize);

impl MaybeAlignment {
    #[inline(always)]
    pub(crate) const fn new(align: usize) -> Option<Self> {
        if align == 0 || align.is_power_of_two() {
            Some(Self(align))
        } else {
            None
        }
    }
    /// # Safety
    /// `align` must be zero or a power of two.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(align: usize) -> Self {
        Self(align)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Offset used by aligned-at allocation APIs.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct AlignmentOffset(usize);

impl AlignmentOffset {
    #[inline(always)]
    pub(crate) const fn new(offset: usize) -> Self {
        Self(offset)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Number of pages linked in a page queue.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageQueueCount(usize);

impl PageQueueCount {
    #[inline(always)]
    pub(crate) const fn new(count: usize) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn increment(self) -> Self {
        Self(self.0 + 1)
    }

    #[inline(always)]
    pub(crate) fn decrement(self) -> Self {
        crate::internal::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }
}

/// Value of the `page_reclaim_on_free` option after raw option lookup.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageReclaimOnFree(c_long);

impl PageReclaimOnFree {
    #[inline(always)]
    pub(crate) const fn new(value: c_long) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub(crate) const fn is_enabled(self) -> bool {
        self.0 >= 0
    }

    #[inline(always)]
    pub(crate) const fn is_cross_thread_allowed(self) -> bool {
        self.0 == 1
    }
}

/// Maximum reclaim scan limit for page queues; negative means disabled.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageReclaimLimit(c_long);

impl PageReclaimLimit {
    #[inline(always)]
    pub(crate) const fn new(value: c_long) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub(crate) const fn decrement(self) -> Self {
        Self(self.0 - 1)
    }

    #[inline(always)]
    pub(crate) const fn is_negative(self) -> bool {
        self.0 < 0
    }

    #[inline(always)]
    pub(crate) const fn is_exhausted(self) -> bool {
        self.0 <= 0
    }

    #[inline(always)]
    pub(crate) const fn as_queue_count(self) -> Option<PageQueueCount> {
        if self.0 < 0 {
            None
        } else {
            Some(PageQueueCount::new(self.0 as usize))
        }
    }
}

/// Number of used blocks in a page.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct UsedBlockCount(u16);

impl UsedBlockCount {
    #[inline(always)]
    pub(crate) const fn new(count: u16) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub(crate) const fn increment_wrapping(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    #[inline(always)]
    pub(crate) fn decrement(self) -> Self {
        crate::internal::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }

    #[inline(always)]
    pub(crate) const fn checked_sub_batch(self, len: BatchLen) -> Option<Self> {
        match self.0.checked_sub(len.get()) {
            Some(used) => Some(Self(used)),
            None => None,
        }
    }
}

/// Non-empty number of blocks owned by a page-homogeneous batch.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct BatchLen(NonZeroU16);

impl BatchLen {
    /// # Safety
    /// `value` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(value: u16) -> Self {
        Self(unsafe { NonZeroU16::new_unchecked(value) })
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0.get()
    }
}

/// Number of reserved blocks in a page.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ReservedBlockCount(u16);

impl ReservedBlockCount {
    #[inline(always)]
    pub(crate) const fn new(count: u16) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Number of committed/available blocks in a page.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageCapacity(u16);

impl PageCapacity {
    #[inline(always)]
    pub(crate) const fn new(
        used: UsedBlockCount,
        capacity: u16,
        reserved: ReservedBlockCount,
    ) -> Option<Self> {
        if used.get() <= capacity && capacity <= reserved.get() {
            Some(Self(capacity))
        } else {
            None
        }
    }

    /// # Safety
    /// Caller must prove `used <= capacity <= reserved`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(capacity: u16) -> Self {
        Self(capacity)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub(crate) fn add(
        self,
        used: UsedBlockCount,
        extend: u16,
        reserved: ReservedBlockCount,
    ) -> Option<Self> {
        let capacity = self.0.checked_add(extend)?;
        Self::new(used, capacity, reserved)
    }
}

/// Page retirement countdown. Zero means the page is not pending retirement.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct RetireExpire(u8);

impl RetireExpire {
    #[inline(always)]
    pub(crate) const fn new(countdown: u8) -> Self {
        Self(countdown)
    }

    #[inline(always)]
    pub(crate) const fn zero() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u8 {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn is_zero(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub(crate) fn decrement(self) -> Self {
        crate::internal::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }
}

/// NUMA node affinity. Negative values mean any NUMA node.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct NumaNode(i32);

impl NumaNode {
    #[inline(always)]
    pub(crate) const fn new(node: i32) -> Self {
        Self(node)
    }

    #[inline(always)]
    pub(crate) const fn any() -> Self {
        Self(-1)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> i32 {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn is_any(self) -> bool {
        self.0 < 0
    }

    #[inline(always)]
    pub(crate) const fn is_suitable_for(self, requested: Self) -> bool {
        requested.is_any() || self.is_any() || self.0 == requested.0
    }

    #[inline(always)]
    pub(crate) fn normalize(self, count: i32) -> Self {
        if self.is_any() || count <= 0 {
            Self::any()
        } else {
            Self(self.0 % count)
        }
    }
}

/// Per-thread sequence number used for arena cycling and diagnostics.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ThreadSeq(usize);

impl ThreadSeq {
    #[inline(always)]
    pub(crate) const fn new(seq: usize) -> Self {
        Self(seq)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Per-heap sequence number used for arena cycling and diagnostics.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct HeapSeq(usize);

impl HeapSeq {
    #[inline(always)]
    pub(crate) const fn new(seq: usize) -> Self {
        Self(seq)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Per-subprocess sequence number used for diagnostics.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct SubprocSeq(usize);

impl SubprocSeq {
    #[inline(always)]
    pub(crate) const fn new(seq: usize) -> Self {
        Self(seq)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Count of arenas registered with a subprocess.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ArenaCount(usize);

impl ArenaCount {
    #[inline(always)]
    pub(crate) const fn new(count: usize) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Thread-heap heartbeat used by deferred free callbacks.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct Heartbeat(u64);

impl Heartbeat {
    #[inline(always)]
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn increment_wrapping(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// OS page size. Always nonzero after OS memory configuration initialization.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct OsPageSize(NonZeroUsize);

impl OsPageSize {
    /// # Safety
    /// `size` must be nonzero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(size: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(size) })
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
}

/// Top-level page-map index. Must be within `MI_PAGE_MAP_COUNT`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageMapIndex(usize);

impl PageMapIndex {
    #[inline(always)]
    pub(crate) const fn new(index: usize) -> Option<Self> {
        if index < crate::internal::MI_PAGE_MAP_COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    /// # Safety
    /// `index` must be less than `MI_PAGE_MAP_COUNT`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(index: usize) -> Self {
        Self(index)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Flat page-map index used by 32-bit targets. Unlike `PageMapIndex`, this
/// spans every arena-slice slot in the flat byte map.
#[cfg(target_pointer_width = "32")]
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageMapFlatIndex(usize);

#[cfg(target_pointer_width = "32")]
impl PageMapFlatIndex {
    pub(crate) const COUNT: usize =
        crate::internal::MI_PAGE_MAP_COUNT * crate::internal::MI_PAGE_MAP_SUB_COUNT;

    #[inline(always)]
    pub(crate) const fn new(index: usize) -> Option<Self> {
        if index < Self::COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

#[cfg(target_pointer_width = "32")]
pub(crate) type PageMapPrimaryIndex = PageMapFlatIndex;

#[cfg(not(target_pointer_width = "32"))]
pub(crate) type PageMapPrimaryIndex = PageMapIndex;

/// Submap index. Must be within `MI_PAGE_MAP_SUB_COUNT`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageMapSubIndex(usize);

impl PageMapSubIndex {
    #[inline(always)]
    pub(crate) const fn new(index: usize) -> Option<Self> {
        if index < crate::internal::MI_PAGE_MAP_SUB_COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    /// # Safety
    /// `index` must be less than `MI_PAGE_MAP_SUB_COUNT`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(index: usize) -> Self {
        Self(index)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Bin index (0 … MI_BIN_COUNT-1).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct Bin(u16);

impl Bin {
    #[inline(always)]
    pub(crate) const fn new(index: u16) -> Option<Self> {
        if (index as usize) < crate::types::MI_BIN_COUNT {
            Some(Self(index))
        } else {
            None
        }
    }
    /// # Safety
    /// `index` must be less than `MI_BIN_COUNT`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(index: u16) -> Self {
        Self(index)
    }
    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Stats bin index (0 … MI_BIN_HUGE).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct StatsBin(u16);

impl StatsBin {
    #[inline(always)]
    pub(crate) const fn new(index: u16) -> Option<Self> {
        if (index as usize) <= crate::allocator::stats::MI_BIN_HUGE {
            Some(Self(index))
        } else {
            None
        }
    }
    /// # Safety
    /// `index` must be less than or equal to `MI_BIN_HUGE`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(index: u16) -> Self {
        Self(index)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0
    }
    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Direct bin index (0 … MI_PAGES_DIRECT-1).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct DirectBin(u16);

impl DirectBin {
    /// # Safety
    /// `index` must be less than `MI_PAGES_DIRECT`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(index: u16) -> Self {
        Self(index)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0
    }
    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Slice index inside an arena.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct SliceIndex(usize);

impl SliceIndex {
    #[inline(always)]
    pub(crate) const fn new(idx: usize) -> Self {
        Self(idx)
    }
    #[inline(always)]
    pub(crate) fn checked_add(self, offset: usize) -> Option<Self> {
        self.0.checked_add(offset).map(Self)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Slice count (non-zero for real arenas).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct SliceCount(NonZeroU32);

impl SliceCount {
    /// Construct from a `usize` count. Returns `None` if zero or too large for `u32`.
    #[inline(always)]
    pub(crate) fn from_usize(count: usize) -> Option<Self> {
        NonZeroU32::new(u32::try_from(count).ok()?).map(Self)
    }
    #[inline(always)]
    pub(crate) const fn from_usize_const(count: usize) -> Option<Self> {
        if count == 0 || count > u32::MAX as usize {
            None
        } else {
            Some(unsafe { Self::new_unchecked(count as u32) })
        }
    }
    /// # Safety
    /// `count` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(count: u32) -> Self {
        Self(unsafe { NonZeroU32::new_unchecked(count) })
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> u32 {
        self.0.get()
    }
}

/// Arena index (0 … MI_MAX_ARENAS-1).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ArenaIndex(u16);

impl ArenaIndex {
    #[inline(always)]
    pub(crate) const fn new(idx: u16) -> Option<Self> {
        if (idx as usize) < crate::types::MI_MAX_ARENAS {
            Some(Self(idx))
        } else {
            None
        }
    }
    #[inline(always)]
    pub(crate) fn from_usize(idx: usize) -> Option<Self> {
        Self::new(u16::try_from(idx).ok()?)
    }
    /// # Safety
    /// `idx` must be less than `MI_MAX_ARENAS`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(idx: u16) -> Self {
        Self(idx)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0
    }
}

/// Good size returned by mi_good_size.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct GoodSize(usize);

impl GoodSize {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests;
