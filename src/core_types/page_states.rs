//! Required types for page typestate signatures referenced in freelist_wrappers.

use core::marker::PhantomData;
use crate::core_types::PagePtr;

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

pub(crate) enum PageFreeOwner {
    Local(LocalPage),
    Foreign(ForeignPage),
    Abandoned(AbandonedPageType),
}

impl PagePtr {
    pub(crate) fn try_local(self, current: crate::core_types::ThreadId) -> Result<LocalPage, PagePtr> {
        unimplemented!()
    }
    
    pub(crate) fn try_abandoned(self) -> Result<AbandonedPageType, PagePtr> {
        unimplemented!()
    }
}
