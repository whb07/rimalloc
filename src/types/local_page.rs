//! Scoped page geometry and exclusive storage returned by a local free-list pop.

use core::{marker::PhantomData, mem::size_of, ptr};

use super::{
    ActivePage, AvailablePage, BlockPtr, PaddedSize, PageKeys, PagePtr, UsableSize, Zeroed,
};
use crate::allocator::tracking::{MI_TRACK_ENABLED, mi_track_mem_defined, mi_track_mem_undefined};
use crate::types::{MI_MAX_ALIGN_SIZE, MI_PADDING, MI_PADDING_CHECK, MI_PADDING_SIZE, mi_block_t};

pub(crate) trait PageAccessState {
    fn page(&self) -> PagePtr;
}

impl PageAccessState for AvailablePage {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        self.page()
    }
}

impl PageAccessState for ActivePage {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        self.page()
    }
}

#[repr(transparent)]
pub(crate) struct PageAccess<'scope, S> {
    state: S,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

#[repr(transparent)]
pub(crate) struct PageRequest<'scope> {
    padded: PaddedSize,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl AvailablePage {
    /// Lend this page and its fitting request during one allocation phase.
    ///
    /// # Safety
    /// The initialized page, page map, encoding keys and committed block storage
    /// must remain live and stable through `f`. The caller owns local list and
    /// accounting mutation; remote publication may follow the atomic protocol.
    /// `padded` must include `MI_PADDING_SIZE` and fit this page's complete
    /// block size. Callbacks must not reenter this page while its loan is active.
    #[inline(always)]
    pub(crate) unsafe fn with_allocation_access<R>(
        self,
        padded: PaddedSize,
        f: impl for<'scope> FnOnce(PageAccess<'scope, Self>, PageRequest<'scope>) -> R,
    ) -> R {
        f(
            PageAccess {
                state: self,
                _scope: PhantomData,
            },
            PageRequest {
                padded,
                _scope: PhantomData,
            },
        )
    }
}

impl<S: PageAccessState> PageAccess<'_, S> {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        self.state.page()
    }

    #[inline(always)]
    pub(crate) fn block_size_raw(&self) -> usize {
        unsafe { self.page().block_size_raw() }
    }

    #[inline(always)]
    pub(crate) fn usable_size(&self) -> UsableSize {
        unsafe { crate::internal::mi_page_usable_block_size(self.page().as_ptr()) }
    }

    #[inline(always)]
    pub(crate) fn free_is_zero(&self) -> bool {
        unsafe { (*self.page().as_ptr()).free_is_zero }
    }

    #[inline(always)]
    fn is_huge(&self) -> bool {
        unsafe { crate::allocator::page::mi_page_is_huge(self.page().as_ptr()) }
    }

    #[inline(always)]
    fn keys(&self) -> PageKeys {
        unsafe { self.page().keys() }
    }

    #[inline(always)]
    pub(crate) fn assert_free_head(&self, allow_empty: bool) {
        let page = self.page().as_ptr();
        let head = unsafe { self.page().free_raw() };
        crate::internal::mi_assert_internal!(
            (allow_empty && head.is_null())
                || (!head.is_null()
                    && unsafe { crate::internal::_mi_ptr_page(head.cast()) == page })
        );
    }

    #[inline(always)]
    pub(crate) fn assert_geometry(&self, size: usize) {
        let page = self.page().as_ptr();
        if self.block_size_raw() != 0 {
            crate::internal::mi_assert_internal!(
                unsafe { crate::internal::mi_page_block_size(page) } >= size
            );
            crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
                unsafe { crate::internal::mi_page_slice_start(page) }.cast(),
                crate::allocator::page::MI_OS_PAGE_SIZE,
            ));
            crate::internal::mi_assert_internal!(unsafe {
                crate::internal::_mi_ptr_page(crate::internal::mi_page_start(page).cast()) == page
            });
        }
    }
}

impl PageRequest<'_> {
    #[inline(always)]
    pub(crate) fn get(&self) -> usize {
        self.padded.get()
    }
}

