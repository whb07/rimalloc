//! DT-14: Padding types.

use core::{ffi::c_void, ptr::NonNull};

use crate::core_types::{BlockPtr, PageKeys, UsableSize};
use crate::internal::mi_ptr_encode_canary;
use crate::types::mi_padding_t;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PaddingCanary(pub(crate) u32);

impl PaddingCanary {
    #[inline(always)]
    pub(crate) fn from_page_block_keys(
        page: *const c_void,
        block: *const c_void,
        keys: PageKeys,
    ) -> Self {
        Self(mi_ptr_encode_canary(page, block, &keys.0))
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
    pub(crate) fn for_usable(delta: usize, usable_size: UsableSize) -> Self {
        crate::mi_debug_assert!(delta <= usable_size.get());
        Self(delta as u32)
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
        Self(unsafe { NonNull::new_unchecked(ptr) })
    }

    /// # Safety
    /// `block` must point to a block with padding metadata at `block + usable_size`.
    #[inline(always)]
    pub(crate) unsafe fn from_block_usable(block: BlockPtr, usable_size: UsableSize) -> Self {
        let padding = unsafe {
            block
                .as_ptr()
                .cast::<u8>()
                .add(usable_size.get())
                .cast::<mi_padding_t>()
        };
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
            canary: PaddingCanary(unsafe { (*ptr).canary }),
            delta: PaddingDelta(unsafe { (*ptr).delta }),
        }
    }

    /// # Safety
    /// The padding memory must be valid for writes.
    #[inline(always)]
    pub(crate) unsafe fn write(self, padding: Padding) {
        let ptr = self.as_ptr();
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
        unsafe { (*self.as_ptr()).delta = delta.get() };
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
