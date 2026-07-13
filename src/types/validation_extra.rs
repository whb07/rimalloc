//! Extra validation types needed by freelist wrapper signatures.
use super::ptr::BlockPtr;

#[repr(transparent)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct AllocatedBlock(BlockPtr);

impl AllocatedBlock {
    /// # Safety
    /// `block` must be a valid block freshly popped from a page available free list.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(block: BlockPtr) -> Self {
        Self(block)
    }

    #[inline(always)]
    #[cfg(test)]
    pub(crate) fn as_ptr(&self) -> BlockPtr {
        self.0
    }

    #[inline(always)]
    pub(crate) fn into_ptr(self) -> BlockPtr {
        self.0
    }
}

#[repr(transparent)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct FreeBlock(BlockPtr);

impl FreeBlock {
    /// # Safety
    /// `block` must be a valid block that has completed pointer validation and
    /// is ready to be linked onto a free list.
    #[inline(always)]
    pub(in crate::types) unsafe fn new_unchecked(block: BlockPtr) -> Self {
        Self(block)
    }

    #[inline(always)]
    pub(crate) fn as_ptr(&self) -> BlockPtr {
        self.0
    }
}
