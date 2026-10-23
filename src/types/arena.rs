//! DT-11: Arena and slice domain types.

use core::num::NonZeroU32;

use super::ptr::ArenaPtr;
use super::scalar::{SliceCount, SliceIndex};

#[must_use = "arena allocation ownership must be transferred to its backing MemId"]
pub(crate) struct ArenaAllocation {
    arena: ArenaPtr,
    range: SliceRange,
    initially_committed: bool,
    initially_zero: bool,
    pinned: bool,
}

impl ArenaAllocation {
    /// # Safety
    /// `arena` must be live and initialized, and the caller must exclusively own
    /// the cleared free-bitmap claim for this in-bounds `range`. No other
    /// allocation owner may represent the same claim.
    #[inline(always)]
    pub(crate) unsafe fn from_claimed(arena: ArenaPtr, range: SliceRange) -> Self {
        crate::internal::mi_assert_internal!(range.count_usize() > 0);
        crate::internal::mi_assert_internal!(
            range.start_usize() < unsafe { (*arena.as_ptr()).slice_count }
        );
        // SAFETY: the live arena's pinned policy is stable throughout this claim.
        let pinned = unsafe { (*arena.as_ptr()).memid.is_pinned };
        Self {
            arena,
            range,
            initially_committed: false,
            initially_zero: false,
            pinned,
        }
    }

    #[inline(always)]
    pub(crate) fn arena(&self) -> ArenaPtr {
        self.arena
    }

    #[inline(always)]
    pub(crate) fn range(&self) -> SliceRange {
        self.range
    }

    #[inline(always)]
    pub(crate) fn set_initially_zero(&mut self, zero: bool) {
        self.initially_zero = zero;
    }

    #[inline(always)]
    pub(crate) fn set_initially_committed(&mut self, committed: bool) {
        self.initially_committed = committed;
    }

    #[inline(always)]
    pub(crate) fn initially_zero(&self) -> bool {
        self.initially_zero
    }

    #[inline(always)]
    pub(crate) fn initially_committed(&self) -> bool {
        self.initially_committed
    }

    #[inline(always)]
    pub(crate) fn into_memid(self) -> crate::internal::memid::mi_memid_t {
        crate::core_types::MemId::<crate::core_types::ArenaMem>::arena(
            self.arena,
            self.range,
            super::memid::InitiallyCommitted(self.initially_committed),
            super::memid::InitiallyZero(self.initially_zero),
            super::memid::Pinned(self.pinned),
        )
        .into_raw()
    }
}

/// Error when creating an arena slice range.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SliceError {
    Overflow,
    OutOfBounds,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SliceRange {
    start: SliceIndex,
    count: SliceCount,
}

impl SliceRange {
    pub(crate) fn new(
        start: usize,
        count: usize,
        arena_slices: SliceCount,
    ) -> Result<Self, SliceError> {
        let start_u32: u32 = start.try_into().map_err(|_| SliceError::Overflow)?;
        let count = SliceCount::from_usize(count).ok_or(SliceError::Overflow)?;
        let end = start_u32
            .checked_add(count.get())
            .ok_or(SliceError::Overflow)?;
        if end > arena_slices.get() {
            return Err(SliceError::OutOfBounds);
        }

        Ok(Self {
            start: SliceIndex::new(start_u32 as usize),
            count,
        })
    }

    pub(crate) fn from_raw(start: u32, count: u32) -> Result<Self, SliceError> {
        start.checked_add(count).ok_or(SliceError::Overflow)?;
        let count = NonZeroU32::new(count).ok_or(SliceError::Overflow)?;

        Ok(Self {
            start: SliceIndex::new(start as usize),
            count: unsafe { SliceCount::new_unchecked(count.get()) },
        })
    }

    pub(crate) fn single_at_offset(self, offset: usize) -> Result<Self, SliceError> {
        if offset >= self.count_usize() {
            return Err(SliceError::OutOfBounds);
        }
        let start = self.checked_add_offset(offset)?;

        Ok(Self {
            start,
            count: unsafe { SliceCount::new_unchecked(1) },
        })
    }

    pub(crate) fn start(self) -> SliceIndex {
        self.start
    }

    #[inline(always)]
    pub(crate) fn start_u32(self) -> u32 {
        crate::internal::mi_assert_internal!(u32::try_from(self.start_usize()).is_ok());
        self.start_usize() as u32
    }

    #[inline(always)]
    pub(crate) fn start_usize(self) -> usize {
        self.start.get()
    }

    pub(crate) fn count(self) -> SliceCount {
        self.count
    }

    #[inline(always)]
    pub(crate) const fn count_u32(self) -> u32 {
        self.count.get()
    }

    #[inline(always)]
    pub(crate) fn count_usize(self) -> usize {
        self.count.get() as usize
    }

    pub(crate) fn end(self) -> SliceIndex {
        self.checked_add_offset(self.count_usize())
            .unwrap_or_else(|_| {
                crate::internal::mi_assert_internal!(false);
                SliceIndex::new(0)
            })
    }

    #[inline(always)]
    pub(crate) fn end_usize(self) -> usize {
        self.end().get()
    }

    pub(crate) fn checked_add_offset(self, offset: usize) -> Result<SliceIndex, SliceError> {
        let index = self
            .start_usize()
            .checked_add(offset)
            .ok_or(SliceError::Overflow)?;
        let index_u32: u32 = index.try_into().map_err(|_| SliceError::Overflow)?;
        Ok(SliceIndex::new(index_u32 as usize))
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MatchNuma(bool);

impl MatchNuma {
    #[inline(always)]
    pub(crate) const fn new(match_numa: bool) -> Self {
        Self(match_numa)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> bool {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AllowPinned(bool);

impl AllowPinned {
    #[inline(always)]
    pub(crate) const fn new(allow_pinned: bool) -> Self {
        Self(allow_pinned)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> bool {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct Exclusive(bool);

impl Exclusive {
    #[inline(always)]
    pub(crate) const fn new(exclusive: bool) -> Self {
        Self(exclusive)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> bool {
        self.0
    }
}

#[cfg(test)]
mod tests;
