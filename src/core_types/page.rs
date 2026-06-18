//! DT-4: PageFlags and PageThreadId domain types.

use core::num::NonZeroUsize;

use crate::types::{
    MI_PAGE_FLAG_MASK, MI_PAGE_HAS_INTERIOR_POINTERS, MI_PAGE_IN_FULL_QUEUE,
    MI_THREADID_ABANDONED_MAPPED,
};

/// Represents the low bits in `xthread_id` specifying page status flags.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageFlags(usize);

impl PageFlags {
    pub(crate) const EMPTY: Self = Self(0);
    pub(crate) const IN_FULL_QUEUE: Self = Self(MI_PAGE_IN_FULL_QUEUE);
    pub(crate) const HAS_INTERIOR_POINTERS: Self = Self(MI_PAGE_HAS_INTERIOR_POINTERS);

    #[inline(always)]
    pub(crate) const fn bits(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }

    #[inline(always)]
    pub(crate) const fn from_low_bits(bits: usize) -> Self {
        Self(bits & MI_PAGE_FLAG_MASK)
    }
}

/// Represents a real owning thread id. Non-zero and must have flag bits clear.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ThreadId(NonZeroUsize);

impl ThreadId {
    /// # Safety
    /// `id` must be non-zero and must not conflict with `MI_PAGE_FLAG_MASK`.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(id: usize) -> Self {
        Self(unsafe { NonZeroUsize::new_unchecked(id) })
    }

    #[inline(always)]
    pub(crate) fn get(self) -> usize {
        self.0.get()
    }
}

pub(crate) enum PageOwnerId {
    Thread(ThreadId),
    Abandoned,
    AbandonedMapped,
}

/// Wraps the raw tagged `xthread_id` page field.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageThreadId(usize);

impl PageThreadId {
    #[inline(always)]
    pub(crate) fn new(tagged_id: usize) -> Self {
        Self(tagged_id)
    }

    #[inline(always)]
    pub(crate) fn raw(self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) fn owner(self) -> PageOwnerId {
        let tid = self.0 & !MI_PAGE_FLAG_MASK;
        if tid == 0 {
            PageOwnerId::Abandoned
        } else if tid == MI_THREADID_ABANDONED_MAPPED {
            PageOwnerId::AbandonedMapped
        } else {
            // SAFETY: Checked against 0 above.
            PageOwnerId::Thread(unsafe { ThreadId::new_unchecked(tid) })
        }
    }

    #[inline(always)]
    pub(crate) fn flags(self) -> PageFlags {
        PageFlags::from_low_bits(self.0)
    }

    #[inline(always)]
    pub(crate) fn owner_bits(self) -> usize {
        self.0 & !MI_PAGE_FLAG_MASK
    }

    #[inline(always)]
    pub(crate) fn is_abandoned(self) -> bool {
        self.owner_bits() == 0
    }

    #[inline(always)]
    pub(crate) fn is_abandoned_mapped(self) -> bool {
        self.owner_bits() == MI_THREADID_ABANDONED_MAPPED
    }

    #[inline(always)]
    pub(crate) fn with_flags(self, flags: PageFlags) -> Self {
        Self(self.owner_bits() | flags.bits())
    }

    #[inline(always)]
    pub(crate) fn with_owner(self, owner: PageOwnerId) -> Self {
        let tid_val = match owner {
            PageOwnerId::Thread(tid) => tid.get(),
            PageOwnerId::Abandoned => 0,
            PageOwnerId::AbandonedMapped => MI_THREADID_ABANDONED_MAPPED,
        };
        Self(tid_val | self.flags().bits())
    }

    #[inline(always)]
    pub(crate) fn xor_current(self, current: ThreadId) -> XThreadDelta {
        XThreadDelta(current.get() ^ self.0)
    }
}

/// Represents the XOR between the current thread ID and the page's tagged thread ID.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct XThreadDelta(usize);

impl XThreadDelta {
    #[inline(always)]
    pub(crate) fn is_local_fast(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub(crate) fn needs_local_slow(self) -> bool {
        self.0 <= MI_PAGE_FLAG_MASK
    }

    #[inline(always)]
    pub(crate) fn is_remote_plain(self) -> bool {
        (self.0 & MI_PAGE_FLAG_MASK) == 0
    }
}

pub(crate) enum FreePathKind {
    LocalFast,
    LocalSlowFlags,
    RemotePlain,
    RemoteSlowFlags,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_thread_id_splits_owner_and_flags() {
        let owner = unsafe { ThreadId::new_unchecked(0x1000) };
        let tagged = PageThreadId::new(owner.get() | PageFlags::IN_FULL_QUEUE.bits());

        match tagged.owner() {
            PageOwnerId::Thread(tid) => assert_eq!(tid.get(), owner.get()),
            PageOwnerId::Abandoned | PageOwnerId::AbandonedMapped => {
                panic!("expected thread owner")
            }
        }
        assert!(tagged.flags().contains(PageFlags::IN_FULL_QUEUE));
        assert!(!tagged.is_abandoned());
        assert!(!tagged.is_abandoned_mapped());
    }

    #[test]
    fn page_thread_id_preserves_flags_when_owner_changes() {
        let tagged = PageThreadId::new(PageFlags::HAS_INTERIOR_POINTERS.bits())
            .with_owner(PageOwnerId::AbandonedMapped);

        assert!(tagged.is_abandoned_mapped());
        assert!(tagged.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));

        let abandoned = tagged.with_owner(PageOwnerId::Abandoned);
        assert!(abandoned.is_abandoned());
        assert!(abandoned.flags().contains(PageFlags::HAS_INTERIOR_POINTERS));
    }

    #[test]
    fn xthread_delta_classifies_fast_and_slow_paths() {
        let tid = unsafe { ThreadId::new_unchecked(0x2000) };
        let local = PageThreadId::new(tid.get());
        assert!(local.xor_current(tid).is_local_fast());

        let flagged = PageThreadId::new(tid.get() | PageFlags::IN_FULL_QUEUE.bits());
        assert!(flagged.xor_current(tid).needs_local_slow());

        let remote = PageThreadId::new(0x4000);
        assert!(remote.xor_current(tid).is_remote_plain());
    }
}
