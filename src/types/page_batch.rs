//! Linear ownership values for detached, page-homogeneous block chains.

use crate::core_types::{
    AvailableFreeList, BatchLen, BlockPtr, LocalFreeList, LocalPage, OwnedAbandonedPage,
    PageBlockLayout, PageCapacity, PagePtr, UsedBlockCount,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BatchValidationError {
    TooLong {
        observed_at_least: usize,
        capacity: PageCapacity,
    },
    ExceedsUsed {
        len: BatchLen,
        used: crate::core_types::UsedBlockCount,
    },
    DifferentPage,
    InvalidLink,
}

#[derive(Debug)]
struct BatchCore {
    page: PagePtr,
    head: BlockPtr,
    tail: BlockPtr,
    len: BatchLen,
}

#[derive(Debug)]
#[must_use = "remote free batches must be collected or republished"]
pub(crate) struct RemoteFreeBatch(BatchCore);

#[must_use = "local free batches must be returned to page availability"]
pub(crate) struct LocalFreeBatch<'access, 'page> {
    access: &'access mut PageCollection<'page>,
    core: BatchCore,
}

impl core::fmt::Debug for LocalFreeBatch<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("LocalFreeBatch").field(&self.core).finish()
    }
}

impl BatchCore {
    #[inline(always)]
    fn page(&self) -> PagePtr {
        self.page
    }

    #[inline(always)]
    fn head(&self) -> BlockPtr {
        self.head
    }

    #[inline(always)]
    fn tail(&self) -> BlockPtr {
        self.tail
    }

    #[inline(always)]
    fn len(&self) -> BatchLen {
        self.len
    }
}

macro_rules! impl_batch_accessors {
    ($batch:ident) => {
        impl $batch {
            #[inline(always)]
            pub(crate) fn page(&self) -> PagePtr {
                self.0.page()
            }

            #[inline(always)]
            pub(crate) fn head(&self) -> BlockPtr {
                self.0.head()
            }

            #[inline(always)]
            pub(crate) fn tail(&self) -> BlockPtr {
                self.0.tail()
            }
        }
    };
}

impl_batch_accessors!(RemoteFreeBatch);

impl RemoteFreeBatch {
    #[inline(always)]
    pub(crate) fn len(&self) -> BatchLen {
        self.0.len()
    }
}

#[inline]
unsafe fn classify_foreign_link(page: PagePtr, block: BlockPtr) -> BatchValidationError {
    let owner = unsafe { crate::internal::_mi_ptr_page(block.as_ptr().cast()) };
    if !owner.is_null() && owner != page.as_ptr() {
        BatchValidationError::DifferentPage
    } else {
        BatchValidationError::InvalidLink
    }
}

#[inline]
unsafe fn scan_detached_chain_impl<const VALIDATE_BLOCKS: bool>(
    page: PagePtr,
    head: BlockPtr,
    capacity: PageCapacity,
) -> Result<BatchCore, BatchValidationError> {
    if capacity.get() == 0 {
        return Err(BatchValidationError::TooLong {
            observed_at_least: 1,
            capacity,
        });
    }

    let validated_range = if VALIDATE_BLOCKS {
        // SAFETY: the caller supplies initialized, stable page geometry for
        // the duration of this uniquely detached chain scan.
        let block_size = unsafe { page.block_size() };
        // SAFETY: the same caller contract keeps the page geometry and block
        // backing live and stable for pointers derived during the scan.
        let Some(layout) = (unsafe { PageBlockLayout::from_page(page, block_size) }) else {
            return Err(unsafe { classify_foreign_link(page, head) });
        };
        let Some(range) = layout.nonempty_range(0, capacity.as_usize()) else {
            return Err(unsafe { classify_foreign_link(page, head) });
        };
        Some(range)
    } else {
        None
    };

    let mut current = head;
    let mut observed = 0usize;
    loop {
        if validated_range.is_some_and(|range| !range.contains(current)) {
            return Err(unsafe { classify_foreign_link(page, current) });
        }

        observed += 1;
        let next = unsafe { current.next(page) };
        let Some(next) = next else {
            let len = unsafe { BatchLen::new_unchecked(observed as u16) };
            return Ok(BatchCore {
                page,
                head,
                tail: current,
                len,
            });
        };

        if observed >= capacity.as_usize() {
            return Err(BatchValidationError::TooLong {
                observed_at_least: observed + 1,
                capacity,
            });
        }
        current = next;
    }
}

