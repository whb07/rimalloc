//! DT-8: Free-list wrappers and EncodedPtr domain types.

use core::marker::PhantomData;

use crate::core_types::page_states::{AvailablePage, LocalPage, OwnedAbandonedPage};
use crate::core_types::validation_extra::{AllocatedBlock, FreeBlock};
use crate::core_types::{BlockPtr, ThreadFreeOwner, ThreadFreeWord};

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

/// The available free list of a page.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);

impl AvailableFreeList {
    pub(crate) fn pop(self, _page: AvailablePage) -> (AllocatedBlock, AvailableFreeList) {
        unimplemented!()
    }

    pub(crate) fn push(self, _page: LocalPage, _block: FreeBlock) -> Self {
        unimplemented!()
    }
}

/// The local free list of a page.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalFreeList(pub(crate) Option<BlockPtr>);

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
