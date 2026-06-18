//! DT-14: Padding types.

use core::ptr::NonNull;

use crate::core_types::{BlockPtr, UsableSize};
use crate::types::mi_padding_t;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PaddingCanary(pub(crate) u32);

impl PaddingCanary {
    #[inline(always)]
    pub(crate) const fn new(canary: u32) -> Self {
        Self(canary)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct PaddingDelta(pub(crate) u32);

impl PaddingDelta {
    #[inline(always)]
    pub(crate) const fn new(delta: u32) -> Self {
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
        Self(NonNull::new_unchecked(ptr))
    }

    /// # Safety
    /// `block` must point to a block with padding metadata at `block + usable_size`.
    #[inline(always)]
    pub(crate) unsafe fn from_block_usable(block: BlockPtr, usable_size: UsableSize) -> Self {
        let padding = block
            .as_ptr()
            .cast::<u8>()
            .add(usable_size.get())
            .cast::<mi_padding_t>();
        Self::new_unchecked(padding)
    }

    #[inline(always)]
    pub(crate) fn as_ptr(self) -> *mut mi_padding_t {
        self.0.as_ptr()
    }

    /// # Safety
    /// The padding memory must be valid for reads.
    #[inline(always)]
    pub(crate) unsafe fn read(self) -> Padding {
        Padding {
            canary: PaddingCanary((*self.as_ptr()).canary),
            delta: PaddingDelta((*self.as_ptr()).delta),
        }
    }

    /// # Safety
    /// The padding memory must be valid for writes.
    #[inline(always)]
    pub(crate) unsafe fn write(self, padding: Padding) {
        (*self.as_ptr()).canary = padding.canary.get();
        (*self.as_ptr()).delta = padding.delta.get();
    }

    /// # Safety
    /// The padding memory must be valid for writes.
    #[inline(always)]
    pub(crate) unsafe fn write_delta(self, delta: PaddingDelta) {
        (*self.as_ptr()).delta = delta.get();
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Padding {
    pub(crate) canary: PaddingCanary,
    pub(crate) delta: PaddingDelta,
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
