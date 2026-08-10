//! DT-4: PageFlags and PageThreadId domain types.

use core::num::NonZeroUsize;

use super::constants::{
    MI_PAGE_FLAG_MASK, MI_PAGE_HAS_INTERIOR_POINTERS, MI_PAGE_IN_FULL_QUEUE,
    MI_THREADID_ABANDONED_MAPPED,
};
use super::ptr::PagePtr;
use super::scalar::{PageMapPrimaryIndex, PageMapSubIndex, SliceCount};
use core::ffi::c_void;

/// Represents the low bits in `xthread_id` specifying page status flags.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageFlags(usize);

impl PageFlags {
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
            // SAFETY: the branch rejects zero and all
            // page flag bits before constructing the non-zero thread id.
            Some(unsafe { Self::new_unchecked(id) })
        }
    }

    /// # Safety
    /// `id` must be non-zero and must not conflict with `MI_PAGE_FLAG_MASK`.
    // SAFETY: callers must prove the raw thread id is
    // non-zero and has no page flag bits set before using this hot constructor.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(id: usize) -> Self {
        // SAFETY: the function safety contract requires
        // `id` to be non-zero, matching `NonZeroUsize::new_unchecked`.
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

/// Typed page-map coordinates derived from an arbitrary address without
/// dereferencing it.
#[cfg(target_pointer_width = "32")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageMapLocation {
    index: PageMapPrimaryIndex,
}

#[cfg(not(target_pointer_width = "32"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageMapLocation {
    index: PageMapPrimaryIndex,
    sub_index: PageMapSubIndex,
}

#[cfg(target_pointer_width = "32")]
impl PageMapLocation {
    #[inline(always)]
    pub(crate) fn from_ptr(ptr: *const c_void) -> Self {
        let raw = super::ptr::ptr_exposed_addr(ptr) >> crate::internal::MI_ARENA_SLICE_SHIFT;
        // Every address representable by a 32-bit pointer maps to one of the
        // complete flat map's slots.
        let index = super::scalar::PageMapFlatIndex(raw);
        Self { index }
    }

    #[inline(always)]
    pub(crate) const fn index(self) -> PageMapPrimaryIndex {
        self.index
    }
}

#[cfg(not(target_pointer_width = "32"))]
impl PageMapLocation {
    #[inline(always)]
    pub(crate) fn from_ptr(ptr: *const c_void) -> Option<Self> {
        let slice = super::ptr::ptr_exposed_addr(ptr) >> crate::internal::MI_ARENA_SLICE_SHIFT;
        let index =
            super::scalar::PageMapIndex::new(slice / crate::internal::MI_PAGE_MAP_SUB_COUNT)?;
        let sub_index = PageMapSubIndex::new(slice % crate::internal::MI_PAGE_MAP_SUB_COUNT)?;
        Some(Self { index, sub_index })
    }

    #[inline(always)]
    pub(crate) const fn index(self) -> PageMapPrimaryIndex {
        self.index
    }

    #[inline(always)]
    pub(crate) const fn sub_index(self) -> PageMapSubIndex {
        self.sub_index
    }
}

/// Evidence for one currently registered page-map range.
///
/// On 32-bit targets the primary index addresses the flat map and `sub_index`
/// is always zero. Registration lifetime remains a caller-proven invariant.
#[derive(Clone, Copy)]
pub(crate) struct RegisteredPageRange {
    page: PagePtr,
    index: PageMapPrimaryIndex,
    sub_index: PageMapSubIndex,
    slice_count: SliceCount,
}

impl RegisteredPageRange {
    /// # Safety
    /// The caller must have registered `page` in the page map for
    /// `index/sub_index..+slice_count`, and that registration must still be
    /// valid for the lifetime of this witness.
    // SAFETY: this constructor only packages the
    // caller's registered page-map range proof into a typed witness.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(
        page: PagePtr,
        index: PageMapPrimaryIndex,
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
    pub(crate) fn index(self) -> PageMapPrimaryIndex {
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

#[cfg(test)]
mod tests;
