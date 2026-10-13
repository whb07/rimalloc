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

#[derive(Debug)]
#[must_use = "local free batches must be returned to page availability"]
pub(crate) struct LocalFreeBatch(BatchCore);

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
impl_batch_accessors!(LocalFreeBatch);

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

    /// # Safety
    /// `owner` and `current` must describe this batch's local destination.
    #[inline]
    pub(crate) unsafe fn collect_into_local_page(
        self,
        owner: &LocalPage,
        current: LocalFreeList,
    ) -> LocalFreeList {
        unsafe { self.collect_into_page(owner.page(), current) }
    }

    /// # Safety
    /// `owner` and `current` must describe this batch's abandoned destination.
    #[inline]
    pub(crate) unsafe fn collect_into_owned_abandoned_page(
        self,
        owner: &OwnedAbandonedPage,
        current: LocalFreeList,
    ) -> LocalFreeList {
        unsafe { self.collect_into_page(owner.page(), current) }
    }

    #[inline]
    unsafe fn collect_into_page(self, owner: PagePtr, current: LocalFreeList) -> LocalFreeList {
        crate::internal::mi_debug_assert!(owner == self.page());
        unsafe { self.tail().set_next(owner, current.head()) };
        let new_local = LocalFreeList::from_head(self.head());
        unsafe {
            owner.set_local_free(LocalFreeList::from_head(self.head()));
            let used = owner.used();
            let Some(used) = used.checked_sub_batch(self.len()) else {
                core::hint::unreachable_unchecked()
            };
            owner.set_used(used);
        }
        new_local
    }
}

impl LocalFreeBatch {
    /// # Safety
    /// `owner` and `available` must describe this batch's page destination.
    #[inline]
    pub(crate) unsafe fn prepend_to_available(
        self,
        owner: &LocalPage,
        available: AvailableFreeList,
    ) -> AvailableFreeList {
        unsafe { self.prepend_to_page(owner.page(), available) }
    }

    /// # Safety
    /// `owner` and `available` must describe this batch's abandoned destination.
    #[inline]
    pub(crate) unsafe fn prepend_to_owned_abandoned_page(
        self,
        owner: &OwnedAbandonedPage,
        available: AvailableFreeList,
    ) -> AvailableFreeList {
        unsafe { self.prepend_to_page(owner.page(), available) }
    }

    #[inline]
    unsafe fn prepend_to_page(
        self,
        owner: PagePtr,
        available: AvailableFreeList,
    ) -> AvailableFreeList {
        crate::internal::mi_debug_assert!(owner == self.page());
        unsafe { self.tail().set_next(owner, available.head()) };
        AvailableFreeList::from_head(self.head())
    }
}

impl LocalPage {
    /// Detach the complete local free list as a validated batch.
    ///
    /// # Safety
    /// The caller must hold current-thread ownership of this page.
    #[inline]
    pub(crate) unsafe fn detach_local_free_batch(
        &self,
    ) -> Result<Option<LocalFreeBatch>, BatchValidationError> {
        unsafe { detach_local_free_batch(self.page()) }
    }
}

impl OwnedAbandonedPage {
    /// Detach the complete local free list as a validated batch.
    ///
    /// # Safety
    /// The caller must hold claimed collection ownership of this abandoned page.
    #[inline]
    pub(crate) unsafe fn detach_local_free_batch(
        &self,
    ) -> Result<Option<LocalFreeBatch>, BatchValidationError> {
        unsafe { detach_local_free_batch(self.page()) }
    }
}

#[inline]
unsafe fn detach_local_free_batch(
    page: PagePtr,
) -> Result<Option<LocalFreeBatch>, BatchValidationError> {
    let local = unsafe { page.local_free() };
    let Some(head) = local.head() else {
        return Ok(None);
    };
    let capacity = unsafe { page.capacity() };
    let core = unsafe { scan_detached_chain(page, head, capacity) }?;
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::local_tail_scan(core.len().get() as usize);
    unsafe { page.set_local_free(LocalFreeList::empty()) };
    Ok(Some(LocalFreeBatch(core)))
}

#[cfg(test)]
mod tests;
