//! DT-6: User pointer validation types.
//!
//! `AllocatedBlock` lives in `validation_extra` for the free-list pop handoff.
//! Validated free requests retain their page and block until a scoped owner
//! consumes the matching storage.

use core::marker::PhantomData;

use super::page_states::{ForeignPage, FreeRouteSnapshot, LocalPage};
use super::ptr::{BlockPtr, PagePtr, UserPtr};

/// Offset from the start of a block.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BlockOffset(pub(crate) usize);

/// Represents a guarded block that needs special handling.
#[repr(transparent)]
#[derive(PartialEq, Eq, Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct GuardedBlock(BlockPtr);

impl GuardedBlock {
    /// # Safety
    /// `block` must have been classified as guarded by the allocator's guarded
    /// block tag or guarded-pointer predicate.
    #[inline(always)]
    pub(crate) unsafe fn after_guarded_detection(block: BlockPtr) -> Self {
        Self(block)
    }

    #[inline(always)]
    pub(crate) fn block(&self) -> BlockPtr {
        self.0
    }
}

/// Location of a pointer within a block.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) enum PointerLocation {
    BlockStart(BlockPtr),
    Interior {
        block: BlockPtr,
        offset: BlockOffset,
    },
    Guarded {
        block: GuardedBlock,
        offset: BlockOffset,
    },
}

/// Block facts derived from a validated pointer location.
#[derive(PartialEq, Eq, Debug)]
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct LocatedBlock {
    block: BlockPtr,
    was_guarded: WasGuarded,
    offset: BlockOffset,
}

impl LocatedBlock {
    #[inline(always)]
    pub(crate) fn block(&self) -> BlockPtr {
        self.block
    }

    #[inline(always)]
    pub(crate) fn was_guarded(&self) -> WasGuarded {
        self.was_guarded
    }

    #[inline(always)]
    pub(crate) fn offset(&self) -> BlockOffset {
        self.offset
    }
}

impl PointerLocation {
    #[inline(always)]
    pub(crate) fn located_block(self) -> LocatedBlock {
        match self {
            PointerLocation::BlockStart(block) => LocatedBlock {
                block,
                was_guarded: WasGuarded::No,
                offset: BlockOffset(0),
            },
            PointerLocation::Interior { block, offset } => LocatedBlock {
                block,
                was_guarded: WasGuarded::No,
                offset,
            },
            PointerLocation::Guarded { block, offset } => LocatedBlock {
                block: block.block(),
                was_guarded: WasGuarded::Yes,
                offset,
            },
        }
    }
}

/// Result of a user pointer lookup.
pub(crate) enum PointerLookup {
    Null,
    Valid(ValidPointerLookup),
    Invalid,
}

#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ValidPointerLookup {
    user: UserPtr,
    page: PagePtr,
    location: PointerLocation,
}

impl ValidPointerLookup {
    #[inline(always)]
    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }

    #[inline(always)]
    pub(crate) fn into_parts(self) -> (UserPtr, PagePtr, PointerLocation) {
        (self.user, self.page, self.location)
    }
}

impl PointerLookup {
    /// # Safety
    /// `user`, `page`, and `location` must come from validating the same live
    /// allocation. The location must describe the exact block and user offset,
    /// including any guarded classification. Its metadata and backing must
    /// remain live and stable while the returned capability is consumed.
    #[inline(always)]
    pub(crate) unsafe fn after_validation(
        user: UserPtr,
        page: PagePtr,
        location: PointerLocation,
    ) -> Self {
        Self::Valid(ValidPointerLookup {
            user,
            page,
            location,
        })
    }

    #[inline(always)]
    pub(crate) fn prepare_free(self) -> Option<ValidatedFree> {
        let PointerLookup::Valid(valid) = self else {
            return None;
        };
        let (user, page, location) = valid.into_parts();
        let located = location.located_block();
        let block = BlockForFree {
            block: located.block(),
            was_guarded: located.was_guarded(),
        };
        Some(ValidatedFree { user, page, block })
    }
}

/// Indicates whether a block was guarded before being passed to free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WasGuarded {
    No,
    Yes,
}

/// A block that has been validated and is ready for the typed free path.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct BlockForFree {
    block: BlockPtr,
    was_guarded: WasGuarded,
}

impl BlockForFree {
    pub(crate) fn block(&self) -> BlockPtr {
        self.block
    }

    pub(crate) fn was_guarded(&self) -> WasGuarded {
        self.was_guarded
    }

    pub(crate) fn is_guarded(&self) -> bool {
        self.was_guarded == WasGuarded::Yes
    }
}

/// A page whose invariant scope identifies its matching block-for-free.
#[repr(transparent)]
#[must_use = "a scoped free page must be consumed with its matching block"]
pub(crate) struct FreePage<'scope, P> {
    page: P,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<P> FreePage<'_, P> {
    #[inline(always)]
    fn new(page: P) -> Self {
        Self {
            page,
            _scope: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn owner(&self) -> &P {
        &self.page
    }
}

impl<'scope> FreePage<'scope, LocalPage> {
    /// Publish the matching block after validation and accounting.
    #[inline(always)]
    pub(crate) fn push_free_block(
        self,
        block: ScopedFreeBlock<'scope>,
    ) -> super::scalar::UsedBlockCount {
        let page = self.page.page().as_ptr();
        let block = block.block.block;
        // The scope's ingress retains this page and block, with exclusive
        // local access. Borrow only the fields mutated by this publication;
        // remote threads may continue to access the disjoint atomic words.
        let head = unsafe { &mut (*page).free };
        unsafe { crate::internal::mi_block_set_next(page, block.as_ptr(), *head) };
        *head = block.as_ptr();
        let free_is_zero = unsafe { &mut (*page).free_is_zero };
        *free_is_zero = false;
        let used = unsafe { &mut (*page).used };
        let next = super::scalar::UsedBlockCount::new(*used).decrement();
        *used = next.get();
        next
    }
}

/// A validated block that cannot be mixed with another free request's page.
#[repr(transparent)]
#[must_use = "a scoped free block must be consumed with its matching page"]
pub(crate) struct ScopedFreeBlock<'scope> {
    block: BlockForFree,
    _scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl ScopedFreeBlock<'_> {
    #[inline(always)]
    pub(crate) fn block(&self) -> BlockPtr {
        self.block.block()
    }

    #[inline(always)]
    pub(crate) fn is_guarded(&self) -> bool {
        self.block.is_guarded()
    }
}

