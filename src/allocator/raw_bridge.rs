//! Raw-to-capability construction owned by the allocator layer.
//!
//! Typed state transitions use their consuming APIs. These helpers are limited
//! by Rust privacy to the allocator module and its descendants for C-shaped
//! ingress and metadata-classification paths that cannot yet carry a witness.

use crate::types::page_states::{Abandoned, RetiredPage};
use crate::types::{
    AbandonedPage, ActivePage, AllFreePage, DestroyedPage, ForeignPage, FullPage, LocalPage,
    OwnedAbandonedPage, Page, PagePtr, Uninit,
};

/// # Safety
/// `page` must identify exclusively held page metadata that has not yet been
/// initialized.
#[inline(always)]
pub(super) unsafe fn assume_uninit_page(page: PagePtr) -> Page<Uninit> {
    unsafe { core::mem::transmute(page) }
}

macro_rules! raw_page_bridge {
    ($name:ident, $output:ty, $contract:literal) => {
        #[doc = "# Safety"]
        #[doc = $contract]
        #[inline(always)]
        pub(super) unsafe fn $name(page: PagePtr) -> $output {
            unsafe { core::mem::transmute(page) }
        }
    };
}

raw_page_bridge!(
    assume_active_page,
    ActivePage,
    "`page` must be initialized, locally owned, active, and not full."
);
raw_page_bridge!(
    assume_retired_page,
    RetiredPage,
    "`page` must be a live initialized current-thread page linked at the head of its stable normal queue with a nonzero retirement countdown."
);

/// # Safety
/// `page` must be the saved identity of an `OwnedAbandonedPage` consumed by an
/// arena-abandon transition that completed terminal arena release.
#[inline(always)]
pub(super) unsafe fn assume_destroyed_after_arena_free(page: PagePtr) -> DestroyedPage {
    unsafe { core::mem::transmute(page) }
}

/// # Safety
/// `page` must be abandoned and have zero used blocks.
#[inline(always)]
pub(super) unsafe fn assume_all_free_abandoned(page: AbandonedPage) -> AllFreePage<Abandoned> {
    unsafe { core::mem::transmute(page) }
}

raw_page_bridge!(
    assume_full_page,
    FullPage,
    "`page` must be initialized, locally owned, and linked in the full queue."
);
raw_page_bridge!(
    assume_local_page,
    LocalPage,
    "`page` must be owned by the current thread heap."
);
raw_page_bridge!(
    assume_foreign_page,
    ForeignPage,
    "`page` must be owned by another thread or abandoned for remote-free routing."
);
raw_page_bridge!(
    assume_abandoned_page,
    AbandonedPage,
    "`page` must be marked abandoned and unowned by a live thread heap."
);
raw_page_bridge!(
    assume_owned_abandoned_page,
    OwnedAbandonedPage,
    "`page` must be abandoned and its atomic thread-free ownership successfully claimed by this caller."
);
