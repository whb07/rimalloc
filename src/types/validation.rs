//! DT-6: User pointer validation types.
//!
//! Block types are intentionally split by validation phase: `AllocatedBlock` and
//! `FreeBlock` live in `validation_extra` because free-list wrappers consume
//! them without depending on user-pointer validation internals. `GuardedBlock`
//! and `BlockForFree` stay here because they are produced only by pointer
//! classification and validation.

use super::ptr::{BlockPtr, PagePtr, UserPtr};

/// Offset from the start of a block.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BlockOffset(pub(crate) usize);

/// Reason for an invalid pointer lookup.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PointerInvalidReason {
    NotOwnedByAllocator,
    BadAlignment,
    BadPadding,
    DoubleFree,
}

/// Represents a guarded block that needs special handling.
#[repr(transparent)]
#[derive(PartialEq, Eq, Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct GuardedBlock(BlockPtr);

impl GuardedBlock {
    #[inline(always)]
    pub(crate) fn after_guarded_detection(block: BlockPtr) -> Self {
        Self(block)
    }

    #[inline(always)]
    pub(crate) fn block(&self) -> BlockPtr {
        self.0
    }
}

/// Location of a pointer within a block.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) enum PointerLocation {
    BlockStart(BlockPtr),
    Interior {
        block: BlockPtr,
        offset: BlockOffset,
    },
    Guarded {
        block: GuardedBlock,
        offset: BlockOffset,
    },
}

/// Block facts derived from a validated pointer location.
#[derive(PartialEq, Eq, Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct LocatedBlock {
    block: BlockPtr,
    was_guarded: WasGuarded,
    offset: BlockOffset,
}

impl LocatedBlock {
    #[inline(always)]
    pub(crate) fn block(&self) -> BlockPtr {
        self.block
    }

    #[inline(always)]
    pub(crate) fn was_guarded(&self) -> WasGuarded {
        self.was_guarded
    }

    #[inline(always)]
    pub(crate) fn offset(&self) -> BlockOffset {
        self.offset
    }
}

impl PointerLocation {
    #[inline(always)]
    pub(crate) fn located_block(self) -> LocatedBlock {
        match self {
            PointerLocation::BlockStart(block) => LocatedBlock {
                block,
                was_guarded: WasGuarded::No,
                offset: BlockOffset(0),
            },
            PointerLocation::Interior { block, offset } => LocatedBlock {
                block,
                was_guarded: WasGuarded::No,
                offset,
            },
            PointerLocation::Guarded { block, offset } => LocatedBlock {
                block: block.block(),
                was_guarded: WasGuarded::Yes,
                offset,
            },
        }
    }

    #[inline(always)]
    pub(crate) fn block(&self) -> BlockPtr {
        match self {
            PointerLocation::BlockStart(block) | PointerLocation::Interior { block, .. } => *block,
            PointerLocation::Guarded { block, .. } => block.block(),
        }
    }
}

/// Result of a user pointer lookup.
pub(crate) enum PointerLookup {
    Null,
    Valid(ValidPointerLookup),
    Invalid {
        user: Option<UserPtr>,
        reason: PointerInvalidReason,
    },
}

#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ValidPointerLookup {
    user: UserPtr,
    page: PagePtr,
    location: PointerLocation,
}

impl ValidPointerLookup {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }

    #[inline(always)]
    pub(crate) fn into_parts(self) -> (UserPtr, PagePtr, PointerLocation) {
        (self.user, self.page, self.location)
    }
}

impl PointerLookup {
    #[inline(always)]
    pub(crate) fn valid(user: UserPtr, page: PagePtr, location: PointerLocation) -> Self {
        Self::Valid(ValidPointerLookup {
            user,
            page,
            location,
        })
    }

    #[inline(always)]
    pub(crate) fn prepare_free(self) -> Option<ValidatedFree> {
        let PointerLookup::Valid(valid) = self else {
            return None;
        };
        let (user, page, location) = valid.into_parts();
        let located = location.located_block();
        let block = BlockForFree::new(located.block(), located.was_guarded());
        Some(ValidatedFree { user, page, block })
    }
}

/// Indicates whether a block was guarded before being passed to free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WasGuarded {
    No,
    Yes,
}

/// A block that has been completely validated and is ready for the typed free path.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct BlockForFree {
    block: BlockPtr,
    was_guarded: WasGuarded,
}

impl BlockForFree {
    fn new(block: BlockPtr, was_guarded: WasGuarded) -> Self {
        Self { block, was_guarded }
    }

    /// # Safety
    /// `block` must be the exact start of a live, non-guarded block that is
    /// ready to be freed. This is only for release fast paths that have already
    /// ruled out interior-pointer and guarded-page modes.
    #[inline(always)]
    pub(crate) unsafe fn from_block_start_unchecked(block: BlockPtr) -> Self {
        Self {
            block,
            was_guarded: WasGuarded::No,
        }
    }

    pub(crate) fn block(&self) -> BlockPtr {
        self.block
    }

    pub(crate) fn was_guarded(&self) -> WasGuarded {
        self.was_guarded
    }

    pub(crate) fn is_guarded(&self) -> bool {
        self.was_guarded == WasGuarded::Yes
    }

    #[inline(always)]
    pub(crate) fn into_free_block(self) -> crate::core_types::FreeBlock {
        // SAFETY[AUDIT:typed-free-validation]: `BlockForFree` is constructed
        // only from a successful pointer lookup or the explicit unchecked fast
        // path that already proves the block is the start of a freeable block.
        unsafe { crate::core_types::validation_extra::FreeBlock::new_unchecked(self.block) }
    }
}

/// A fully validated free request ready to be routed.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ValidatedFree {
    user: UserPtr,
    page: PagePtr,
    block: BlockForFree,
}

impl ValidatedFree {
    pub(crate) fn user(&self) -> UserPtr {
        self.user
    }

    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }

    pub(crate) fn block(&self) -> &BlockForFree {
        &self.block
    }

    pub(crate) fn into_parts(self) -> ValidatedFreeParts {
        ValidatedFreeParts {
            user: self.user,
            page: self.page,
            block: self.block,
        }
    }
}

#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ValidatedFreeParts {
    pub(crate) user: UserPtr,
    pub(crate) page: PagePtr,
    pub(crate) block: BlockForFree,
}

pub(super) fn __lint_touch() {
    let _ = PointerInvalidReason::BadPadding;
    let lookup = PointerLookup::Invalid {
        user: None,
        reason: PointerInvalidReason::DoubleFree,
    };
    if let PointerLookup::Invalid { user, reason } = lookup {
        let _ = (user, reason);
    }
}

#[cfg(test)]
mod tests;
