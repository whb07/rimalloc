//! DT-8: Free-list wrappers and EncodedPtr domain types.

use core::{ffi::c_void, marker::PhantomData};

use crate::core_types::BlockPtr;
use crate::core_types::page_states::{AvailablePage, LocalPage, OwnedAbandonedPage};
use crate::core_types::validation_extra::{AllocatedBlock, FreeBlock};
use crate::internal::{mi_block_next, mi_block_set_next};
use crate::types::mi_block_t;

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
    /// Read the raw encoded free-list word stored in a block link.
    ///
    /// # Safety
    /// `block` must point to a valid block whose `next` field is accessible for
    /// the current tracking state.
    #[inline(always)]
    pub(crate) unsafe fn read_word_from_raw(block: *const mi_block_t) -> Self {
        Self {
            word: EncodedWord(unsafe { (*block.cast_mut()).next }),
            _ty: PhantomData,
        }
    }

    /// Write the raw encoded free-list word into a block link.
    ///
    /// # Safety
    /// `block` must point to a valid block whose `next` field is accessible for
    /// the current tracking state.
    #[inline(always)]
    pub(crate) unsafe fn write_word_to_raw(self, block: BlockPtr) {
        unsafe { (*block.as_ptr()).next = self.word.0 };
    }

    /// Wrap an unencoded raw block pointer as a free-list word.
    #[inline(always)]
    pub(crate) fn from_unencoded_raw(block: *const mi_block_t) -> Self {
        Self {
            word: EncodedWord(block as usize),
            _ty: PhantomData,
        }
    }

    /// Interpret this word as an unencoded raw block pointer.
    #[inline(always)]
    pub(crate) fn as_unencoded_raw(self) -> *mut mi_block_t {
        self.word.0 as *mut mi_block_t
    }

    /// Decode an encoded block-pointer word using a raw null context.
    ///
    /// # Safety
    /// `null_context` and `keys` must belong to the page that owns this block
    /// link.
    #[inline(always)]
    pub(crate) unsafe fn decode_block_raw(
        self,
        null_context: *const c_void,
        keys: PageKeys,
    ) -> *mut mi_block_t {
        crate::internal::mi_ptr_decode(null_context, self.word.0, &keys.0).cast()
    }

    /// Encode a raw block pointer using a raw null context.
    ///
    /// # Safety
    /// `null_context` and `keys` must belong to the page that owns this block
    /// link.
    #[inline(always)]
    pub(crate) unsafe fn encode_block_raw(
        block: *const mi_block_t,
        null_context: *const c_void,
        keys: PageKeys,
    ) -> Self {
        let word = crate::internal::mi_ptr_encode(null_context, block.cast(), &keys.0);
        Self {
            word: EncodedWord(word),
            _ty: PhantomData,
        }
    }

    /// Decode an encoded block-pointer word back to a `BlockPtr`.
    ///
    /// Returns `None` if the decoded value is the page's null sentinel. This is
    /// the low-level encoding primitive; the block-level free-list wrappers
    /// (`AvailableFreeList`/`LocalFreeList`) use `mi_block_next`/`mi_block_set_next`
    /// which call this under the hood.
    ///
    /// # Safety
    /// `null_context` must be the page's null sentinel base and `keys` the page's
    /// encoding keys; mismatched keys yield a garbage pointer, not UB, but break
    /// the free list.
    #[inline]
    pub(crate) unsafe fn decode_block(
        self,
        null_context: crate::core_types::PagePtr,
        keys: PageKeys,
    ) -> Option<BlockPtr> {
        let raw =
            crate::internal::mi_ptr_decode(null_context.as_ptr().cast(), self.word.0, &keys.0);
        if raw.is_null() {
            None
        } else {
            // SAFETY: `raw` is a non-null decoded block pointer.
            Some(unsafe { BlockPtr::new_unchecked(raw.cast()) })
        }
    }

    /// Encode a block pointer (or `None` for null) into an encoded word.
    ///
    /// # Safety
    /// See `decode_block`; `null_context` and `keys` must belong to the page.
    #[inline]
    pub(crate) unsafe fn encode_block(
        block: Option<BlockPtr>,
        null_context: crate::core_types::PagePtr,
        keys: PageKeys,
    ) -> Self {
        let p = block
            .map(|b| b.as_ptr().cast::<core::ffi::c_void>())
            .unwrap_or(core::ptr::null_mut());
        let word = crate::internal::mi_ptr_encode(null_context.as_ptr().cast(), p, &keys.0);
        Self {
            word: EncodedWord(word),
            _ty: PhantomData,
        }
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

    #[inline(always)]
    pub(crate) const fn is_guarded(self) -> bool {
        self.0 == Self::GUARDED.0
    }

    #[inline(always)]
    pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self {
        Self(unsafe { (*block.cast_mut()).next })
    }

    #[inline(always)]
    pub(crate) unsafe fn write_to(self, block: BlockPtr) {
        unsafe { (*block.as_ptr()).next = self.0 };
    }
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
        // SAFETY: the caller (`mi_page_malloc_zero`) checks `(*page).free` for
        // null before constructing this `AvailableFreeList`, so `self.0` is
        // always `Some` here. Using `unwrap_unchecked` keeps the hot path free
        // of a panic branch.
        let head = unsafe { self.0.unwrap_unchecked() };
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
    /// Move the local free list into the page's available (`free`) list.
    ///
    /// Mirrors step 2 of `_mi_page_free_collect` (the local→available move/splice).
    /// Does NOT collect the remote thread-free list — that is
    /// `RemoteFreeList::collect_into_local`. The caller is responsible for
    /// writing the returned `(available, local)` back into `(*page).free` /
    /// `(*page).local_free` and clearing `free_is_zero` if applicable.
    ///
    /// # Safety
    /// `page` must be a valid `LocalPage` whose `local_free` field matches
    /// `self` and whose `free` field matches `available`.
    #[inline]
    pub(crate) unsafe fn collect_into_available(
        self,
        page: LocalPage,
        available: AvailableFreeList,
        force: ForceCollect,
    ) -> (AvailableFreeList, LocalFreeList) {
        let raw_page = page.0.as_ptr();
        // If local is empty, nothing to move.
        if self.0.is_none() {
            return (available, self);
        }
        match (available.0, force) {
            // available is empty: move local -> available directly.
            (None, _) => (AvailableFreeList(self.0), LocalFreeList(None)),
            // available non-empty, not forced: leave unchanged (matches the
            // `else` branch of `_mi_page_free_collect` which skips the merge).
            (Some(_), ForceCollect::No) => (available, self),
            // available non-empty, forced: splice local tail -> available head,
            // then move local -> available.
            (Some(avail_head), ForceCollect::Yes) => {
                let Some(local_head) = self.0.map(|head| head.as_ptr()) else {
                    return (available, self);
                };
                let mut tail = local_head;
                loop {
                    let next = unsafe { mi_block_next(raw_page, tail) };
                    if next.is_null() {
                        break;
                    }
                    tail = next;
                }
                unsafe { mi_block_set_next(raw_page, tail, avail_head.as_ptr()) };
                (AvailableFreeList(self.0), LocalFreeList(None))
            }
        }
    }
}

