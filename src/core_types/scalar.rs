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