/// Owns one writable block extent until it escapes to the allocation caller.
pub(crate) struct AllocatedBlockStorage<'scope, S = Popped> {
    block: BlockPtr,
    requested: PaddedSize,
    usable: UsableSize,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
    _state: PhantomData<S>,
}

pub(crate) enum Popped {}
pub(crate) enum Writable {}

impl<'scope> PageAccess<'scope, AvailablePage> {
    #[inline(always)]
    pub(crate) fn pop(
        self,
        request: PageRequest<'scope>,
    ) -> (
        AllocatedBlockStorage<'scope>,
        PageAccess<'scope, ActivePage>,
    ) {
        let usable = self.usable_size();
        let (block, active) = self.state.pop_free_block();
        let page = PageAccess {
            state: active,
            _scope: PhantomData,
        };
        (
            AllocatedBlockStorage {
                block: block.into_ptr(),
                requested: request.padded,
                usable,
                _scope: PhantomData,
                _state: PhantomData,
            },
            page,
        )
    }
}

impl<S> AllocatedBlockStorage<'_, S> {
    #[inline(always)]
    pub(crate) fn usable_size(&self) -> UsableSize {
        self.usable
    }

    #[inline(always)]
    pub(crate) fn assert_alignment(&self, block_size: usize) {
        crate::internal::mi_assert_internal!(
            block_size < MI_MAX_ALIGN_SIZE
                || crate::internal::_mi_is_aligned(self.block.as_ptr().cast(), MI_MAX_ALIGN_SIZE)
        );
    }
}

impl<'scope> AllocatedBlockStorage<'scope> {
    #[inline(always)]
    pub(crate) fn assert_zero_tail(&self, page: &PageAccess<'scope, ActivePage>) {
        if cfg!(debug_assertions)
            && page.free_is_zero()
            && self.requested.get() > size_of::<mi_block_t>()
        {
            crate::internal::mi_assert_expensive!(unsafe {
                crate::internal::mi_mem_is_zero(
                    self.block.as_ptr().add(1).cast(),
                    self.requested.get() - size_of::<mi_block_t>(),
                )
            });
        }
    }

    #[inline(always)]
    pub(crate) fn track_undefined(self) -> AllocatedBlockStorage<'scope, Writable> {
        unsafe { mi_track_mem_undefined(self.block.as_ptr().cast(), self.usable.get()) };
        AllocatedBlockStorage {
            block: self.block,
            requested: self.requested,
            usable: self.usable,
            _scope: PhantomData,
            _state: PhantomData,
        }
    }
}

impl<'scope> AllocatedBlockStorage<'scope, Writable> {
    #[inline(always)]
    pub(crate) fn finish(
        mut self,
        page: &PageAccess<'scope, ActivePage>,
        zeroed: Zeroed,
    ) -> BlockPtr {
        self.initialize(page, zeroed);
        self.write_padding(page);
        self.block
    }

    #[inline(always)]
    fn initialize(&mut self, page: &PageAccess<'scope, ActivePage>, zeroed: Zeroed) {
        let block = self.block.as_ptr();
        if zeroed == Zeroed::No {
            unsafe { super::BlockTag::write_aligned_to(self.block) };
            if cfg!(debug_assertions) && MI_TRACK_ENABLED == 0 && !page.is_huge() {
                unsafe {
                    ptr::write_bytes(
                        block.cast::<u8>(),
                        crate::types::MI_DEBUG_UNINIT,
                        self.usable.get(),
                    )
                };
            }
        } else if !page.free_is_zero() {
            unsafe { crate::internal::_mi_memzero_aligned(block.cast(), self.usable.get()) };
        } else {
            unsafe { super::BlockTag::write_aligned_to(self.block) };
            unsafe { mi_track_mem_defined(block.cast(), self.usable.get()) };
        }
    }

