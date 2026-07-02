use super::super::PageArenaBehaviorSources;

#[path = "memid_view/memory_kinds.rs"]
mod memory_kinds;
#[path = "memid_view/thread_heap_refcount.rs"]
mod thread_heap_refcount;

pub(super) fn assert_memid_view_dispatch(sources: &PageArenaBehaviorSources<'_>) {
    thread_heap_refcount::assert_thread_heap_refcount_dispatch(sources);
    memory_kinds::assert_memid_view_memory_kind_dispatch(sources);
}
