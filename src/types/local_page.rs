//! Scoped local fields and bounded storage owned by one allocation phase.

use core::{
    marker::PhantomData,
    mem::{MaybeUninit, size_of},
};

use super::{
    ActivePage, AvailablePage, BlockPtr, BlockSize, PaddedSize, PageKeys, PagePtr, UsableSize,
    Zeroed,
};
use crate::allocator::tracking::{MI_TRACK_ENABLED, mi_track_mem_defined, mi_track_mem_undefined};
use crate::types::{
    MI_MAX_ALIGN_SIZE, MI_PADDING, MI_PADDING_CHECK, MI_PADDING_SIZE, mi_block_t, mi_padding_t,
};

pub(crate) struct PageAccess<'scope, S> {
    page: PagePtr,
    free: &'scope mut *mut mi_block_t,
    used: &'scope mut u16,
    free_is_zero: &'scope bool,
    block_size: BlockSize,
    reserved: &'scope u16,
    memid: &'scope crate::internal::memid::mi_memid_t,
    keys: &'scope [usize; 2],
    _state: PhantomData<S>,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

pub(crate) struct PageRequest<'scope> {
    padded: PaddedSize,
    usable: Option<&'scope mut MaybeUninit<usize>>,
    _thread: PhantomData<*mut ()>,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl AvailablePage {
    /// Lend this page and its fitting request during one allocation phase.
    ///
    /// # Safety
    /// The initialized page, page map, encoding keys and committed block storage
    /// must remain live and stable through `f`. The caller owns local list and
    /// accounting mutation; remote publication may follow the atomic protocol.
    /// `padded` includes `MI_PADDING_SIZE` and fits this page's complete block.
    /// That block is aligned for its link and padding, fits in one allocation,
    /// and has length no greater than `isize::MAX`. Callbacks must not reenter
    /// this page while its loan is active. If nonnull, `usable` is aligned and
    /// exclusively writable throughout the phase, disjoint from page metadata
    /// and free-list nodes; its previous value need not be initialized.
    #[inline(always)]
    pub(crate) unsafe fn with_allocation_access<R>(
        self,
        padded: PaddedSize,
        usable: *mut usize,
        f: impl for<'scope> FnOnce(PageAccess<'scope, Self>, PageRequest<'scope>) -> R,
    ) -> R {
        let page = self.page();
        let raw = page.as_ptr();
        // Project only local fields and stable geometry. No aggregate page
        // reference overlaps the independently accessible remote atomic words.
        unsafe {
            f(
                PageAccess {
                    page,
                    free: &mut (*raw).free,
                    used: &mut (*raw).used,
                    free_is_zero: &(*raw).free_is_zero,
                    block_size: BlockSize::new_unchecked((*raw).block_size),
                    reserved: &(*raw).reserved,
                    memid: &(*raw).memid,
                    keys: {
                        #[cfg(any(
                            feature = "secure",
                            debug_assertions,
                            feature = "track-valgrind",
                            feature = "track-asan"
                        ))]
                        {
                            &(*raw).keys
                        }
                        #[cfg(not(any(
                            feature = "secure",
                            debug_assertions,
                            feature = "track-valgrind",
                            feature = "track-asan"
                        )))]
                        {
                            &[0, 0]
                        }
                    },
                    _state: PhantomData,
                    _scope: PhantomData,
                },
                PageRequest {
                    padded,
                    usable: usable.cast::<MaybeUninit<usize>>().as_mut(),
                    _thread: PhantomData,
                    _scope: PhantomData,
                },
            )
        }
    }
}

