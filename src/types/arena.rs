//! DT-11: Arena and slice domain types.

use core::num::NonZeroU32;

use super::ptr::ArenaPtr;
use super::scalar::{SliceCount, SliceIndex};

#[derive(Clone, Copy)]
pub(crate) struct ArenaAllocation {
    arena: ArenaPtr,
    range: SliceRange,
    memid: crate::internal::memid::mi_memid_t,
}

impl ArenaAllocation {
    #[inline(always)]
    pub(crate) const fn new(
        arena: ArenaPtr,
        range: SliceRange,
        memid: crate::internal::memid::mi_memid_t,
    ) -> Self {
        Self {
            arena,
            range,
            memid,
        }
    }

    #[inline(always)]
    pub(crate) fn arena(self) -> ArenaPtr {
        self.arena
    }

    #[inline(always)]
    pub(crate) fn range(self) -> SliceRange {
        self.range
    }

    #[inline(always)]
    pub(crate) fn memid_mut(&mut self) -> &mut crate::internal::memid::mi_memid_t {
        &mut self.memid
    }

    #[inline(always)]
    pub(crate) fn memid(self) -> crate::internal::memid::mi_memid_t {
        self.memid
    }

    #[inline(always)]
    pub(crate) fn into_memid(self) -> crate::internal::memid::mi_memid_t {
        self.memid
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
    pub(crate) fn new(start: usize, count: usize, arena: ArenaPtr) -> Result<Self, SliceError> {
        let start_u32: u32 = start.try_into().map_err(|_| SliceError::Overflow)?;
        let count_u32: u32 = count.try_into().map_err(|_| SliceError::Overflow)?;
        let nz_count = NonZeroU32::new(count_u32).ok_or(SliceError::Overflow)?;
        let end = start_u32
            .checked_add(count_u32)
            .ok_or(SliceError::Overflow)?;
        // SAFETY: callers pass initialized arena
        // metadata, and this checked constructor only reads the typed arena
        // slice-count bound before accepting the range.
        if end > unsafe { arena.slice_count().get() } {
            return Err(SliceError::OutOfBounds);
        }

        Ok(Self {
            start: SliceIndex(start_u32 as usize),
            // SAFETY: `nz_count` proves the raw
            // count is non-zero and the earlier conversions prove it fits in
            // the SliceCount `u32` domain.
            count: unsafe { SliceCount::new_unchecked(nz_count.get()) },
        })
    }

    pub(crate) fn from_raw(start: u32, count: u32) -> Result<Self, SliceError> {
        start.checked_add(count).ok_or(SliceError::Overflow)?;
        let count = NonZeroU32::new(count).ok_or(SliceError::Overflow)?;

        Ok(Self {
            start: SliceIndex(start as usize),
            // SAFETY: `count` is a NonZeroU32 and
            // `start + count` was checked above before building the range.
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
            // SAFETY: one is the smallest non-zero
            // valid slice count for the single-slice witness.
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
        self.start.0
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
        self.count.0.get() as usize
    }

    pub(crate) fn end(self) -> SliceIndex {
        self.checked_add_offset(self.count_usize())
            .unwrap_or_else(|_| {
                crate::internal::mi_assert_internal!(false);
                SliceIndex(0)
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
        Ok(SliceIndex(index_u32 as usize))
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
