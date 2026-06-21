//! DT-1 scalar domain types (zero-cost newtypes over usize / NonZeroUsize).
//! All types are #[repr(transparent)], Copy, Clone, PartialEq, etc.

use core::{
    ffi::{c_int, c_long},
    num::{NonZeroU32, NonZeroUsize},
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
pub(crate) struct RequestedSize(pub(crate) usize);

impl RequestedSize {
    #[inline(always)]
    pub(crate) const fn new(size: usize) -> Self {
        Self(size)
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
    #[inline]
    pub(crate) fn checked_add(self, add: usize) -> Result<Self, AllocSizeError> {
        self.0
            .checked_add(add)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
    #[inline]
    pub(crate) fn checked_mul(self, mul: usize) -> Result<Self, AllocSizeError> {
        self.0
            .checked_mul(mul)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
    #[inline]
    pub(crate) fn checked_padded(self) -> Result<PaddedSize, AllocSizeError> {
        self.0
            .checked_add(crate::types::MI_PADDING_SIZE)
            .and_then(NonZeroUsize::new)
            .map(PaddedSize)
            .ok_or(AllocSizeError::Overflow)
    }

    /// Padded size without overflow checking (branchless in release).
    ///
    /// # Safety (invariant)
    ///
    /// The caller must guarantee `self.get() + MI_PADDING_SIZE` does not overflow
    /// and is non-zero. On the small allocation path this holds because
    /// `size <= MI_SMALL_SIZE_MAX` (≤ 1024) and `MI_PADDING_SIZE` is tiny; on
    /// other paths where overflow is possible, use `checked_padded` instead.
    /// A `debug_assert!` catches violations in debug builds.
    #[inline(always)]
    pub(crate) fn padded_unchecked(self) -> PaddedSize {
        crate::mi_debug_assert!(
            self.0.checked_add(crate::types::MI_PADDING_SIZE).is_some(),
            "requested size + padding must not overflow"
        );
        // SAFETY: caller guarantees the sum is non-zero and does not overflow.
        unsafe { PaddedSize::new_unchecked(self.0.wrapping_add(crate::types::MI_PADDING_SIZE)) }
    }
}

/// Effective size after mimalloc zero-size normalization (never zero in Rust paths).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct EffectiveSize(pub(crate) NonZeroUsize);

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
    #[inline]
    pub(crate) fn checked_add(self, add: usize) -> Result<Self, AllocSizeError> {
        self.get()
            .checked_add(add)
            .and_then(NonZeroUsize::new)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
}

/// Padded request (requested + MI_PADDING_SIZE). Never zero.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PaddedSize(pub(crate) NonZeroUsize);

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
    #[inline]
    pub(crate) fn checked_add(self, add: usize) -> Result<Self, AllocSizeError> {
        self.get()
            .checked_add(add)
            .and_then(NonZeroUsize::new)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
    #[inline]
    pub(crate) fn checked_sub_padding(self) -> Result<RequestedSize, AllocSizeError> {
        self.get()
            .checked_sub(crate::types::MI_PADDING_SIZE)
            .map(RequestedSize)
            .ok_or(AllocSizeError::Overflow)
    }
}

/// Usable size visible to the user (excludes padding).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct UsableSize(pub(crate) usize);

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
    pub(crate) fn checked_add(self, add: usize) -> Result<Self, AllocSizeError> {
        self.0
            .checked_add(add)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
    #[inline]
    pub(crate) fn checked_sub(self, sub: usize) -> Result<Self, AllocSizeError> {
        self.0
            .checked_sub(sub)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
}

/// Raw page block size (includes padding). Never zero.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct BlockSize(pub(crate) NonZeroUsize);

impl BlockSize {
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
    #[inline]
    pub(crate) fn checked_add(self, add: usize) -> Result<Self, AllocSizeError> {
        self.get()
            .checked_add(add)
            .and_then(NonZeroUsize::new)
            .map(Self)
            .ok_or(AllocSizeError::Overflow)
    }
    #[inline]
    pub(crate) fn checked_usable(self) -> Result<UsableSize, AllocSizeError> {
        self.get()
            .checked_sub(crate::types::MI_PADDING_SIZE)
            .map(UsableSize)
            .ok_or(AllocSizeError::Overflow)
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
        crate::mi_debug_assert!(
            self.get() >= crate::types::MI_PADDING_SIZE,
            "Block size must be at least as large as padding size"
        );
        UsableSize(self.get().wrapping_sub(crate::types::MI_PADDING_SIZE))
    }
}

/// Full block size including allocator metadata and padding. Never zero.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct FullBlockSize(pub(crate) NonZeroUsize);

impl FullBlockSize {
    /// # Safety
    /// `size` must be non-zero and include allocator metadata/padding.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(size: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(size) })
    }

    #[inline(always)]
    pub(crate) const fn from_block_size(block_size: BlockSize) -> Self {
        Self(block_size.0)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
}

/// Alignment requirement (power of two, non-zero).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct Alignment(pub(crate) NonZeroUsize);

impl Alignment {
    /// Safely creates an `Alignment`, verifying it is a non-zero power of two.
    #[inline(always)]
    pub(crate) fn new(align: usize) -> Option<Self> {
        if align.is_power_of_two() {
            // is_power_of_two() guarantees it's non-zero
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
pub(crate) struct MaybeAlignment(pub(crate) usize);

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
    #[inline(always)]
    pub(crate) const fn is_some(self) -> bool {
        self.0 != 0
    }
}

/// Offset used by aligned-at allocation APIs.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct AlignmentOffset(pub(crate) usize);

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

/// Count of machine words needed to cover a byte size.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct WordCount(pub(crate) usize);

impl WordCount {
    #[inline(always)]
    pub(crate) const fn new(count: usize) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn from_size(size: usize) -> Self {
        Self(size.div_ceil(core::mem::size_of::<usize>()))
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Number of pages linked in a page queue.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageQueueCount(pub(crate) usize);

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
        crate::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }

    #[inline(always)]
    pub(crate) const fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
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
    pub(crate) const fn get(self) -> c_long {
        self.0
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
pub(crate) struct UsedBlockCount(pub(crate) u16);

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
        crate::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }

    #[inline(always)]
    pub(crate) fn subtract(self, count: u16) -> Self {
        crate::mi_debug_assert!(self.0 >= count);
        Self(self.0 - count)
    }
}

/// Number of reserved blocks in a page.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ReservedBlockCount(pub(crate) u16);

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
pub(crate) struct PageCapacity(pub(crate) u16);

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
pub(crate) struct RetireExpire(pub(crate) u8);

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
        crate::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }
}

/// NUMA node affinity. Negative values mean any NUMA node.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct NumaNode(pub(crate) i32);

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
pub(crate) struct ThreadSeq(pub(crate) usize);

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
pub(crate) struct HeapSeq(pub(crate) usize);

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
pub(crate) struct SubprocSeq(pub(crate) usize);

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
pub(crate) struct ArenaCount(pub(crate) usize);

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

/// Count of threads associated with a subprocess.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct ThreadCount(pub(crate) usize);

impl ThreadCount {
    #[inline(always)]
    pub(crate) const fn new(count: usize) -> Self {
        Self(count)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Count of heaps associated with a subprocess.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct HeapCount(pub(crate) usize);

impl HeapCount {
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
pub(crate) struct Heartbeat(pub(crate) u64);

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
pub(crate) struct OsPageSize(pub(crate) NonZeroUsize);

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
pub(crate) struct PageMapIndex(pub(crate) usize);

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

/// Submap index. Must be within `MI_PAGE_MAP_SUB_COUNT`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct PageMapSubIndex(pub(crate) usize);

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
pub(crate) struct Bin(pub(crate) u16);

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
    pub(crate) const fn get(self) -> u16 {
        self.0
    }
    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Stats bin index (0 … MI_BIN_HUGE).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct StatsBin(pub(crate) u16);

impl StatsBin {
    #[inline(always)]
    pub(crate) const fn new(index: u16) -> Option<Self> {
        if (index as usize) <= crate::stats::MI_BIN_HUGE {
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
pub(crate) struct DirectBin(pub(crate) u16);

impl DirectBin {
    #[inline(always)]
    pub(crate) const fn new(index: u16) -> Option<Self> {
        if (index as usize) < crate::types::MI_PAGES_DIRECT {
            Some(Self(index))
        } else {
            None
        }
    }
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
pub(crate) struct SliceIndex(pub(crate) usize);

impl SliceIndex {
    #[inline(always)]
    pub(crate) const fn new(idx: usize) -> Self {
        Self(idx)
    }
    #[inline(always)]
    pub(crate) fn as_u32(self) -> Option<u32> {
        u32::try_from(self.0).ok()
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
pub(crate) struct SliceCount(pub(crate) NonZeroU32);

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
pub(crate) struct ArenaIndex(pub(crate) u16);

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
pub(crate) struct GoodSize(pub(crate) usize);

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

pub(super) fn __lint_touch() {
    let requested = RequestedSize::new(1);
    let _ = requested.checked_add(1);
    let _ = requested.checked_mul(1);
    let effective = unsafe { EffectiveSize::new_unchecked(1) };
    let _ = effective.checked_add(1);
    let padded = unsafe { PaddedSize::new_unchecked(1) };
    let _ = padded.checked_add(1);
    let _ = padded.checked_sub_padding();
    let usable = UsableSize::new(1);
    let _ = usable.checked_add(1);
    let block = unsafe { BlockSize::new_unchecked(1) };
    let _ = block.checked_add(1);
    let _ = block.checked_usable();
    let _ = unsafe { FullBlockSize::new_unchecked(1) };
    let alignment = unsafe { MaybeAlignment::new_unchecked(1) };
    let _ = alignment.is_some();
    let _ = WordCount::new(1);
    let _ = PageReclaimLimit::new(1).get();
    let thread_count = ThreadCount::new(1);
    let _ = thread_count.get();
    let heap_count = HeapCount::new(1);
    let _ = heap_count.get();
    let bin = unsafe { Bin::new_unchecked(0) };
    let _ = bin.get();
    let _ = DirectBin::new(0);
    let _ = SliceIndex::new(1).as_u32();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_size_padding_checks_overflow() {
        assert!(RequestedSize::new(16).checked_padded().is_ok());
        assert_eq!(
            RequestedSize::new(usize::MAX).checked_padded(),
            Err(AllocSizeError::Overflow)
        );
    }

    #[test]
    fn size_wrappers_return_typed_errors_for_checked_arithmetic() {
        assert_eq!(
            RequestedSize::new(16)
                .checked_add(4)
                .map(RequestedSize::get),
            Ok(20)
        );
        assert_eq!(
            RequestedSize::new(8).checked_mul(4).map(RequestedSize::get),
            Ok(32)
        );
        assert_eq!(
            RequestedSize::new(usize::MAX).checked_add(1),
            Err(AllocSizeError::Overflow)
        );
        assert_eq!(
            RequestedSize::new(usize::MAX).checked_mul(2),
            Err(AllocSizeError::Overflow)
        );

        let effective = unsafe { EffectiveSize::new_unchecked(16) };
        assert_eq!(effective.checked_add(4).map(EffectiveSize::get), Ok(20));
        assert_eq!(
            unsafe { EffectiveSize::new_unchecked(usize::MAX) }.checked_add(1),
            Err(AllocSizeError::Overflow)
        );

        let padded = unsafe { PaddedSize::new_unchecked(crate::types::MI_PADDING_SIZE + 16) };
        assert_eq!(
            padded.checked_add(4).map(PaddedSize::get),
            Ok(padded.get() + 4)
        );
        assert_eq!(padded.checked_sub_padding().map(RequestedSize::get), Ok(16));
        assert_eq!(
            unsafe { PaddedSize::new_unchecked(usize::MAX) }.checked_add(1),
            Err(AllocSizeError::Overflow)
        );
        assert_eq!(
            unsafe { PaddedSize::new_unchecked(1) }.checked_sub_padding(),
            Err(AllocSizeError::Overflow)
        );

        let block = unsafe { BlockSize::new_unchecked(crate::types::MI_PADDING_SIZE + 32) };
        assert_eq!(
            block.checked_add(4).map(BlockSize::get),
            Ok(block.get() + 4)
        );
        assert_eq!(block.checked_usable().map(UsableSize::get), Ok(32));
        assert_eq!(
            unsafe { BlockSize::new_unchecked(usize::MAX) }.checked_add(1),
            Err(AllocSizeError::Overflow)
        );
        assert_eq!(
            unsafe { BlockSize::new_unchecked(1) }.checked_usable(),
            Err(AllocSizeError::Overflow)
        );

        assert_eq!(
            UsableSize::new(16).checked_add(4).map(UsableSize::get),
            Ok(20)
        );
        assert_eq!(
            UsableSize::new(16).checked_sub(4).map(UsableSize::get),
            Ok(12)
        );
        assert_eq!(
            UsableSize::new(usize::MAX).checked_add(1),
            Err(AllocSizeError::Overflow)
        );
        assert_eq!(
            UsableSize::new(0).checked_sub(1),
            Err(AllocSizeError::Overflow)
        );
    }

    #[test]
    fn scalar_constructors_preserve_zero_and_upper_bound_values() {
        assert_eq!(RequestedSize::new(0).get(), 0);
        assert_eq!(RequestedSize::new(usize::MAX).get(), usize::MAX);
        assert_eq!(UsableSize::new(0).get(), 0);
        assert_eq!(UsableSize::new(usize::MAX).get(), usize::MAX);
        assert_eq!(AlignmentOffset::new(usize::MAX).get(), usize::MAX);
        assert_eq!(WordCount::new(usize::MAX).get(), usize::MAX);
        assert_eq!(PageQueueCount::new(usize::MAX).get(), usize::MAX);
        assert_eq!(UsedBlockCount::new(u16::MAX).get(), u16::MAX);
        assert_eq!(ReservedBlockCount::new(u16::MAX).get(), u16::MAX);
        assert_eq!(RetireExpire::new(u8::MAX).get(), u8::MAX);
        assert_eq!(NumaNode::new(i32::MAX).get(), i32::MAX);
        assert_eq!(NumaNode::new(i32::MIN).get(), i32::MIN);
        assert_eq!(ThreadSeq::new(usize::MAX).get(), usize::MAX);
        assert_eq!(HeapSeq::new(usize::MAX).get(), usize::MAX);
        assert_eq!(SubprocSeq::new(usize::MAX).get(), usize::MAX);
        assert_eq!(Heartbeat::new(u64::MAX).get(), u64::MAX);
        assert_eq!(SliceIndex::new(usize::MAX).get(), usize::MAX);
        assert_eq!(GoodSize::new(usize::MAX).get(), usize::MAX);
    }

    #[test]
    fn slice_index_checked_add_reports_overflow() {
        assert_eq!(
            SliceIndex::new(3).checked_add(4).map(SliceIndex::get),
            Some(7)
        );
        assert_eq!(SliceIndex::new(usize::MAX).checked_add(1), None);
    }

    #[test]
    fn slice_index_reports_raw_u32_conversion_overflow() {
        assert_eq!(SliceIndex::new(7).as_u32(), Some(7));
        assert_eq!(SliceIndex::new(u32::MAX as usize).as_u32(), Some(u32::MAX));
        assert!(SliceIndex::new(u32::MAX as usize + 1).as_u32().is_none());
    }

    #[test]
    fn unchecked_hot_path_constructors_preserve_valid_values() {
        assert_eq!(unsafe { EffectiveSize::new_unchecked(1) }.get(), 1);
        assert_eq!(unsafe { PaddedSize::new_unchecked(1) }.get(), 1);
        assert_eq!(unsafe { BlockSize::new_unchecked(1) }.get(), 1);
        assert_eq!(unsafe { FullBlockSize::new_unchecked(1) }.get(), 1);
        assert_eq!(unsafe { Alignment::new_unchecked(64) }.get(), 64);
        assert_eq!(unsafe { MaybeAlignment::new_unchecked(64) }.get(), 64);
        assert_eq!(unsafe { OsPageSize::new_unchecked(4096) }.get(), 4096);
        assert_eq!(unsafe { PageMapIndex::new_unchecked(0) }.get(), 0);
        assert_eq!(unsafe { PageMapSubIndex::new_unchecked(0) }.get(), 0);
        assert_eq!(unsafe { Bin::new_unchecked(0) }.get(), 0);
        assert_eq!(unsafe { StatsBin::new_unchecked(0) }.get(), 0);
        assert_eq!(unsafe { DirectBin::new_unchecked(0) }.get(), 0);
        assert_eq!(unsafe { SliceCount::new_unchecked(1) }.get(), 1);
        assert_eq!(unsafe { ArenaIndex::new_unchecked(0) }.get(), 0);
        assert_eq!(unsafe { PageCapacity::new_unchecked(0) }.get(), 0);
    }

    #[test]
    fn scalar_constructor_boundaries_cover_overflow_and_limits() {
        let max_power_of_two = 1usize << (usize::BITS - 1);
        assert_eq!(
            Alignment::new(max_power_of_two).map(Alignment::get),
            Some(max_power_of_two)
        );
        assert_eq!(
            MaybeAlignment::new(max_power_of_two).map(MaybeAlignment::get),
            Some(max_power_of_two)
        );

        let max_capacity = PageCapacity::new(
            UsedBlockCount::new(u16::MAX),
            u16::MAX,
            ReservedBlockCount::new(u16::MAX),
        )
        .expect("u16 max is valid when used and reserved match");
        assert_eq!(max_capacity.get(), u16::MAX);
        assert!(
            max_capacity
                .add(
                    UsedBlockCount::new(u16::MAX),
                    1,
                    ReservedBlockCount::new(u16::MAX)
                )
                .is_none()
        );

        assert_eq!(
            SliceCount::from_usize(u32::MAX as usize).map(SliceCount::get),
            Some(u32::MAX)
        );
        assert!(SliceCount::from_usize(u32::MAX as usize + 1).is_none());
    }

    #[test]
    fn block_size_usable_subtracts_padding_domain() {
        let raw = crate::types::MI_PADDING_SIZE + 64;
        let block = unsafe { BlockSize::new_unchecked(raw) };
        assert_eq!(block.usable().get(), 64);
    }

    #[test]
    fn full_block_size_preserves_block_size_domain() {
        let raw = crate::types::MI_PADDING_SIZE + 128;
        let block = unsafe { BlockSize::new_unchecked(raw) };
        assert_eq!(FullBlockSize::from_block_size(block).get(), raw);
    }

    #[test]
    fn alignment_offset_preserves_raw_offset_domain() {
        assert_eq!(AlignmentOffset::new(0).get(), 0);
        assert_eq!(AlignmentOffset::new(17).get(), 17);
    }

    #[test]
    fn word_count_rounds_byte_size_up_to_words() {
        let word = core::mem::size_of::<usize>();
        assert_eq!(WordCount::from_size(0).get(), 0);
        assert_eq!(WordCount::from_size(1).get(), 1);
        assert_eq!(WordCount::from_size(word).get(), 1);
        assert_eq!(WordCount::from_size(word + 1).get(), 2);
    }

    #[test]
    fn page_queue_count_arithmetic_preserves_domain() {
        let count = PageQueueCount::new(1).increment();
        assert_eq!(count.get(), 2);
        assert_eq!(count.decrement().get(), 1);
        assert_eq!(count.add(PageQueueCount::new(3)).get(), 5);
    }

    #[test]
    fn page_count_domains_preserve_raw_counts() {
        let used = UsedBlockCount::new(3);
        let reserved = ReservedBlockCount::new(9);

        assert_eq!(used.get(), 3);
        assert_eq!(used.as_usize(), 3);
        assert_eq!(used.increment_wrapping().get(), 4);
        assert_eq!(used.decrement().get(), 2);
        assert_eq!(used.subtract(2).get(), 1);

        assert_eq!(reserved.get(), 9);
        assert_eq!(reserved.as_usize(), 9);
    }

    #[test]
    fn page_capacity_checks_used_capacity_reserved_ordering() {
        let used = UsedBlockCount::new(3);
        let reserved = ReservedBlockCount::new(9);

        let capacity = PageCapacity::new(used, 6, reserved).expect("valid page capacity");
        assert_eq!(capacity.get(), 6);
        assert_eq!(capacity.as_usize(), 6);
        assert_eq!(
            capacity.add(used, 2, reserved).map(PageCapacity::get),
            Some(8)
        );

        assert!(PageCapacity::new(UsedBlockCount::new(7), 6, reserved).is_none());
        assert!(PageCapacity::new(used, 10, reserved).is_none());
        assert!(capacity.add(used, 4, reserved).is_none());
    }

    #[test]
    fn retire_expire_models_zero_and_countdown_values() {
        assert!(RetireExpire::zero().is_zero());
        assert_eq!(RetireExpire::zero().get(), 0);

        let expire = RetireExpire::new(3);
        assert!(!expire.is_zero());
        assert_eq!(expire.decrement().get(), 2);
    }

    #[test]
    fn numa_node_models_any_and_normalized_affinity() {
        let any = NumaNode::any();
        let node = NumaNode::new(5);

        assert_eq!(any.get(), -1);
        assert!(any.is_any());
        assert!(!node.is_any());
        assert!(NumaNode::new(2).is_suitable_for(any));
        assert!(any.is_suitable_for(node));
        assert!(NumaNode::new(5).is_suitable_for(node));
        assert!(!NumaNode::new(4).is_suitable_for(node));
        assert_eq!(node.normalize(4).get(), 1);
        assert_eq!(node.normalize(0).get(), -1);
    }

    #[test]
    fn sequence_domains_preserve_raw_values() {
        assert_eq!(ThreadSeq::new(11).get(), 11);
        assert_eq!(HeapSeq::new(13).get(), 13);
        assert_eq!(SubprocSeq::new(17).get(), 17);
        assert_eq!(ArenaCount::new(19).get(), 19);
        assert_eq!(ThreadCount::new(23).get(), 23);
        assert_eq!(HeapCount::new(29).get(), 29);
    }

    #[test]
    fn heartbeat_preserves_raw_value_and_wraps_on_increment() {
        assert_eq!(Heartbeat::new(41).increment_wrapping().get(), 42);
        assert_eq!(Heartbeat::new(u64::MAX).increment_wrapping().get(), 0);
    }

    #[test]
    fn os_page_size_preserves_nonzero_raw_value() {
        let page_size = unsafe { OsPageSize::new_unchecked(4096) };
        assert_eq!(page_size.get(), 4096);
    }

    #[test]
    fn errno_preserves_raw_error_codes() {
        assert_eq!(Errno::new(crate::types::EINVAL).get(), crate::types::EINVAL);
        assert_eq!(Errno::EAGAIN.get(), crate::types::EAGAIN);
        assert_eq!(Errno::ENOMEM.get(), crate::types::ENOMEM);
        assert_eq!(Errno::EFAULT.get(), crate::types::EFAULT);
        assert_eq!(Errno::EINVAL.get(), crate::types::EINVAL);
        assert_eq!(Errno::EOVERFLOW.get(), crate::types::EOVERFLOW);
    }

    #[test]
    fn bin_rejects_indexes_outside_page_queue_table() {
        assert_eq!(Bin::new(0).map(Bin::get), Some(0));
        assert_eq!(
            Bin::new((crate::types::MI_BIN_COUNT - 1) as u16).map(Bin::get),
            Some((crate::types::MI_BIN_COUNT - 1) as u16)
        );
        assert!(Bin::new(crate::types::MI_BIN_COUNT as u16).is_none());

        let unchecked = unsafe { Bin::new_unchecked(3) };
        assert_eq!(unchecked.get(), 3);
    }

    #[test]
    fn stats_bin_rejects_indexes_above_huge_bin() {
        assert_eq!(StatsBin::new(0).map(StatsBin::get), Some(0));
        assert_eq!(
            StatsBin::new(crate::stats::MI_BIN_HUGE as u16).map(StatsBin::get),
            Some(crate::stats::MI_BIN_HUGE as u16)
        );
        assert!(StatsBin::new((crate::stats::MI_BIN_HUGE + 1) as u16).is_none());

        let unchecked = unsafe { StatsBin::new_unchecked(3) };
        assert_eq!(unchecked.as_usize(), 3);
    }

    #[test]
    fn direct_bin_rejects_indexes_outside_direct_table() {
        assert_eq!(DirectBin::new(0).map(DirectBin::get), Some(0));
        assert_eq!(
            DirectBin::new((crate::types::MI_PAGES_DIRECT - 1) as u16).map(DirectBin::get),
            Some((crate::types::MI_PAGES_DIRECT - 1) as u16)
        );
        assert!(DirectBin::new(crate::types::MI_PAGES_DIRECT as u16).is_none());

        let unchecked = unsafe { DirectBin::new_unchecked(3) };
        assert_eq!(unchecked.as_usize(), 3);
    }

    #[test]
    fn arena_index_rejects_indexes_outside_arena_table() {
        assert_eq!(ArenaIndex::new(0).map(ArenaIndex::get), Some(0));
        assert_eq!(
            ArenaIndex::new((crate::types::MI_MAX_ARENAS - 1) as u16).map(ArenaIndex::get),
            Some((crate::types::MI_MAX_ARENAS - 1) as u16)
        );
        assert!(ArenaIndex::new(crate::types::MI_MAX_ARENAS as u16).is_none());
        assert_eq!(ArenaIndex::from_usize(1).map(ArenaIndex::get), Some(1));
        assert!(ArenaIndex::from_usize(crate::types::MI_MAX_ARENAS).is_none());
        assert!(ArenaIndex::from_usize(u16::MAX as usize + 1).is_none());

        let unchecked = unsafe { ArenaIndex::new_unchecked(3) };
        assert_eq!(unchecked.get(), 3);
    }

    #[test]
    fn maybe_alignment_accepts_zero_or_power_of_two() {
        assert_eq!(MaybeAlignment::new(0).map(MaybeAlignment::get), Some(0));
        assert_eq!(MaybeAlignment::new(64).map(MaybeAlignment::get), Some(64));
        assert!(MaybeAlignment::new(24).is_none());

        let unchecked = unsafe { MaybeAlignment::new_unchecked(24) };
        assert_eq!(unchecked.get(), 24);
    }

    #[test]
    fn page_map_indexes_enforce_domain_bounds() {
        assert_eq!(PageMapIndex::new(0).map(PageMapIndex::get), Some(0));
        assert_eq!(
            PageMapIndex::new(crate::internal::MI_PAGE_MAP_COUNT - 1).map(PageMapIndex::get),
            Some(crate::internal::MI_PAGE_MAP_COUNT - 1)
        );
        assert!(PageMapIndex::new(crate::internal::MI_PAGE_MAP_COUNT).is_none());
        assert!(PageMapIndex::new(usize::MAX).is_none());

        assert_eq!(PageMapSubIndex::new(0).map(PageMapSubIndex::get), Some(0));
        assert_eq!(
            PageMapSubIndex::new(crate::internal::MI_PAGE_MAP_SUB_COUNT - 1)
                .map(PageMapSubIndex::get),
            Some(crate::internal::MI_PAGE_MAP_SUB_COUNT - 1)
        );
        assert!(PageMapSubIndex::new(crate::internal::MI_PAGE_MAP_SUB_COUNT).is_none());
        assert!(PageMapSubIndex::new(usize::MAX).is_none());
    }

    #[test]
    fn alignment_accepts_only_nonzero_power_of_two() {
        assert_eq!(Alignment::new(1).map(Alignment::get), Some(1));
        assert_eq!(Alignment::new(64).map(Alignment::get), Some(64));
        assert!(Alignment::new(0).is_none());
        assert!(Alignment::new(24).is_none());
    }

    #[test]
    fn slice_count_rejects_zero_and_too_large_values() {
        assert!(SliceCount::from_usize(0).is_none());
        assert_eq!(SliceCount::from_usize(7).map(SliceCount::get), Some(7));
        assert!(SliceCount::from_usize(usize::MAX).is_none());
        assert!(SliceCount::from_usize_const(0).is_none());
        assert_eq!(
            SliceCount::from_usize_const(7).map(SliceCount::get),
            Some(7)
        );
        assert!(SliceCount::from_usize_const(u32::MAX as usize + 1).is_none());
    }
}
