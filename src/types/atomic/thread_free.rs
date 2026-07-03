//! Atomic helpers for the page `xthread_free` ownership word.
//!
//! This module owns the packed-word protocol for the remote free-list head plus
//! owner bit. Callers get typed operations for push, collect, claim, and release
//! instead of spelling bit manipulation or CAS ordering at each use site.

use core::sync::atomic::Ordering;

use super::{AtomicUsizeField, BlockPtr};
use crate::core_types::{LocalFreeList, OwnedAbandonedPage, ThreadFreeOwner, ThreadFreeWord};

pub(crate) enum RemotePushOutcome {
    Success,
    Failed(BlockPtr),
}

/// The remote free list collected from a thread-free word.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RemoteFreeList(Option<BlockPtr>);

impl RemoteFreeList {
    #[inline(always)]
    pub(crate) fn from_detached_thread_free_head(head: Option<BlockPtr>) -> Self {
        Self(head)
    }

    #[inline(always)]
    pub(crate) fn from_detached_head(head: BlockPtr) -> Self {
        Self(Some(head))
    }

    #[inline(always)]
    pub(crate) fn head(self) -> Option<BlockPtr> {
        self.0
    }

    #[inline(always)]
    pub(crate) fn is_empty(self) -> bool {
        self.0.is_none()
    }

