//! DT-8: Free-list wrappers and EncodedPtr domain types.

use core::marker::PhantomData;

use crate::core_types::page_states::{AvailablePage, LocalPage, OwnedAbandonedPage};
use crate::core_types::validation_extra::{AllocatedBlock, FreeBlock};
use crate::core_types::{BlockPtr, PagePtr, ThreadFreeOwner, ThreadFreeWord};
use crate::internal::{mi_block_next, mi_block_set_next};
use crate::types::{mi_block_t, mi_page_t};

/// Keys used to encode and decode free-list pointers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageKeys(pub(crate) [usize; 2]);

impl PageKeys {
    pub(crate) const fn disabled() -> Self {
        Self([0, 0])
    }
}

/// An encoded pointer word.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct EncodedWord(pub(crate) usize);

/// An encoded pointer wrapper parameterized by its referent type.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodedPtr<T> {
    pub(crate) word: EncodedWord,
    _ty: PhantomData<*mut T>,
}

impl EncodedPtr<crate::types::mi_block_t> {
    /// Note: Actual encoding/decoding implementations will rely on `mi_ptr_decode` etc.,
    /// but the API contract starts here.
    pub(crate) fn decode_block(
        self,
        _null_context: crate::core_types::PagePtr,
        _keys: PageKeys,
    ) -> Option<BlockPtr> {
        // Implementation omitted in structural phase.
        unimplemented!()
    }

    pub(crate) fn encode_block(
        _block: Option<BlockPtr>,
        _null_context: crate::core_types::PagePtr,
        _keys: PageKeys,
    ) -> Self {
        // Implementation omitted in structural phase.
        unimplemented!()
    }
}

/// Defines whether a block link is a normal free-list link or guarded.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BlockTag(usize);

impl BlockTag {
    pub(crate) const ALIGNED: Self = Self(0);
    // Note: MI_BLOCK_TAG_GUARDED typically is !0 in C
    pub(crate) const GUARDED: Self = Self(!0);
}

pub(crate) enum BlockLink {
    FreeList(EncodedPtr<crate::types::mi_block_t>),
    Guarded(BlockTag),
}

/// The available free list of a page. This is `page.free`: the LIFO stack of
/// free blocks served by the allocation fast path.
///
/// `#[repr(transparent)]` over `Option<BlockPtr>` so it stays zero-cost and the
/// stored value is bit-identical to the raw `mi_block_t*` it replaces.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);

impl AvailableFreeList {
    /// Pop a block off the page's available free list, returning the allocated
    /// block and the updated list head. This is the allocation fast path.
    ///
    /// # Safety
    /// `page` must be a valid `AvailablePage` whose `free` field matches `self`,
    /// and the caller must subsequently store the returned list head back into
    /// `(*page).free` and increment `(*page).used`.
    #[inline(always)]
    pub(crate) unsafe fn pop(self, page: AvailablePage) -> (AllocatedBlock, AvailableFreeList) {
        let raw_page = page.0.as_ptr();
        let head = self.0.expect("available free list was empty on pop");
        let head_raw = head.as_ptr();
        // SAFETY: `head` is a valid block pointer in `page`'s free list; the page
        // keys and encoding are page-local and `head_raw` is non-null.
        let next_raw = unsafe { mi_block_next(raw_page, head_raw) };
        let next = if next_raw.is_null() {
            None
        } else {
            // SAFETY: `next_raw` is a non-null block in the same page.
            Some(unsafe { BlockPtr::new_unchecked(next_raw) })
        };
        (AllocatedBlock(head), AvailableFreeList(next))
    }

    /// Construct from a raw `mi_block_t*` head (the value of `(*page).free`).
    #[inline(always)]
    pub(crate) unsafe fn from_raw(head: *mut mi_block_t) -> Self {
        if head.is_null() {
            Self(None)
        } else {
            // SAFETY: caller guarantees `head` is a non-null block pointer.
            Self(Some(unsafe { BlockPtr::new_unchecked(head) }))
        }
    }
}

/// The local free list of a page. This is `page.local_free`: the LIFO stack
/// that freed blocks from the owning thread are pushed onto.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalFreeList(pub(crate) Option<BlockPtr>);

impl LocalFreeList {
    /// Push a freed block onto the page's local free list, returning the new
    /// list head. This is the local free fast path.
    ///
    /// # Safety
    /// `page` must be a valid `LocalPage` whose `local_free` field matches
    /// `self`, and the caller must subsequently store the returned head back
    /// into `(*page).local_free` and decrement `(*page).used`.
    #[inline(always)]
    pub(crate) unsafe fn push(self, page: LocalPage, block: FreeBlock) -> Self {
        let raw_page = page.0.as_ptr();
        let block_raw = block.0.as_ptr();
        let prev_raw = self.0.map(|b| b.as_ptr()).unwrap_or(core::ptr::null_mut());
        // SAFETY: `block_raw` is a valid block in `page`; `prev_raw` is the
        // current local_free head (null if empty). Encoding is page-local.
        unsafe { mi_block_set_next(raw_page, block_raw, prev_raw) };
        // SAFETY: `block_raw` is non-null.
        Self(Some(unsafe { BlockPtr::new_unchecked(block_raw) }))
    }

    /// Construct from a raw `mi_block_t*` head (the value of `(*page).local_free`).
    #[inline(always)]
    pub(crate) unsafe fn from_raw(head: *mut mi_block_t) -> Self {
        if head.is_null() {
            Self(None)
        } else {
            // SAFETY: caller guarantees `head` is a non-null block pointer.
            Self(Some(unsafe { BlockPtr::new_unchecked(head) }))
        }
    }
}

/// How forceful a free-list collection should be.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ForceCollect {
    No,
    Yes,
}

impl LocalFreeList {
    pub(crate) fn collect_into_available(
        self,
        _page: LocalPage,
        _available: AvailableFreeList,
        _force: ForceCollect,
    ) -> (AvailableFreeList, LocalFreeList) {
        unimplemented!()
    }
}

/// The remote free list collected from a thread-free word.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RemoteFreeList(pub(crate) Option<BlockPtr>);

impl RemoteFreeList {
    pub(crate) fn collect_into_local(
        self,
        _page: OwnedAbandonedPage,
        _local: LocalFreeList,
    ) -> LocalFreeList {
        unimplemented!()
    }
}

/// Represents an explicitly encoded free list representation.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodedFreeList(pub(crate) Option<BlockPtr>);
