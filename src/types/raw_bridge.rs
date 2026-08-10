//! Audited raw-to-capability construction at allocator ingress boundaries.
//!
//! Legal callers are C-shaped/exported entry points, initialization code,
//! queue lookup or classification boundaries, atomic ownership-success paths,
//! and synthetic tests. Typed state transitions must use their consuming APIs
//! instead of returning through this module. These helpers inspect no metadata
//! and perform no mutation; each caller establishes the stated invariant.

use super::page_states::Abandoned;
use super::{
    AbandonedPage, ActivePage, AllFreePage, ForeignPage, FullPage, LocalPage, OwnedAbandonedPage,
    Page, PagePtr, Uninit,
};

/// # Safety
/// `page` must identify exclusively held page metadata that has not yet been
/// initialized. Legal callers: C-shaped page initialization entry points.
#[inline(always)]
pub(crate) unsafe fn assume_uninit_page(page: PagePtr) -> Page<Uninit> {
    // SAFETY[AUDIT:raw-bridge-uninit-page]: the initialization ingress owns
    // the raw metadata exclusively and this bridge performs no mutation.
    unsafe { Page::<Uninit>::new_unchecked(page) }
}

macro_rules! raw_page_bridge {
    ($name:ident, $output:ty, $marker:literal, $contract:literal) => {
        #[doc = "# Safety"]
        #[doc = $contract]
        #[inline(always)]
        #[doc = $marker]
        pub(crate) unsafe fn $name(page: PagePtr) -> $output {
            // SAFETY[AUDIT:raw-bridge-page-witness]: this module is the sole
            // allocator-facing constructor bridge; the function-specific
            // contract is established by its audited ingress caller.
            unsafe { <$output>::new_unchecked(page) }
        }
    };
}

raw_page_bridge!(
    assume_active_page,
    ActivePage,
    "SAFETY[AUDIT:raw-bridge-active-page]",
    "`page` must be initialized, locally owned, active, and not full. Legal callers: C-shaped allocation/page entry points, initialization, and queue lookup."
);

/// # Safety
/// `page` must be abandoned and have zero used blocks. Legal callers: raw
/// arena free classification boundaries.
#[inline(always)]
pub(crate) unsafe fn assume_all_free_abandoned(page: AbandonedPage) -> AllFreePage<Abandoned> {
    // SAFETY[AUDIT:raw-bridge-all-free-abandoned-page]: the caller establishes
    // the all-free metadata predicate at the arena ingress boundary.
    unsafe { AllFreePage::abandoned_unchecked(page) }
}
raw_page_bridge!(
    assume_full_page,
    FullPage,
    "SAFETY[AUDIT:raw-bridge-full-page]",
    "`page` must be initialized, locally owned, and linked in the full queue. Legal callers: C-shaped page entry points and queue classification."
);
raw_page_bridge!(
    assume_local_page,
    LocalPage,
    "SAFETY[AUDIT:raw-bridge-local-page]",
    "`page` must be owned by the current thread heap. Legal callers: validated free classification and C-shaped local collection entry points."
);
raw_page_bridge!(
    assume_foreign_page,
    ForeignPage,
    "SAFETY[AUDIT:raw-bridge-foreign-page]",
    "`page` must be owned by another thread or abandoned for remote-free routing. Legal callers: validated free classification boundaries."
);
raw_page_bridge!(
    assume_abandoned_page,
    AbandonedPage,
    "SAFETY[AUDIT:raw-bridge-abandoned-page]",
    "`page` must be marked abandoned and unowned by a live thread heap. Legal callers: arena lookup/classification, C-shaped abandon entry points, and abandoned block visitation."
);
raw_page_bridge!(
    assume_owned_abandoned_page,
    OwnedAbandonedPage,
    "SAFETY[AUDIT:raw-bridge-owned-abandoned-page]",
    "`page` must be abandoned and its atomic thread-free ownership successfully claimed by this caller. Legal callers: atomic ownership success and raw compatibility ingress."
);
