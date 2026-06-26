//! DT-8: Free-list wrappers and EncodedPtr domain types.

use core::{ffi::c_void, marker::PhantomData, ptr};

use crate::core_types::BlockPtr;
use crate::core_types::page_states::{AvailablePage, LocalPage};
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

impl EncodedPtr<crate::types::mi_block_t> {
    /// Read the raw encoded free-list word stored in a block link.
    ///
    /// # Safety
    /// `block` must point to a valid block whose `next` field is accessible for
    /// the current tracking state.
    #[inline(always)]
    pub(crate) unsafe fn read_word_from_raw(block: *const mi_block_t) -> Self {
        Self {
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
        unsafe { (*block.as_ptr()).next = self.word.get() };
    }

    /// Wrap an unencoded raw block pointer as a free-list word.
    #[inline(always)]
    pub(crate) fn from_unencoded_raw(block: *const mi_block_t) -> Self {
        let word = crate::core_types::ptr_exposed_addr(block);
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
        crate::internal::mi_ptr_decode(null_context, self.word, &keys.0).cast()
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
        let raw = crate::internal::mi_ptr_decode(null_context.as_ptr().cast(), self.word, &keys.0);
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
        unsafe { Self::aligned().write_to(block) };
    }

    #[inline(always)]
    pub(crate) unsafe fn write_guarded_to(block: BlockPtr) {
        unsafe { Self::guarded().write_to(block) };
    }

    #[inline(always)]
    pub(crate) unsafe fn read_guarded_from_raw(
        block: *const mi_block_t,
    ) -> Option<GuardedBlockTag> {
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
        Self(unsafe { (*block.cast_mut()).next })
    }

    #[inline(always)]
    unsafe fn write_to(self, block: BlockPtr) {
        unsafe { (*block.as_ptr()).next = self.0 };
    }
}

impl AlignedBlockTag {
    #[inline(always)]
    unsafe fn write_to(self, block: BlockPtr) {
        unsafe { self.0.write_to(block) };
    }
}

impl GuardedBlockTag {
    #[inline(always)]
    unsafe fn write_to(self, block: BlockPtr) {
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
        if let Some(tag) = unsafe { BlockTag::read_guarded_from_raw(block) } {
            Self::Guarded(tag)
        } else {
            Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn is_guarded_raw(block: *const mi_block_t) -> bool {
        unsafe { BlockTag::read_guarded_from_raw(block).is_some() }
    }

    #[inline(always)]
    pub(crate) unsafe fn write_to(self, block: BlockPtr) {
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
pub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);

impl AvailableFreeList {
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
        let free = unsafe { AvailableFreeList::from_raw((*raw_page).free) };
        let head = unsafe { free.0.unwrap_unchecked() };
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
        (
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
        let block_raw = block.block().as_ptr();
        let prev_raw = self.as_raw();
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

/// Represents an explicitly encoded free list representation.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodedFreeList(pub(crate) Option<BlockPtr>);

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
mod tests {
    use super::*;
    use crate::core_types::{ActivePage, LocalPage, PagePtr};
    use crate::types::{mi_block_t, mi_page_t};
    use core::{mem::MaybeUninit, ptr};

    #[test]
    fn encoded_pointer_round_trips_block_and_null() {
        let mut page = MaybeUninit::<mi_page_t>::uninit();
        let mut block = mi_block_t::default();
        unsafe {
            let raw_page = page.as_mut_ptr();
            #[cfg(any(
                feature = "secure",
                debug_assertions,
                feature = "track-valgrind",
                feature = "track-asan"
            ))]
            ptr::addr_of_mut!((*raw_page).keys).write([0x1234_5678, 0x8765_4321]);
            let page_ptr = PagePtr::new_unchecked(raw_page);
            let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));

            let keys = page_ptr.keys();
            let encoded = EncodedPtr::encode_block(Some(block_ptr), page_ptr, keys);
            assert_eq!(
                encoded.decode_block(page_ptr, keys).map(BlockPtr::as_ptr),
                Some(block_ptr.as_ptr())
            );

            let encoded_null = EncodedPtr::encode_block(None, page_ptr, keys);
            assert!(encoded_null.decode_block(page_ptr, keys).is_none());
        }
    }

    #[test]
    fn block_link_read_from_raw_distinguishes_guarded_and_free_list_words() {
        let mut block = mi_block_t::default();
        unsafe {
            let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
            BlockTag::write_guarded_to(block_ptr);
            assert!(matches!(
                BlockLink::read_from_raw(ptr::addr_of!(block)),
                BlockLink::Guarded(_)
            ));

            let free_link = EncodedPtr::from_unencoded_raw(ptr::null());
            BlockLink::FreeList(free_link).write_to(block_ptr);
            assert!(matches!(
                BlockLink::read_from_raw(ptr::addr_of!(block)),
                BlockLink::FreeList(_)
            ));
        }
    }

    #[test]
    fn block_tag_explicit_aligned_write_is_not_guarded() {
        let mut block = mi_block_t::default();
        unsafe {
            let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
            BlockTag::write_guarded_to(block_ptr);
            assert!(BlockLink::is_guarded_raw(ptr::addr_of!(block)));

            BlockTag::write_aligned_to(block_ptr);
            assert!(!BlockLink::is_guarded_raw(ptr::addr_of!(block)));
        }
    }

    #[test]
    fn available_pop_consumes_available_page_witness() {
        let mut page = MaybeUninit::<mi_page_t>::uninit();
        let mut block = mi_block_t::default();
        unsafe {
            let raw_page = page.as_mut_ptr();
            #[cfg(any(
                feature = "secure",
                debug_assertions,
                feature = "track-valgrind",
                feature = "track-asan"
            ))]
            ptr::addr_of_mut!((*raw_page).keys).write([0x1234_5678, 0x8765_4321]);
            ptr::addr_of_mut!((*raw_page).free).write(ptr::addr_of_mut!(block));
            ptr::addr_of_mut!((*raw_page).used).write(0);
            mi_block_set_next(raw_page, ptr::addr_of_mut!(block), ptr::null());

            let active = ActivePage::new_unchecked(PagePtr::new_unchecked(raw_page));
            let available = active
                .available()
                .expect("free head creates available page");
            let (allocated, next) = AvailableFreeList::pop(available);

            assert_eq!(allocated.block().as_ptr(), ptr::addr_of_mut!(block));
            assert!(next.0.is_none());
        }
    }

    #[test]
    fn local_push_requires_free_block_witness() {
        let mut page = MaybeUninit::<mi_page_t>::uninit();
        let mut block = mi_block_t::default();
        unsafe {
            let raw_page = page.as_mut_ptr();
            #[cfg(any(
                feature = "secure",
                debug_assertions,
                feature = "track-valgrind",
                feature = "track-asan"
            ))]
            ptr::addr_of_mut!((*raw_page).keys).write([0x1234_5678, 0x8765_4321]);
            ptr::addr_of_mut!((*raw_page).local_free).write(ptr::null_mut());

            let page_ptr = PagePtr::new_unchecked(raw_page);
            let local_page = LocalPage::new_unchecked(page_ptr);
            let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
            let free = FreeBlock::new_unchecked(block_ptr);
            let list = LocalFreeList::from_raw(ptr::null_mut()).push(local_page, free);

            assert_eq!(list.0.map(BlockPtr::as_ptr), Some(block_ptr.as_ptr()));
            assert!(mi_block_next(raw_page, block_ptr.as_ptr()).is_null());
        }
    }
}
