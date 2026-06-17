//! DT-11: Arena and slice domain types.

use core::num::NonZeroU32;

use crate::core_types::{ArenaPtr, SliceCount, SliceIndex};

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
    /// Note: arena validation logic omitted in data-types stage.
    pub(crate) fn new(start: usize, count: usize, _arena: ArenaPtr) -> Result<Self, SliceError> {
        let start_u32: u32 = start.try_into().map_err(|_| SliceError::Overflow)?;
        let count_u32: u32 = count.try_into().map_err(|_| SliceError::Overflow)?;
        let nz_count = NonZeroU32::new(count_u32).ok_or(SliceError::Overflow)?;

        Ok(Self {
            start: SliceIndex(start_u32 as usize),
            count: unsafe { SliceCount::new_unchecked(nz_count.get()) },
        })
    }

    pub(crate) fn from_raw(start: u32, count: u32) -> Self {
        // A valid arena memid always carries a non-zero slice count. The
        // `debug_assert!` catches corrupted memids in debug builds; release builds
        // trust the invariant to stay branchless on the arena free path.
        crate::mi_debug_assert!(count != 0, "arena memid slice count is non-zero");
        // SAFETY: `count` is non-zero per the invariant above.
        Self {
            start: SliceIndex(start as usize),
            count: unsafe { SliceCount::new_unchecked(count) },
        }
    }

    pub(crate) fn start(self) -> SliceIndex {
        self.start
    }

    #[inline(always)]
    pub(crate) fn start_usize(self) -> usize {
        self.start.0
    }

    pub(crate) fn count(self) -> SliceCount {
        self.count
    }

    #[inline(always)]
    pub(crate) fn count_usize(self) -> usize {
        self.count.0.get() as usize
    }
}

#[repr(transparent)]
pub(crate) struct SharedArena(pub(crate) ArenaPtr);

#[repr(transparent)]
pub(crate) struct ExclusiveArena(pub(crate) ArenaPtr);

#[repr(transparent)]
pub(crate) struct ParentArena(pub(crate) ArenaPtr);

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicUsize};

#[repr(transparent)]
pub(crate) struct ArenaPageTablePtr(pub(crate) NonNull<crate::types::mi_arena_pages_t>);

#[repr(transparent)]
pub(crate) struct AbandonedCountCell(pub(crate) *const AtomicUsize);

#[repr(transparent)]
pub(crate) struct ArenaPageCell(pub(crate) *const AtomicPtr<c_void>);
