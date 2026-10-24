//! Atomic helpers for the page `xthread_free` ownership word.
//!
//! This module owns the packed-word protocol for the remote free-list head plus
//! owner bit. Callers get typed operations for push, collect, claim, and release
//! instead of spelling bit manipulation or CAS ordering at each use site.

use core::sync::atomic::{AtomicUsize, Ordering};

use super::{AtomicUsizeField, BlockPtr};
use crate::core_types::{OwnedAbandonedPage, ThreadFreeOwner, ThreadFreeWord};

#[must_use = "remote push outcomes must be consumed to preserve publication ownership"]
pub(crate) enum RemotePushResult {
    Published {
        previous: ThreadFreeWord,
        head: PublishedRemoteHead,
    },
    Retry {
        observed: ThreadFreeWord,
    },
}

/// A block head that remains published in the page's atomic thread-free word.
#[repr(transparent)]
#[derive(Debug)]
#[must_use = "published remote heads remain owned by the atomic thread-free word"]
pub(crate) struct PublishedRemoteHead(BlockPtr);

impl PublishedRemoteHead {
    /// # Safety
    /// `head` must be the head still published in a valid page's thread-free
    /// word at a documented raw compatibility ingress.
    #[inline(always)]
    pub(crate) unsafe fn from_raw_compat(head: BlockPtr) -> Self {
        Self(head)
    }

    #[inline(always)]
    pub(crate) fn head(&self) -> BlockPtr {
        self.0
    }

    /// Consume the transient publication witness when another owner is
    /// responsible for collecting the still-published head.
    #[inline(always)]
    pub(crate) fn leave_published(self) {
        let Self(_head) = self;
    }

    /// Detach only the tail while leaving this head published atomically.
    ///
    /// # Safety
    /// `page` must own the abandoned-page thread-free protocol and `self`
    /// must be the head currently published for that page.
    #[inline(always)]
    pub(crate) unsafe fn detach_tail(
        self,
        page: &OwnedAbandonedPage,
    ) -> (PublishedRemoteHead, Option<DetachedRemoteList>) {
        let page_raw = page.page().as_ptr();
        let head_raw = self.0.as_ptr();
        let next = unsafe { crate::internal::mi_block_next(page_raw, head_raw) };
        if next.is_null() {
            return (self, None);
        }
        unsafe { crate::internal::mi_block_set_next(page_raw, head_raw, core::ptr::null_mut()) };
        let tail = unsafe { BlockPtr::new_unchecked(next) };
        (self, Some(DetachedRemoteList(Some(tail))))
    }
}

/// A remote chain no longer owned by the atomic thread-free word.
#[repr(transparent)]
#[derive(Debug)]
#[must_use = "detached remote lists must be collected or explicitly republished"]
pub(crate) struct DetachedRemoteList(Option<BlockPtr>);

impl DetachedRemoteList {
    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn head(&self) -> Option<BlockPtr> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    #[inline(always)]
    pub(in crate::types) fn into_head(self) -> Option<BlockPtr> {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadFree<'a>(AtomicUsizeField<'a>);

impl<'a> AtomicThreadFree<'a> {
    #[inline(always)]
    pub(crate) fn from_cell(field: &'a AtomicUsize) -> Self {
        Self(AtomicUsizeField::from_cell(field))
    }

    #[inline(always)]
    fn field(self) -> AtomicUsizeField<'a> {
        self.0
    }

    pub(crate) fn load_relaxed(self) -> ThreadFreeWord {
        ThreadFreeWord::from_raw(self.field().load(Ordering::Relaxed))
    }

    /// # Safety
    /// `block` must be uniquely owned for remote publication to this page, with
    /// its link initialized to `expected.head()` using this page's encoding.
    /// The page and block must stay live through the CAS and its result.
    pub(crate) unsafe fn try_push(
        self,
        expected: ThreadFreeWord,
        block: BlockPtr,
    ) -> RemotePushResult {
        let new_tf = ThreadFreeWord::from_parts(Some(block), ThreadFreeOwner::Owned);
        let mut raw_expected = expected.raw();
        if self.field().compare_exchange_weak(
            &mut raw_expected,
            new_tf.raw(),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            RemotePushResult::Published {
                previous: expected,
                head: PublishedRemoteHead(block),
            }
        } else {
            #[cfg(feature = "bench-diagnostics")]
            crate::allocator::bench_diagnostics::remote_free_cas_retry();
            RemotePushResult::Retry {
                observed: ThreadFreeWord::from_raw(raw_expected),
            }
        }
    }

    /// Clear the owned bit if `expected` still matches the live thread-free
    /// word, preserving the expected remote-list head. On failure, updates
    /// `expected` with the current typed word for the caller's collect/retry
    /// protocol.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool {
        if !expected.is_owned() {
            return false;
        }
        let new = expected.with_owner(ThreadFreeOwner::Unowned);
        let mut raw_expected = expected.raw();
        if self.field().compare_exchange_weak(
            &mut raw_expected,
            new.raw(),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            true
        } else {
            *expected = ThreadFreeWord::from_raw(raw_expected);
            false
        }
    }

    /// Atomically detach the current remote free-list head while preserving the
    /// ownership bit in `xthread_free`.
    ///
    /// Returning a `DetachedRemoteList` makes subsequent collection consume a
    /// uniquely owned chain instead of a raw `mi_block_t*`.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn detach_head(self) -> DetachedRemoteList {
        let mut old = ThreadFreeWord::from_raw(self.field().load(Ordering::Relaxed));
        loop {
            let head = old.head();
            if head.is_none() {
                return DetachedRemoteList(None);
            }
            let new = old.without_head();
            let mut raw_old = old.raw();
            if self.field().compare_exchange_weak(
                &mut raw_old,
                new.raw(),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                return DetachedRemoteList(head);
            }
            old = ThreadFreeWord::from_raw(raw_old);
        }
    }

    /// Claim ownership of the page's thread-free list by setting the owned bit
    /// (bit 0 of `xthread_free`). Returns `true` if the bit was previously clear
    /// (i.e. this thread successfully claimed ownership).
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn claim_acq_rel(self) -> bool {
        let old = self.field().fetch_or(1, Ordering::AcqRel);
        (old & 1) == 0
    }
}
