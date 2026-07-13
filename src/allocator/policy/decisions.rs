use core::ffi::c_long;
use core::num::NonZeroU16;

use crate::core_types::{PageQueueCount, PageReclaimOnFree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ThreadHeapPolicy {
    allow_page_reclaim: bool,
    allow_page_abandon: bool,
    page_full_retain: PageFullRetain,
}

impl ThreadHeapPolicy {
    pub(super) const fn new(
        allow_page_reclaim: bool,
        allow_page_abandon: bool,
        page_full_retain: PageFullRetain,
    ) -> Self {
        Self {
            allow_page_reclaim,
            allow_page_abandon,
            page_full_retain,
        }
    }

    #[inline(always)]
    pub(crate) const fn from_fields(
        allow_page_reclaim: bool,
        allow_page_abandon: bool,
        page_full_retain: c_long,
    ) -> Self {
        Self::new(
            allow_page_reclaim,
            allow_page_abandon,
            PageFullRetain::new(page_full_retain),
        )
    }

    #[inline(always)]
    pub(crate) const fn allow_page_reclaim(self) -> bool {
        self.allow_page_reclaim
    }

    #[inline(always)]
    pub(crate) const fn allow_page_abandon(self) -> bool {
        self.allow_page_abandon
    }

    #[inline(always)]
    pub(crate) const fn page_full_retain(self) -> PageFullRetain {
        self.page_full_retain
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageFullRetain(c_long);

impl PageFullRetain {
    pub(super) const fn new(value: c_long) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> c_long {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageExtendCount(NonZeroU16);

impl PageExtendCount {
    pub(super) const fn new(value: NonZeroU16) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> u16 {
        self.0.get()
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GenericMaintenanceInterval(c_long);

impl GenericMaintenanceInterval {
    pub(super) const fn new(value: c_long) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub(crate) const fn get(self) -> c_long {
        self.0
    }

    #[inline(always)]
    pub(crate) const fn reached_by(self, count: c_long) -> bool {
        count >= self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RemoteCollectMode {
    PartialKeepPublishedHead,
    FullDetach,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReclaimContext {
    SameThreadHeap,
    CrossThreadHeap,
    ThreadPool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReclaimDecision {
    reclaim_on_free: PageReclaimOnFree,
    max_queue_count: Option<PageQueueCount>,
}

impl ReclaimDecision {
    pub(crate) const fn new(
        reclaim_on_free: PageReclaimOnFree,
        max_queue_count: Option<PageQueueCount>,
    ) -> Self {
        Self {
            reclaim_on_free,
            max_queue_count,
        }
    }

    #[inline(always)]
    pub(crate) const fn reclaim_on_free(self) -> PageReclaimOnFree {
        self.reclaim_on_free
    }

    #[inline(always)]
    pub(crate) const fn max_queue_count(self) -> Option<PageQueueCount> {
        self.max_queue_count
    }
}