    /// Collect the remote (thread-free) list into the page's local free list.
    ///
    /// Delegates to `mi_page_thread_collect_to_local_typed`, which splices
    /// `self`'s head into `(*page).local_free`, runs the corruption/length
    /// checks, and decrements `(*page).used`. Returns the updated local free
    /// list.
    ///
    /// Constrained to `OwnedAbandonedPage` (the reclaim path); regular-page
    /// collection uses the raw `_mi_page_free_collect` entry point directly.
    ///
    /// # Safety
    /// `page` must be a valid owned abandoned page; `self` must be the head
    /// popped from `(*page).xthread_free` via the ownership CAS protocol.
    #[inline]
    pub(crate) unsafe fn collect_into_local(
        self,
        page: OwnedAbandonedPage,
        local: LocalFreeList,
    ) -> LocalFreeList {
        // SAFETY[AUDIT:typed-atomic-thread-free]: the caller supplies the owned
        // abandoned page and remote-list head produced by the xthread_free CAS
        // protocol.
        unsafe { crate::page::mi_page_thread_collect_to_local_typed(page, self, local) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);

impl AtomicThreadFree {
    #[inline(always)]
    fn field(self) -> AtomicUsizeField {
        self.0
    }

    pub(crate) fn load_relaxed(self) -> ThreadFreeWord {
        // SAFETY[AUDIT:typed-atomic-thread-free]: `AtomicThreadFree` is constructed
        // only for valid `xthread_free` atomic fields and preserves relaxed load.
        ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) })
    }

    pub(crate) fn push_remote_acq_rel(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        let new_tf = ThreadFreeWord::from_parts(Some(block), ThreadFreeOwner::Owned);
        let mut expected = old.raw();
        // SAFETY[AUDIT:typed-atomic-thread-free]: the caller provides a valid
        // `xthread_free` field; failure updates the typed expected word.
        if unsafe {
            self.field().compare_exchange_weak(
                &mut expected,
                new_tf.raw(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
        } {
            Ok(new_tf)
        } else {
            *old = ThreadFreeWord(expected);
            Err(*old)
        }
    }

    pub(crate) fn compare_exchange_push(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        self.push_remote_acq_rel(old, block)
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
        // SAFETY[AUDIT:typed-atomic-thread-free]: the caller provides a valid
        // `xthread_free` field; the CAS clears only the ownership bit.
        if unsafe {
            self.field().compare_exchange_weak(
                &mut raw_expected,
                new.raw(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
        } {
            true
        } else {
            *expected = ThreadFreeWord(raw_expected);
            false
        }
    }

    pub(crate) unsafe fn compare_exchange_release_ownership(
        self,
        expected: &mut ThreadFreeWord,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-thread-free]: compatibility shim forwards to the
        // typed ownership-release CAS with the same xthread_free contract.
        unsafe { self.release_acq_rel(expected) }
    }

    /// Atomically detach the current remote free-list head while preserving the
    /// ownership bit in `xthread_free`.
    ///
    /// Mirrors the CAS loop in `mi_page_thread_free_collect`; returning a
    /// `RemoteFreeList` makes the subsequent abandoned-page collection consume
    /// the typed free-list wrapper instead of a raw `mi_block_t*`.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList {
        // SAFETY[AUDIT:typed-atomic-thread-free]: the caller provides a valid
        // `xthread_free` field; relaxed load mirrors the C collect loop pre-read.
        let mut old = ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) });
        loop {
            let head = old.head();
            if head.is_none() {
                return RemoteFreeList::from_detached_thread_free_head(None);
            }
            let new = old.without_head();
            let mut raw_old = old.raw();
            // SAFETY[AUDIT:typed-atomic-thread-free]: the CAS detaches only the
            // remote-list head while preserving the owner bit in the typed word.
            if unsafe {
                self.field().compare_exchange_weak(
                    &mut raw_old,
                    new.raw(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
            } {
                return RemoteFreeList::from_detached_thread_free_head(head);
            }
            old = ThreadFreeWord(raw_old);
        }
    }

    pub(crate) unsafe fn take_head_preserving_owner(self) -> RemoteFreeList {
        // SAFETY[AUDIT:typed-atomic-thread-free]: compatibility shim forwards to the
        // typed remote-head detach operation with the same xthread_free contract.
        unsafe { self.collect_acq_rel() }
    }

    /// Claim ownership of the page's thread-free list by setting the owned bit
    /// (bit 0 of `xthread_free`). Returns `true` if the bit was previously clear
    /// (i.e. this thread successfully claimed ownership).
    ///
    /// Mirrors `mi_page_claim_ownership` in `internal.rs`: a single
    /// `fetch_or(1, AcqRel)`.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field.
    #[inline]
    pub(crate) unsafe fn claim_acq_rel(self) -> bool {
        // SAFETY[AUDIT:typed-atomic-thread-free]: the caller provides a valid
        // `xthread_free` field and this RMW sets only the owner bit.
        let old = unsafe { self.field().fetch_or(1, Ordering::AcqRel) };
        (old & 1) == 0
    }

    pub(crate) unsafe fn claim_ownership(self) -> bool {
        // SAFETY[AUDIT:typed-atomic-thread-free]: compatibility shim forwards to the
        // typed ownership-claim RMW with the same xthread_free contract.
        unsafe { self.claim_acq_rel() }
    }

    /// Release ownership of the page's thread-free list, preserving the current
    /// list head. Loops with a weak CAS clearing the owned bit, retrying if the
    /// head changed concurrently. Returns `true` if the owned bit was cleared
    /// by this call.
    ///
    /// `expected_head`, if provided, is the head the caller believes is current;
    /// the CAS still validates against the live atomic value, so a mismatch just
    /// causes a retry rather than UB.
    ///
    /// # Safety
    /// `self` must point to a valid page's `xthread_free` field, and the caller
    /// must ensure no concurrent collector can leave the list in an inconsistent
    /// state (see `mi_abandoned_page_unown_from_free` for the full protocol).
    #[inline]
    pub(crate) unsafe fn release_ownership(self, _expected_head: Option<BlockPtr>) -> bool {
        loop {
            // SAFETY[AUDIT:typed-atomic-thread-free]: the caller provides a valid
            // `xthread_free` field; acquire load observes concurrent list updates.
            let current = unsafe { self.field().load(Ordering::Acquire) };
            let current_word = ThreadFreeWord(current);
            let new = current_word.with_owner(ThreadFreeOwner::Unowned).raw();
            let mut expected = current;
            // SAFETY[AUDIT:typed-atomic-thread-free]: the CAS clears only the
            // ownership bit while preserving the observed remote-list head.
            if unsafe {
                self.field().compare_exchange_weak(
                    &mut expected,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
            } {
                return current_word.is_owned();
            }
        }
    }
}
