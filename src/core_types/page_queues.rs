//! DT-10: Page queue types.

use core::marker::PhantomData;
use crate::core_types::{PageQueuePtr, PagePtr};
use crate::core_types::page_states::{Page, Active, FullPage, ActivePage};

pub(crate) enum NormalQueue {}
pub(crate) enum HugeQueue {}
pub(crate) enum FullQueue {}

#[repr(transparent)]
pub(crate) struct PageQueue<K> {
    pub(crate) ptr: PageQueuePtr,
    _kind: PhantomData<K>,
}

#[repr(transparent)]
pub(crate) struct NormalPageQueue(pub(crate) PageQueuePtr);

#[repr(transparent)]
pub(crate) struct HugePageQueue(pub(crate) PageQueuePtr);

#[repr(transparent)]
pub(crate) struct FullPageQueue(pub(crate) PageQueuePtr);

/// Witness that a page is currently inside a specific queue.
pub(crate) struct QueuedPage<K> {
    pub(crate) page: PagePtr,
    pub(crate) queue: PageQueue<K>,
}

/// Witness that a page is definitively *not* in any queue.
pub(crate) struct UnqueuedPage<S> {
    pub(crate) page: Page<S>,
}

// Strictly typed transitions to prevent illegal states, per DT-10.

#[inline]
pub(crate) fn move_to_full_queue(
    _page: ActivePage,
    _from: NormalPageQueue,
    _to: FullPageQueue,
) -> FullPage {
    unimplemented!()
}

#[inline]
pub(crate) fn unfull_page(
    _page: FullPage,
    _from: FullPageQueue,
    _to: NormalPageQueue,
) -> ActivePage {
    unimplemented!()
}

#[inline]
pub(crate) fn free_all_free_page_to_arena(
    _page: crate::core_types::page_states::AllFreePage<Active>,
    _owner: crate::core_types::page_states::ThreadHeapLocal,
) -> crate::core_types::page_states::DestroyedPage {
    unimplemented!()
}
