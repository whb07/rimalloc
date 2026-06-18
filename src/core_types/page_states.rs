//! Required types for page typestate signatures referenced in freelist_wrappers.

use crate::core_types::{PagePtr, ThreadHeapPtr};
use core::marker::PhantomData;

pub(crate) struct Page<S> {
    pub(crate) ptr: PagePtr,
    _state: PhantomData<S>,
}

pub(crate) enum Uninit {}
pub(crate) enum Active {}
pub(crate) enum Full {}
pub(crate) enum Retired {}
pub(crate) enum Abandoned {}
pub(crate) enum AbandonedMapped {}
pub(crate) enum Destroyed {}

#[repr(transparent)]
pub(crate) struct ActivePage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct FullPage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct RetiredPage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct AvailablePage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct LocalPage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct ForeignPage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct AbandonedPageType(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct AbandonedMappedPageType(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct OwnedAbandonedPage(pub(crate) PagePtr);

#[repr(transparent)]
pub(crate) struct DestroyedPage(pub(crate) PagePtr);

pub(crate) struct AllFreePage<S> {
    pub(crate) page: Page<S>,
}

impl AllFreePage<Active> {
    /// # Safety
    /// `page` must be an active page whose blocks are all free.
    #[inline(always)]
    pub(crate) unsafe fn active_unchecked(page: ActivePage) -> Self {
        Self {
            page: Page {
                ptr: page.0,
                _state: PhantomData,
            },
        }
    }
}

impl AllFreePage<Full> {
    /// # Safety
    /// `page` must be a full-queue page whose blocks are all free.
    #[inline(always)]
    pub(crate) unsafe fn full_unchecked(page: FullPage) -> Self {
        Self {
            page: Page {
                ptr: page.0,
                _state: PhantomData,
            },
        }
    }
}

pub(crate) enum PageFreeOwner {
    Local(LocalPage),
    Foreign(ForeignPage),
    Abandoned(AbandonedPageType),
}

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
pub(crate) struct ThreadHeapLocal(pub(crate) ThreadHeapPtr);

impl PagePtr {
    pub(crate) unsafe fn try_local(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<LocalPage, PagePtr> {
        match self.classify_for_free(current) {
            PageFreeOwner::Local(lp) => Ok(lp),
            _ => Err(self),
        }
    }

    pub(crate) unsafe fn try_abandoned(
        self,
        current: crate::core_types::ThreadId,
    ) -> Result<AbandonedPageType, PagePtr> {
        match self.classify_for_free(current) {
            PageFreeOwner::Abandoned(ap) => Ok(ap),
            _ => Err(self),
        }
    }

    pub(crate) unsafe fn classify_for_free(
        self,
        current: crate::core_types::ThreadId,
    ) -> PageFreeOwner {
        let tid = crate::core_types::PageThreadId::new(unsafe { (*self.as_ptr()).xthread_id });
        if tid.is_abandoned() || tid.is_abandoned_mapped() {
            PageFreeOwner::Abandoned(AbandonedPageType(self))
        } else if tid.xor_current(current).is_local_fast() {
            PageFreeOwner::Local(LocalPage(self))
        } else {
            PageFreeOwner::Foreign(ForeignPage(self))
        }
    }
}
