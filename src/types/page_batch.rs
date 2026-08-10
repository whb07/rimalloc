//! Linear ownership values for detached, page-homogeneous block chains.

use crate::core_types::validation_extra::AllocatedBlock;
use crate::core_types::{
    ActivePage, AvailableFreeList, AvailablePage, BatchLen, BatchLimit, BlockPtr, LocalFreeList,
    LocalPage, OwnedAbandonedPage, PageCapacity, PagePtr, UsedBlockCount,
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

#[derive(Debug)]
#[must_use = "leased batches must be allocated from or returned to their page"]
pub(crate) struct LeasedBatch(BatchCore);

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

            #[inline(always)]
            pub(crate) fn len(&self) -> BatchLen {
                self.0.len()
            }
        }
    };
}

impl_batch_accessors!(RemoteFreeBatch);
impl_batch_accessors!(LocalFreeBatch);
impl_batch_accessors!(LeasedBatch);

#[inline]
unsafe fn block_belongs_to_page(page: PagePtr, block: BlockPtr, capacity: PageCapacity) -> bool {
    // SAFETY[AUDIT:typed-page-batch-scan]: the caller supplies initialized page
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
    // SAFETY[AUDIT:typed-page-batch-scan]: allocator page lookup accepts an
    // arbitrary candidate address and returns null when it is unregistered.
    let owner = unsafe { crate::internal::_mi_ptr_page(block.as_ptr().cast()) };
    if !owner.is_null() && owner != page.as_ptr() {
        BatchValidationError::DifferentPage
    } else {
        BatchValidationError::InvalidLink
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
    if capacity.get() == 0 {
        return Err(BatchValidationError::TooLong {
            observed_at_least: 1,
            capacity,
        });
    }

    let mut current = head;
    let mut observed = 0usize;
    loop {
        // SAFETY[AUDIT:typed-page-batch-scan]: membership classification reads
        // only live page metadata and never dereferences a rejected block.
        if !unsafe { block_belongs_to_page(page, current, capacity) } {
            return Err(unsafe { classify_foreign_link(page, current) });
        }

        observed += 1;
        // SAFETY[AUDIT:typed-page-batch-scan]: membership was checked before
        // reading the encoded link with the owning page's keys.
        let next = unsafe { current.next(page) };
        let Some(next) = next else {
            // SAFETY[AUDIT:typed-page-batch-scan]: observed is in 1..=capacity
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
        // SAFETY[AUDIT:typed-page-batch-scan]: the detached-list owner provides
        // live page metadata and unique traversal access.
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::remote_list_detached();
        let core = match unsafe { scan_detached_chain(page, head, capacity) } {
            Ok(core) => core,
            Err(error) => {
                #[cfg(feature = "bench-diagnostics")]
                crate::bench_diagnostics::remote_validation_failure(error);
                return Err(error);
            }
        };
        #[cfg(feature = "bench-diagnostics")]
        crate::bench_diagnostics::remote_list_scanned(core.len().get() as usize);
        if core.len().get() > used.get() {
            let error = BatchValidationError::ExceedsUsed {
                len: core.len(),
                used,
            };
            #[cfg(feature = "bench-diagnostics")]
            crate::bench_diagnostics::remote_validation_failure(error);
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
        // SAFETY[AUDIT:typed-page-batch-remote-collect]: the typed local owner
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
        // SAFETY[AUDIT:typed-page-batch-remote-collect]: claimed abandoned-page
        // ownership and the detached batch exclude competing collectors.
        unsafe { self.collect_into_page(owner.page(), current) }
    }

    #[inline]
    unsafe fn collect_into_page(self, owner: PagePtr, current: LocalFreeList) -> LocalFreeList {
        crate::internal::mi_debug_assert!(owner == self.page());
        // SAFETY[AUDIT:typed-page-batch-remote-collect]: the batch tail is
        // detached and all links share the owner's page-local encoding keys.
        unsafe { self.tail().set_next(owner, current.head()) };
        let new_local = LocalFreeList::from_head(self.head());
        // SAFETY[AUDIT:typed-page-batch-remote-collect]: batch construction
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
        // SAFETY[AUDIT:typed-page-batch-local-collect]: LocalPage proves
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
        // SAFETY[AUDIT:typed-page-batch-local-collect]: claimed abandoned-page
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
        // SAFETY[AUDIT:typed-page-batch-local-collect]: the local batch is
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
        // SAFETY[AUDIT:typed-page-batch-local-detach]: LocalPage proves
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
        // SAFETY[AUDIT:typed-page-batch-abandoned-local-detach]: claimed
        // abandoned-page ownership excludes competing local-list collectors.
        unsafe { detach_local_free_batch(self.page()) }
    }
}

#[inline]
unsafe fn detach_local_free_batch(
    page: PagePtr,
) -> Result<Option<LocalFreeBatch>, BatchValidationError> {
    // SAFETY[AUDIT:typed-page-batch-local-detach]: the local owner may read
    // its page-local list and capacity metadata.
    let local = unsafe { page.local_free() };
    let Some(head) = local.head() else {
        return Ok(None);
    };
    // SAFETY[AUDIT:typed-page-batch-local-detach]: the local owner may read
    // initialized page capacity while validating its current chain.
    let capacity = unsafe { page.capacity() };
    // SAFETY[AUDIT:typed-page-batch-local-detach]: the current local list
    // remains published and unchanged throughout the bounded scan.
    let core = unsafe { scan_detached_chain(page, head, capacity) }?;
    #[cfg(feature = "bench-diagnostics")]
    crate::bench_diagnostics::local_tail_scan(core.len().get() as usize);
    // SAFETY[AUDIT:typed-page-batch-local-detach]: validation succeeded, so
    // clearing the field uniquely transfers the complete chain to the batch.
    unsafe { page.set_local_free(LocalFreeList::empty()) };
    Ok(Some(LocalFreeBatch(core)))
}

impl AvailablePage {
    /// Detach up to `limit` available blocks into a lease.
    ///
    /// # Safety
    /// This witness must uniquely represent the page's non-empty available list.
    #[inline]
    pub(crate) unsafe fn lease(self, limit: BatchLimit) -> (LeasedBatch, ActivePage) {
        let page = self.page();
        // SAFETY[AUDIT:typed-page-batch-lease]: AvailablePage proves a nonempty
        // page free list and exclusive current-thread mutation access.
        let available = unsafe { page.free() };
        let Some(head) = available.head() else {
            // SAFETY[AUDIT:typed-page-batch-lease]: contradicting AvailablePage
            // is unreachable under its construction invariant.
            unsafe { core::hint::unreachable_unchecked() }
        };
        let mut tail = head;
        let mut len = 1u16;
        while len < limit.get() {
            // SAFETY[AUDIT:typed-page-batch-lease]: tail is a block in this
            // page's available chain and is readable with page-local keys.
            let Some(next) = (unsafe { tail.next(page) }) else {
                break;
            };
            tail = next;
            len += 1;
        }
        // SAFETY[AUDIT:typed-page-batch-lease]: tail is still linked in the
        // available chain, so its successor is the remaining free-list head.
        let remaining = unsafe { tail.next(page) };
        // SAFETY[AUDIT:typed-page-batch-lease]: detaching the recorded tail
        // creates a null-terminated, page-homogeneous leased batch.
        unsafe { tail.set_next(page, None) };
        let remaining = match remaining {
            Some(head) => AvailableFreeList::from_head(head),
            None => AvailableFreeList::empty(),
        };
        // SAFETY[AUDIT:typed-page-batch-lease]: len is initialized at one and
        // bounded by the nonzero u16 limit.
        let len = unsafe { BatchLen::new_unchecked(len) };
        // SAFETY[AUDIT:typed-page-batch-lease]: available blocks are excluded
        // from used, so leasing at most the available chain preserves capacity.
        unsafe {
            page.set_free(remaining);
            let used = page.used();
            let capacity = page.capacity();
            let Some(used) = used.checked_add_batch(len, capacity) else {
                core::hint::unreachable_unchecked()
            };
            page.set_used(used);
        }
        let batch = LeasedBatch(BatchCore {
            page,
            head,
            tail,
            len,
        });
        // Leasing changes only free/used, so consuming the available witness
        // returns the same initialized page in its active state.
        (batch, self.into_active())
    }
}

impl LeasedBatch {
    /// # Safety
    /// The lease must remain uniquely owned and detached.
    #[inline]
    pub(crate) unsafe fn take_one(self) -> (AllocatedBlock, Option<LeasedBatch>) {
        let head = self.head();
        let remaining = if self.len().get() == 1 {
            None
        } else {
            // SAFETY[AUDIT:typed-page-batch-lease]: a lease longer than one has
            // a non-null second block in the same detached chain.
            let Some(next) = (unsafe { head.next(self.page()) }) else {
                unsafe { core::hint::unreachable_unchecked() }
            };
            // SAFETY[AUDIT:typed-page-batch-lease]: len > 1, so len - 1 remains nonzero.
            let len = unsafe { BatchLen::new_unchecked(self.len().get() - 1) };
            Some(LeasedBatch(BatchCore {
                page: self.page(),
                head: next,
                tail: self.tail(),
                len,
            }))
        };
        // SAFETY[AUDIT:typed-page-batch-lease]: the detached lease head is now
        // uniquely removed and becomes an allocated-block mechanism witness.
        (unsafe { AllocatedBlock::new_unchecked(head) }, remaining)
    }

    /// # Safety
    /// `owner` must describe this lease's page destination.
    #[inline]
    pub(crate) unsafe fn return_to_page(self, owner: &LocalPage) {
        crate::internal::mi_debug_assert!(owner.page() == self.page());
        let page = owner.page();
        // SAFETY[AUDIT:typed-page-batch-lease]: LocalPage grants exclusive
        // access to the page's current available-list head.
        let available = unsafe { page.free() };
        // SAFETY[AUDIT:typed-page-batch-lease]: the lease tail is detached and
        // can be linked to the current page-local available head.
        unsafe { self.tail().set_next(page, available.head()) };
        // SAFETY[AUDIT:typed-page-batch-lease]: leased blocks remain counted in
        // used, so returning this batch subtracts its exact validated length.
        let used = unsafe {
            let used = page.used();
            let Some(used) = used.checked_sub_batch(self.len()) else {
                core::hint::unreachable_unchecked()
            };
            used
        };
        // SAFETY[AUDIT:typed-page-batch-lease]: publish both sides of the lease
        // return together so callers cannot update free without used, or vice versa.
        unsafe {
            page.set_free(AvailableFreeList::from_head(self.head()));
            page.set_used(used);
        }
    }
}

pub(super) fn __lint_touch() {
    // SAFETY[AUDIT:typed-page-batch-lint-fixture]: one is a valid non-empty
    // batch length used only to keep the staged error shape typechecked.
    let one = unsafe { BatchLen::new_unchecked(1) };
    let _exceeds_used = BatchValidationError::ExceedsUsed {
        len: one,
        used: crate::core_types::UsedBlockCount::new(0),
    };
    let _scan: unsafe fn(
        PagePtr,
        BlockPtr,
        PageCapacity,
    ) -> Result<BatchCore, BatchValidationError> = scan_detached_chain;
    let _remote = (
        RemoteFreeBatch::page as fn(&RemoteFreeBatch) -> PagePtr,
        RemoteFreeBatch::head as fn(&RemoteFreeBatch) -> BlockPtr,
        RemoteFreeBatch::tail as fn(&RemoteFreeBatch) -> BlockPtr,
        RemoteFreeBatch::len as fn(&RemoteFreeBatch) -> BatchLen,
    );
    let _local = (
        LocalFreeBatch::page as fn(&LocalFreeBatch) -> PagePtr,
        LocalFreeBatch::head as fn(&LocalFreeBatch) -> BlockPtr,
        LocalFreeBatch::tail as fn(&LocalFreeBatch) -> BlockPtr,
        LocalFreeBatch::len as fn(&LocalFreeBatch) -> BatchLen,
    );
    let _leased = (
        LeasedBatch::page as fn(&LeasedBatch) -> PagePtr,
        LeasedBatch::head as fn(&LeasedBatch) -> BlockPtr,
        LeasedBatch::tail as fn(&LeasedBatch) -> BlockPtr,
        LeasedBatch::len as fn(&LeasedBatch) -> BatchLen,
    );
    let _lease: unsafe fn(AvailablePage, BatchLimit) -> (LeasedBatch, ActivePage) =
        AvailablePage::lease;
    let _take_one: unsafe fn(LeasedBatch) -> (AllocatedBlock, Option<LeasedBatch>) =
        LeasedBatch::take_one;
    let _return: unsafe fn(LeasedBatch, &LocalPage) = LeasedBatch::return_to_page;
}

#[cfg(test)]
#[path = "../page_batch_tests/tests.rs"]
mod tests;
