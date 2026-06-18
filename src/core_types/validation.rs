//! DT-6: User pointer validation types.

use crate::core_types::{BlockPtr, PagePtr, UserPtr};

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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct GuardedBlock(pub(crate) BlockPtr);

/// Location of a pointer within a block.
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct LocatedBlock {
    block: BlockPtr,
    was_guarded: WasGuarded,
    offset: BlockOffset,
}

impl LocatedBlock {
    #[inline(always)]
    pub(crate) fn block(self) -> BlockPtr {
        self.block
    }

    #[inline(always)]
    pub(crate) fn was_guarded(self) -> WasGuarded {
        self.was_guarded
    }

    #[inline(always)]
    pub(crate) fn offset(self) -> BlockOffset {
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
                block: block.0,
                was_guarded: WasGuarded::Yes,
                offset,
            },
        }
    }

    #[inline(always)]
    pub(crate) fn block(self) -> BlockPtr {
        self.located_block().block()
    }
}

/// Result of a user pointer lookup.
pub(crate) enum PointerLookup {
    Null,
    Valid {
        user: UserPtr,
        page: PagePtr,
        location: PointerLocation,
    },
    Invalid {
        user: Option<UserPtr>,
        reason: PointerInvalidReason,
    },
}

/// Indicates whether a block was guarded before being passed to free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WasGuarded {
    No,
    Yes,
}

/// A block that has been completely validated and is ready for the typed free path.
#[derive(Clone, Copy)]
pub(crate) struct BlockForFree {
    pub(crate) block: BlockPtr,
    pub(crate) was_guarded: WasGuarded,
}

impl BlockForFree {
    pub(crate) fn new(block: BlockPtr, was_guarded: WasGuarded) -> Self {
        Self { block, was_guarded }
    }

    pub(crate) fn block(self) -> BlockPtr {
        self.block
    }

    pub(crate) fn was_guarded(self) -> WasGuarded {
        self.was_guarded
    }
}

/// A fully validated free request ready to be routed.
#[derive(Clone, Copy)]
pub(crate) struct ValidatedFree {
    pub(crate) user: UserPtr,
    pub(crate) page: PagePtr,
    pub(crate) block: BlockForFree,
}

impl ValidatedFree {
    pub(crate) fn new(user: UserPtr, page: PagePtr, block: BlockForFree) -> Self {
        Self { user, page, block }
    }
}
