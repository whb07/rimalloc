//! DT-11: Arena and slice domain types.

use core::num::NonZeroU32;

use super::ptr::ArenaPtr;
use super::scalar::{SliceCount, SliceIndex};
use crate::internal::bitmap::{BBitmapView, BitmapView, MI_BIT_CLEAR, MI_BIT_SET};

/// Borrowed atomic bitmaps for one live arena allocation phase.
pub(crate) struct ArenaSliceAccess<'a> {
    arena: ArenaPtr,
    slice_count: SliceCount,
    pinned: bool,
    free: BBitmapView<'a>,
    dirty: BitmapView<'a>,
    committed: BitmapView<'a>,
}

impl<'a> ArenaSliceAccess<'a> {
    /// # Safety
    /// The arena and its matching free, dirty and committed bitmaps must remain
    /// initialized and live for `'a`, with stable policy, slice count and
    /// bitmap backing. Their published extents must cover the arena's slices.
    /// Teardown is excluded; concurrent access follows the bitmap protocol.
    #[inline(always)]
    pub(crate) unsafe fn from_raw(arena: ArenaPtr) -> Self {
        unsafe {
            let raw = arena.as_ptr();
            Self {
                arena,
                slice_count: arena.slice_count(),
                pinned: (*raw).memid.is_pinned,
                free: BBitmapView::from_raw((*raw).slices_free),
                dirty: BitmapView::from_raw((*raw).slices_dirty),
                committed: BitmapView::from_raw((*raw).slices_committed),
            }
        }
    }

    #[forbid(unsafe_code)]
    #[inline(always)]
    pub(crate) fn try_claim(
        &self,
        slice_count: SliceCount,
        tseq: super::ThreadSeq,
    ) -> Option<ClaimedArenaSlices<'_, 'a>> {
        let count = slice_count.get() as usize;
        let mut start = 0usize;
        if !self
            .free
            .try_find_and_clear_n(tseq.get(), count, &mut start)
        {
            return None;
        }
        let Ok(range) = SliceRange::new(start, count, self.slice_count) else {
            crate::internal::mi_assert_internal!(false);
            self.free.set_n(start, count);
            return None;
        };
        Some(ClaimedArenaSlices {
            access: self,
            range,
            initially_committed: false,
            initially_zero: false,
        })
    }
}

/// A cleared range borrowing the exact arena bitmaps that granted its claim.
/// Drop restores only the free bits; dirty/commit changes retain C semantics.
pub(crate) struct ClaimedArenaSlices<'access, 'arena> {
    access: &'access ArenaSliceAccess<'arena>,
    range: SliceRange,
    initially_committed: bool,
    initially_zero: bool,
}

#[forbid(unsafe_code)]
impl ClaimedArenaSlices<'_, '_> {
    #[inline(always)]
    pub(crate) fn range(&self) -> SliceRange {
        self.range
    }

    #[inline(always)]
    pub(crate) fn initially_zero(&self) -> bool {
        self.initially_zero
    }

    #[inline(always)]
    pub(crate) fn set_initially_zero(&mut self, zero: bool) {
        self.initially_zero = zero;
    }

    #[inline(always)]
    pub(crate) fn initially_committed(&self) -> bool {
        self.initially_committed
    }

    #[inline(always)]
    pub(crate) fn set_initially_committed(&mut self, committed: bool) {
        self.initially_committed = committed;
    }

    #[inline(always)]
    pub(crate) fn mark_dirty(&mut self) -> usize {
        let mut already_dirty = 0;
        self.initially_zero = self.access.dirty.set_n(
            self.range.start_usize(),
            self.range.count_usize(),
            Some(&mut already_dirty),
        );
        already_dirty
    }

    #[inline(always)]
    pub(crate) fn committed_count(&self) -> usize {
        self.access
            .committed
            .popcount_n(self.range.start_usize(), self.range.count_usize())
    }

    #[inline(always)]
    pub(crate) fn mark_committed(&self, already_committed: Option<&mut usize>) {
        self.access.committed.set_n(
            self.range.start_usize(),
            self.range.count_usize(),
            already_committed,
        );
    }

    #[inline(always)]
    pub(crate) fn clear_committed(&self) {
        self.access
            .committed
            .clear_n(self.range.start_usize(), self.range.count_usize());
    }

    #[inline(always)]
    pub(crate) fn is_committed(&self) -> bool {
        self.access.committed.is_xset_n(
            MI_BIT_SET,
            self.range.start_usize(),
            self.range.count_usize(),
        )
    }

    #[inline(always)]
    pub(crate) fn is_dirty(&self) -> bool {
        self.access.dirty.is_xset_n(
            MI_BIT_SET,
            self.range.start_usize(),
            self.range.count_usize(),
        )
    }

    #[inline(always)]
    pub(crate) fn free_range_is_claimed(&self) -> bool {
        self.access.free.is_xset_n(
            MI_BIT_CLEAR,
            self.range.start_usize(),
            self.range.count_usize(),
        )
    }

    #[inline(always)]
    pub(crate) fn publish(self) -> ArenaAllocation {
        let allocation = ArenaAllocation {
            arena: self.access.arena,
            range: self.range,
            initially_committed: self.initially_committed,
            initially_zero: self.initially_zero,
            pinned: self.access.pinned,
        };
        core::mem::forget(self);
        allocation
    }
}

#[forbid(unsafe_code)]
impl Drop for ClaimedArenaSlices<'_, '_> {
    fn drop(&mut self) {
        self.access
            .free
            .set_n(self.range.start_usize(), self.range.count_usize());
    }
}

#[must_use = "arena allocation ownership must be transferred to its backing MemId"]
pub(crate) struct ArenaAllocation {
    arena: ArenaPtr,
    range: SliceRange,
    initially_committed: bool,
    initially_zero: bool,
    pinned: bool,
}

impl ArenaAllocation {
    #[inline(always)]
    pub(crate) fn arena(&self) -> ArenaPtr {
        self.arena
    }

    #[inline(always)]
    pub(crate) fn range(&self) -> SliceRange {
        self.range
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
    pub(crate) const FIRST_SLICE: Self = Self {
        start: SliceIndex::new(0),
        count: SliceCount::ONE,
    };

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
            count: SliceCount::from_nonzero(count),
        })
    }

    pub(crate) fn single_at_offset(self, offset: usize) -> Result<Self, SliceError> {
        if offset >= self.count_usize() {
            return Err(SliceError::OutOfBounds);
        }
        let start = self.checked_add_offset(offset)?;

        Ok(Self {
            start,
            count: SliceCount::ONE,
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
