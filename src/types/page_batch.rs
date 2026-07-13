//! Linear ownership values for detached, page-homogeneous block chains.

use crate::core_types::{BatchLen, BlockPtr, PageCapacity, PagePtr};

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
}

#[cfg(test)]
#[path = "../page_batch_tests/tests.rs"]
mod tests;
