//! Extra validation types needed by freelist wrapper signatures.
use crate::core_types::BlockPtr;

#[repr(transparent)]
pub(crate) struct AllocatedBlock(pub(crate) BlockPtr);

#[repr(transparent)]
pub(crate) struct FreeBlock(pub(crate) BlockPtr);