impl<S> PageAccess<'_, S> {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        self.page
    }

    #[inline(always)]
    pub(crate) fn block_size_raw(&self) -> usize {
        self.block_size.get()
    }

    #[inline(always)]
    pub(crate) fn usable_size(&self) -> UsableSize {
        self.block_size.usable()
    }

    #[inline(always)]
    pub(crate) fn free_is_zero(&self) -> bool {
        *self.free_is_zero
    }

    #[inline(always)]
    fn is_huge(&self) -> bool {
        if *self.reserved != 1 {
            return false;
        }
        if self.block_size.get() > crate::allocator::page::MI_LARGE_MAX_OBJ_SIZE {
            return true;
        }
        match super::MemId::view(*self.memid) {
            super::MemIdView::Os(os) => os
                .base
                .is_some_and(|base| base.as_ptr() < self.page.as_ptr().cast::<core::ffi::c_void>()),
            _ => false,
        }
    }

    #[inline(always)]
    fn keys(&self) -> PageKeys {
        PageKeys::from_raw(*self.keys)
    }

    #[inline(always)]
    pub(crate) fn assert_free_head(&self, allow_empty: bool) {
        let page = self.page.as_ptr();
        let head = *self.free;
        crate::internal::mi_assert_internal!(
            (allow_empty && head.is_null())
                || (!head.is_null()
                    && unsafe { crate::internal::_mi_ptr_page(head.cast()) == page })
        );
    }

    #[inline(always)]
    pub(crate) fn assert_geometry(&self, size: usize) {
        let page = self.page.as_ptr();
        crate::internal::mi_assert_internal!(self.block_size.get() >= size);
        // Page-map lookup and page/slice address classification are raw memory
        // boundaries; all scalar geometry above is read through the loan.
        unsafe {
            crate::internal::mi_assert_internal!(crate::internal::_mi_is_aligned(
                crate::internal::mi_page_slice_start(page).cast(),
                crate::allocator::page::MI_OS_PAGE_SIZE,
            ));
            crate::internal::mi_assert_internal!(
                crate::internal::_mi_ptr_page(crate::internal::mi_page_start(page).cast()) == page
            );
        }
    }
}

impl PageRequest<'_> {
    #[forbid(unsafe_code)]
    #[inline(always)]
    pub(crate) fn get(&self) -> usize {
        self.padded.get()
    }

    #[forbid(unsafe_code)]
    #[inline(always)]
    pub(crate) fn write_usable(&mut self, usable_size: UsableSize) {
        if let Some(usable) = &mut self.usable {
            usable.write(usable_size.get());
        }
    }
}

/// A popped block whose allocation tracking transition has not yet occurred.
pub(crate) struct PoppedBlockStorage<'scope> {
    block: BlockPtr,
    requested: PaddedSize,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

/// Owns the exact writable block extent, including its padding tail.
pub(crate) struct AllocatedBlockStorage<'scope> {
    block: BlockPtr,
    requested: PaddedSize,
    usable: UsableSize,
    bytes: &'scope mut [MaybeUninit<u8>],
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<'scope> PageAccess<'scope, AvailablePage> {
    #[inline(always)]
    pub(crate) fn pop(
        self,
        request: PageRequest<'scope>,
    ) -> (PoppedBlockStorage<'scope>, PageAccess<'scope, ActivePage>) {
        // The available-page ingress proves a nonnull readable link belonging
        // to this page. Link decoding retains its existing tracking protocol.
        let (block, next) = unsafe {
            let block = BlockPtr::new_unchecked(*self.free);
            let next = crate::internal::mi_block_next(self.page.as_ptr(), block.as_ptr());
            (block, next)
        };
        *self.free = next;
        *self.used = super::UsedBlockCount::new(*self.used)
            .increment_wrapping()
            .get();
        (
            PoppedBlockStorage {
                block,
                requested: request.padded,
                _scope: PhantomData,
            },
            PageAccess {
                page: self.page,
                free: self.free,
                used: self.used,
                free_is_zero: self.free_is_zero,
                block_size: self.block_size,
                reserved: self.reserved,
                memid: self.memid,
                keys: self.keys,
                _state: PhantomData,
                _scope: PhantomData,
            },
        )
    }
}

