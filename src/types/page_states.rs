//! Required types for page typestate signatures referenced in freelist_wrappers.

use super::ptr::{PagePtr, ThreadHeapPtr};
use core::marker::PhantomData;

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
    // SAFETY[AUDIT:typed-page-uninit-initialization-boundary]: callers establish exclusive
    // initialization ownership before creating the uninitialized page witness.
    pub(crate) unsafe fn new_unchecked(ptr: PagePtr) -> Self {
        Self {
            ptr,
            _state: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn initialize(self, _owner: ThreadHeapLocal) -> ActivePage {
        // SAFETY[AUDIT:typed-page-uninit-initialization-boundary]: consuming `Page<Uninit>`
        // plus a local thread-heap owner establishes an initialized active
        // page witness for the same page pointer.
        unsafe { ActivePage::new_unchecked(self.ptr) }
    }
}

pub(crate) enum Uninit {}
pub(crate) enum Active {}
pub(crate) enum Full {}
pub(crate) enum Retired {}
pub(crate) enum Abandoned {}
pub(crate) enum AbandonedMapped {}
pub(crate) enum Destroyed {}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct ActivePage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct FullPage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct RetiredPage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AvailablePage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct LocalPage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct ForeignPage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AbandonedPageType(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct AbandonedMappedPageType(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct OwnedAbandonedPage(PagePtr);

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct DestroyedPage(PagePtr);

macro_rules! impl_page_witness {
    ($name:ident, $doc:expr) => {
        impl $name {
            /// # Safety
            #[doc = $doc]
            #[inline(always)]
            // SAFETY[AUDIT:typed-page-witness-constructor-boundary]: each transparent page
            // witness is constructed only by callers that establish the
            // state-specific invariant in the generated safety contract.
            pub(crate) unsafe fn new_unchecked(page: PagePtr) -> Self {
                Self(page)
            }

            #[inline(always)]
            pub(crate) fn page(self) -> PagePtr {
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
    // SAFETY[AUDIT:typed-page-all-free-witness-boundary]: callers have already checked that
    // this active page has no used blocks.
    pub(crate) unsafe fn active_unchecked(page: ActivePage) -> Self {
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
    // SAFETY[AUDIT:typed-page-all-free-witness-boundary]: callers have already checked that
    // this abandoned page has no used blocks.
    pub(crate) unsafe fn abandoned_unchecked(page: AbandonedPageType) -> Self {
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
    // SAFETY[AUDIT:typed-page-active-transition-boundary]: callers hold an active page
    // witness and may inspect the page-local free-list head.
    pub(crate) unsafe fn available(self) -> Option<AvailablePage> {
        let page = self.page();
        // SAFETY[AUDIT:typed-page-active-transition-boundary]: active page metadata is live;
        // a null free head means there is no available block witness.
        if unsafe { (*page.as_ptr()).free.is_null() } {
            None
        } else {
            // SAFETY[AUDIT:typed-page-active-transition-boundary]: a non-null free head
            // proves at least one available block for this active page.
            Some(unsafe { AvailablePage::new_unchecked(page) })
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn to_full(
        self,
        from: crate::core_types::NormalPageQueue,
        to: crate::core_types::FullPageQueue,
    ) -> FullPage {
        // SAFETY[AUDIT:typed-page-active-transition-boundary]: the caller supplies matching
        // typed source/target queues for an active page that is transitioning
        // into the full queue.
        unsafe {
            crate::page::mi_page_queue_enqueue_from_raw(to.ptr(), from.ptr(), true, self.page());
            crate::page::mi_page_set_in_full(self.page().as_ptr(), true);
            FullPage::new_unchecked(self.page())
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn mark_all_free(self) -> Option<AllFreePage<Active>> {
        let page = self.page();
        // SAFETY[AUDIT:typed-page-active-transition-boundary]: active page metadata is live
        // and the used count determines the all-free witness.
        let used = unsafe { (*page.as_ptr()).used };
        if used == 0 {
            // SAFETY[AUDIT:typed-page-active-transition-boundary]: zero used blocks proves
            // the active all-free page witness.
            Some(unsafe { AllFreePage::active_unchecked(self) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn for_abandon(self) -> PageForAbandon {
        PageForAbandon::Active(self)
    }
}

impl FullPage {
    #[inline(always)]
    pub(crate) unsafe fn mark_all_free(self) -> Option<AllFreePage<Full>> {
        let page = self.page();
        // SAFETY[AUDIT:typed-page-full-transition-boundary]: full page metadata is live
        // and the used count determines the all-free witness.
        let used = unsafe { (*page.as_ptr()).used };
        if used == 0 {
            // SAFETY[AUDIT:typed-page-full-transition-boundary]: zero used blocks proves
            // the full all-free page witness.
            Some(unsafe { AllFreePage::full_unchecked(self) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn for_abandon(self) -> PageForAbandon {
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
        // SAFETY[AUDIT:typed-page-available-pop-boundary]: `AvailablePage` proves a
        // non-empty page-local available list to pop from.
        let (block, next) = unsafe { crate::core_types::AvailableFreeList::pop(self) };
        // SAFETY[AUDIT:typed-page-available-pop-boundary]: after popping one available
        // block, the page free head and used count are updated together.
        unsafe {
            page.set_free(next);
            page.set_used(page.used().increment_wrapping());
        }
        // SAFETY[AUDIT:typed-page-available-pop-boundary]: after consuming the
        // available witness, the page remains active.
        (block, unsafe { ActivePage::new_unchecked(page) })
    }
}

impl AllFreePage<Active> {
    #[inline(always)]
    pub(crate) fn retire(self) -> RetiredPage {
        // SAFETY[AUDIT:typed-page-retire-destroy-boundary]: an active all-free page can
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

impl RetiredPage {
    #[inline(always)]
    pub(crate) fn expire(self) -> DestroyedPage {
        // SAFETY[AUDIT:typed-page-retire-destroy-boundary]: an expired retired page has
        // completed the transition to destroyed/free-to-arena state.
        unsafe { DestroyedPage::new_unchecked(self.page()) }
    }
}

impl AbandonedPageType {
    #[inline(always)]
    pub(crate) unsafe fn claim_ownership(self) -> Option<OwnedAbandonedPage> {
        let page = self.page();
        // SAFETY[AUDIT:typed-page-abandoned-claim-boundary]: abandoned page metadata is
        // live and owns the atomic thread-free cell used for claiming.
        let atomic = unsafe { page.thread_free_cell() };
        if unsafe { atomic.claim_acq_rel() } {
            // SAFETY[AUDIT:typed-page-abandoned-claim-boundary]: a successful claim owns
            // the abandoned page for reclaim or reabandon.
            Some(unsafe { OwnedAbandonedPage::new_unchecked(page) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn remote_free_page(self) -> ForeignPage {
        // SAFETY[AUDIT:typed-page-remote-free-classification-boundary]: abandoned pages are not local
        // to the current thread and route remote frees through `ForeignPage`.
        unsafe { ForeignPage::new_unchecked(self.page()) }
    }
}

impl AbandonedMappedPageType {
    #[inline(always)]
    pub(crate) fn remote_free_page(self) -> ForeignPage {
        // SAFETY[AUDIT:typed-page-remote-free-classification-boundary]: abandoned-mapped pages are
        // not local to the current thread and route remote frees as foreign.
        unsafe { ForeignPage::new_unchecked(self.page()) }
    }
}

impl OwnedAbandonedPage {
    #[inline(always)]
    pub(crate) fn reclaim(self, _target: ThreadHeapLocal) -> ActivePage {
        // SAFETY[AUDIT:typed-page-abandoned-reclaim-boundary]: ownership has been claimed
        // and the target local thread heap reclaims this page as active.
        unsafe { ActivePage::new_unchecked(self.page()) }
    }

    #[inline(always)]
    pub(crate) fn reabandon_to_mapped(self) -> AbandonedMappedPageType {
        // SAFETY[AUDIT:typed-page-abandoned-reclaim-boundary]: the owned abandoned page is
        // being returned to mapped abandoned state.
        unsafe { AbandonedMappedPageType::new_unchecked(self.page()) }
    }
}

impl AllFreePage<Full> {
    /// # Safety
    /// `page` must be a full-queue page whose blocks are all free.
    #[inline(always)]
    // SAFETY[AUDIT:typed-page-all-free-witness-boundary]: callers have already checked that
    // this full page has no used blocks.
    pub(crate) unsafe fn full_unchecked(page: FullPage) -> Self {
        Self {
            page: Page {
                ptr: page.page(),
                _state: PhantomData,
            },
        }
    }
}

pub(crate) enum PageFreeOwner {
    Local(LocalPage),
    Foreign(ForeignPage),
    Abandoned(AbandonedPageType),
    AbandonedMapped(AbandonedMappedPageType),
}

#[derive(Clone, Copy)]
pub(crate) enum PageForAbandon {
    Active(ActivePage),
    Full(FullPage),
}

pub(crate) enum PageAbandonOutcome {
    Abandoned(AbandonedPageType),
    Destroyed(DestroyedPage),
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
    // SAFETY[AUDIT:typed-thread-heap-local-witness-boundary]: callers establish that this
    // thread-heap pointer is live and local for the page-state operation.
    pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self {
        Self(theap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ThreadHeapPtr {
        self.0
    }

    #[inline(always)]
    pub(crate) fn borrowed(self) -> crate::core_types::ThreadHeapBorrowed {
        // SAFETY[AUDIT:typed-thread-heap-local-witness-boundary]: a local thread heap is also a
        // live borrowed thread-heap witness.
        unsafe { crate::core_types::ThreadHeapBorrowed::new_unchecked(self.0) }
    }

    /// # Safety
    /// The wrapped thread heap must still be live.
    #[inline(always)]
    pub(crate) unsafe fn state(self) -> crate::core_types::ThreadHeapState {
        // SAFETY[AUDIT:typed-thread-heap-local-witness-boundary]: forwarded to the wrapped live
        // thread-heap pointer under this method's safety contract.
        unsafe { self.0.state() }
    }
}

impl PagePtr {
    pub(crate) unsafe fn try_local(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<LocalPage, PagePtr> {
        // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: classification reads the
        // page owner word from live page metadata.
        match unsafe { self.classify_for_free(current) } {
            PageFreeOwner::Local(lp) => Ok(lp),
            _ => Err(self),
        }
    }

    pub(crate) unsafe fn try_abandoned(self) -> Result<AbandonedPageType, PagePtr> {
        // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: page metadata is live and its
        // typed thread-id cell owns the abandoned bit classification.
        let tid = unsafe { self.thread_id_cell().load_relaxed() };
        if tid.is_abandoned() {
            // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: the thread-id word marks
            // this page as abandoned.
            Ok(unsafe { AbandonedPageType::new_unchecked(self) })
        } else {
            Err(self)
        }
    }

    pub(crate) unsafe fn classify_for_free(
        self,
        current: crate::core_types::ThreadId,
    ) -> PageFreeOwner {
        // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: page metadata is live and its
        // typed thread-id cell owns free-path classification bits.
        let tid = unsafe { self.thread_id_cell().load_relaxed() };
        if tid.is_abandoned() {
            // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: the thread-id word marks
            // this page as abandoned.
            PageFreeOwner::Abandoned(unsafe { AbandonedPageType::new_unchecked(self) })
        } else if tid.is_abandoned_mapped() {
            // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: the thread-id word marks
            // this page as abandoned-mapped.
            PageFreeOwner::AbandonedMapped(unsafe { AbandonedMappedPageType::new_unchecked(self) })
        } else {
            match tid.classify_free_path(current) {
                crate::core_types::FreePathKind::LocalFast
                | crate::core_types::FreePathKind::LocalSlowFlags => {
                    // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: the owner bits
                    // classify this page as local to `current`.
                    PageFreeOwner::Local(unsafe { LocalPage::new_unchecked(self) })
                }
                crate::core_types::FreePathKind::RemotePlain
                | crate::core_types::FreePathKind::RemoteSlowFlags => {
                    // SAFETY[AUDIT:typed-page-free-owner-classification-boundary]: the owner bits
                    // classify this page as foreign to `current`.
                    PageFreeOwner::Foreign(unsafe { ForeignPage::new_unchecked(self) })
                }
            }
        }
    }
}

pub(super) fn __lint_touch() {
    let _ = core::mem::size_of::<Retired>();
    let _ = core::mem::size_of::<AbandonedMapped>();
    let _ = core::mem::size_of::<Destroyed>();

    // SAFETY[AUDIT:typed-page-state-test-fixtures]: lint-touch uses
    // non-dereferenced dangling pointers only to keep typestate accessors and
    // function items visible to dead-code analysis.
    let page = unsafe { PagePtr::new_unchecked(core::ptr::dangling_mut()) };
    // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic page pointer
    // is never dereferenced; the witness is consumed only by accessor coverage.
    let retired = unsafe { RetiredPage::new_unchecked(page) };
    let _ = retired.page();
    let _ = retired.expire();
    // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic abandoned
    // witness is used only to keep remote-free and function-item APIs live.
    let abandoned = unsafe { AbandonedPageType::new_unchecked(page) };
    let _ = AbandonedPageType::claim_ownership
        as unsafe fn(AbandonedPageType) -> Option<OwnedAbandonedPage>;
    let _ = abandoned.remote_free_page();
    // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic mapped
    // abandoned witness is not dereferenced.
    let mapped = unsafe { AbandonedMappedPageType::new_unchecked(page) };
    let _ = mapped.remote_free_page();
    // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic owned
    // abandoned witness is used only for transition API reachability.
    let owned = unsafe { OwnedAbandonedPage::new_unchecked(page) };
    let _ = owned.reabandon_to_mapped();

    if let PageFreeOwner::Local(local) =
        // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic page is
        // wrapped but not dereferenced while checking enum-field reachability.
        PageFreeOwner::Local(unsafe { LocalPage::new_unchecked(page) })
    {
        let _ = local.page();
    }
    if let PageFreeOwner::Foreign(foreign) =
        // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic page is
        // wrapped but not dereferenced while checking enum-field reachability.
        PageFreeOwner::Foreign(unsafe { ForeignPage::new_unchecked(page) })
    {
        let _ = foreign.page();
    }
    if let PageAbandonOutcome::Abandoned(abandoned) = PageAbandonOutcome::Abandoned(abandoned) {
        let _ = abandoned.page();
    }
    if let PageAbandonOutcome::Destroyed(destroyed) =
        // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic
        // destroyed witness is used only for enum-field reachability.
        PageAbandonOutcome::Destroyed(unsafe { DestroyedPage::new_unchecked(page) })
    {
        let _ = destroyed.page();
    }

    // SAFETY[AUDIT:typed-page-state-test-fixtures]: the dangling thread-heap
    // pointer is used as identity only and is not dereferenced by this helper.
    let theap = unsafe { ThreadHeapPtr::new_unchecked(core::ptr::dangling_mut()) };
    // SAFETY[AUDIT:typed-page-state-test-fixtures]: the synthetic local
    // thread-heap witness is used only for accessor/function-item reachability.
    let local = unsafe { ThreadHeapLocal::new_unchecked(theap) };
    let _ = local.borrowed();
    let _ =
        ThreadHeapLocal::state as unsafe fn(ThreadHeapLocal) -> crate::core_types::ThreadHeapState;
    let _ = PagePtr::try_local
        as unsafe fn(PagePtr, crate::core_types::ThreadId) -> Result<LocalPage, PagePtr>;
    let _ = PagePtr::try_abandoned as unsafe fn(PagePtr) -> Result<AbandonedPageType, PagePtr>;
}

#[cfg(test)]
mod tests;
