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
pub(crate) struct RequestedSize(pub(crate) usize);

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
        crate::internal::mi_debug_assert!(
            self.0.checked_add(crate::types::MI_PADDING_SIZE).is_some(),
            "requested size + padding must not overflow"
        );
        // SAFETY: caller guarantees the padded
        // sum is non-zero and cannot overflow on this hot path.
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
    // SAFETY: unchecked effective-size
    // construction is only valid after callers prove the non-zero size domain.
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
    // SAFETY: unchecked padded-size construction
    // is only valid after callers prove the non-zero padded-size domain.
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

/// Nullable `usize*` out-parameter used by `mi_u*` allocation/free APIs.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct UsableOut(*mut usize);

impl UsableOut {
    #[inline(always)]
    pub(crate) const fn new(out: *mut usize) -> Self {
        Self(out)
    }

    #[inline(always)]
    pub(crate) const fn as_ptr(self) -> *mut usize {
        self.0
    }

    /// # Safety
    /// If non-null, the wrapped pointer must be valid for a `usize` write.
    // SAFETY: nullable usable-size out pointer is
    // checked for null before writing the caller-provided storage.
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
pub(crate) struct BlockSize(pub(crate) NonZeroUsize);

impl BlockSize {
    /// # Safety
    /// `size` must be non-zero.
    // SAFETY: unchecked block-size construction is
    // only valid after callers prove the non-zero block-size domain.
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
        crate::internal::mi_debug_assert!(
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
    // SAFETY: unchecked full-block construction
    // is only valid after callers prove the non-zero full-block domain.
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
            // SAFETY: `usize::is_power_of_two`
            // rejects zero and proves the alignment domain.
            Some(unsafe { Self::new_unchecked(align) })
        } else {
            None
        }
    }
    /// # Safety
    /// `align` must be a non-zero power of two.
    // SAFETY: unchecked alignment construction is
    // only valid after callers prove non-zero power-of-two alignment.
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
    // SAFETY: unchecked optional alignment keeps
    // the raw zero-or-power-of-two domain in a transparent wrapper.
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
        crate::internal::mi_debug_assert!(self.0 > 0);
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
        crate::internal::mi_debug_assert!(self.0 > 0);
        Self(self.0 - 1)
    }

    #[inline(always)]
    pub(crate) fn subtract(self, count: u16) -> Self {
        crate::internal::mi_debug_assert!(self.0 >= count);
        Self(self.0 - count)
    }

    #[inline(always)]
    pub(crate) const fn checked_add_batch(
        self,
        len: BatchLen,
        capacity: PageCapacity,
    ) -> Option<Self> {
        match self.0.checked_add(len.get()) {
            Some(used) if used <= capacity.get() => Some(Self(used)),
            _ => None,
        }
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
    #[inline(always)]
    pub(crate) const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// # Safety
    /// `value` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(value: u16) -> Self {
        // SAFETY: the caller proves the batch is non-empty.
        Self(unsafe { NonZeroU16::new_unchecked(value) })
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0.get()
    }

    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.get() as usize
    }
}

/// Non-zero upper bound on the number of blocks leased from a page.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct BatchLimit(NonZeroU16);

