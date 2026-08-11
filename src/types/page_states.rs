//! Required types for page typestate signatures referenced in freelist_wrappers.

use super::freelist_wrappers::{AvailableFreeList, LocalFreeList};
use super::ptr::{PagePtr, ThreadHeapPtr};
use core::{marker::PhantomData, ptr};

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
    // SAFETY: callers establish exclusive
    // initialization ownership before creating the uninitialized page witness.
    pub(in crate::types) unsafe fn new_unchecked(ptr: PagePtr) -> Self {
        Self {
            ptr,
            _state: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn initialize(self, _owner: ThreadHeapLocal) -> ActivePage {
        // SAFETY: consuming `Page<Uninit>`
        // plus a local thread-heap owner establishes an initialized active
        // page witness for the same page pointer.
        unsafe { ActivePage::new_unchecked(self.ptr) }
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
            // SAFETY: each transparent page
            // witness is constructed only by callers that establish the
            // state-specific invariant in the generated safety contract.
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
    "`page` must be initialized, thread-owned, and not currently full, retired, destroyed, or abandoned."
);
impl_page_witness!(
    FullPage,
    "`page` must be initialized, thread-owned, and currently present in the full queue."
);
impl_page_witness!(
    RetiredPage,
    "`page` must be initialized, thread-owned, and in the retired countdown state."
);
impl_page_witness!(
    AvailablePage,
    "`page` must be initialized, thread-owned, and have at least one available block."
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
    "`page` must be marked abandoned and not currently owned by a live thread heap."
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
}

impl AllFreePage<Active> {
    /// # Safety
    /// `page` must be an active page whose blocks are all free.
    #[inline(always)]
    // SAFETY: callers have already checked that
    // this active page has no used blocks.
    pub(in crate::types) unsafe fn active_unchecked(page: ActivePage) -> Self {
        Self {
            page: Page {
                ptr: page.page(),
                _state: PhantomData,
            },
        }
    }
}

impl AllFreePage<Abandoned> {
    /// # Safety
    /// `page` must be an abandoned page whose blocks are all free.
    #[inline(always)]
    // SAFETY: callers have already checked that
    // this abandoned page has no used blocks.
    pub(in crate::types) unsafe fn abandoned_unchecked(page: AbandonedPageType) -> Self {
        Self {
            page: Page {
                ptr: page.page(),
                _state: PhantomData,
            },
        }
    }
}

impl ActivePage {
    #[inline(always)]
    // SAFETY: callers hold an active page
    // witness and may inspect the page-local free-list head.
    pub(crate) unsafe fn try_available(self) -> Result<AvailablePage, ActivePage> {
        let page = self.page();
        // SAFETY: active page metadata is live;
        // a null free head means there is no available block witness.
        if unsafe { page.free_raw().is_null() } {
            Err(self)
        } else {
            // SAFETY: a non-null free head
            // proves at least one available block for this active page.
            Ok(unsafe { AvailablePage::new_unchecked(page) })
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn into_full(
        self,
        from: crate::core_types::NormalPageQueue,
        to: crate::core_types::FullPageQueue,
    ) -> FullPage {
        // SAFETY: the caller supplies matching
        // typed source/target queues for an active page that is transitioning
        // into the full queue.
        unsafe {
            crate::allocator::page::mi_page_queue_enqueue_from_raw(
                to.ptr(),
                from.ptr(),
                true,
                self.page(),
            );
            crate::allocator::page::mi_page_set_in_full(self.page().as_ptr(), true);
            FullPage::new_unchecked(self.page())
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn try_mark_all_free(self) -> Result<AllFreePage<Active>, ActivePage> {
        let page = self.page();
        // SAFETY: active page metadata is live
        // and the used count determines the all-free witness.
        let used = unsafe { page.used().get() };
        if used == 0 {
            // SAFETY: zero used blocks proves
            // the active all-free page witness.
            Ok(unsafe { AllFreePage::active_unchecked(self) })
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub(crate) fn into_abandon_candidate(self) -> PageForAbandon {
        PageForAbandon::Active(self)
    }
}

impl FullPage {
    #[inline(always)]
    pub(crate) unsafe fn try_mark_all_free(self) -> Result<AllFreePage<Full>, FullPage> {
        let page = self.page();
        // SAFETY: full page metadata is live
        // and the used count determines the all-free witness.
        let used = unsafe { page.used().get() };
        if used == 0 {
            // SAFETY: zero used blocks proves
            // the full all-free page witness.
            Ok(unsafe { AllFreePage::full_unchecked(self) })
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub(crate) fn into_abandon_candidate(self) -> PageForAbandon {
        PageForAbandon::Full(self)
    }
}

impl AvailablePage {
    #[inline(always)]
    pub(crate) unsafe fn pop_free_block(
        self,
    ) -> (
        crate::core_types::validation_extra::AllocatedBlock,
        ActivePage,
    ) {
        let page = self.page();
        // SAFETY: `AvailablePage` proves a
        // non-empty page-local available list to pop from.
        let (block, next) = unsafe { crate::core_types::AvailableFreeList::pop_one(self) };
        // SAFETY: after popping one available
        // block, the page free head and used count are updated together.
        unsafe {
            page.set_free(next);
            page.set_used(page.used().increment_wrapping());
        }
        // SAFETY: after consuming the
        // available witness, the page remains active.
        (block, unsafe { ActivePage::new_unchecked(page) })
    }
}

impl LocalPage {
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

    #[inline(always)]
    pub(crate) unsafe fn push_free_block(
        self,
        block: crate::core_types::validation_extra::FreeBlock,
    ) -> crate::core_types::UsedBlockCount {
        let page = self.page();
        // SAFETY: `LocalPage`
        // proves current-thread ownership, and the caller supplies a free-block
        // witness for this page's local free-list domain.
        let local = unsafe { page.local_free() };
        // SAFETY: pushing the block
        // updates only the page-local encoded free-list link for this local
        // page.
        let new_local = unsafe { local.push(&self, block) };
        // SAFETY: after pushing one
        // block to the local free list, local_free and used are updated
        // together to preserve page accounting.
        unsafe {
            page.set_local_free(new_local);
            let used = page.used().decrement();
            page.set_used(used);
            used
        }
    }
}

impl AllFreePage<Active> {
    #[inline(always)]
    pub(crate) fn into_retired(self) -> RetiredPage {
        // SAFETY: an active all-free page can
        // enter the retired countdown state for the same page pointer.
        unsafe { RetiredPage::new_unchecked(self.page()) }
    }
}

impl<S> AllFreePage<S> {
    #[inline(always)]
    pub(crate) fn free_to_arena(self, owner: ThreadHeapLocal) -> DestroyedPage {
        crate::core_types::free_all_free_page_to_arena(self, owner)
    }
}

impl AbandonedPageType {
    #[inline(always)]
    pub(crate) unsafe fn try_claim_ownership(
        self,
    ) -> Result<OwnedAbandonedPage, AbandonedPageType> {
        let page = self.page();
        // SAFETY: abandoned page metadata is
        // live and owns the atomic thread-free cell used for claiming.
        let atomic = unsafe { page.thread_free_cell() };
        if unsafe { atomic.claim_acq_rel() } {
            // SAFETY: a successful claim owns
            // the abandoned page for reclaim or reabandon.
            Ok(unsafe { OwnedAbandonedPage::new_unchecked(page) })
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub(crate) fn remote_free_page(self) -> ForeignPage {
        // SAFETY: abandoned pages are not local
        // to the current thread and route remote frees through `ForeignPage`.
        unsafe { ForeignPage::new_unchecked(self.page()) }
    }
}

impl AbandonedMappedPageType {
    #[inline(always)]
    pub(crate) fn remote_free_page(self) -> ForeignPage {
        // SAFETY: abandoned-mapped pages are
        // not local to the current thread and route remote frees as foreign.
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
        // SAFETY: owned abandoned
        // page metadata is live and its used count determines the all-free witness.
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

    #[inline(always)]
    pub(crate) fn into_reclaimed(self, _target: ThreadHeapLocal) -> ActivePage {
        // SAFETY: ownership has been claimed
        // and the target local thread heap reclaims this page as active.
        unsafe { ActivePage::new_unchecked(self.page()) }
    }

    #[inline(always)]
    pub(crate) fn into_mapped_abandoned(self) -> AbandonedMappedPageType {
        // SAFETY: the owned abandoned page is
        // being returned to mapped abandoned state.
        unsafe { AbandonedMappedPageType::new_unchecked(self.page()) }
    }

    #[inline(always)]
    pub(crate) fn into_abandoned(self) -> AbandonedPageType {
        // SAFETY: the caller has
        // released atomic ownership while preserving abandoned metadata.
        unsafe { AbandonedPageType::new_unchecked(self.page()) }
    }
}

#[inline(always)]
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
    // SAFETY: callers have already checked that
    // this full page has no used blocks.
    pub(in crate::types) unsafe fn full_unchecked(page: FullPage) -> Self {
        Self {
            page: Page {
                ptr: page.page(),
                _state: PhantomData,
            },
        }
    }
}

impl AllFreePage<Abandoned> {
    /// Consume the all-free abandoned proof after the arena has released the
    /// corresponding raw page metadata.
    #[inline(always)]
    pub(crate) fn into_destroyed_after_arena_free(self) -> DestroyedPage {
        // SAFETY: the caller
        // performs arena release before invoking this terminal transition.
        unsafe { DestroyedPage::new_unchecked(self.page()) }
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
    StillAbandoned(AbandonedPageType),
    Mapped(AbandonedMappedPageType),
    Reclaimed(ActivePage),
    Destroyed(DestroyedPage),
}

impl AbandonedDisposition {
    /// Explicitly discharge the terminal disposition at a caller that does
    /// not need to continue with the resulting page state.
    #[inline(always)]
    pub(crate) fn discard(self) {
        match self {
            Self::StillAbandoned(page) => {
                let _ = page.page();
            }
            Self::Mapped(page) => {
                let _ = page.page();
            }
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
    // SAFETY: callers establish that this
    // thread-heap pointer is live and local for the page-state operation.
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
        // SAFETY: page metadata is live and its
        // typed thread-id cell owns free-path classification bits.
        let tid = unsafe { self.thread_id_cell().load_relaxed() };
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
