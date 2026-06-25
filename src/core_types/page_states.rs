//! Required types for page typestate signatures referenced in freelist_wrappers.

use crate::core_types::{PagePtr, ThreadHeapPtr};
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
    pub(crate) unsafe fn new_unchecked(ptr: PagePtr) -> Self {
        Self {
            ptr,
            _state: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn initialize(self, _owner: ThreadHeapLocal) -> ActivePage {
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
    pub(crate) unsafe fn available(self) -> Option<AvailablePage> {
        let page = self.page();
        if unsafe { (*page.as_ptr()).free.is_null() } {
            None
        } else {
            Some(unsafe { AvailablePage::new_unchecked(page) })
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn to_full(
        self,
        from: crate::core_types::NormalPageQueue,
        to: crate::core_types::FullPageQueue,
    ) -> FullPage {
        unsafe {
            crate::page::mi_page_queue_enqueue_from_raw(to.ptr(), from.ptr(), true, self.page());
            crate::page::mi_page_set_in_full(self.page().as_ptr(), true);
            FullPage::new_unchecked(self.page())
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn mark_all_free(self) -> Option<AllFreePage<Active>> {
        let page = self.page();
        let used = unsafe { (*page.as_ptr()).used };
        if used == 0 {
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
        let used = unsafe { (*page.as_ptr()).used };
        if used == 0 {
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
        let (block, next) = unsafe { crate::core_types::AvailableFreeList::pop(self) };
        unsafe {
            page.set_free(next);
            page.set_used(page.used().increment_wrapping());
        }
        (block, unsafe { ActivePage::new_unchecked(page) })
    }
}

impl AllFreePage<Active> {
    #[inline(always)]
    pub(crate) fn retire(self) -> RetiredPage {
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
        unsafe { DestroyedPage::new_unchecked(self.page()) }
    }
}

impl AbandonedPageType {
    #[inline(always)]
    pub(crate) unsafe fn claim_ownership(self) -> Option<OwnedAbandonedPage> {
        let page = self.page();
        let atomic = unsafe { page.thread_free_cell() };
        if unsafe { atomic.claim_acq_rel() } {
            Some(unsafe { OwnedAbandonedPage::new_unchecked(page) })
        } else {
            None
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
    #[inline(always)]
    pub(crate) fn reclaim(self, _target: ThreadHeapLocal) -> ActivePage {
        unsafe { ActivePage::new_unchecked(self.page()) }
    }

    #[inline(always)]
    pub(crate) fn try_reabandon_to_mapped(
        self,
    ) -> Result<AbandonedMappedPageType, OwnedAbandonedPage> {
        Ok(unsafe { AbandonedMappedPageType::new_unchecked(self.page()) })
    }
}

impl AllFreePage<Full> {
    /// # Safety
    /// `page` must be a full-queue page whose blocks are all free.
    #[inline(always)]
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
    pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self {
        Self(theap)
    }

    #[inline(always)]
    pub(crate) fn ptr(self) -> ThreadHeapPtr {
        self.0
    }

    #[inline(always)]
    pub(crate) fn borrowed(self) -> crate::core_types::ThreadHeapBorrowed {
        unsafe { crate::core_types::ThreadHeapBorrowed::new_unchecked(self.0) }
    }

    /// # Safety
    /// The wrapped thread heap must still be live.
    #[inline(always)]
    pub(crate) unsafe fn state(self) -> crate::core_types::ThreadHeapState {
        unsafe { self.0.state() }
    }
}

impl PagePtr {
    pub(crate) unsafe fn try_local(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<LocalPage, PagePtr> {
        match unsafe { self.classify_for_free(current) } {
            PageFreeOwner::Local(lp) => Ok(lp),
            _ => Err(self),
        }
    }

    pub(crate) unsafe fn try_abandoned(self) -> Result<AbandonedPageType, PagePtr> {
        let tid = unsafe { self.thread_id_cell().load_relaxed() };
        if tid.is_abandoned() {
            Ok(unsafe { AbandonedPageType::new_unchecked(self) })
        } else {
            Err(self)
        }
    }

    pub(crate) unsafe fn classify_for_free(
        self,
        current: crate::core_types::ThreadId,
    ) -> PageFreeOwner {
        let tid = unsafe { self.thread_id_cell().load_relaxed() };
        if tid.is_abandoned() {
            PageFreeOwner::Abandoned(unsafe { AbandonedPageType::new_unchecked(self) })
        } else if tid.is_abandoned_mapped() {
            PageFreeOwner::AbandonedMapped(unsafe { AbandonedMappedPageType::new_unchecked(self) })
        } else {
            match tid.classify_free_path(current) {
                crate::core_types::FreePathKind::LocalFast
                | crate::core_types::FreePathKind::LocalSlowFlags => {
                    PageFreeOwner::Local(unsafe { LocalPage::new_unchecked(self) })
                }
                crate::core_types::FreePathKind::RemotePlain
                | crate::core_types::FreePathKind::RemoteSlowFlags => {
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

    let page = unsafe { PagePtr::new_unchecked(core::ptr::dangling_mut()) };
    let retired = unsafe { RetiredPage::new_unchecked(page) };
    let _ = retired.page();
    let _ = retired.expire();
    let abandoned = unsafe { AbandonedPageType::new_unchecked(page) };
    let _ = AbandonedPageType::claim_ownership
        as unsafe fn(AbandonedPageType) -> Option<OwnedAbandonedPage>;
    let _ = abandoned.remote_free_page();
    let mapped = unsafe { AbandonedMappedPageType::new_unchecked(page) };
    let _ = mapped.remote_free_page();
    let owned = unsafe { OwnedAbandonedPage::new_unchecked(page) };
    let _ = owned.try_reabandon_to_mapped();

    if let PageFreeOwner::Local(local) =
        PageFreeOwner::Local(unsafe { LocalPage::new_unchecked(page) })
    {
        let _ = local.page();
    }
    if let PageFreeOwner::Foreign(foreign) =
        PageFreeOwner::Foreign(unsafe { ForeignPage::new_unchecked(page) })
    {
        let _ = foreign.page();
    }
    if let PageAbandonOutcome::Abandoned(abandoned) = PageAbandonOutcome::Abandoned(abandoned) {
        let _ = abandoned.page();
    }
    if let PageAbandonOutcome::Destroyed(destroyed) =
        PageAbandonOutcome::Destroyed(unsafe { DestroyedPage::new_unchecked(page) })
    {
        let _ = destroyed.page();
    }

    let theap = unsafe { ThreadHeapPtr::new_unchecked(core::ptr::dangling_mut()) };
    let local = unsafe { ThreadHeapLocal::new_unchecked(theap) };
    let _ = local.borrowed();
    let _ =
        ThreadHeapLocal::state as unsafe fn(ThreadHeapLocal) -> crate::core_types::ThreadHeapState;
    let _ = PagePtr::try_local
        as unsafe fn(PagePtr, crate::core_types::ThreadId) -> Result<LocalPage, PagePtr>;
    let _ = PagePtr::try_abandoned as unsafe fn(PagePtr) -> Result<AbandonedPageType, PagePtr>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_types::{
        PageFlags, PageOwnerId, PageThreadId, ThreadFreeOwner, ThreadFreeWord, ThreadHeapPtr,
        ThreadId,
    };
    use crate::types::{mi_block_t, mi_page_t, mi_theap_t};
    use core::{mem::MaybeUninit, ptr};

    fn with_page<R>(xthread_id: usize, f: impl FnOnce(PagePtr) -> R) -> R {
        let mut page = MaybeUninit::<mi_page_t>::uninit();
        unsafe {
            ptr::addr_of_mut!((*page.as_mut_ptr()).xthread_id).write(xthread_id);
            f(PagePtr::new_unchecked(page.as_mut_ptr()))
        }
    }

    fn heap_owner() -> ThreadHeapLocal {
        unsafe {
            ThreadHeapLocal::new_unchecked(ThreadHeapPtr::new_unchecked(
                core::ptr::NonNull::<mi_theap_t>::dangling().as_ptr(),
            ))
        }
    }

    #[test]
    fn uninit_initialize_yields_active_page() {
        with_page(0, |page| unsafe {
            let active = Page::<Uninit>::new_unchecked(page).initialize(heap_owner());
            assert_eq!(active.page().as_ptr(), page.as_ptr());
        });
    }

    #[test]
    fn active_available_and_pop_free_block_update_page_state() {
        let mut page = MaybeUninit::<mi_page_t>::uninit();
        let mut block = mi_block_t { next: 0 };
        unsafe {
            let raw = page.as_mut_ptr();
            ptr::addr_of_mut!((*raw).free).write(ptr::addr_of_mut!(block));
            ptr::addr_of_mut!((*raw).used).write(0);
            #[cfg(any(
                feature = "secure",
                debug_assertions,
                feature = "track-valgrind",
                feature = "track-asan"
            ))]
            ptr::addr_of_mut!((*raw).keys).write([0, 0]);

            let active = ActivePage::new_unchecked(PagePtr::new_unchecked(raw));
            let available = active.available().expect("non-null free head is available");
            let (allocated, active_again) = available.pop_free_block();
            assert_eq!(allocated.block().as_ptr(), ptr::addr_of_mut!(block));
            assert_eq!(active_again.page().as_ptr(), raw);
            assert!((*raw).free.is_null());
            assert_eq!((*raw).used, 1);
            assert!(active_again.available().is_none());
        }
    }

    #[test]
    fn all_free_retire_expire_and_arena_free_preserve_page_identity() {
        with_page(0, |page| unsafe {
            ptr::addr_of_mut!((*page.as_ptr()).used).write(0);
            let active = ActivePage::new_unchecked(page);
            let all_free = active.mark_all_free().expect("used == 0");
            let retired = all_free.retire();
            assert_eq!(retired.page().as_ptr(), page.as_ptr());
            let destroyed = retired.expire();
            assert_eq!(destroyed.page().as_ptr(), page.as_ptr());

            let active = ActivePage::new_unchecked(page);
            let destroyed = active
                .mark_all_free()
                .expect("used remains zero")
                .free_to_arena(heap_owner());
            assert_eq!(destroyed.page().as_ptr(), page.as_ptr());

            ptr::addr_of_mut!((*page.as_ptr()).used).write(1);
            assert!(ActivePage::new_unchecked(page).mark_all_free().is_none());
        });
    }

    #[test]
    fn abandon_and_reclaim_transitions_preserve_page_identity() {
        with_page(0, |page| unsafe {
            match ActivePage::new_unchecked(page).for_abandon() {
                PageForAbandon::Active(active) => assert_eq!(active.page().as_ptr(), page.as_ptr()),
                PageForAbandon::Full(_) => panic!("active page became full abandon witness"),
            }
            match FullPage::new_unchecked(page).for_abandon() {
                PageForAbandon::Full(full) => assert_eq!(full.page().as_ptr(), page.as_ptr()),
                PageForAbandon::Active(_) => panic!("full page became active abandon witness"),
            }

            let owned = OwnedAbandonedPage::new_unchecked(page);
            let reclaimed = owned.reclaim(heap_owner());
            assert_eq!(reclaimed.page().as_ptr(), page.as_ptr());
            let mapped = match owned.try_reabandon_to_mapped() {
                Ok(mapped) => mapped,
                Err(_) => panic!("owned page reabandoned to mapped witness"),
            };
            assert_eq!(mapped.page().as_ptr(), page.as_ptr());
        });
    }

    #[test]
    fn abandoned_claim_ownership_uses_thread_free_ownership_bit() {
        with_page(0, |page| unsafe {
            let thread_free = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned);
            ptr::addr_of_mut!((*page.as_ptr()).xthread_free).write(thread_free.raw());
            let abandoned = AbandonedPageType::new_unchecked(page);
            let owned = abandoned
                .claim_ownership()
                .expect("unowned abandoned page can be claimed");
            assert_eq!(owned.page().as_ptr(), page.as_ptr());
            assert!(abandoned.claim_ownership().is_none());
        });
    }

    #[test]
    fn abandoned_mapped_classification_remains_distinct() {
        let current = ThreadId::new(0x2000).expect("valid thread id");
        let flags = PageFlags::HAS_INTERIOR_POINTERS;
        let mapped = PageThreadId::from_parts(PageOwnerId::AbandonedMapped, flags);

        with_page(mapped.raw(), |page| unsafe {
            match page.classify_for_free(current) {
                PageFreeOwner::AbandonedMapped(mapped_page) => {
                    assert_eq!(mapped_page.page().as_ptr(), page.as_ptr());
                }
                PageFreeOwner::Abandoned(_) => panic!("abandoned-mapped collapsed to abandoned"),
                PageFreeOwner::Local(_) | PageFreeOwner::Foreign(_) => {
                    panic!("abandoned-mapped classified as owned")
                }
            }
            assert!(page.try_abandoned().is_err());
        });

        let abandoned = PageThreadId::from_parts(PageOwnerId::Abandoned, flags);
        with_page(abandoned.raw(), |page| unsafe {
            match page.classify_for_free(current) {
                PageFreeOwner::Abandoned(abandoned_page) => {
                    assert_eq!(abandoned_page.page().as_ptr(), page.as_ptr());
                }
                PageFreeOwner::AbandonedMapped(_) => {
                    panic!("abandoned classified as abandoned-mapped")
                }
                PageFreeOwner::Local(_) | PageFreeOwner::Foreign(_) => {
                    panic!("abandoned classified as owned")
                }
            }
            assert!(page.try_abandoned().is_ok());
        });
    }
}