/// The remote free list collected from a thread-free word.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RemoteFreeList(pub(crate) Option<BlockPtr>);

impl RemoteFreeList {
    /// Collect the remote (thread-free) list into the page's local free list.
    ///
    /// Delegates to `mi_page_thread_collect_to_local`, which splices `self`'s
    /// head into `(*page).local_free`, runs the corruption/length checks, and
    /// decrements `(*page).used`. Returns the updated local free list read back
    /// from the page. The `_local` argument is ignored because the raw function
    /// reads `(*page).local_free` directly; the caller must ensure it matches.
    ///
    /// Constrained to `OwnedAbandonedPage` (the reclaim path); regular-page
    /// collection uses the raw `_mi_page_free_collect` entry point directly.
    ///
    /// # Safety
    /// `page` must be a valid owned abandoned page; `self` must be the head
    /// popped from `(*page).xthread_free` via the ownership CAS protocol.
    #[inline]
    pub(crate) unsafe fn collect_into_local(
        self,
        page: OwnedAbandonedPage,
        _local: LocalFreeList,
    ) -> LocalFreeList {
        let raw_page = page.0.as_ptr();
        let head = self.0.map(|b| b.as_ptr()).unwrap_or(core::ptr::null_mut());
        if !head.is_null() {
            unsafe { crate::page::mi_page_thread_collect_to_local(raw_page, head) };
        }
        unsafe { LocalFreeList::from_raw((*raw_page).local_free) }
    }
}

/// Represents an explicitly encoded free list representation.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodedFreeList(pub(crate) Option<BlockPtr>);
