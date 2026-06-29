//! DT-11: Arena and slice domain types.

use core::num::NonZeroU32;

use crate::core_types::{ArenaPtr, SliceCount, SliceIndex};

#[derive(Clone, Copy)]
pub(crate) struct ArenaAllocation {
    pub(crate) arena: ArenaPtr,
    pub(crate) range: SliceRange,
    pub(crate) memid: crate::internal::mi_memid_t,
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
        if end > unsafe { arena.slice_count().get() } {
            return Err(SliceError::OutOfBounds);
        }

        Ok(Self {
            start: SliceIndex(start_u32 as usize),
            count: unsafe { SliceCount::new_unchecked(nz_count.get()) },
        })
    }

    pub(crate) fn from_raw(start: u32, count: u32) -> Result<Self, SliceError> {
        start.checked_add(count).ok_or(SliceError::Overflow)?;
        let count = NonZeroU32::new(count).ok_or(SliceError::Overflow)?;

        Ok(Self {
            start: SliceIndex(start as usize),
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
        crate::mi_assert_internal!(u32::try_from(self.start_usize()).is_ok());
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
                crate::mi_assert_internal!(false);
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
pub(crate) struct SharedArena(pub(crate) ArenaPtr);

impl SharedArena {
    /// # Safety
    /// `arena` must point to initialized arena metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {
        if unsafe { arena.is_exclusive() } {
            None
        } else {
            Some(Self(arena))
        }
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ArenaPtr {
        self.0
    }
}

#[repr(transparent)]
pub(crate) struct ExclusiveArena(pub(crate) ArenaPtr);

impl ExclusiveArena {
    /// # Safety
    /// `arena` must point to initialized arena metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {
        if unsafe { arena.is_exclusive() } {
            Some(Self(arena))
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ArenaPtr {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MatchNuma(pub(crate) bool);

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
pub(crate) struct AllowPinned(pub(crate) bool);

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
pub(crate) struct Exclusive(pub(crate) bool);

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

#[repr(transparent)]
pub(crate) struct ParentArena(pub(crate) ArenaPtr);

impl ParentArena {
    /// # Safety
    /// `arena` must point to initialized arena metadata.
    #[inline(always)]
    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {
        if unsafe { arena.parent().is_none() } {
            Some(Self(arena))
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ArenaPtr {
        self.0
    }
}

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicUsize};

#[repr(transparent)]
pub(crate) struct ArenaPageTablePtr(pub(crate) NonNull<crate::types::mi_arena_pages_t>);

#[repr(transparent)]
pub(crate) struct AbandonedCountCell(pub(crate) *const AtomicUsize);

#[repr(transparent)]
pub(crate) struct ArenaPageCell(pub(crate) *const AtomicPtr<c_void>);

pub(super) fn __lint_touch() {
    let _ = core::mem::size_of::<SharedArena>();
    let _ = core::mem::size_of::<ExclusiveArena>();
    let _ = core::mem::size_of::<ParentArena>();
    let _ = core::mem::size_of::<ArenaPageTablePtr>();
    let _ = core::mem::size_of::<AbandonedCountCell>();
    let _ = core::mem::size_of::<ArenaPageCell>();
    let _new: unsafe fn(ArenaPtr) -> Option<SharedArena> = SharedArena::new;
    let _ptr: fn(SharedArena) -> ArenaPtr = SharedArena::ptr;
    let _new: unsafe fn(ArenaPtr) -> Option<ExclusiveArena> = ExclusiveArena::new;
    let _ptr: fn(ExclusiveArena) -> ArenaPtr = ExclusiveArena::ptr;
    let _new: unsafe fn(ArenaPtr) -> Option<ParentArena> = ParentArena::new;
    let _ptr: fn(ParentArena) -> ArenaPtr = ParentArena::ptr;
}

#[cfg(test)]
mod tests;
