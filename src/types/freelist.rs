//! DT-5: ThreadFreeWord domain types.

use super::constants::mi_block_t;
use super::ptr::{BlockPtr, ptr_exposed_addr};

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

    /// Return the nullable raw block head stored in the thread-free word.
    ///
    /// Use `head()` when a non-null value needs a typed `BlockPtr`; this is only
    /// for storage and C-compatibility boundaries that already expect null.
    #[inline(always)]
    pub(crate) fn head_raw(self) -> *mut mi_block_t {
        let ptr_val = self.0 & !Self::OWNED_BIT;
        if ptr_val == 0 {
            return core::ptr::null_mut();
        }

        #[cfg(miri)]
        {
            crate::provenance::resolve_address(ptr_val)
                .unwrap_or_else(|| crate::provenance::missing_address(ptr_val))
        }

        #[cfg(not(miri))]
        {
            // PROVENANCE[AUDIT:packed-thread-free-word]: the word was formed
            // from a block pointer plus the low ownership bit in this type.
            ptr_val as *mut mi_block_t
        }
    }

    #[inline(always)]
    pub(crate) fn head(self) -> Option<BlockPtr> {
        let ptr = self.head_raw();
        if ptr.is_null() {
            None
        } else {
            // SAFETY: The block pointer is non-null if not 0.
            Some(unsafe { BlockPtr::new_unchecked(ptr) })
        }
    }

    #[inline(always)]
    pub(crate) const fn is_owned(self) -> bool {
        (self.0 & Self::OWNED_BIT) != 0
    }

    #[inline(always)]
    pub(crate) const fn with_owner(self, owned: ThreadFreeOwner) -> Self {
        let bit = match owned {
            ThreadFreeOwner::Owned => Self::OWNED_BIT,
            ThreadFreeOwner::Unowned => 0,
        };
        Self((self.0 & !Self::OWNED_BIT) | bit)
    }

    #[inline(always)]
    pub(crate) const fn without_head(self) -> Self {
        Self(self.0 & Self::OWNED_BIT)
    }

    #[inline(always)]
    pub(crate) fn from_parts(head: Option<BlockPtr>, owned: ThreadFreeOwner) -> Self {
        let ptr_val = match head {
            Some(ptr) => {
                // PROVENANCE[AUDIT:packed-thread-free-word]: expose only the
                // block address bits needed for the xthread_free packed word.
                let val = ptr_exposed_addr(ptr.as_ptr());
                crate::internal::mi_assert_internal!((val & Self::OWNED_BIT) == 0);
                crate::provenance::remember_address(val, ptr.as_ptr());
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

#[cfg(test)]
mod tests;