/// An owner classification sharing its invariant scope with one free block.
#[must_use = "a classified free request must be consumed"]
pub(crate) enum FreeRoute<'scope> {
    LocalFast(FreePage<'scope, LocalPage>),
    LocalSlow(FreePage<'scope, LocalPage>),
    ForeignPlain(FreePage<'scope, ForeignPage>),
    ForeignSlow(FreePage<'scope, ForeignPage>),
    Abandoned(FreePage<'scope, ForeignPage>),
    AbandonedMapped(FreePage<'scope, ForeignPage>),
}

/// A fully validated free request retaining its matching page, block and user pointer.
#[must_use = "allocator state capabilities must be consumed or explicitly returned"]
pub(crate) struct ValidatedFree {
    user: UserPtr,
    page: PagePtr,
    block: BlockForFree,
}

impl ValidatedFree {
    /// # Safety
    /// `block` must be the exact start of one live non-guarded allocation from
    /// `page`, ready to be freed. Interior-pointer and guarded-page modes must
    /// already be excluded by the release fast-path classification.
    #[inline(always)]
    pub(crate) unsafe fn from_block_start_unchecked(page: PagePtr, block: BlockPtr) -> Self {
        Self {
            user: block.into(),
            page,
            block: BlockForFree {
                block,
                was_guarded: WasGuarded::No,
            },
        }
    }

    pub(crate) fn user(&self) -> UserPtr {
        self.user
    }

    pub(crate) fn page(&self) -> PagePtr {
        self.page
    }

    pub(crate) fn block(&self) -> &BlockForFree {
        &self.block
    }

    /// Classify ownership and consume this request in a fresh invariant scope.
    ///
    /// # Safety
    /// `current` must identify the executing thread. The page's registration,
    /// backing and owner metadata must remain live and protocol-stable through
    /// `f`. The validated block's storage must remain exclusively owned until
    /// publication. Local free-list, count and zero-state mutation must stay
    /// exclusive; callbacks must not reenter those local fields.
    #[inline(always)]
    pub(crate) unsafe fn with_route<R>(
        self,
        current: super::page::ThreadId,
        f: impl for<'scope> FnOnce(FreeRoute<'scope>, ScopedFreeBlock<'scope>) -> R,
    ) -> R {
        let route = match unsafe { self.page.snapshot_free_route(current) } {
            FreeRouteSnapshot::LocalFast(page) => FreeRoute::LocalFast(FreePage::new(page)),
            FreeRouteSnapshot::LocalSlow(page) => FreeRoute::LocalSlow(FreePage::new(page)),
            FreeRouteSnapshot::ForeignPlain(page) => FreeRoute::ForeignPlain(FreePage::new(page)),
            FreeRouteSnapshot::ForeignSlow(page) => FreeRoute::ForeignSlow(FreePage::new(page)),
            FreeRouteSnapshot::Abandoned(page) => {
                FreeRoute::Abandoned(FreePage::new(page.remote_free_page()))
            }
            FreeRouteSnapshot::AbandonedMapped(page) => {
                FreeRoute::AbandonedMapped(FreePage::new(page.remote_free_page()))
            }
        };
        f(
            route,
            ScopedFreeBlock {
                block: self.block,
                _scope: PhantomData,
            },
        )
    }

    /// # Safety
    /// The initialized page and validated block must remain live and locally
    /// exclusive through `f`. The local list, used count and zero flag must be
    /// valid for their coupled free transition, with stable page keys; callbacks
    /// must not incompatibly reenter that local state.
    #[inline(always)]
    pub(crate) unsafe fn with_local<R>(
        self,
        f: impl for<'scope> FnOnce(FreePage<'scope, LocalPage>, ScopedFreeBlock<'scope>) -> R,
    ) -> R {
        let page = unsafe { LocalPage::new_unchecked(self.page) };
        f(
            FreePage::new(page),
            ScopedFreeBlock {
                block: self.block,
                _scope: PhantomData,
            },
        )
    }

    /// # Safety
    /// The page must remain live and foreign to the executing thread through
    /// `f`, with publication and ownership changes following allocator atomics.
    #[inline(always)]
    pub(crate) unsafe fn with_foreign<R>(
        self,
        f: impl for<'scope> FnOnce(FreePage<'scope, ForeignPage>, ScopedFreeBlock<'scope>) -> R,
    ) -> R {
        let page = unsafe { ForeignPage::new_unchecked(self.page) };
        f(
            FreePage::new(page),
            ScopedFreeBlock {
                block: self.block,
                _scope: PhantomData,
            },
        )
    }
}

#[cfg(test)]
mod tests;