impl PoppedBlockStorage<'_> {
    #[cfg(test)]
    pub(crate) fn into_ptr(self) -> BlockPtr {
        self.block
    }

    #[inline(always)]
    pub(crate) fn assert_alignment(&self, block_size: usize) {
        crate::internal::mi_assert_internal!(
            block_size < MI_MAX_ALIGN_SIZE
                || crate::internal::_mi_is_aligned(self.block.as_ptr().cast(), MI_MAX_ALIGN_SIZE)
        );
    }
}

impl<'scope> PoppedBlockStorage<'scope> {
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
    pub(crate) fn track_undefined(
        self,
        page: &PageAccess<'scope, ActivePage>,
    ) -> AllocatedBlockStorage<'scope> {
        let usable = page.usable_size();
        // The pop transfers this complete block, not any neighbouring client
        // allocation. MaybeUninit permits its currently undefined bytes.
        let bytes = unsafe {
            mi_track_mem_undefined(self.block.as_ptr().cast(), usable.get());
            core::slice::from_raw_parts_mut(
                self.block.as_ptr().cast::<MaybeUninit<u8>>(),
                page.block_size.get(),
            )
        };
        AllocatedBlockStorage {
            block: self.block,
            requested: self.requested,
            usable,
            bytes,
            _scope: PhantomData,
        }
    }
}

impl<'scope> AllocatedBlockStorage<'scope> {
    #[inline(always)]
    pub(crate) fn usable_size(&self) -> UsableSize {
        self.usable
    }

    #[forbid(unsafe_code)]
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

    #[forbid(unsafe_code)]
    #[inline(always)]
    fn initialize(&mut self, page: &PageAccess<'scope, ActivePage>, zeroed: Zeroed) {
        if zeroed == Zeroed::No {
            self.write_aligned_tag();
            if cfg!(debug_assertions) && MI_TRACK_ENABLED == 0 && !page.is_huge() {
                self.bytes[..self.usable.get()]
                    .fill(MaybeUninit::new(crate::types::MI_DEBUG_UNINIT));
            }
        } else if !page.free_is_zero() {
            self.zero_aligned();
        } else {
            self.write_aligned_tag();
            self.define_usable();
        }
    }

    #[inline(always)]
    fn write_aligned_tag(&mut self) {
        // Every available block includes one aligned link word. Reborrow it
        // from the owned byte extent so the original raw pointer is not used
        // to mutate storage behind the byte loan.
        let tag = unsafe { &mut *self.bytes.as_mut_ptr().cast::<MaybeUninit<mi_block_t>>() };
        tag.write(mi_block_t { next: 0 });
    }

    #[inline(always)]
    fn zero_aligned(&mut self) {
        // Retain the existing aligned bulk-zero primitive and its native
        // dispatch thresholds over this block's bounded usable subrange.
        unsafe {
            crate::internal::_mi_memzero_aligned(self.bytes.as_mut_ptr().cast(), self.usable.get())
        };
    }

    #[inline(always)]
    fn define_usable(&mut self) {
        unsafe { mi_track_mem_defined(self.bytes.as_mut_ptr().cast(), self.usable.get()) };
    }

    #[inline(always)]
    fn padding_slot(&mut self) -> Option<&mut MaybeUninit<mi_padding_t>> {
        if MI_PADDING == 0 {
            return None;
        }
        // Reborrow the disjoint padding slot without reading its old contents.
        unsafe {
            let padding = self
                .bytes
                .as_mut_ptr()
                .add(self.usable.get())
                .cast::<MaybeUninit<mi_padding_t>>();
            mi_track_mem_defined(padding.cast(), size_of::<mi_padding_t>());
            Some(&mut *padding)
        }
    }

