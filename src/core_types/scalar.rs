//! DT-1 scalar domain types (zero-cost newtypes over usize / NonZeroUsize).
//! All types are #[repr(transparent)], Copy, Clone, PartialEq, etc.

use core::num::{NonZeroU32, NonZeroUsize};

/// Domain-specific error for allocation size overflows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AllocSizeError {
    Overflow,
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
        Self(NonZeroUsize::new_unchecked(size))
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
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
        Self(NonZeroUsize::new_unchecked(size))
    }
    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
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
        Self(NonZeroUsize::new_unchecked(size))
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
        crate::mi_debug_assert!(
            self.get() >= crate::types::MI_PADDING_SIZE,
            "Block size must be at least as large as padding size"
        );
        UsableSize(self.get().wrapping_sub(crate::types::MI_PADDING_SIZE))
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
        Self(NonZeroUsize::new_unchecked(align))
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
    pub(crate) const fn new(align: usize) -> Self {
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

/// Bin index (0 … MI_BIN_COUNT-1).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct Bin(pub(crate) u16);

impl Bin {
    #[inline(always)]
    pub(crate) const fn new(index: u16) -> Self {
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
    pub(crate) const fn new(index: u16) -> Self {
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
    pub(crate) const fn new(index: u16) -> Self {
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
    /// # Safety
    /// `count` must be non-zero.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(count: u32) -> Self {
        Self(NonZeroU32::new_unchecked(count))
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
    pub(crate) const fn new(idx: u16) -> Self {
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
