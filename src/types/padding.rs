//! DT-14: Padding types.

use core::ptr::NonNull;

use super::constants::mi_padding_t;
use super::freelist_wrappers::PageKeys;
use super::ptr::{BlockPtr, PagePtr};
use super::scalar::UsableSize;
use crate::internal::mi_ptr_encode_canary;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PaddingCanary(u32);

impl PaddingCanary {
    #[inline(always)]
    pub(crate) fn from_page_block_keys(page: PagePtr, block: BlockPtr, keys: PageKeys) -> Self {
        Self(mi_ptr_encode_canary(
            page.as_ptr().cast(),
            block.as_ptr().cast(),
            &keys.0,
        ))
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct PaddingDelta(u32);

impl PaddingDelta {
    #[inline(always)]
    pub(crate) fn for_usable(delta: usize, usable_size: UsableSize) -> Option<Self> {
        if delta > usable_size.get() {
            return None;
        }
        u32::try_from(delta).ok().map(Self)
    }

    #[inline(always)]
    pub(crate) const fn new_unchecked(delta: u32) -> Self {
        Self(delta)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PaddingPtr(pub(crate) NonNull<mi_padding_t>);

impl PaddingPtr {
    pub(crate) const SIZE: usize = core::mem::size_of::<mi_padding_t>();

    /// # Safety
    /// `ptr` must be non-null, properly aligned, and point to live padding metadata.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(ptr: *mut mi_padding_t) -> Self {
        // SAFETY[AUDIT:typed-padding-metadata]: callers uphold non-null,
        // alignment, and liveness for padding metadata pointer witnesses.
        Self(unsafe { NonNull::new_unchecked(ptr) })
    }

    /// # Safety
    /// `block` must point to a block with padding metadata at `block + usable_size`.
    #[inline(always)]
    pub(crate) unsafe fn from_block_usable(block: BlockPtr, usable_size: UsableSize) -> Self {
        // SAFETY[AUDIT:typed-padding-metadata]: callers supply a live block
        // whose usable-size boundary is followed by padding metadata.
        let padding = unsafe {
            block
                .as_ptr()
                .cast::<u8>()
                .add(usable_size.get())
                .cast::<mi_padding_t>()
        };
        // SAFETY[AUDIT:typed-padding-metadata]: the derived padding pointer is
        // non-null and aligned by the block plus usable-size padding contract.
        unsafe { Self::new_unchecked(padding) }
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *mut mi_padding_t {
        self.0.as_ptr()
    }

    /// # Safety
    /// The padding memory must be valid for reads.
    #[inline(always)]
    pub(crate) unsafe fn read(self) -> Padding {
        let ptr = self.as_ptr();
        Padding {
            // SAFETY[AUDIT:typed-padding-metadata]: callers guarantee readable
            // live padding metadata, and this copies the raw canary field into
            // the typed scalar wrapper.
            canary: PaddingCanary(unsafe { (*ptr).canary }),
            // SAFETY[AUDIT:typed-padding-metadata]: callers guarantee readable
            // live padding metadata, and this copies the raw delta field into
            // the typed scalar wrapper.
            delta: PaddingDelta(unsafe { (*ptr).delta }),
        }
    }

    /// # Safety
    /// The padding memory must be valid for writes.
    #[inline(always)]
    pub(crate) unsafe fn write(self, padding: Padding) {
        let ptr = self.as_ptr();
        // SAFETY[AUDIT:typed-padding-metadata]: callers guarantee writable live
        // padding metadata, and values are supplied through typed wrappers.
        unsafe {
            (*ptr).canary = padding.canary.get();
            (*ptr).delta = padding.delta.get();
        }
    }

    /// # Safety
    /// The padding memory must be valid for writes.
    #[cfg(not(mi_c_malloc_fast))]
    #[inline(always)]
    pub(crate) unsafe fn write_delta(self, delta: PaddingDelta) {
        // SAFETY[AUDIT:typed-padding-metadata]: callers guarantee writable live
        // padding metadata; only the typed delta field is updated.
        unsafe { (*self.as_ptr()).delta = delta.get() };
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Padding {
    canary: PaddingCanary,
    delta: PaddingDelta,
}

impl Padding {
    #[inline(always)]
    pub(crate) const fn new(canary: PaddingCanary, delta: PaddingDelta) -> Self {
        Self { canary, delta }
    }

    #[inline(always)]
    pub(crate) const fn canary(self) -> PaddingCanary {
        self.canary
    }

    #[inline(always)]
    pub(crate) const fn delta(self) -> PaddingDelta {
        self.delta
    }
}

#[cfg(test)]
mod tests;
