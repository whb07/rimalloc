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

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::MaybeUninit;

    fn with_arena_slice_count<R>(count: usize, f: impl FnOnce(ArenaPtr) -> R) -> R {
        let mut raw = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
        let arena = unsafe { ArenaPtr::new_unchecked(raw.as_mut_ptr()) };
        unsafe {
            arena.set_slice_count(SliceCount::from_usize(count).unwrap());
        }
        f(arena)
    }

    fn with_arena_state<R>(
        is_exclusive: bool,
        parent: Option<ArenaPtr>,
        f: impl FnOnce(ArenaPtr) -> R,
    ) -> R {
        let mut raw = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
        let arena = unsafe { ArenaPtr::new_unchecked(raw.as_mut_ptr()) };
        unsafe {
            arena.set_is_exclusive(is_exclusive);
            arena.set_parent(parent);
        }
        f(arena)
    }

    #[test]
    fn slice_range_new_accepts_range_ending_at_arena_slice_count() {
        let range = with_arena_slice_count(16, |arena| SliceRange::new(12, 4, arena).unwrap());

        assert_eq!(range.start_usize(), 12);
        assert_eq!(range.start_u32(), 12);
        assert_eq!(range.count_usize(), 4);
        assert_eq!(range.count_u32(), 4);
        assert_eq!(range.end_usize(), 16);
    }

    #[test]
    fn slice_range_new_rejects_range_past_arena_slice_count() {
        let err = with_arena_slice_count(16, |arena| SliceRange::new(13, 4, arena).unwrap_err());

        assert_eq!(err, SliceError::OutOfBounds);
    }

    #[test]
    fn slice_range_new_rejects_overflowing_range_end() {
        let err = with_arena_slice_count(u32::MAX as usize, |arena| {
            SliceRange::new(u32::MAX as usize, 1, arena).unwrap_err()
        });

        assert_eq!(err, SliceError::Overflow);
    }

    #[test]
    fn slice_range_checked_offset_rejects_overflowing_addition() {
        let range = SliceRange::from_raw(u32::MAX - 1, 1).unwrap();

        let err = range.checked_add_offset(2).unwrap_err();

        assert_eq!(err, SliceError::Overflow);
    }

    #[test]
    fn slice_range_single_at_offset_uses_checked_range_bounds() {
        let arena = with_arena_slice_count(16, |arena| SliceRange::new(12, 4, arena).unwrap());

        let single = arena.single_at_offset(3).unwrap();
        let err = arena.single_at_offset(4).unwrap_err();

        assert_eq!(single.start_usize(), 15);
        assert_eq!(single.count_usize(), 1);
        assert_eq!(err, SliceError::OutOfBounds);
    }

    #[test]
    fn arena_role_constructors_follow_raw_exclusive_state() {
        with_arena_state(false, None, |arena| {
            assert_eq!(
                unsafe { SharedArena::new(arena).unwrap().ptr().as_ptr() },
                arena.as_ptr()
            );
            assert!(unsafe { ExclusiveArena::new(arena).is_none() });
        });

        with_arena_state(true, None, |arena| {
            assert_eq!(
                unsafe { ExclusiveArena::new(arena).unwrap().ptr().as_ptr() },
                arena.as_ptr()
            );
            assert!(unsafe { SharedArena::new(arena).is_none() });
        });
    }

    #[test]
    fn parent_arena_constructor_rejects_child_arena_state() {
        let mut raw_parent = MaybeUninit::<crate::types::mi_arena_t>::zeroed();
        let parent = unsafe { ArenaPtr::new_unchecked(raw_parent.as_mut_ptr()) };

        with_arena_state(false, None, |arena| {
            assert_eq!(
                unsafe { ParentArena::new(arena).unwrap().ptr().as_ptr() },
                arena.as_ptr()
            );
        });

        with_arena_state(false, Some(parent), |arena| {
            assert!(unsafe { ParentArena::new(arena).is_none() });
        });
    }
}
