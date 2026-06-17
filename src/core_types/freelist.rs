//! DT-5: ThreadFreeWord domain types.

use crate::core_types::BlockPtr;

/// Represents ownership state of a thread-free word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ThreadFreeOwner {
    Owned,
    Unowned,
}

/// Represents the `xthread_free` atomic field.
/// Packs a block pointer (aligned) and an ownership bit.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ThreadFreeWord(pub(crate) usize);

impl ThreadFreeWord {
    pub(crate) const OWNED_BIT: usize = 1;

    #[inline(always)]
    pub(crate) const fn raw(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) fn head(self) -> Option<BlockPtr> {
        let ptr_val = self.0 & !Self::OWNED_BIT;
        if ptr_val == 0 {
            None
        } else {
            // SAFETY: The block pointer is non-null if not 0.
            Some(unsafe { BlockPtr::new_unchecked(ptr_val as *mut crate::types::mi_block_t) })
        }
    }

    #[inline(always)]
    pub(crate) fn is_owned(self) -> bool {
        (self.0 & Self::OWNED_BIT) != 0
    }

    #[inline(always)]
    pub(crate) fn from_parts(head: Option<BlockPtr>, owned: ThreadFreeOwner) -> Self {
        let ptr_val = match head {
            Some(ptr) => {
                let val = ptr.as_ptr() as usize;
                crate::mi_assert_internal!((val & Self::OWNED_BIT) == 0);
                val
            }
            None => 0,
        };
        let bit = if owned == ThreadFreeOwner::Owned {
            Self::OWNED_BIT
        } else {
            0
        };
        Self(ptr_val | bit)
    }
}

pub(crate) struct ThreadFreeSnapshot {
    head: Option<BlockPtr>,
    owner: ThreadFreeOwner,
}