impl BatchLimit {
    #[inline(always)]
    pub(crate) const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// # Safety
    /// `value` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(value: u16) -> Self {
        // SAFETY: the caller proves the limit is non-zero.
        Self(unsafe { NonZeroU16::new_unchecked(value) })
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0.get()
    }

    #[inline(always)]
    pub(crate) const fn as_usize(self) -> usize {
        self.get() as usize
    }

    #[inline(always)]
    pub(crate) fn clamp_to_remaining(self, remaining: usize) -> Option<Self> {
        let remaining = u16::try_from(remaining).unwrap_or(u16::MAX);
        Self::new(self.get().min(remaining))
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
    // SAFETY: unchecked page capacity construction
    // is reserved for callers that have already proved page count ordering.
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
        crate::internal::mi_debug_assert!(self.0 > 0);
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
    // SAFETY: unchecked OS page-size construction
    // is only valid after platform initialization proves a non-zero page size.
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
    // SAFETY: unchecked page-map index
    // construction is reserved for callers that already proved top-level bounds.
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
    // SAFETY: unchecked page-map sub-index
    // construction is reserved for callers that already proved submap bounds.
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
    // SAFETY: unchecked bin construction is
    // reserved for callers that already proved the page-queue bin domain.
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
        if (index as usize) <= crate::allocator::stats::MI_BIN_HUGE {
            Some(Self(index))
        } else {
            None
        }
    }
    /// # Safety
    /// `index` must be less than or equal to `MI_BIN_HUGE`.
    // SAFETY: unchecked stats-bin construction is
    // reserved for callers that already proved the stats bin domain.
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
    // SAFETY: unchecked direct-bin construction is
    // reserved for callers that already proved the direct-page bin domain.
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
            // SAFETY: the const branch rejects zero
            // and values outside the non-zero `u32` slice-count domain.
            Some(unsafe { Self::new_unchecked(count as u32) })
        }
    }
    /// # Safety
    /// `count` must be non-zero.
    // SAFETY: unchecked slice-count construction
    // is only valid after callers prove the non-zero `u32` count domain.
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
    // SAFETY: unchecked arena-index construction
    // is reserved for callers that already proved arena table bounds.
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
    // SAFETY: one is a valid non-zero lint fixture.
    let batch_limit = unsafe { BatchLimit::new_unchecked(1) };
    let _ = batch_limit.as_usize();
    let _ = BatchLimit::new(1);
    let _ = batch_limit.clamp_to_remaining(1);
    let _ = BatchLen::new(1);
    // SAFETY: one is a valid non-zero lint fixture.
    let batch_len = unsafe { BatchLen::new_unchecked(1) };
    let _ = batch_len.as_usize();
    let used = UsedBlockCount::new(1);
    // SAFETY: one satisfies the standalone
    // capacity fixture and is never published to page metadata.
    let capacity = unsafe { PageCapacity::new_unchecked(1) };
    let _ = UsedBlockCount::new(0).checked_add_batch(batch_len, capacity);
    let _ = used.checked_sub_batch(batch_len);
    let requested = RequestedSize::new(1);
    let _ = requested.checked_add(1);
    let _ = requested.checked_mul(1);
    // SAFETY: lint-touch fixtures use known-valid
    // non-zero values solely to keep scalar wrapper methods typechecked.
    let effective = unsafe { EffectiveSize::new_unchecked(1) };
    let _ = effective.checked_add(1);
    let padded = unsafe { PaddedSize::new_unchecked(1) };
    let _ = padded.checked_add(1);
    let _ = padded.checked_sub_padding();
    let usable = UsableSize::new(1);
    let _ = usable.checked_add(1);
    let usable_out = UsableOut::new(core::ptr::null_mut());
    let _ = usable_out.as_ptr();
    // SAFETY: lint-touch fixtures use known-valid
    // scalar domains and never publish these values to allocator state.
    let block = unsafe { BlockSize::new_unchecked(1) };
    let _ = block.checked_add(1);
    let _ = block.checked_usable();
    let _ = unsafe { FullBlockSize::new_unchecked(1) };
    // SAFETY: one is a valid optional alignment
    // for this compile-time-only lint fixture.
    let alignment = unsafe { MaybeAlignment::new_unchecked(1) };
    let _ = alignment.is_some();
    let _ = WordCount::new(1);
    let _ = PageReclaimLimit::new(1).get();
    let thread_count = ThreadCount::new(1);
    let _ = thread_count.get();
    let heap_count = HeapCount::new(1);
    let _ = heap_count.get();
    // SAFETY: zero is a valid page-queue bin for
    // this compile-time-only lint fixture.
    let bin = unsafe { Bin::new_unchecked(0) };
    let _ = bin.get();
    let _ = DirectBin::new(0);
    let _ = SliceIndex::new(1).as_u32();
}

#[cfg(test)]
mod tests;
