//! Linear ownership values for detached, page-homogeneous block chains.

use crate::core_types::{
    AvailableFreeList, BatchLen, BlockPtr, LocalFreeList, LocalPage, OwnedAbandonedPage,
    PageCapacity, PagePtr, UsedBlockCount,
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
unsafe fn block_belongs_to_page(page: PagePtr, block: BlockPtr, capacity: PageCapacity) -> bool {
    // SAFETY: the caller supplies initialized page
    // metadata; arithmetic is bounded by the page capacity and block size.
    let start = unsafe { page.start().as_ptr() } as usize;
    let block_size = unsafe { page.block_size().get() };
    let address = block.as_ptr() as usize;
    let Some(offset) = address.checked_sub(start) else {
        return false;
    };
    let Some(committed_bytes) = block_size.checked_mul(capacity.as_usize()) else {
        return false;
    };
    offset < committed_bytes && offset % block_size == 0
}

#[inline]
unsafe fn classify_foreign_link(page: PagePtr, block: BlockPtr) -> BatchValidationError {
    // SAFETY: allocator page lookup accepts an
    // arbitrary candidate address and returns null when it is unregistered.
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

    let mut current = head;
    let mut observed = 0usize;
    loop {
        // SAFETY: hardened callers classify
        // membership before dereferencing a block. Trusted callers discharge
        // the same-page invariant through the detached-list owner proof.
        if VALIDATE_BLOCKS && !unsafe { block_belongs_to_page(page, current, capacity) } {
            return Err(unsafe { classify_foreign_link(page, current) });
        }

        observed += 1;
        // SAFETY: hardened callers validated
        // membership above; trusted callers guarantee membership and readable
        // link storage through the constructor contract.
        let next = unsafe { current.next(page) };
        let Some(next) = next else {
            // SAFETY: observed is in 1..=capacity
            // and therefore fits the page's u16 count domain.
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
    // SAFETY: the caller provides the live page
    // and unique detached chain required by the validating scanner.
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
    // SAFETY: the caller's detached-list
    // ownership proof establishes page identity; the scanner retains C's
    // capacity bound before following more than one page of links.
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
        // SAFETY: the detached-list owner provides
        // live page metadata and unique traversal access.
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::remote_list_detached();
        let core = match if cfg!(any(
            debug_assertions,
            feature = "secure",
            feature = "bench-diagnostics"
        )) {
            // SAFETY: hardened builds validate
            // every block before following its encoded link.
            unsafe { scan_detached_chain(page, head, capacity) }
        } else {
            // SAFETY: the unsafe
            // constructor contract requires this uniquely detached chain to
            // belong to `page`; ordinary release retains C's bounded scan.
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
        // SAFETY: the typed local owner
        // and detached batch establish exclusive page-local splice access.
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
        // SAFETY: claimed abandoned-page
        // ownership and the detached batch exclude competing collectors.
        unsafe { self.collect_into_page(owner.page(), current) }
    }

    #[inline]
    unsafe fn collect_into_page(self, owner: PagePtr, current: LocalFreeList) -> LocalFreeList {
        crate::internal::mi_debug_assert!(owner == self.page());
        // SAFETY: the batch tail is
        // detached and all links share the owner's page-local encoding keys.
        unsafe { self.tail().set_next(owner, current.head()) };
        let new_local = LocalFreeList::from_head(self.head());
        // SAFETY: batch construction
        // proved len <= used, so subtraction and metadata publication agree.
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
        // SAFETY: LocalPage proves
        // exclusive current-thread access to the destination page lists.
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
        // SAFETY: claimed abandoned-page
        // ownership excludes competing collectors for the destination lists.
        unsafe { self.prepend_to_page(owner.page(), available) }
    }

    #[inline]
    unsafe fn prepend_to_page(
        self,
        owner: PagePtr,
        available: AvailableFreeList,
    ) -> AvailableFreeList {
        crate::internal::mi_debug_assert!(owner == self.page());
        // SAFETY: the local batch is
        // uniquely detached and its tail may be linked to the current free head.
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
        // SAFETY: LocalPage proves
        // exclusive current-thread access to the page-local list.
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
        // SAFETY: claimed
        // abandoned-page ownership excludes competing local-list collectors.
        unsafe { detach_local_free_batch(self.page()) }
    }
}

#[inline]
unsafe fn detach_local_free_batch(
    page: PagePtr,
) -> Result<Option<LocalFreeBatch>, BatchValidationError> {
    // SAFETY: the local owner may read
    // its page-local list and capacity metadata.
    let local = unsafe { page.local_free() };
    let Some(head) = local.head() else {
        return Ok(None);
    };
    // SAFETY: the local owner may read
    // initialized page capacity while validating its current chain.
    let capacity = unsafe { page.capacity() };
    // SAFETY: the current local list
    // remains published and unchanged throughout the bounded scan.
    let core = unsafe { scan_detached_chain(page, head, capacity) }?;
    #[cfg(feature = "bench-diagnostics")]
    crate::allocator::bench_diagnostics::local_tail_scan(core.len().get() as usize);
    // SAFETY: validation succeeded, so
    // clearing the field uniquely transfers the complete chain to the batch.
    unsafe { page.set_local_free(LocalFreeList::empty()) };
    Ok(Some(LocalFreeBatch(core)))
}

#[cfg(test)]
#[path = "../page_batch_tests/tests.rs"]
mod tests;
