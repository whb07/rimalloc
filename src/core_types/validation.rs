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
pub(crate) struct BlockForFree {
    block: BlockPtr,
    was_guarded: WasGuarded,
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
pub(crate) struct ValidatedFree {
    user: UserPtr,
    page: PagePtr,
    block: BlockForFree,
}

impl ValidatedFree {
    pub(crate) fn new(user: UserPtr, page: PagePtr, block: BlockForFree) -> Self {
        Self { user, page, block }
    }
}
