//! Extra validation types needed by freelist wrapper signatures.
use crate::core_types::BlockPtr;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AllocatedBlock(BlockPtr);

impl AllocatedBlock {
    /// # Safety
    /// `block` must be a valid block freshly popped from a page available free list.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(block: BlockPtr) -> Self {
        Self(block)
    }

    #[inline(always)]
    pub(crate) fn block(self) -> BlockPtr {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct FreeBlock(BlockPtr);

impl FreeBlock {
    /// # Safety
    /// `block` must be a valid block that has completed pointer validation and
    /// is ready to be linked onto a free list.
    #[inline(always)]
    pub(in crate::core_types) unsafe fn new_unchecked(block: BlockPtr) -> Self {
        Self(block)
    }

    #[inline(always)]
    pub(crate) fn block(self) -> BlockPtr {
        self.0
    }
}
