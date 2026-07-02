//! DT-13: Atomic field wrappers.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};

use super::freelist::{ThreadFreeOwner, ThreadFreeWord};
use super::freelist_wrappers::LocalFreeList;
use super::page::{PageFlags, PageOwnerId, PageThreadId};
use super::page_states::OwnedAbandonedPage;
use super::ptr::BlockPtr;

pub(crate) type MemoryOrder = Ordering;

/// Atomic access to a raw `usize` field.
///
/// # Safety
/// The pointer must be non-null, properly aligned for `AtomicUsize`, and valid
/// for the duration of each operation. While this wrapper is used, all
/// concurrent accesses to the same storage must be atomic and use compatible
/// aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);

impl AtomicUsizeField {
    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder) {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_add(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_sub(value, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_or(self, bits: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_or(bits, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn fetch_and(self, bits: usize, order: MemoryOrder) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        atomic.fetch_and(bits, order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(
        self,
        expected: &mut usize,
        desired: usize,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_weak(
        self,
        expected: &mut usize,
        desired: usize,
        success: MemoryOrder,
        failure: MemoryOrder,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic usize field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicUsize::from_ptr(self.0) };
        match atomic.compare_exchange_weak(*expected, desired, success, failure) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

/// Atomic access to a raw `i64` field.
///
/// # Safety
/// The pointer must be non-null, properly aligned for `AtomicI64`, and valid
/// for the duration of each operation. While this wrapper is used, all
/// concurrent accesses to the same storage must be atomic and use compatible
/// aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicI64Field(pub(crate) *mut i64);

impl AtomicI64Field {
    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64 {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder) {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic i64 field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicI64::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

/// Atomic access to a raw pointer field.
///
/// # Safety
/// The pointer-to-pointer must be non-null, properly aligned for
/// `AtomicPtr<T>`, and valid for the duration of each operation. While this
/// wrapper is used, all concurrent accesses to the same storage must be atomic
/// and use compatible aliases.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);

impl<T> AtomicPtrField<T> {
    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.load(order)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder) {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.store(value, order);
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        atomic.swap(value, Ordering::AcqRel)
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_release(
        self,
        expected: &mut *mut T,
        desired: *mut T,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }

    /// # Safety
    /// See the type-level safety contract.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_acq_rel(
        self,
        expected: &mut *mut T,
        desired: *mut T,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: callers uphold this wrapper's
        // atomic pointer field contract before creating the temporary atomic view.
        let atomic = unsafe { AtomicPtr::from_ptr(self.0) };
        match atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => true,
            Err(actual) => {
                *expected = actual;
                false
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);

impl AtomicPageThreadId {
    /// Load the page's tagged thread-id word (thread id + flag bits) as a typed
    /// `PageThreadId`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn load(self) -> PageThreadId {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid page
        // `xthread_id` atomic field and this wrapper preserves relaxed loading.
        PageThreadId::new(unsafe { self.0.load(Ordering::Relaxed) })
    }

    /// Compatibility shim for callers not yet migrated to the `load` name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn load_relaxed(self) -> PageThreadId {
        // SAFETY[AUDIT:typed-atomic-domain]: compatibility shim forwards to the
        // typed relaxed page-thread-id loader with the same caller contract.
        unsafe { self.load() }
    }

    /// Set or clear the given flag bits in the page's `xthread_id` field using a
    /// relaxed `fetch_or` / `fetch_and`. Returns the previous `PageThreadId`.
    ///
    /// Mirrors `mi_page_flags_set`: a single relaxed atomic RMW on `xthread_id`.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn set_flags(self, flags: PageFlags, set: bool) -> PageThreadId {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid page
        // `xthread_id` atomic field; the RMW keeps C's relaxed flag update.
        let old = if set {
            unsafe { self.0.fetch_or(flags.bits(), Ordering::Relaxed) }
        } else {
            unsafe { self.0.fetch_and(!flags.bits(), Ordering::Relaxed) }
        };
        PageThreadId::new(old)
    }

    /// Release ownership of the page by writing the owner-encoding of `owner`
    /// (clearing the live thread id, preserving flag bits) with a relaxed store.
    /// Used when a thread abandons a page.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field, and the caller
    /// must hold the page in a state where a relaxed store is safe (no
    /// concurrent free-path readers that require Acq/Rel ordering).
    #[inline(always)]
    pub(crate) unsafe fn store_owner(self, owner: PageOwnerId) {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid page
        // `xthread_id` field and page state that permits relaxed owner updates.
        let current = unsafe { self.0.load(Ordering::Relaxed) };
        // `with_owner` preserves the existing flag bits, so no separate
        // `.with_flags(...)` is needed.
        let new = PageThreadId::new(current).with_owner(owner);
        // SAFETY[AUDIT:typed-atomic-domain]: the new typed word preserves the
        // existing flag bits and writes only the page owner encoding.
        unsafe { self.0.store(new.raw(), Ordering::Relaxed) };
    }

    /// Compatibility shim for callers not yet migrated to the `store_owner`
    /// name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field, and the caller
    /// must hold the page in a state where a relaxed store is safe.
    #[inline(always)]
    pub(crate) unsafe fn set_owner_release(self, owner: PageOwnerId) {
        // SAFETY[AUDIT:typed-atomic-domain]: compatibility shim forwards to the
        // typed owner store with the same page-state contract.
        unsafe { self.store_owner(owner) };
    }

    /// Release-CAS the page's tagged thread id from `expected` to `new`.
    /// On failure, updates `expected` with the observed typed value.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_owner(
        self,
        expected: &mut PageThreadId,
        new: PageThreadId,
    ) -> bool {
        let mut raw_expected = expected.raw();
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid page
        // `xthread_id` field; failure writes the observed raw word back to a typed
        // `PageThreadId`.
        let exchanged = unsafe {
            self.0.compare_exchange_weak(
                &mut raw_expected,
                new.raw(),
                Ordering::Release,
                Ordering::Relaxed,
            )
        };
        if !exchanged {
            *expected = PageThreadId::new(raw_expected);
        }
        exchanged
    }

    /// Compatibility shim for callers not yet migrated to the
    /// `compare_exchange_owner` name.
    ///
    /// # Safety
    /// `self.0` must point to a valid page's `xthread_id` field.
    #[inline(always)]
    pub(crate) unsafe fn compare_exchange_release(
        self,
        expected: &mut PageThreadId,
        new: PageThreadId,
    ) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: compatibility shim forwards to the
        // typed owner CAS with the same page-thread-id contract.
        unsafe { self.compare_exchange_owner(expected, new) }
    }
}

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
        // SAFETY[AUDIT:typed-atomic-domain]: the caller supplies the owned
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
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicThreadFree` is constructed
        // only for valid `xthread_free` atomic fields and preserves relaxed load.
        ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) })
    }

    pub(crate) fn push_remote_acq_rel(
        self,
        old: &mut ThreadFreeWord,
        block: BlockPtr,
    ) -> Result<ThreadFreeWord, ThreadFreeWord> {
        let new_tf =
            ThreadFreeWord::from_parts(Some(block), crate::core_types::ThreadFreeOwner::Owned);
        let mut expected = old.raw();
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
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
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
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
        // SAFETY[AUDIT:typed-atomic-domain]: compatibility shim forwards to the
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
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
        // `xthread_free` field; relaxed load mirrors the C collect loop pre-read.
        let mut old = ThreadFreeWord(unsafe { self.field().load(Ordering::Relaxed) });
        loop {
            let head = old.head();
            if head.is_none() {
                return RemoteFreeList::from_detached_thread_free_head(None);
            }
            let new = old.without_head();
            let mut raw_old = old.raw();
            // SAFETY[AUDIT:typed-atomic-domain]: the CAS detaches only the
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
        // SAFETY[AUDIT:typed-atomic-domain]: compatibility shim forwards to the
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
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
        // `xthread_free` field and this RMW sets only the owner bit.
        let old = unsafe { self.field().fetch_or(1, Ordering::AcqRel) };
        (old & 1) == 0
    }

    pub(crate) unsafe fn claim_ownership(self) -> bool {
        // SAFETY[AUDIT:typed-atomic-domain]: compatibility shim forwards to the
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
            // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
            // `xthread_free` field; acquire load observes concurrent list updates.
            let current = unsafe { self.field().load(Ordering::Acquire) };
            let current_word = ThreadFreeWord(current);
            let new = current_word.with_owner(ThreadFreeOwner::Unowned).raw();
            let mut expected = current;
            // SAFETY[AUDIT:typed-atomic-domain]: the CAS clears only the
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

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);

impl AtomicAbandonedCount {
    #[inline(always)]
    fn field(self) -> AtomicUsizeField {
        AtomicUsizeField(self.0.cast_mut().cast::<usize>())
    }

    pub(crate) unsafe fn load_relaxed(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed loading.
        unsafe { self.field().load(Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed increment.
        unsafe { self.field().fetch_add(value, Ordering::Relaxed) }
    }

    pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicAbandonedCount` points to an
        // allocator-owned abandoned-count atomic and preserves relaxed decrement.
        unsafe { self.field().fetch_sub(value, Ordering::Relaxed) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);

impl AtomicThreadHeapRefcount {
    pub(crate) unsafe fn store_seq_cst(self, value: usize) {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid thread
        // heap refcount field and preserves the existing SeqCst store.
        unsafe { self.0.store(value, Ordering::SeqCst) }
    }

    pub(crate) unsafe fn incref_acq_rel(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid thread
        // heap refcount field and preserves the existing AcqRel increment.
        unsafe { self.0.fetch_add(1, Ordering::AcqRel) }
    }

    pub(crate) unsafe fn decref_acq_rel(self) -> usize {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid thread
        // heap refcount field and preserves the existing AcqRel decrement.
        unsafe { self.0.fetch_sub(1, Ordering::AcqRel) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);

impl AtomicArenaPage {
    pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t {
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicArenaPage` points to an
        // allocator-owned page-table pointer atomic and preserves acquire load.
        let atomic = unsafe { &*self.0 };
        atomic
            .load(Ordering::Acquire)
            .cast::<crate::types::mi_arena_pages_t>()
    }

    pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t) {
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicArenaPage` points to an
        // allocator-owned page-table pointer atomic and preserves release store.
        let atomic = unsafe { &*self.0 };
        atomic.store(value.cast::<c_void>(), Ordering::Release);
    }
}

pub(crate) struct AtomicGuard<'a> {
    pub(crate) field: AtomicUsizeField,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

impl<'a> AtomicGuard<'a> {
    /// # Safety
    /// `field` must refer to a valid atomic guard word.
    #[inline(always)]
    pub(crate) unsafe fn try_acquire(field: AtomicUsizeField) -> Option<Self> {
        let mut expected = 0;
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid guard
        // word and acquisition uses the existing AcqRel/Acquire CAS contract.
        if unsafe { field.compare_exchange_acq_rel(&mut expected, 1) } {
            Some(Self {
                field,
                _borrow: PhantomData,
            })
        } else {
            None
        }
    }
}

impl Drop for AtomicGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-atomic-domain]: `AtomicGuard` is constructed only
        // after acquiring this guard word.
        unsafe { self.field.store(0, Ordering::Release) };
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct OnceStatePtr(pub(crate) *mut usize);

pub(crate) struct OnceGuard<'a> {
    pub(crate) once: OnceStatePtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

impl<'a> OnceGuard<'a> {
    /// # Safety
    /// `once` must refer to a valid allocator once-state.
    #[inline(always)]
    pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self> {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
        // allocator once-state and the guard releases it in Drop.
        if unsafe {
            crate::atomic::_mi_atomic_once_enter(once.0.cast::<crate::atomic::mi_atomic_once_t>())
        } {
            Some(Self {
                once,
                _borrow: PhantomData,
            })
        } else {
            None
        }
    }
}

impl Drop for OnceGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-atomic-domain]: `OnceGuard` is constructed only
        // after entering this once-state.
        unsafe {
            crate::atomic::_mi_atomic_once_release(
                self.once.0.cast::<crate::atomic::mi_atomic_once_t>(),
            )
        };
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>);

pub(crate) struct MiLockGuard<'a> {
    pub(crate) lock: MiLockPtr,
    pub(crate) _borrow: PhantomData<&'a ()>,
}

impl<'a> MiLockGuard<'a> {
    /// # Safety
    /// `lock` must refer to a valid allocator lock.
    #[inline(always)]
    pub(crate) unsafe fn acquire(lock: MiLockPtr) -> Self {
        // SAFETY[AUDIT:typed-atomic-domain]: the caller provides a valid
        // allocator lock and the guard releases it in Drop.
        unsafe { crate::atomic::mi_lock_acquire(lock.0.as_ptr()) };
        Self {
            lock,
            _borrow: PhantomData,
        }
    }
}

impl Drop for MiLockGuard<'_> {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-atomic-domain]: `MiLockGuard` is constructed only
        // after acquiring this lock.
        unsafe { crate::atomic::mi_lock_release(self.lock.0.as_ptr()) };
    }
}

pub(super) fn __lint_touch() {
    let _ = AtomicPageThreadId::load_relaxed as unsafe fn(AtomicPageThreadId) -> PageThreadId;
    let _ = AtomicPageThreadId::set_owner_release as unsafe fn(AtomicPageThreadId, PageOwnerId);
    let _ = AtomicPageThreadId::compare_exchange_release
        as unsafe fn(AtomicPageThreadId, &mut PageThreadId, PageThreadId) -> bool;
    let _ = core::mem::size_of::<RemotePushOutcome>();
    if let RemotePushOutcome::Failed(block) =
        // SAFETY[AUDIT:typed-atomic-domain]: lint-only fixture uses a dangling
        // non-dereferenced block identity to keep enum field reachability checked.
        RemotePushOutcome::Failed(unsafe {
            BlockPtr::new_unchecked(core::ptr::dangling_mut())
        })
    {
        let _ = block.as_ptr();
    }
    let _ = RemotePushOutcome::Success;
    let _ = AtomicThreadFree::compare_exchange_push
        as fn(
            AtomicThreadFree,
            &mut ThreadFreeWord,
            BlockPtr,
        ) -> Result<ThreadFreeWord, ThreadFreeWord>;
    let _ = AtomicThreadFree::compare_exchange_release_ownership
        as unsafe fn(AtomicThreadFree, &mut ThreadFreeWord) -> bool;
    let _ = AtomicThreadFree::take_head_preserving_owner
        as unsafe fn(AtomicThreadFree) -> RemoteFreeList;
    let _ = AtomicThreadFree::claim_ownership as unsafe fn(AtomicThreadFree) -> bool;
    let _ = AtomicThreadFree::release_ownership
        as unsafe fn(AtomicThreadFree, Option<BlockPtr>) -> bool;
}

#[cfg(test)]
mod tests;

#[cfg(all(test, not(miri)))]
mod concurrency_models;
