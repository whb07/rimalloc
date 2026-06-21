//! DT-4: PageFlags and PageThreadId domain types.

use core::num::NonZeroUsize;

use super::ptr::PagePtr;
use super::scalar::{PageMapIndex, PageMapSubIndex, SliceCount};
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
    pub(crate) const fn new(bits: usize) -> Option<Self> {
        if (bits & !MI_PAGE_FLAG_MASK) == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

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
    #[inline(always)]
    pub(crate) fn new(id: usize) -> Option<Self> {
        if id == 0 || (id & MI_PAGE_FLAG_MASK) != 0 {
            None
        } else {
            Some(unsafe { Self::new_unchecked(id) })
        }
    }

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

#[derive(Clone, Copy)]
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
    pub(crate) fn from_parts(owner: PageOwnerId, flags: PageFlags) -> Self {
        let tid_val = match owner {
            PageOwnerId::Thread(tid) => tid.get(),
            PageOwnerId::Abandoned => 0,
            PageOwnerId::AbandonedMapped => MI_THREADID_ABANDONED_MAPPED,
        };
        Self(tid_val | flags.bits())
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
        Self::from_parts(owner, self.flags())
    }

    #[inline(always)]
    pub(crate) fn xor_current(self, current: ThreadId) -> XThreadDelta {
        XThreadDelta(current.get() ^ self.0)
    }

    #[inline(always)]
    pub(crate) fn classify_free_path(self, current: ThreadId) -> FreePathKind {
        let delta = self.xor_current(current);
        if delta.is_local_fast() {
            FreePathKind::LocalFast
        } else if delta.needs_local_slow() {
            FreePathKind::LocalSlowFlags
        } else if delta.is_remote_plain() {
            FreePathKind::RemotePlain
        } else {
            FreePathKind::RemoteSlowFlags
        }
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FreePathKind {
    LocalFast,
    LocalSlowFlags,
    RemotePlain,
    RemoteSlowFlags,
}

#[derive(Clone, Copy)]
pub(crate) struct RegisteredPageRange {
    page: PagePtr,
    index: PageMapIndex,
    sub_index: PageMapSubIndex,
    slice_count: SliceCount,
}

impl RegisteredPageRange {
    /// # Safety
    /// The caller must have registered `page` in the page map for
    /// `index/sub_index..+slice_count`, and that registration must still be
    /// valid for the lifetime of this witness.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(
        page: PagePtr,
        index: PageMapIndex,
        sub_index: PageMapSubIndex,
        slice_count: SliceCount,
    ) -> Self {
        Self {
            page,
            index,
            sub_index,
            slice_count,
        }
    }

    #[inline(always)]
    pub(crate) fn page(self) -> PagePtr {
        self.page
    }

    #[inline(always)]
    pub(crate) fn index(self) -> PageMapIndex {
        self.index
    }

    #[inline(always)]
    pub(crate) fn sub_index(self) -> PageMapSubIndex {
        self.sub_index
    }

    #[inline(always)]
    pub(crate) fn slice_count(self) -> SliceCount {
        self.slice_count
    }
}

pub(super) fn __lint_touch() {
    let _ = PageFlags::EMPTY;
    let tid = PageThreadId::new(0);
    let _ = tid.owner();
    let _ = tid.with_flags(PageFlags::EMPTY);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::MaybeUninit;

    #[test]
    fn page_thread_id_splits_owner_and_flags() {
        let owner = ThreadId::new(0x1000).expect("valid thread id");
        let tagged = PageThreadId::from_parts(PageOwnerId::Thread(owner), PageFlags::IN_FULL_QUEUE);

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
    fn page_thread_id_from_parts_round_trips_owner_and_flags() {
        let owner = ThreadId::new(0x3000).expect("valid thread id");
        let flags = PageFlags::new(MI_PAGE_FLAG_MASK).expect("valid flags");
        let tagged = PageThreadId::from_parts(PageOwnerId::Thread(owner), flags);

        assert_eq!(tagged.raw(), owner.get() | MI_PAGE_FLAG_MASK);
        assert_eq!(tagged.flags().bits(), flags.bits());
        match tagged.owner() {
            PageOwnerId::Thread(tid) => assert_eq!(tid.get(), owner.get()),
            PageOwnerId::Abandoned | PageOwnerId::AbandonedMapped => {
                panic!("expected thread owner")
            }
        }

        let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, flags);
        assert!(abandoned.is_abandoned());
        assert_eq!(abandoned.flags().bits(), flags.bits());

        let mapped = PageThreadId::from_parts(PageOwnerId::AbandonedMapped, flags);
        assert!(mapped.is_abandoned_mapped());
        assert_eq!(mapped.flags().bits(), flags.bits());
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
        let tid = ThreadId::new(0x2000).expect("valid thread id");
        let local = PageThreadId::new(tid.get());
        assert!(local.xor_current(tid).is_local_fast());
        assert_eq!(local.classify_free_path(tid), FreePathKind::LocalFast);

        let flagged = PageThreadId::new(tid.get() | PageFlags::IN_FULL_QUEUE.bits());
        assert!(flagged.xor_current(tid).needs_local_slow());
        assert_eq!(
            flagged.classify_free_path(tid),
            FreePathKind::LocalSlowFlags
        );

        let remote = PageThreadId::new(0x4000);
        assert!(remote.xor_current(tid).is_remote_plain());
        assert_eq!(remote.classify_free_path(tid), FreePathKind::RemotePlain);

        let remote_flagged = PageThreadId::new(0x4000 | PageFlags::IN_FULL_QUEUE.bits());
        assert_eq!(
            remote_flagged.classify_free_path(tid),
            FreePathKind::RemoteSlowFlags
        );
    }

    #[test]
    fn free_path_classifier_matches_raw_xor_branch_rules() {
        let current = ThreadId::new(0x2000).expect("valid thread id");
        let cases = [
            PageThreadId::new(current.get()),
            PageThreadId::new(current.get() | PageFlags::IN_FULL_QUEUE.bits()),
            PageThreadId::new(0x4000),
            PageThreadId::new(0x4000 | PageFlags::HAS_INTERIOR_POINTERS.bits()),
        ];

        for tagged in cases {
            let raw_delta = current.get() ^ tagged.raw();
            let expected = if raw_delta == 0 {
                FreePathKind::LocalFast
            } else if raw_delta <= MI_PAGE_FLAG_MASK {
                FreePathKind::LocalSlowFlags
            } else if (raw_delta & MI_PAGE_FLAG_MASK) == 0 {
                FreePathKind::RemotePlain
            } else {
                FreePathKind::RemoteSlowFlags
            };

            assert_eq!(tagged.classify_free_path(current), expected);
        }
    }

    #[test]
    fn thread_id_new_rejects_zero_and_flag_bits() {
        assert!(ThreadId::new(0).is_none());
        assert!(ThreadId::new(MI_PAGE_FLAG_MASK).is_none());
        assert_eq!(ThreadId::new(0x1000).map(ThreadId::get), Some(0x1000));
    }

    #[test]
    fn page_flags_new_rejects_bits_outside_flag_mask() {
        assert_eq!(PageFlags::new(0).map(PageFlags::bits), Some(0));
        assert_eq!(
            PageFlags::new(MI_PAGE_FLAG_MASK).map(PageFlags::bits),
            Some(MI_PAGE_FLAG_MASK)
        );
        assert!(PageFlags::new(MI_PAGE_FLAG_MASK + 1).is_none());
    }

    #[test]
    fn registered_page_range_preserves_registration_domain() {
        let mut raw = MaybeUninit::<crate::types::mi_page_t>::zeroed();
        let page = unsafe { PagePtr::new_unchecked(raw.as_mut_ptr()) };
        let index = PageMapIndex::new(7).expect("valid page-map index");
        let sub_index = PageMapSubIndex::new(3).expect("valid page-map sub-index");
        let slice_count = SliceCount::from_usize(5).expect("valid slice count");

        let range =
            unsafe { RegisteredPageRange::new_unchecked(page, index, sub_index, slice_count) };

        assert_eq!(range.page().as_ptr(), page.as_ptr());
        assert_eq!(range.index(), index);
        assert_eq!(range.sub_index(), sub_index);
        assert_eq!(range.slice_count(), slice_count);
    }
}
