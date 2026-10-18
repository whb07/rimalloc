//! Required types for page typestate signatures referenced in freelist_wrappers.

use super::freelist_wrappers::{AvailableFreeList, LocalFreeList};
use super::ptr::{PagePtr, ThreadHeapPtr};
use core::{marker::PhantomData, ptr};

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct Page<S> {
    ptr: PagePtr,
    _state: PhantomData<S>,
}

impl<S> Page<S> {
    #[inline(always)]
    pub(crate) fn ptr(&self) -> PagePtr {
        self.ptr
    }
}

impl Page<Uninit> {
    /// # Safety
    /// `ptr` must identify page metadata that is being initialized and is not
    /// yet represented by another page-state witness.
    #[inline(always)]
    #[cfg(test)]
    pub(in crate::types) unsafe fn new_unchecked(ptr: PagePtr) -> Self {
        Self {
            ptr,
            _state: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn initialize(self, _owner: ThreadHeapLocal) -> ActivePage {
        unsafe { ActivePage::new_unchecked(self.ptr) }
    }

    /// Consume a fresh page whose initialization failed after its owner was
    /// returned to the abandoned state.
    ///
    /// # Safety
    /// The page must remain registered with live, stable metadata and backing,
    /// have an initialized MemId and geometry, have `used == 0`, retain the
    /// claimed thread-free owner bit, be marked abandoned, and have empty queue
    /// links. No competing mutation, relink, claim, or teardown may occur.
    #[inline(always)]
    pub(crate) unsafe fn into_all_free_after_failed_init(self) -> AllFreePage<Uninit> {
        AllFreePage { page: self }
    }
}

#[derive(Debug)]
pub(crate) enum Uninit {}
#[derive(Debug)]
pub(crate) enum Active {}
#[derive(Debug)]
pub(crate) enum Full {}
#[derive(Debug)]
pub(crate) enum Abandoned {}

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ActivePage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct FullPage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct RetiredPage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct AvailablePage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct LocalPage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ForeignPage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct AbandonedPageType(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct AbandonedMappedPageType(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct OwnedAbandonedPage(PagePtr);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct DestroyedPage(PagePtr);

macro_rules! impl_page_witness {
    ($name:ident, $doc:expr) => {
        impl $name {
            /// # Safety
            #[doc = $doc]
            #[inline(always)]
            pub(in crate::types) unsafe fn new_unchecked(page: PagePtr) -> Self {
                Self(page)
            }

            #[inline(always)]
            pub(crate) fn page(&self) -> PagePtr {
                self.0
            }
        }
    };
}

impl_page_witness!(
    ActivePage,
    "`page` must be live, initialized, exclusively thread-owned, and not currently full, retired, destroyed, or abandoned."
);

impl_page_witness!(
    FullPage,
    "`page` must be live, initialized, thread-owned, and currently present in its stable full queue."
);
impl_page_witness!(
    RetiredPage,
    "`page` must be initialized, thread-owned, and in the retired countdown state."
);
impl_page_witness!(
    AvailablePage,
    "`page` must be live, initialized, exclusively thread-owned, have a valid non-empty available list, and satisfy `used < capacity`."
);
impl_page_witness!(
    LocalPage,
    "`page` must be owned by the current thread heap for the operation being performed."
);
impl_page_witness!(
    ForeignPage,
    "`page` must be owned by another thread for the operation being performed."
);
impl_page_witness!(
    AbandonedPageType,
    "`page` must be live and initialized, marked abandoned, have a live aligned thread-free atomic governed by the remote-free protocol, and not be owned by a live thread heap."
);
impl_page_witness!(
    AbandonedMappedPageType,
    "`page` must be marked abandoned-mapped and retain mapped abandoned state."
);
impl_page_witness!(
    OwnedAbandonedPage,
    "`page` must be an abandoned page whose thread-free ownership was claimed by the caller."
);
impl_page_witness!(
    DestroyedPage,
    "`page` must have completed the transition to destroyed/free-to-arena state."
);

#[repr(transparent)]
#[derive(Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct AllFreePage<S> {
    page: Page<S>,
}

impl<S> AllFreePage<S> {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page.ptr()
    }

    /// Consume the all-free proof after the arena released this page's raw
    /// metadata and backing allocation.
    ///
    /// # Safety
    /// The caller must have unregistered and released this page's arena backing;
    /// the page pointer is retained only as a terminal identity and must not be
    /// dereferenced through the resulting witness.
    #[inline(always)]
    pub(crate) unsafe fn into_destroyed_after_arena_free(self) -> DestroyedPage {
        // SAFETY: the caller completed arena release; this terminal witness
        // preserves only the consumed page identity and grants no access.
        unsafe { DestroyedPage::new_unchecked(self.page()) }
    }
}

impl AllFreePage<Active> {
    /// # Safety
    /// `page` must be an active page whose blocks are all free.
    #[inline(always)]
    pub(in crate::types) unsafe fn active_unchecked(page: ActivePage) -> Self {
        Self {
            page: Page {
                ptr: page.page(),
                _state: PhantomData,
            },
        }
    }
}

impl ActivePage {
    /// Return an available-page witness when this active page has a free head.
    /// The consumed active witness carries local ownership and metadata stability.
    #[inline(always)]
    pub(crate) fn try_available(self) -> Result<AvailablePage, ActivePage> {
        let page = self.page();
        if unsafe { page.free_raw().is_null() } {
            Err(self)
        } else {
            Ok(unsafe { AvailablePage::new_unchecked(page) })
        }
    }

    /// Refine this locally owned active page when its stable used count is zero.
    #[inline(always)]
    pub(crate) fn try_mark_all_free(self) -> Result<AllFreePage<Active>, ActivePage> {
        let page = self.page();
        let used = unsafe { page.used().get() };
        if used == 0 {
            Ok(unsafe { AllFreePage::active_unchecked(self) })
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub(crate) fn into_abandon_candidate(self) -> PageForAbandon {
        PageForAbandon::Active(self)
    }

    /// Complete the externally performed detach-and-owner-clear transition.
    ///
    /// # Safety
    /// This live initialized page must be detached with empty queue links, marked abandoned after
    /// its current-thread owner was cleared, and retain the claimed thread-free owner bit. Its
    /// former heap association, page metadata, and remote-free atomic protocol must remain live and
    /// stable, with competing claim, relink, mutation, and teardown excluded.
    #[inline(always)]
    pub(crate) unsafe fn into_owned_abandoned_after_owner_clear(self) -> OwnedAbandonedPage {
        // SAFETY: the caller completed queue detach and abandoned-owner publication while retaining
        // the thread-free claim; consuming Active leaves the sole replacement ownership capability.
        unsafe { OwnedAbandonedPage::new_unchecked(self.page()) }
    }
}

impl FullPage {
    /// Refine this owned full page when its stable used count is zero.
    #[inline(always)]
    pub(crate) fn try_mark_all_free(self) -> Result<AllFreePage<Full>, FullPage> {
        let page = self.page();
        let used = unsafe { page.used().get() };
        if used == 0 {
            Ok(unsafe { AllFreePage::full_unchecked(self) })
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub(crate) fn into_abandon_candidate(self) -> PageForAbandon {
        PageForAbandon::Full(self)
    }

    /// Complete a raw full-queue removal that cleared the page's full flag.
    ///
    /// # Safety
    /// This page must remain live, initialized, and current-thread owned after being removed from
    /// its full queue. Its queue links must be empty, its full flag must be clear, and the caller
    /// must exclusively own the detach/state transition while excluding relink or teardown.
    #[inline(always)]
    pub(crate) unsafe fn into_active_after_detach(self) -> ActivePage {
        // SAFETY: raw queue removal completed the full-to-detached-active metadata transition;
        // consuming Full leaves one replacement Active capability for the same page.
        unsafe { ActivePage::new_unchecked(self.page()) }
    }
}

impl AvailablePage {
    /// Pop one block while consuming the witness that proves a non-empty
    /// available list and exclusive local ownership of its accounting transition.
    #[inline(always)]
    pub(crate) fn pop_free_block(
        self,
    ) -> (
        crate::core_types::validation_extra::AllocatedBlock,
        ActivePage,
    ) {
        let page = self.page();
        let (block, next) = unsafe { crate::core_types::AvailableFreeList::pop_one(self) };
        unsafe {
            page.set_free(next);
            page.set_used(page.used().increment_wrapping());
        }
        (block, unsafe { ActivePage::new_unchecked(page) })
    }
}

impl LocalPage {
    /// Consume a locally owned page after classifying it as full.
    ///
    /// # Safety
    /// This live initialized page must remain current-thread owned and its stable `in_full`
    /// classification must prove that it is linked in the owning heap's full queue. The caller
    /// must exclusively own local page and queue mutation, excluding relink or teardown while the
    /// returned capability is used. Protocol atomic remote-free publication remains permitted.
    #[inline(always)]
    pub(crate) unsafe fn into_full_after_classification(self) -> FullPage {
        // SAFETY: the caller-proven stable full classification refines the consumed local-page
        // capability into the corresponding full-page capability without duplicating ownership.
        unsafe { FullPage::new_unchecked(self.page()) }
    }

    /// Consume a locally owned page after classifying it as active and non-full.
    ///
    /// # Safety
    /// This live initialized page must remain current-thread owned and its stable classification
    /// must prove that it is active rather than full, retired, abandoned, or destroyed. The caller
    /// must exclusively own local page and queue mutation, excluding relink or teardown while the
    /// returned capability is used. Protocol atomic remote-free publication remains permitted.
    #[inline(always)]
    pub(crate) unsafe fn into_active_after_classification(self) -> ActivePage {
        // SAFETY: the caller-proven stable active classification refines the consumed local-page
        // capability into the corresponding active-page capability without duplicating ownership.
        unsafe { ActivePage::new_unchecked(self.page()) }
    }

    /// Ensure this locally owned page has an available block.
    ///
    /// # Safety
    /// This witness must represent the current thread's live page ownership,
    /// and its available and local free lists must be valid for that page. The
    /// caller must have exclusive write access to the page-local list fields
    /// and `free_is_zero` if a local-list transfer is required.
    #[inline(always)]
    pub(crate) unsafe fn try_ensure_available(self) -> Result<AvailablePage, LocalPage> {
        let page = self.page();
        // SAFETY: the local owner proves the page metadata and its available
        // list are live for this exclusive page-local operation.
        if !unsafe { page.free() }.is_empty() {
            // SAFETY: the observed non-empty available list establishes the
            // available-page witness for the same locally owned page.
            return Ok(unsafe { AvailablePage::new_unchecked(page) });
        }
        // SAFETY: the local owner proves the page metadata and local list are
        // live for this exclusive page-local operation.
        let local = unsafe { page.local_free() };
        if local.is_empty() {
            return Err(self);
        }
        // SAFETY: the available list was observed empty and `local` is the
        // current non-empty local list loaded from this same owner.
        unsafe { self.transfer_local_to_available(local) };
        // SAFETY: the ordered transfer just published a non-empty available
        // list for this same locally owned page.
        Ok(unsafe { AvailablePage::new_unchecked(page) })
    }

    /// Publish a recycled local list as the page's available list.
    ///
    /// # Safety
    /// `local` must be the current non-empty local free list for this page, the
    /// current available list must be empty, and the caller must have exclusive
    /// write access to both page-local list fields and `free_is_zero`.
    #[inline(always)]
    pub(crate) unsafe fn transfer_local_to_available(&self, local: LocalFreeList) {
        // SAFETY: the local owner and caller-proven list identity authorize the
        // ordered page-local metadata transition.
        unsafe { transfer_local_to_available(self.page(), local) };
    }

    /// Publish an already-spliced recycled list as the page's available list.
    ///
    /// # Safety
    /// `available` must be the new non-empty available list for this page, and
    /// the caller must have exclusive write access to the available-list field
    /// and `free_is_zero`.
    #[inline(always)]
    pub(crate) unsafe fn publish_recycled_available(&self, available: AvailableFreeList) {
        // SAFETY: the local owner and caller-proven list identity authorize the
        // ordered available-list publication.
        unsafe { publish_recycled_available(self.page(), available) };
    }

    /// Push a block freed by the owner onto the available list (LIFO reuse).
    ///
    /// # Safety
    /// `self` must remain the current thread's live page. `block` must be a
    /// valid free block owned by that page, and the caller must exclusively own
    /// its link word plus the page's available-list head, `free_is_zero`, and
    /// used-count transition.
    #[inline(always)]
    pub(crate) unsafe fn push_free_block(
        self,
        block: crate::core_types::validation_extra::FreeBlock,
    ) -> crate::core_types::UsedBlockCount {
        let page = self.page();
        let free = unsafe { page.free() };
        let new_free = unsafe { free.push(&self, block) };
        unsafe {
            publish_recycled_available(page, new_free);
            let used = page.used().decrement();
            page.set_used(used);
            used
        }
    }
}

impl AllFreePage<Active> {
    #[inline(always)]
    pub(crate) fn into_retired(self) -> RetiredPage {
        unsafe { RetiredPage::new_unchecked(self.page()) }
    }
}

impl RetiredPage {
    /// Leave the retired state after its countdown has been cleared.
    ///
    /// # Safety
    /// This page must remain live, initialized, locally owned, and not full. Its retirement
    /// countdown must already be zero, and the caller must exclusively own the countdown/state
    /// transition while preventing queue relink or teardown.
    #[inline(always)]
    pub(crate) unsafe fn into_active_after_countdown_cleared(self) -> ActivePage {
        // SAFETY: the caller has completed the external countdown-clear mutation; consuming the
        // retired witness leaves exactly one active capability for the same live queued page.
        unsafe { ActivePage::new_unchecked(self.page()) }
    }
}

impl AbandonedPageType {
    /// Atomically refine an abandoned-page witness into temporary ownership.
    /// The consumed abandoned witness carries the live atomic-field protocol.
    #[inline(always)]
    pub(crate) fn try_claim_ownership(self) -> Result<OwnedAbandonedPage, AbandonedPageType> {
        let page = self.page();
        let atomic = unsafe { page.thread_free_cell() };
        // SAFETY: the consumed abandoned witness keeps this atomic field live
        // and permits only its atomic ownership transition.
        if unsafe { atomic.claim_acq_rel() } {
            Ok(unsafe { OwnedAbandonedPage::new_unchecked(page) })
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub(crate) fn remote_free_page(self) -> ForeignPage {
        unsafe { ForeignPage::new_unchecked(self.page()) }
    }
}

impl AbandonedMappedPageType {
    #[inline(always)]
    pub(crate) fn remote_free_page(self) -> ForeignPage {
        unsafe { ForeignPage::new_unchecked(self.page()) }
    }
}

impl OwnedAbandonedPage {
    /// Publish a recycled local list as the page's available list.
    ///
    /// # Safety
    /// `local` must be the current non-empty local free list for this page, the
    /// current available list must be empty, and the caller must retain claimed
    /// ownership and exclusive write access to both page-local list fields and
    /// `free_is_zero`.
    #[inline(always)]
    pub(crate) unsafe fn transfer_local_to_available(&self, local: LocalFreeList) {
        // SAFETY: claimed abandoned ownership and caller-proven list identity
        // authorize the ordered page-local metadata transition.
        unsafe { transfer_local_to_available(self.page(), local) };
    }

    /// Publish an already-spliced recycled list as the page's available list.
    ///
    /// # Safety
    /// `available` must be the new non-empty available list for this page, and
    /// the caller must retain claimed ownership and exclusive write access to
    /// the available-list field and `free_is_zero`.
    #[inline(always)]
    pub(crate) unsafe fn publish_recycled_available(&self, available: AvailableFreeList) {
        // SAFETY: claimed abandoned ownership and caller-proven list identity
        // authorize the ordered available-list publication.
        unsafe { publish_recycled_available(self.page(), available) };
    }

    #[inline(always)]
    pub(crate) fn try_mark_all_free(self) -> Result<AllFreePage<Abandoned>, OwnedAbandonedPage> {
        let page = self.page();
        if unsafe { page.used().get() == 0 } {
            Ok(AllFreePage {
                page: Page {
                    ptr: page,
                    _state: PhantomData,
                },
            })
        } else {
            Err(self)
        }
    }

    /// # Safety
    /// The caller must have reassigned this live page's heap owner to `target`,
    /// collected its abandoned lists into local state, and established exclusive,
    /// non-full active metadata ready for insertion. The page must not yet be
    /// linked in a target queue, and concurrent teardown must remain excluded.
    #[inline(always)]
    pub(crate) unsafe fn into_reclaimed(self, _target: ThreadHeapLocal) -> ActivePage {
        // SAFETY: the caller completed the pre-enqueue ownership/list transfer;
        // the page is exclusively active and ready for target-queue insertion.
        unsafe { ActivePage::new_unchecked(self.page()) }
    }
}

#[inline(always)]
/// # Safety
/// `page` must remain live and exclusively locally owned. `local` must be its
/// current non-empty local list, the available list must be empty, and the
/// caller must exclusively own `free`, `local_free`, and `free_is_zero` for the
/// ordered publication.
unsafe fn transfer_local_to_available(page: PagePtr, local: LocalFreeList) {
    let raw = page.as_ptr();
    // SAFETY: the caller proves exclusive access, that `local` is this page's
    // current non-empty local list, and that the current available list is
    // empty. Preserve the allocator publication order: publish free, clear
    // local_free, then clear zero status.
    unsafe {
        page.set_free(local.into_available());
        page.set_local_free(LocalFreeList::empty());
        ptr::addr_of_mut!((*raw).free_is_zero).write(false);
    }
}

#[inline(always)]
/// # Safety
/// `page` must remain live under exclusive local or claimed-abandoned ownership.
/// `available` must be its new non-empty recycled list, and the caller must
/// exclusively own `free` and `free_is_zero` for the ordered publication.
unsafe fn publish_recycled_available(page: PagePtr, available: AvailableFreeList) {
    let raw = page.as_ptr();
    // SAFETY: the caller proves exclusive access and that `available` is this
    // page's new non-empty recycled list. Preserve free-before-flag ordering.
    unsafe {
        page.set_free(available);
        ptr::addr_of_mut!((*raw).free_is_zero).write(false);
    }
}

impl AllFreePage<Full> {
    /// # Safety
    /// `page` must be a full-queue page whose blocks are all free.
    #[inline(always)]
    pub(in crate::types) unsafe fn full_unchecked(page: FullPage) -> Self {
        Self {
            page: Page {
                ptr: page.page(),
                _state: PhantomData,
            },
        }
    }

    /// Complete a raw full-queue removal that cleared the page's full flag.
    ///
    /// # Safety
    /// This all-free page must remain live, initialized, and current-thread owned after being
    /// removed from its full queue. Its queue links must be empty, its full flag must be clear,
    /// and the caller must exclusively own the detach/state transition while excluding relink or
    /// teardown.
    #[inline(always)]
    pub(crate) unsafe fn into_active_after_detach(self) -> AllFreePage<Active> {
        // SAFETY: raw queue removal completed the full-to-detached-active metadata transition;
        // consuming the full all-free witness leaves one active all-free capability for the same
        // live page.
        unsafe { AllFreePage::active_unchecked(ActivePage::new_unchecked(self.page())) }
    }
}

#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
/// A single-load snapshot coupling a page's free path with the corresponding
/// owner capability.
pub(crate) enum FreeRouteSnapshot {
    LocalFast(LocalPage),
    LocalSlow(LocalPage),
    ForeignPlain(ForeignPage),
    ForeignSlow(ForeignPage),
    Abandoned(AbandonedPageType),
    AbandonedMapped(AbandonedMappedPageType),
}

#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) enum PageForAbandon {
    Active(ActivePage),
    Full(FullPage),
}

#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) enum PageAbandonOutcome {
    Abandoned,
    Destroyed,
}

#[must_use = "abandoned dispositions must be consumed or explicitly returned"]
pub(crate) enum AbandonedDisposition {
    StillAbandoned,
    Mapped,
    Reclaimed(ActivePage),
    Destroyed(DestroyedPage),
}

impl AbandonedDisposition {
    /// Explicitly discharge the terminal disposition at a caller that does
    /// not need to continue with the resulting page state.
    #[inline(always)]
    pub(crate) fn discard(self) {
        match self {
            Self::StillAbandoned | Self::Mapped => {}
            Self::Reclaimed(page) => {
                let _ = page.page();
            }
            Self::Destroyed(page) => {
                let _ = page.page();
            }
        }
    }
}

/// A local thread heap representation
#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct ThreadHeapLocal(ThreadHeapPtr);

impl ThreadHeapLocal {
    /// # Safety
    /// `theap` must be initialized and known to belong to the current thread, or
    /// be at an initialization/reclaim boundary that establishes local ownership.
    #[inline(always)]
    pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self {
        Self(theap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ThreadHeapPtr {
        self.0
    }

    /// Load an immediately available page from this thread heap's direct
    /// cache. The empty-page sentinel and ordinary pages with an empty current
    /// `free` head are both fast misses.
    ///
    /// # Safety
    /// This witness must identify the current live local thread heap. Its
    /// direct-cache slot must be the initialized empty-page sentinel or a live
    /// locally owned page whose current `free` head is valid to inspect.
    #[inline(always)]
    pub(crate) unsafe fn direct_available_page(
        self,
        bin: crate::core_types::DirectBin,
    ) -> Option<AvailablePage> {
        // SAFETY: the caller proves the direct-cache metadata is initialized.
        let page = unsafe { self.ptr().direct_page(bin) };
        // SAFETY: the caller proves either initialized sentinel metadata or a
        // live local page; a null free head is the shared fast-miss encoding.
        if unsafe { page.free_raw().is_null() } {
            None
        } else {
            // SAFETY: a non-null direct-cache free head proves immediate
            // availability on the same locally owned page.
            Some(unsafe { AvailablePage::new_unchecked(page) })
        }
    }
}

impl PagePtr {
    /// Load and classify this page's current free-route owner state once.
    ///
    /// # Safety
    /// `self` must identify live initialized page metadata registered for the
    /// free operation, and `current` must identify the executing thread.
    pub(crate) unsafe fn snapshot_free_route(
        self,
        current: crate::core_types::ThreadId,
    ) -> FreeRouteSnapshot {
        let tid = unsafe { self.thread_id_cell().load() };
        match tid.classify_free_path(current) {
            crate::core_types::FreePathKind::LocalFast => {
                // SAFETY: the owner bits classify this page as local to `current`.
                FreeRouteSnapshot::LocalFast(unsafe { LocalPage::new_unchecked(self) })
            }
            crate::core_types::FreePathKind::LocalSlowFlags => {
                // SAFETY: the owner bits classify this page as local to `current`.
                FreeRouteSnapshot::LocalSlow(unsafe { LocalPage::new_unchecked(self) })
            }
            remote_path => {
                if tid.is_abandoned() {
                    // SAFETY: the thread-id word marks this page as abandoned.
                    FreeRouteSnapshot::Abandoned(unsafe { AbandonedPageType::new_unchecked(self) })
                } else if tid.is_abandoned_mapped() {
                    // SAFETY: the thread-id word marks this page as abandoned-mapped.
                    FreeRouteSnapshot::AbandonedMapped(unsafe {
                        AbandonedMappedPageType::new_unchecked(self)
                    })
                } else if matches!(remote_path, crate::core_types::FreePathKind::RemotePlain) {
                    // SAFETY: the owner bits classify this page as foreign to `current`.
                    FreeRouteSnapshot::ForeignPlain(unsafe { ForeignPage::new_unchecked(self) })
                } else {
                    // SAFETY: the owner bits classify this page as foreign to `current`.
                    FreeRouteSnapshot::ForeignSlow(unsafe { ForeignPage::new_unchecked(self) })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
