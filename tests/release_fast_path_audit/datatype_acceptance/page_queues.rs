use super::DatatypeAcceptanceSources;

pub(super) fn assert_page_queue_and_memid_acceptance(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let page_queues_rs = audit_sources.source("src/types/page_queues.rs");
    for witness in ["NormalPageQueue", "HugePageQueue", "FullPageQueue"] {
        assert!(
            page_queues_rs.contains(&format!("pub(crate) struct {witness}(PageQueuePtr);")),
            "{witness} should keep its queue pointer field private"
        );
        assert!(
            !page_queues_rs.contains(&format!(
                "pub(crate) struct {witness}(pub(crate) PageQueuePtr);"
            )),
            "{witness} should not expose tuple construction outside page_queues"
        );
    }
    assert!(
        page_queues_rs.contains("pub(crate) unsafe fn new_unchecked(ptr: PageQueuePtr) -> Self")
            && page_queues_rs
                .contains("pub(crate) unsafe fn new(ptr: PageQueuePtr) -> Option<Self>")
            && page_queues_rs.contains("pub(crate) fn ptr(self) -> PageQueuePtr"),
        "page queue witnesses should expose checked/unchecked constructors and queue accessors"
    );
    assert!(
        page_queues_rs.contains("block_size.get() <= crate::types::MI_LARGE_MAX_OBJ_SIZE")
            && page_queues_rs
                .contains("crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>()")
            && page_queues_rs
                .contains("crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>())")
            && page_queues_rs
                .contains("const _: unsafe fn(PageQueuePtr) -> Option<NormalPageQueue> = NormalPageQueue::new;")
            && page_queues_rs
                .contains("const _: unsafe fn(PageQueuePtr) -> Option<HugePageQueue> = HugePageQueue::new;")
            && page_queues_rs
                .contains("const _: unsafe fn(PageQueuePtr) -> Option<FullPageQueue> = FullPageQueue::new;"),
        "page queue checked constructors should inspect normal/huge/full queue block-size identity"
    );
    assert!(
        page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::ActivePage,\n        queue: NormalPageQueue,"
            )
            && page_queues_rs.contains("impl QueuedPage<FullQueue>")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: FullPageQueue,"
            )
            && page_queues_rs.contains("crate::page::mi_page_queue_contains(queue.ptr().as_ptr(), page.page().as_ptr())")
            && page_queues_rs.contains("queue: PageQueue::normal(queue)")
            && page_queues_rs.contains("queue: PageQueue::full(queue)"),
        "queued-page witnesses should be constructed only after matching page/queue witnesses and linked-list membership checks"
    );
    assert!(
        page_queues_rs.contains("pub(crate) struct PageQueue<K> {\n    ptr: PageQueuePtr,")
            && !page_queues_rs.contains("pub(crate) ptr: PageQueuePtr")
            && page_queues_rs.contains("impl PageQueue<NormalQueue>")
            && page_queues_rs.contains("impl PageQueue<FullQueue>")
            && page_queues_rs.contains("pub(crate) fn normal(queue: NormalPageQueue) -> Self")
            && page_queues_rs.contains("pub(crate) fn full(queue: FullPageQueue) -> Self"),
        "generic PageQueue should keep its raw field private and expose only kind-specific constructors"
    );
    let memid_rs = audit_sources.source("src/types/memid.rs");
    let memid_tests_rs = audit_sources.source("src/types/memid/tests.rs");
    assert!(
        memid_rs.contains("raw: crate::internal::mi_memid_t,")
            && !memid_rs.contains("pub(crate) raw: crate::internal::mi_memid_t,")
            && memid_rs.contains("pub(crate) const fn into_raw(self)")
            && memid_rs.contains("pub(crate) const fn as_raw(&self)"),
        "MemId should keep raw mi_memid_t private behind explicit raw forwarding accessors"
    );
    assert!(
        memid_rs.contains("pub(crate) enum MemIdView {")
            && memid_rs.contains("Unknown(MemKindCode),")
            && memid_rs
                .contains("pub(crate) fn view(raw: crate::internal::mi_memid_t) -> MemIdView")
            && memid_rs.contains("let os = unsafe { raw.mem.os };")
            && memid_rs.contains("let m = unsafe { raw.mem.meta };")
            && memid_rs.contains("let a = unsafe { raw.mem.arena };")
            && memid_rs.contains("return MemIdView::Unknown(code);")
            && memid_tests_rs
                .contains("fn memid_view_returns_unknown_for_null_meta_or_arena_union_pointers()")
            && memid_tests_rs
                .contains("fn memid_view_returns_unknown_for_invalid_arena_slice_range()")
            && memid_tests_rs.contains("MemKind::from(MemKindCode(99))")
            && memid_tests_rs.contains("MemKind::Unknown(MemKindCode(99))")
            && memid_rs.contains("MemKind::Unknown(c) => MemIdView::Unknown(c),"),
        "MemId::view should own union-arm reads and map unknown or corrupt discriminants to Unknown"
    );
}