/// Scan a uniquely detached chain using its page-local encoded links.
///
/// # Safety
/// `page` must identify live page metadata and `head` must identify readable
/// block-link storage. The chain must be uniquely detached for the duration of
/// the scan.
#[inline]
unsafe fn scan_detached_chain(
    page: PagePtr,
    head: BlockPtr,
    capacity: PageCapacity,
) -> Result<BatchCore, BatchValidationError> {
    unsafe { scan_detached_chain_impl::<true>(page, head, capacity) }
}

/// Scan a detached remote chain whose page identity is already proven by its
/// linear owner and page-owner witness.
///
/// # Safety
/// Every block in the uniquely detached chain must belong to `page`, use its
/// encoded-link domain, and remain readable for the scan. `capacity` must be
/// the live page capacity.
#[inline]
unsafe fn scan_trusted_detached_chain(
    page: PagePtr,
    head: BlockPtr,
    capacity: PageCapacity,
) -> Result<BatchCore, BatchValidationError> {
    unsafe { scan_detached_chain_impl::<false>(page, head, capacity) }
}

impl RemoteFreeBatch {
    /// # Safety
    /// `head` must own a uniquely detached chain belonging to `page`.
    #[inline]
    pub(crate) unsafe fn from_detached(
        page: PagePtr,
        head: BlockPtr,
        capacity: PageCapacity,
        used: UsedBlockCount,
    ) -> Result<Self, BatchValidationError> {
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::remote_list_detached();
        let core = match if cfg!(any(
            debug_assertions,
            feature = "secure",
            feature = "bench-diagnostics"
        )) {
            unsafe { scan_detached_chain(page, head, capacity) }
        } else {
            unsafe { scan_trusted_detached_chain(page, head, capacity) }
        } {
            Ok(core) => core,
            Err(error) => {
                #[cfg(feature = "bench-diagnostics")]
                crate::allocator::bench_diagnostics::remote_validation_failure(error);
                return Err(error);
            }
        };
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::remote_list_scanned(core.len().get() as usize);
        if core.len().get() > used.get() {
            let error = BatchValidationError::ExceedsUsed {
                len: core.len(),
                used,
            };
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::remote_validation_failure(error);
            return Err(error);
        }
        Ok(Self(core))
    }

    #[inline]
    fn collect_into(self, access: &mut PageCollection<'_>) {
        crate::internal::mi_debug_assert!(access.page == self.page());
        // The private caller validated this chain against this access's page
        // and used count; no callback or other writer intervenes.
        unsafe {
            self.tail()
                .set_next(access.page, LocalFreeList::from_raw(*access.local).head())
        };
        *access.local = self.head().as_ptr();
        *access.used -= self.len().get();
    }
}

/// The local fields borrowed for one callback-free collection phase.
/// Remote producers retain independent access to the atomic publication word.
pub(crate) struct PageCollection<'page> {
    page: PagePtr,
    free: &'page mut *mut crate::types::mi_block_t,
    local: &'page mut *mut crate::types::mi_block_t,
    used: &'page mut u16,
    free_is_zero: &'page mut bool,
    capacity: &'page u16,
    remote: crate::types::atomic::AtomicThreadFree<'page>,
}

#[must_use = "a detached remote chain must be collected"]
pub(crate) struct DetachedForPage<'access, 'page> {
    access: &'access mut PageCollection<'page>,
    detached: crate::core_types::DetachedRemoteList,
}

impl<'page> PageCollection<'page> {
    /// # Safety
    /// `owner` must retain exclusive local-list/accounting access to its live,
    /// initialized page and nodes for this loan. Geometry and encoding keys
    /// remain stable. Only atomic remote publication may run concurrently;
    /// callbacks and incompatible reentry must wait until the loan ends.
    #[inline(always)]
    pub(crate) unsafe fn for_local(owner: &'page LocalPage) -> Self {
        unsafe { Self::from_page(owner.page()) }
    }

    /// # Safety
    /// The claimed abandoned owner must retain the same exclusive fields,
    /// node storage and stable geometry required by `for_local` for this loan.
    #[inline(always)]
    pub(crate) unsafe fn for_abandoned(owner: &'page OwnedAbandonedPage) -> Self {
        unsafe { Self::from_page(owner.page()) }
    }

