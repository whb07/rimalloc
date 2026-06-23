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

    /// Return the nullable raw block head stored in the thread-free word.
    ///
    /// Use `head()` when a non-null value needs a typed `BlockPtr`; this is only
    /// for storage and C-compatibility boundaries that already expect null.
    #[inline(always)]
    pub(crate) fn head_raw(self) -> *mut crate::types::mi_block_t {
        let ptr_val = self.0 & !Self::OWNED_BIT;
        ptr_val as *mut crate::types::mi_block_t
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_free_word_round_trips_owner_without_head() {
        let owned = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned);
        assert_eq!(owned.raw(), ThreadFreeWord::OWNED_BIT);
        assert!(owned.is_owned());
        assert!(owned.head().is_none());
        assert!(owned.head_raw().is_null());

        let unowned = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned);
        assert_eq!(unowned.raw(), 0);
        assert!(!unowned.is_owned());
        assert!(unowned.head().is_none());
        assert!(unowned.head_raw().is_null());
    }

    #[test]
    fn thread_free_word_preserves_aligned_head_pointer() {
        let mut block = crate::types::mi_block_t::default();
        let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };

        let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
        assert!(word.is_owned());
        assert_eq!(word.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));
        assert_eq!(word.head_raw(), head.as_ptr());
        assert_eq!(
            word.raw() & !ThreadFreeWord::OWNED_BIT,
            head.as_ptr() as usize
        );
    }

    #[test]
    fn thread_free_word_updates_owner_without_decoding_head() {
        let mut block = crate::types::mi_block_t::default();
        let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
        let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);

        let unowned = word.with_owner(ThreadFreeOwner::Unowned);
        assert!(!unowned.is_owned());
        assert_eq!(unowned.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));

        let owned = unowned.with_owner(ThreadFreeOwner::Owned);
        assert_eq!(owned.raw(), word.raw());
    }

    #[test]
    fn thread_free_word_clears_head_without_decoding_owner() {
        let mut block = crate::types::mi_block_t::default();
        let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
        let owned = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
        let unowned = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Unowned);

        let owned_without_head = owned.without_head();
        assert!(owned_without_head.is_owned());
        assert!(owned_without_head.head().is_none());
        assert_eq!(owned_without_head.raw(), ThreadFreeWord::OWNED_BIT);

        let unowned_without_head = unowned.without_head();
        assert!(!unowned_without_head.is_owned());
        assert!(unowned_without_head.head().is_none());
        assert_eq!(unowned_without_head.raw(), 0);
    }
}