    #[inline(always)]
    fn padding_tail(&mut self) -> &mut [MaybeUninit<u8>] {
        let requested = self.requested.get() - MI_PADDING_SIZE;
        let delta = self.usable.get() - requested;
        // The fitting request bounds the writable tail below padding metadata.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.bytes.as_mut_ptr().add(requested),
                delta.min(MI_MAX_ALIGN_SIZE),
            )
        }
    }

    #[forbid(unsafe_code)]
    #[inline(always)]
    fn write_padding(&mut self, page: &PageAccess<'scope, ActivePage>) {
        if MI_PADDING == 0 {
            return;
        }
        let delta = self.usable.get() - (self.requested.get() - MI_PADDING_SIZE);
        if cfg!(debug_assertions) {
            crate::internal::mi_assert_internal!(delta <= self.usable.get());
        }
        let value = super::Padding::new(
            super::PaddingCanary::from_page_block_keys(page.page(), self.block, page.keys()),
            super::PaddingDelta::for_usable(delta, self.usable).unwrap_or_else(|| {
                crate::internal::mi_assert_internal!(false);
                super::PaddingDelta::new_unchecked(delta as u32)
            }),
        );
        let Some(padding) = self.padding_slot() else {
            return;
        };
        value.write_to(padding);
        if MI_PADDING_CHECK != 0 && !page.is_huge() {
            self.padding_tail()
                .fill(MaybeUninit::new(crate::types::MI_DEBUG_PADDING));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::mi_page_t;
    use core::ptr;

    #[repr(align(16))]
    struct Blocks([u8; 128]);

    #[test]
    fn popped_storage_initializes_only_its_matching_block() {
        for zeroed in [Zeroed::No, Zeroed::Yes] {
            for free_is_zero in [false, true] {
                let mut blocks = Box::new(Blocks([if free_is_zero { 0 } else { 0x55 }; 128]));
                let mut page = Box::<mi_page_t>::new_uninit();
                // The zero representation initializes every page field. Two
                // disjoint committed blocks remain live through the loan.
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
                    let remote = &*ptr::addr_of!((*raw).xthread_free)
                        .cast::<core::sync::atomic::AtomicUsize>();
                    remote.store(
                        super::super::ThreadFreeWord::OWNED_BIT,
                        core::sync::atomic::Ordering::Relaxed,
                    );

                    let available = AvailablePage::new_unchecked(PagePtr::new_unchecked(raw));
                    let requested = 16;
                    let mut usable = MaybeUninit::uninit();
                    let result = available.with_allocation_access(
                        PaddedSize::new_unchecked(requested + MI_PADDING_SIZE),
                        usable.as_mut_ptr(),
                        |access, mut request| {
                            assert_eq!(
                                remote.load(core::sync::atomic::Ordering::Relaxed),
                                super::super::ThreadFreeWord::OWNED_BIT
                            );
                            request.write_usable(access.usable_size());
                            let (storage, access) = access.pop(request);
                            let storage = storage.track_undefined(&access);
                            assert_eq!(storage.usable_size().get(), 64 - MI_PADDING_SIZE);
                            storage.finish(&access, zeroed)
                        },
                    );
                    assert_eq!(usable.assume_init(), 64 - MI_PADDING_SIZE);
                    assert_eq!(
                        remote.load(core::sync::atomic::Ordering::Relaxed),
                        super::super::ThreadFreeWord::OWNED_BIT
                    );
                    assert_eq!(result.as_ptr(), start.cast());
                    assert_eq!((*raw).used, 1);
                    assert_eq!((*raw).free, second);
                    if zeroed == Zeroed::Yes {
                        assert_eq!(&blocks.0[..requested], &[0; 16]);
                    } else if cfg!(debug_assertions) && MI_TRACK_ENABLED == 0 {
                        assert_eq!(&blocks.0[..requested], &[crate::types::MI_DEBUG_UNINIT; 16]);
                    } else {
                        assert_eq!(
                            &blocks.0[..size_of::<mi_block_t>()],
                            &[0; size_of::<mi_block_t>()]
                        );
                    }
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
}