    #[inline(always)]
    unsafe fn from_page(page: PagePtr) -> Self {
        let raw = page.as_ptr();
        // Borrow disjoint fields, never the aggregate page or client blocks.
        unsafe {
            Self {
                page,
                free: &mut (*raw).free,
                local: &mut (*raw).local_free,
                used: &mut (*raw).used,
                free_is_zero: &mut (*raw).free_is_zero,
                capacity: &(*raw).capacity,
                remote: crate::types::atomic::AtomicThreadFree::from_cell(
                    &*page.thread_free_field(),
                ),
            }
        }
    }

    #[inline(always)]
    pub(crate) fn detach_remote(&mut self) -> DetachedForPage<'_, 'page> {
        // The field loan retains the collection claim and backing for the CAS.
        let detached = unsafe { self.remote.detach_head() };
        DetachedForPage {
            access: self,
            detached,
        }
    }

    /// # Safety
    /// `detached` must be the uniquely detached chain for this exact page.
    /// Used only where an external atomic or published-head operation detached
    /// the chain before projecting these local fields.
    #[inline]
    pub(crate) unsafe fn collect_detached(
        &mut self,
        detached: crate::core_types::DetachedRemoteList,
    ) -> Result<(), BatchValidationError> {
        let Some(head) = detached.into_head() else {
            return Ok(());
        };
        unsafe { self.collect_head(head) }
    }

    /// # Safety
    /// `head` must own a uniquely detached chain for this exact page.
    #[inline(always)]
    pub(crate) unsafe fn collect_head(
        &mut self,
        head: BlockPtr,
    ) -> Result<(), BatchValidationError> {
        let batch = unsafe {
            RemoteFreeBatch::from_detached(
                self.page,
                head,
                PageCapacity::new_unchecked(*self.capacity),
                UsedBlockCount::new(*self.used),
            )
        }?;
        batch.collect_into(self);
        Ok(())
    }

    #[forbid(unsafe_code)]
    #[inline(always)]
    pub(crate) fn local_is_empty(&self) -> bool {
        self.local.is_null()
    }

    #[forbid(unsafe_code)]
    #[inline(always)]
    pub(crate) fn transfer_local_if_available_empty(&mut self) -> bool {
        if !self.free.is_null() || self.local.is_null() {
            return false;
        }
        *self.free = *self.local;
        *self.local = core::ptr::null_mut();
        *self.free_is_zero = false;
        true
    }

    #[inline]
    pub(crate) fn detach_local_free_batch(
        &mut self,
    ) -> Result<Option<LocalFreeBatch<'_, 'page>>, BatchValidationError> {
        let Some(head) = BlockPtr::new(*self.local) else {
            return Ok(None);
        };
        let core = unsafe {
            scan_detached_chain(self.page, head, PageCapacity::new_unchecked(*self.capacity))
        }?;
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::local_tail_scan(core.len().get() as usize);
        *self.local = core::ptr::null_mut();
        Ok(Some(LocalFreeBatch { access: self, core }))
    }

    #[forbid(unsafe_code)]
    #[inline(always)]
    pub(crate) fn recycle_local(&mut self, force: bool) -> Result<(), BatchValidationError> {
        if self.local_is_empty() {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_local_empty();
        } else if self.transfer_local_if_available_empty() {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_moved_to_free(false);
        } else if force {
            if let Some(batch) = self.detach_local_free_batch()? {
                batch.prepend_to_available();
                #[cfg(feature = "bench-diagnostics")]
                crate::allocator::bench_diagnostics::page_free_collect_moved_to_free(true);
            }
        } else {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::page_free_collect_noop_local_blocked();
        }
        Ok(())
    }
}

impl DetachedForPage<'_, '_> {
    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.detached.is_empty()
    }

    #[inline(always)]
    pub(crate) fn collect(self) -> Result<(), BatchValidationError> {
        // Detaching and this exact field loan are stored together, so callers
        // cannot substitute an unrelated page or stale destination head.
        unsafe { self.access.collect_detached(self.detached) }
    }
}

impl LocalFreeBatch<'_, '_> {
    #[inline]
    pub(crate) fn prepend_to_available(self) {
        // The batch borrows its destination and was scanned with the same
        // stable encoding domain; its tail remains exclusively owned.
        unsafe {
            self.core.tail().set_next(
                self.access.page,
                AvailableFreeList::from_raw(*self.access.free).head(),
            )
        };
        *self.access.free = self.core.head().as_ptr();
        *self.access.free_is_zero = false;
    }
}

#[cfg(test)]
mod tests;