    #[inline(always)]
    fn write_padding(&mut self, page: &PageAccess<'scope, ActivePage>) {
        if MI_PADDING == 0 {
            return;
        }
        let block = self.block.as_ptr();
        // The matching page/request loan bounds the complete usable range and
        // its trailing padding. The block retains exclusive write ownership.
        let padding = unsafe { super::PaddingPtr::from_block_usable(self.block, self.usable) };
        let padding_ptr = padding.as_ptr();
        let delta = (padding_ptr as usize).wrapping_sub(block as usize)
            - (self.requested.get() - MI_PADDING_SIZE);
        if cfg!(debug_assertions) {
            crate::internal::mi_assert_internal!(
                delta <= self.usable.get()
                    && self.usable.get() >= self.requested.get() - MI_PADDING_SIZE + delta
            );
        }
        unsafe { mi_track_mem_defined(padding_ptr.cast(), super::PaddingPtr::SIZE) };
        let value = super::Padding::new(
            super::PaddingCanary::from_page_block_keys(page.page(), self.block, page.keys()),
            super::PaddingDelta::for_usable(delta, self.usable).unwrap_or_else(|| {
                crate::internal::mi_assert_internal!(false);
                super::PaddingDelta::new_unchecked(delta as u32)
            }),
        );
        unsafe { padding.write(value) };
        if MI_PADDING_CHECK != 0 && !page.is_huge() {
            let maxpad = delta.min(MI_MAX_ALIGN_SIZE);
            // The matching request bounds this writable tail within the
            // exclusively owned block. Its bytes may still be uninitialized.
            let fill = unsafe {
                core::slice::from_raw_parts_mut(
                    padding_ptr
                        .cast::<u8>()
                        .sub(delta)
                        .cast::<core::mem::MaybeUninit<u8>>(),
                    maxpad,
                )
            };
            fill.fill(core::mem::MaybeUninit::new(crate::types::MI_DEBUG_PADDING));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::mi_page_t;

    #[repr(align(16))]
    struct Blocks([u8; 128]);

    #[test]
    fn popped_storage_initializes_only_its_matching_block() {
        for free_is_zero in [false, true] {
            let mut blocks = Box::new(Blocks([if free_is_zero { 0 } else { 0x55 }; 128]));
            let mut page = Box::<mi_page_t>::new_uninit();
            // The zero representation initializes every page field, including
            // the inert memory-origin discriminant. Two committed block
            // extents and their encoding keys remain live through the loan.
            unsafe {
                page.as_mut_ptr().write_bytes(0, 1);
                let raw = page.as_mut_ptr();
                let start = blocks.0.as_mut_ptr();
                let second = start.add(64).cast::<mi_block_t>();
                (*raw).block_size = 64;
                (*raw).page_start = start;
                (*raw).capacity = 2;
                (*raw).reserved = 2;
                (*raw).free = start.cast();
                (*raw).free_is_zero = free_is_zero;
                crate::internal::mi_block_set_next(raw, start.cast(), second);
                crate::internal::mi_block_set_next(raw, second, ptr::null());
                let second_before = core::slice::from_raw_parts(start.add(64), 64).to_vec();

                let available = AvailablePage::new_unchecked(PagePtr::new_unchecked(raw));
                let requested = 16;
                let result = available.with_allocation_access(
                    PaddedSize::new_unchecked(requested + MI_PADDING_SIZE),
                    |access, request| {
                        let (storage, access) = access.pop(request);
                        assert_eq!(storage.usable_size().get(), 64 - MI_PADDING_SIZE);
                        storage.track_undefined().finish(&access, Zeroed::Yes)
                    },
                );
                assert_eq!(result.as_ptr(), start.cast());
                assert_eq!((*raw).used, 1);
                assert_eq!((*raw).free, second);
                assert_eq!(&blocks.0[..requested], &[0; 16]);
                if MI_PADDING_CHECK != 0 {
                    assert_eq!(
                        &blocks.0[requested..requested + MI_MAX_ALIGN_SIZE],
                        &[crate::types::MI_DEBUG_PADDING; MI_MAX_ALIGN_SIZE],
                    );
                }
                assert_eq!(&blocks.0[64..], second_before);
            }
        }
    }
}
