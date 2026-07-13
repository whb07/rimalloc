//! DT-8: Free-list wrappers and EncodedPtr domain types.

use core::{ffi::c_void, marker::PhantomData, ptr};

use super::constants::mi_block_t;
use super::page_states::{AvailablePage, LocalPage};
use super::ptr::{BlockPtr, ptr_exposed_addr};
use super::validation_extra::{AllocatedBlock, FreeBlock};
use crate::internal::{mi_block_next, mi_block_set_next};

/// Keys used to encode and decode free-list pointers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PageKeys([usize; 2]);

impl PageKeys {
    #[inline(always)]
    pub(crate) const fn from_raw(keys: [usize; 2]) -> Self {
        Self(keys)
    }

    pub(crate) const fn disabled() -> Self {
        Self([0, 0])
    }

    #[inline(always)]
    pub(crate) const fn as_raw(&self) -> &[usize; 2] {
        &self.0
    }
}

/// An encoded pointer word.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct EncodedWord(usize);

impl EncodedWord {
    #[inline(always)]
    pub(crate) const fn new(word: usize) -> Self {
        Self(word)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// An encoded pointer wrapper parameterized by its referent type.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodedPtr<T> {
    word: EncodedWord,
    _ty: PhantomData<*mut T>,
}

impl EncodedPtr<mi_block_t> {
    /// Read the raw encoded free-list word stored in a block link.
    ///
    /// # Safety
    /// `block` must point to a valid block whose `next` field is accessible for
    /// the current tracking state.
    #[inline(always)]
    pub(crate) unsafe fn read_word_from_raw(block: *const mi_block_t) -> Self {
        Self {
            // SAFETY[AUDIT:typed-freelist-encoded-link-wrappers]: callers provide a valid
            // block link word for the current tracking state; this reads only
            // the upstream-compatible `next` word.
            word: EncodedWord::new(unsafe { (*block.cast_mut()).next }),
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
        // SAFETY[AUDIT:typed-freelist-encoded-link-wrappers]: `block` is a non-null block
        // witness and callers guarantee its link word is currently writable.
        unsafe { (*block.as_ptr()).next = self.word.get() };
    }

    /// Wrap an unencoded raw block pointer as a free-list word.
    #[inline(always)]
    pub(crate) fn from_unencoded_raw(block: *const mi_block_t) -> Self {
        let word = ptr_exposed_addr(block);
        crate::provenance::remember_address(word, block);
        Self {
            word: EncodedWord::new(word),
            _ty: PhantomData,
        }
    }

    /// Interpret this word as an unencoded raw block pointer.
    #[inline(always)]
    pub(crate) fn as_unencoded_raw(self) -> *mut mi_block_t {
        let word = self.word.get();
        if word == 0 {
            return ptr::null_mut();
        }

        #[cfg(miri)]
        {
            crate::provenance::resolve_address(word)
                .unwrap_or_else(|| crate::provenance::missing_address(word))
        }

        #[cfg(not(miri))]
        {
            word as *mut mi_block_t
        }
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
        crate::internal::mi_ptr_decode(null_context, self.word, keys.as_raw()).cast()
    }

    /// Encode a raw block pointer using a raw null context.
    ///
    /// # Safety
    /// `null_context` and `keys` must belong to the page that owns this block
    /// link.
    // Retained for secure encoded free-list builds; routine non-secure builds
    // use the unencoded path.
    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) unsafe fn encode_block_raw(
        block: *const mi_block_t,
        null_context: *const c_void,
        keys: PageKeys,
    ) -> Self {
        let word = crate::internal::mi_ptr_encode(null_context, block.cast(), keys.as_raw());
        Self {
            word,
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
            crate::internal::mi_ptr_decode(null_context.as_ptr().cast(), self.word, keys.as_raw());
        if raw.is_null() {
            None
        } else {
            // SAFETY[AUDIT:typed-freelist-encoded-link-wrappers]: `raw` is a non-null
            // decoded block pointer for the page/key context supplied by the
            // caller.
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
        let word = crate::internal::mi_ptr_encode(null_context.as_ptr().cast(), p, keys.as_raw());
        Self {
            word,
            _ty: PhantomData,
        }
    }
}

/// Defines whether a block link is a normal free-list link or guarded.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BlockTag(usize);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct AlignedBlockTag(BlockTag);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct GuardedBlockTag(BlockTag);

impl BlockTag {
    const ALIGNED: Self = Self(0);
    // Note: MI_BLOCK_TAG_GUARDED typically is !0 in C
    const GUARDED: Self = Self(!0);

    #[inline(always)]
    pub(crate) const fn aligned() -> AlignedBlockTag {
        AlignedBlockTag(Self::ALIGNED)
    }

    #[inline(always)]
    pub(crate) const fn guarded() -> GuardedBlockTag {
        GuardedBlockTag(Self::GUARDED)
    }

    #[inline(always)]
    pub(crate) unsafe fn write_aligned_to(block: BlockPtr) {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: callers provide a block whose
        // link word may be tagged, and `block` is already non-null.
        unsafe { Self::aligned().write_to(block) };
    }

    #[inline(always)]
    pub(crate) unsafe fn write_guarded_to(block: BlockPtr) {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: callers provide a block whose
        // link word may be tagged, and `block` is already non-null.
        unsafe { Self::guarded().write_to(block) };
    }

    #[inline(always)]
    pub(crate) unsafe fn read_guarded_from_raw(
        block: *const mi_block_t,
    ) -> Option<GuardedBlockTag> {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: callers provide a readable
        // block link word; the raw tag read is immediately classified.
        let tag = unsafe { Self::read_from_raw(block) };
        if tag.is_guarded() {
            Some(GuardedBlockTag(tag))
        } else {
            None
        }
    }

    #[inline(always)]
    const fn is_guarded(self) -> bool {
        self.0 == Self::GUARDED.0
    }

    #[inline(always)]
    unsafe fn read_from_raw(block: *const mi_block_t) -> Self {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: callers provide a readable
        // block link word and this wrapper preserves the raw tag domain.
        Self(unsafe { (*block.cast_mut()).next })
    }

    #[inline(always)]
    unsafe fn write_to(self, block: BlockPtr) {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: `block` is non-null and
        // callers guarantee its link word can receive this tag value.
        unsafe { (*block.as_ptr()).next = self.0 };
    }
}

impl AlignedBlockTag {
    #[inline(always)]
    unsafe fn write_to(self, block: BlockPtr) {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: the aligned tag delegates to
        // the raw tag writer with the caller's writable block proof.
        unsafe { self.0.write_to(block) };
    }
}

impl GuardedBlockTag {
    #[inline(always)]
    unsafe fn write_to(self, block: BlockPtr) {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: the guarded tag delegates to
        // the raw tag writer with the caller's writable block proof.
        unsafe { self.0.write_to(block) };
    }
}

pub(crate) enum BlockLink {
    FreeList(EncodedPtr<crate::types::mi_block_t>),
    Guarded(GuardedBlockTag),
}

impl BlockLink {
    #[inline(always)]
    pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: callers provide a readable
        // block link word; the guarded tag probe decides whether the word is a
        // tag or an encoded free-list link.
        if let Some(tag) = unsafe { BlockTag::read_guarded_from_raw(block) } {
            Self::Guarded(tag)
        } else {
            // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: the guarded probe above
            // ruled out the reserved guarded tag, so the same link word is
            // interpreted as an encoded free-list pointer.
            Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn is_guarded_raw(block: *const mi_block_t) -> bool {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: callers provide a readable
        // block link word; this only classifies whether it is guarded.
        unsafe { BlockTag::read_guarded_from_raw(block).is_some() }
    }

    #[inline(always)]
    pub(crate) unsafe fn write_to(self, block: BlockPtr) {
        // SAFETY[AUDIT:typed-freelist-block-tag-wrappers]: `block` is non-null and
        // callers guarantee its link word can receive either encoded link or
        // guarded tag state.
        match self {
            Self::FreeList(link) => unsafe { link.write_word_to_raw(block) },
            Self::Guarded(tag) => unsafe { tag.write_to(block) },
        }
    }
}

/// The available free list of a page. This is `page.free`: the LIFO stack of
/// free blocks served by the allocation fast path.
///
/// `#[repr(transparent)]` over `Option<BlockPtr>` so it stays zero-cost and the
/// stored value is bit-identical to the raw `mi_block_t*` it replaces.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct AvailableFreeList(Option<BlockPtr>);

impl AvailableFreeList {
    #[inline(always)]
    pub(crate) fn from_head(head: BlockPtr) -> Self {
        Self(Some(head))
    }

    #[inline(always)]
    pub(crate) fn is_empty(self) -> bool {
        self.0.is_none()
    }

    #[inline(always)]
    pub(crate) fn head(self) -> Option<BlockPtr> {
        self.0
    }

    /// Return the raw nullable head stored in `(*page).free`.
    #[inline(always)]
    pub(crate) fn as_raw(self) -> *mut mi_block_t {
        self.0.map(BlockPtr::as_ptr).unwrap_or(ptr::null_mut())
    }

    /// Pop a block off the page's available free list, returning the allocated
    /// block and the updated list head. This is the allocation fast path.
    ///
    /// # Safety
    /// `page` must be a valid `AvailablePage`; the caller must subsequently
    /// store the returned list head back into `(*page).free` and increment
    /// `(*page).used`.
    #[inline(always)]
    pub(crate) unsafe fn pop(page: AvailablePage) -> (AllocatedBlock, AvailableFreeList) {
        let raw_page = page.page().as_ptr();
        // SAFETY[AUDIT:typed-freelist-available-list-wrappers]: `page` proves the page has an
        // available free-list head, so reading the raw `free` field is valid.
        let free = unsafe { AvailableFreeList::from_raw((*raw_page).free) };
        // SAFETY[AUDIT:typed-freelist-available-list-wrappers]: `AvailablePage` guarantees the
        // free-list head is present on the allocation fast path.
        let head = unsafe { free.0.unwrap_unchecked() };
        let head_raw = head.as_ptr();
        // SAFETY[AUDIT:typed-freelist-available-list-wrappers]: `head` is a valid block
        // pointer in `page`'s free list; the page keys and encoding are
        // page-local and `head_raw` is non-null.
        let next_raw = unsafe { mi_block_next(raw_page, head_raw) };
        let next = if next_raw.is_null() {
            None
        } else {
            // SAFETY[AUDIT:typed-freelist-available-list-wrappers]: `next_raw` is a non-null
            // block in the same page's available free-list domain.
            Some(unsafe { BlockPtr::new_unchecked(next_raw) })
        };
        (
            // SAFETY[AUDIT:typed-freelist-available-list-wrappers]: the popped head came from
            // the available free-list, so it becomes the allocated-block
            // witness returned to the caller.
            unsafe { AllocatedBlock::new_unchecked(head) },
            AvailableFreeList(next),
        )
    }

    /// Construct from a raw `mi_block_t*` head (the value of `(*page).free`).
    #[inline(always)]
    pub(crate) unsafe fn from_raw(head: *mut mi_block_t) -> Self {
        if head.is_null() {
            Self(None)
        } else {
            // SAFETY[AUDIT:typed-freelist-available-list-wrappers]: caller guarantees `head`
            // is a non-null block pointer from the page's available list.
            Self(Some(unsafe { BlockPtr::new_unchecked(head) }))
        }
    }
}

/// The local free list of a page. This is `page.local_free`: the LIFO stack
/// that freed blocks from the owning thread are pushed onto.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalFreeList(Option<BlockPtr>);

impl LocalFreeList {
    #[inline(always)]
    pub(crate) const fn empty() -> Self {
        Self(None)
    }

    #[inline(always)]
    pub(crate) fn from_head(head: BlockPtr) -> Self {
        Self(Some(head))
    }

    #[inline(always)]
    pub(crate) fn is_empty(self) -> bool {
        self.0.is_none()
    }

    #[inline(always)]
    pub(crate) fn into_available(self) -> AvailableFreeList {
        AvailableFreeList(self.0)
    }

    /// Return the raw nullable head stored in `(*page).local_free`.
    #[inline(always)]
    pub(crate) fn as_raw(self) -> *mut mi_block_t {
        self.0.map(BlockPtr::as_ptr).unwrap_or(ptr::null_mut())
    }

    /// Push a freed block onto the page's local free list, returning the new
    /// list head. This is the local free fast path.
    ///
    /// # Safety
    /// `page` must be a valid `LocalPage` whose `local_free` field matches
    /// `self`, and the caller must subsequently store the returned head back
    /// into `(*page).local_free` and decrement `(*page).used`.
    #[inline(always)]
    pub(crate) unsafe fn push(self, page: LocalPage, block: FreeBlock) -> Self {
        let raw_page = page.page().as_ptr();
        let block_raw = block.as_ptr().as_ptr();
        let prev_raw = self.as_raw();
        // SAFETY[AUDIT:typed-freelist-local-list-wrappers]: `block_raw` is a valid block
        // in `page`; `prev_raw` is the current local_free head (null if empty).
        // Encoding is page-local.
        unsafe { mi_block_set_next(raw_page, block_raw, prev_raw) };
        // SAFETY[AUDIT:typed-freelist-local-list-wrappers]: `block_raw` came from a
        // FreeBlock witness and is therefore non-null.
        Self(Some(unsafe { BlockPtr::new_unchecked(block_raw) }))
    }

    /// Prepend this local free list to the available free list, returning the
    /// new available-list head.
    ///
    /// # Safety
    /// `page` must be the local owner for both lists, and `self` must describe
    /// the current `page.local_free` chain. The caller remains responsible for
    /// storing the returned available head and clearing `page.local_free`.
    #[inline]
    pub(crate) unsafe fn prepend_to_available(
        self,
        page: LocalPage,
        available: AvailableFreeList,
    ) -> AvailableFreeList {
        let Some(head) = self.0 else {
            return available;
        };
        if available.is_empty() {
            return AvailableFreeList::from_head(head);
        }

        let raw_page = page.page().as_ptr();
        let mut tail = head.as_ptr();
        loop {
            // SAFETY[AUDIT:typed-freelist-local-list-wrappers]: `self` is the
            // page's local free-list chain, so walking encoded next links until
            // the null tail stays within the page-local free-list domain.
            let next = unsafe { mi_block_next(raw_page, tail) };
            if next.is_null() {
                break;
            }
            tail = next;
        }
        // SAFETY[AUDIT:typed-freelist-local-list-wrappers]: `tail` is the null
        // terminator block of the local list and `available` is the current
        // page.free head, so linking them preserves page-local encoding.
        unsafe { mi_block_set_next(raw_page, tail, available.as_raw()) };
        AvailableFreeList::from_head(head)
    }

    /// Construct from a raw `mi_block_t*` head (the value of `(*page).local_free`).
    #[inline(always)]
    pub(crate) unsafe fn from_raw(head: *mut mi_block_t) -> Self {
        if head.is_null() {
            Self(None)
        } else {
            // SAFETY[AUDIT:typed-freelist-local-list-wrappers]: caller guarantees `head`
            // is a non-null block pointer from the page's local free-list.
            Self(Some(unsafe { BlockPtr::new_unchecked(head) }))
        }
    }
}

/// Represents an explicitly encoded free list representation.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodedFreeList(Option<BlockPtr>);

pub(super) fn __lint_touch() {
    let _ = PageKeys::disabled();
    let _decode: unsafe fn(
        EncodedPtr<crate::types::mi_block_t>,
        crate::core_types::PagePtr,
        PageKeys,
    ) -> Option<BlockPtr> = EncodedPtr::<crate::types::mi_block_t>::decode_block;
    let _read: unsafe fn(*const mi_block_t) -> BlockLink = BlockLink::read_from_raw;
    let _ = core::mem::size_of::<EncodedFreeList>();
}

#[cfg(test)]
mod tests;
