pub(super) struct DatatypeAcceptanceSources<'a> {
    pub(super) rust_sources: &'a [(String, String)],
    pub(super) types_spec: &'a str,
    pub(super) unsafe_migration_allowlist: &'a str,
    pub(super) strict_type_allowlist: &'a str,
    pub(super) strict_type_checker: &'a str,
}

impl<'a> DatatypeAcceptanceSources<'a> {
    fn source(&self, rel: &str) -> &'a str {
        self.rust_sources
            .iter()
            .find_map(|(path, source)| (path == rel).then_some(source.as_str()))
            .unwrap_or_else(|| panic!("collected Rust sources should include {rel}"))
    }
}

pub(super) fn assert_datatype_acceptance_criteria_have_cross_cutting_guards(
    audit_sources: DatatypeAcceptanceSources<'_>,
) {
    let core_mod = audit_sources.source("src/types/mod.rs");
    let core_layout_tests = audit_sources.source("src/types/layout_tests.rs");
    let heap_rs = audit_sources.source("src/types/heap.rs");
    let heap_api_rs = audit_sources.source("src/allocator/heap.rs");
    let ptr_rs = audit_sources.source("src/types/ptr.rs");
    let scalar_rs = audit_sources.source("src/types/scalar.rs");
    let unsafe_migration_allowlist = audit_sources.unsafe_migration_allowlist;
    let page_states_rs = audit_sources.source("src/types/page_states.rs");
    let validation_extra_rs = audit_sources.source("src/types/validation_extra.rs");
    for module in [
        "arena",
        "atomic",
        "free_actions",
        "freelist",
        "freelist_wrappers",
        "heap",
        "layout",
        "memid",
        "page",
        "page_queues",
        "page_states",
        "ptr",
        "scalar",
        "validation",
        "validation_extra",
    ] {
        assert!(
            core_mod.contains(&format!("mod {module};")),
            "core_types should own the {module} semantic wrapper module"
        );
    }

    for witness in [
        "LocalPage",
        "ForeignPage",
        "AbandonedPageType",
        "AbandonedMappedPageType",
        "OwnedAbandonedPage",
        "ActivePage",
        "AvailablePage",
        "FullPage",
        "RetiredPage",
        "DestroyedPage",
    ] {
        assert!(
            page_states_rs.contains(&format!("pub(crate) struct {witness}(PagePtr);")),
            "{witness} should keep its page pointer field private"
        );
        assert!(
            !page_states_rs.contains(&format!("pub(crate) struct {witness}(pub(crate) PagePtr);")),
            "{witness} should not expose tuple construction outside page_states"
        );
    }
    assert!(
        page_states_rs.contains("pub(crate) unsafe fn new_unchecked(page: PagePtr) -> Self")
            && page_states_rs.contains("pub(crate) fn page(self) -> PagePtr"),
        "page-state witnesses should expose explicit unchecked constructors and page accessors"
    );
    assert!(
        page_states_rs.contains("pub(crate) struct ThreadHeapLocal(ThreadHeapPtr);")
            && page_states_rs
                .contains("pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self")
            && page_states_rs.contains("pub(crate) fn ptr(self) -> ThreadHeapPtr")
            && page_states_rs
                .contains("pub(crate) fn borrowed(self) -> crate::core_types::ThreadHeapBorrowed")
            && page_states_rs
                .contains("pub(crate) unsafe fn state(self) -> crate::core_types::ThreadHeapState")
            && !page_states_rs
                .contains("pub(crate) struct ThreadHeapLocal(pub(crate) ThreadHeapPtr);"),
        "ThreadHeapLocal should keep its thread-heap pointer private behind explicit construction"
    );
    assert!(
        heap_rs.contains("pub(crate) struct ThreadHeapBorrowed(ThreadHeapPtr);")
            && heap_rs.contains("pub(crate) enum ThreadHeapState")
            && heap_rs.contains("Uninitialized(ThreadHeapPtr)")
            && heap_rs.contains("Initialized(ThreadHeapBorrowed)")
            && heap_rs.contains("Main(ThreadHeapBorrowed)")
            && heap_rs.contains("Deleted(ThreadHeapPtr)")
            && heap_rs.contains("pub(crate) unsafe fn new(theap: ThreadHeapPtr) -> Option<Self>")
            && heap_rs.contains("if unsafe { theap.heap() }.is_some()")
            && heap_rs.contains("pub(crate) unsafe fn try_local(")
            && heap_rs.contains("match unsafe { self.state() }")
            && heap_rs.contains("ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return Err(self),")
            && heap_rs.contains("pub(crate) unsafe fn state(self) -> ThreadHeapState")
            && heap_rs.contains("if unsafe { self.tld() }.is_none()")
            && heap_rs.contains("crate::internal::_mi_is_heap_main(heap.as_ptr())")
            && heap_rs.contains(
                "const _: unsafe fn(ThreadHeapPtr) -> ThreadHeapState = ThreadHeapPtr::state;"
            )
            && core_mod.contains("ThreadHeapBorrowed")
            && core_mod.contains("ThreadHeapState")
            && core_layout_tests.contains("assert_layout!(ThreadHeapBorrowed, ThreadHeapPtr);"),
        "thread-heap witnesses should encode borrowed/local initialized, main, and deleted states"
    );
    assert!(
        heap_api_rs.contains("HeapPtr")
            && heap_api_rs.contains("ThreadHeapPtr")
            && heap_api_rs.contains("unsafe fn mi_heap_theap_typed(heap: Option<HeapPtr>) -> Option<ThreadHeapPtr>")
            && heap_api_rs.contains("unsafe { mi_heap_theap_typed(HeapPtr::new(heap)) }")
            && heap_api_rs.contains(".map(ThreadHeapPtr::as_ptr)")
            && heap_api_rs.contains("ThreadHeapPtr::new(unsafe { _mi_heap_theap(heap) })")
            && !heap_api_rs.contains("pub unsafe extern \"C\" fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t {\n    // SAFETY: upheld by the caller contract for this C ABI boundary.\n    unsafe { _mi_heap_theap(heap) }\n}"),
        "mi_heap_theap should parse the raw ABI pointer once and use HeapPtr/ThreadHeapPtr internally"
    );
    assert!(
        ptr_rs.contains("pub(crate) unsafe fn numa_node(self) -> NumaNode")
            && ptr_rs.contains("pub(crate) unsafe fn set_numa_node(self, node: NumaNode)")
            && heap_api_rs.contains("HeapPtr::new(unsafe { mi_heap_main() })")
            && heap_api_rs.contains("HeapPtr::new(heap)")
            && heap_api_rs.contains(
                "unsafe fn mi_heap_set_numa_affinity_typed(heap: HeapPtr, numa_node: crate::core_types::NumaNode)"
            )
            && heap_api_rs.contains("heap.set_numa_node(numa_node.normalize(numa_count))")
            && !heap_api_rs.contains("(*heap).numa_node = numa_node.normalize(numa_count).get();"),
        "mi_heap_set_numa_affinity should parse the raw ABI pointer once and use HeapPtr/NumaNode internally"
    );
    for witness in ["MainHeap", "OwnedHeap", "BorrowedHeap"] {
        assert!(
            heap_rs.contains(&format!("pub(crate) struct {witness}(HeapPtr);"))
                && heap_rs.contains("pub(crate) unsafe fn new_unchecked(heap: HeapPtr) -> Self")
                && heap_rs.contains("pub(crate) fn ptr(self) -> HeapPtr")
                && !heap_rs.contains(&format!("pub(crate) struct {witness}(pub(crate) HeapPtr);")),
            "{witness} should be a live heap wrapper with explicit construction and pointer access"
        );
        assert!(
            core_mod.contains("pub(crate) use heap::")
                && core_mod.contains(witness)
                && core_layout_tests.contains(&format!("assert_layout!({witness}, HeapPtr);")),
            "{witness} should be re-exported and layout-checked"
        );
    }
    assert!(
        heap_rs.contains("pub(crate) unsafe fn new(heap: HeapPtr) -> Option<Self>")
            && heap_rs.contains("crate::internal::_mi_is_heap_main(heap.as_ptr())")
            && heap_rs.contains("const _: unsafe fn(HeapPtr) -> Option<MainHeap> = MainHeap::new;"),
        "MainHeap should have a checked constructor backed by the runtime main-heap predicate"
    );
    assert!(
        heap_rs.contains("const _: unsafe fn(HeapPtr) -> Option<OwnedHeap> = OwnedHeap::new;")
            && heap_rs.matches("SAFETY[AUDIT:typed-heap-domain]").count() >= 15
            && heap_rs.contains("impl OwnedHeap {\n    /// # Safety\n    /// `heap` must point to initialized heap metadata owned by the current\n    /// delete or destroy path.\n    #[inline(always)]\n    pub(crate) unsafe fn new(heap: HeapPtr) -> Option<Self>")
            && heap_rs.contains(
                "if unsafe { crate::internal::_mi_is_heap_main(heap.as_ptr()) } {\n            None\n        } else {\n            Some(Self(heap))"
            ),
        "OwnedHeap should have a checked constructor that rejects the main heap"
    );
    assert!(
        heap_rs.contains("pub(crate) unsafe fn from_allocation_raw(heap: *mut mi_heap_t) -> Option<Self>")
            && heap_rs.contains("pub(crate) unsafe fn from_lookup_raw(heap: *mut mi_heap_t) -> Option<Self>")
            && heap_rs.contains("const _: unsafe fn(*mut mi_heap_t) -> Option<BorrowedHeap> = BorrowedHeap::from_allocation_raw;")
            && heap_rs.contains("const _: unsafe fn(*mut mi_heap_t) -> Option<BorrowedHeap> = BorrowedHeap::from_lookup_raw;")
            && heap_rs.matches("HeapPtr::new(heap).map(Self)").count() >= 2,
        "BorrowedHeap should have checked raw constructors for allocation and lookup contexts"
    );
    for witness in ["MainSubproc", "BorrowedSubproc"] {
        assert!(
            heap_rs.contains(&format!("pub(crate) struct {witness}(SubprocPtr);"))
                && heap_rs.contains("pub(crate) fn ptr(self) -> SubprocPtr")
                && !heap_rs.contains(&format!(
                    "pub(crate) struct {witness}(pub(crate) SubprocPtr);"
                ))
                && core_mod.contains(witness)
                && core_layout_tests.contains(&format!("assert_layout!({witness}, SubprocPtr);")),
            "{witness} should be a live subproc wrapper with explicit construction and pointer access"
        );
    }
    assert!(
        heap_rs.contains(
            "const _: unsafe fn(SubprocPtr) -> MainSubproc = MainSubproc::new_unchecked;"
        ) && heap_rs
            .contains("const _: unsafe fn(SubprocPtr) -> BorrowedSubproc = BorrowedSubproc::new;"),
        "subproc witness constructors should be compile-time checked"
    );
    assert!(
        ptr_rs.contains("pub(crate) unsafe fn thread_id(self) -> Option<ThreadId>")
            && ptr_rs
                .contains("pub(crate) unsafe fn set_thread_id(self, thread_id: Option<ThreadId>)")
            && ptr_rs.contains("pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr>")
            && ptr_rs
                .contains("pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>)")
            && ptr_rs.contains("pub(crate) unsafe fn stats(self) -> Option<*mut mi_stats_t>")
            && ptr_rs.contains("self.subproc().map(|subproc| subproc.stats())")
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena_count_cell(self) -> Option<AtomicUsizeField>"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena(self, index: ArenaIndex) -> Option<ArenaPtr>"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn set_arena(self, index: ArenaIndex, arena: Option<ArenaPtr>)"
            )
            && ptr_rs.contains("pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t"),
        "TldPtr should expose typed thread id, subproc, stats, memid, and arena-data accessors"
    );
    assert!(
        scalar_rs.contains("pub(crate) struct SubprocSeq(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct ArenaCount(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct ThreadCount(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct HeapCount(pub(crate) usize);")
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder)"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn thread_count(self, order: MemoryOrder) -> ThreadCount"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn thread_total_count(self, order: MemoryOrder) -> ThreadCount"
            )
            && ptr_rs
                .contains("pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount")
            && ptr_rs.contains(
                "pub(crate) unsafe fn heap_total_count(self, order: MemoryOrder) -> HeapCount"
            ),
        "subproc sequence and count fields should have scalar wrappers plus typed SubprocPtr accessors"
    );
    assert!(
        !unsafe_migration_allowlist.contains("src/types/scalar.rs")
            && scalar_rs
                .matches("SAFETY[AUDIT:scalar-domain-wrappers]")
                .count()
                >= 20
            && scalar_rs
                .contains("Scalar domain wrappers for allocator sizes, counts, indexes, and bins.")
            && scalar_rs.contains("pub(crate) struct UsableOut(*mut usize);")
            && scalar_rs.contains("pub(crate) unsafe fn write_if_present")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(size: usize) -> Self")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(index: usize) -> Self")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(count: u32) -> Self")
            && scalar_rs.contains("pub(crate) const unsafe fn new_unchecked(idx: u16) -> Self"),
        "scalar domain wrappers should stay locally audited and off the unsafe migration allowlist"
    );
    let heap_delete_rs = audit_sources.source("src/allocator/heap.rs");
    let heap_delete_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_delete(heap: *mut mi_heap_t)")
        .nth(1)
        .expect("find mi_heap_delete body");
    let owned_reject = heap_delete_body
        .find("let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else")
        .expect("mi_heap_delete should reject the main heap through OwnedHeap");
    let delete_typed = heap_delete_body
        .find("unsafe { mi_heap_delete_typed(heap) };")
        .expect("mi_heap_delete should enter the typed delete path after validation");
    assert!(
        owned_reject < delete_typed,
        "mi_heap_delete must reject MainHeap before entering the OwnedHeap delete path"
    );
    let heap_delete_typed_body = heap_delete_rs
        .split("unsafe fn mi_heap_delete_typed(heap: OwnedHeap)")
        .nth(1)
        .expect("find mi_heap_delete_typed body");
    assert!(
        heap_delete_typed_body.contains("mi_heap_free_theaps(heap);")
            && heap_delete_typed_body.contains("_mi_heap_move_pages(heap_raw, mi_heap_main());")
            && heap_delete_typed_body.contains("mi_heap_free(heap);"),
        "mi_heap_delete_typed should free thread heaps through OwnedHeap, move pages, and free OwnedHeap"
    );
    assert!(
        heap_delete_rs.contains("unsafe fn mi_heap_delete_typed(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_free_theaps(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && !heap_delete_rs.contains("unsafe fn mi_heap_delete_typed(heap: MainHeap)")
            && !heap_delete_rs.contains("unsafe fn mi_heap_free(heap: MainHeap)"),
        "main heaps should have no typed entry point into owned deletion helpers"
    );
    let heap_destroy_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_destroy(heap: *mut mi_heap_t)")
        .nth(1)
        .expect("find mi_heap_destroy body");
    let destroy_main_reject = heap_destroy_body
        .find("let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else")
        .expect("mi_heap_destroy should reject the main heap through OwnedHeap");
    let force_destroy = heap_destroy_body
        .find("mi_heap_force_destroy_owned(heap)")
        .expect("mi_heap_destroy should force-destroy an OwnedHeap after validation");
    assert!(
        destroy_main_reject < force_destroy,
        "mi_heap_destroy must reject MainHeap before the owned destroy path"
    );
    assert!(
        heap_delete_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_force_destroy_main(heap: MainHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_force_destroy_owned(heap: OwnedHeap)")
            && heap_delete_rs.contains("if let Some(heap) = MainHeap::new(heap)")
            && heap_delete_rs.contains("} else if let Some(heap) = OwnedHeap::new(heap)"),
        "heap deletion helpers should use OwnedHeap/MainHeap typed teardown witnesses"
    );
    let heap_check_owned_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_check_owned(heap: *mut mi_heap_t, p: *const c_void) -> bool")
        .nth(1)
        .expect("find mi_heap_check_owned body");
    assert!(
        heap_check_owned_body.contains("BorrowedHeap::from_lookup_raw(heap)")
            && heap_check_owned_body.contains("unsafe { mi_heap_check_owned_typed(heap, p) }")
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_typed(heap: BorrowedHeap, p: *const c_void) -> bool"
            )
            && heap_delete_rs.contains("OwnedHeap::new(heap.ptr())")
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_owned(heap: OwnedHeap, p: *const c_void) -> bool"
            )
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_borrowed(heap: BorrowedHeap, p: *const c_void) -> bool"
            ),
        "mi_heap_check_owned should parse heap arguments through BorrowedHeap and route non-main checks through OwnedHeap"
    );
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
    let page_core_rs = audit_sources.source("src/types/page.rs");
    let atomic_rs = audit_sources.source("src/types/atomic.rs");
    let internal_atomic_rs = audit_sources.source("src/internal/atomic.rs");
    let strict_check_rs = audit_sources.strict_type_checker;
    assert!(
        page_core_rs.contains("pub(crate) struct PageFlags(usize);")
            && page_core_rs.contains("pub(crate) const fn new(bits: usize) -> Option<Self>")
            && page_core_rs.contains("if (bits & !MI_PAGE_FLAG_MASK) == 0")
            && page_core_rs.contains("pub(crate) struct PageThreadId(usize);")
            && page_core_rs.contains("pub(crate) fn from_parts(owner: PageOwnerId, flags: PageFlags) -> Self")
            && page_core_rs.contains("Self(tid_val | flags.bits())")
            && page_core_rs.contains("pub(crate) fn flags(self) -> PageFlags")
            && page_core_rs.contains("pub(crate) fn owner_bits(self) -> usize")
            && page_core_rs.contains("self.0 & !MI_PAGE_FLAG_MASK")
            && page_core_rs.contains("pub(crate) fn classify_free_path(self, current: ThreadId) -> FreePathKind")
            && atomic_rs.contains("pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);")
            && atomic_rs.contains("pub(crate) unsafe fn load(self) -> PageThreadId")
            && atomic_rs.contains("pub(crate) unsafe fn set_flags(self, flags: PageFlags, set: bool) -> PageThreadId")
            && atomic_rs.contains("unsafe { self.0.fetch_or(flags.bits(), Ordering::Relaxed) }")
            && atomic_rs.contains("unsafe { self.0.fetch_and(!flags.bits(), Ordering::Relaxed) }")
            && atomic_rs.contains("pub(crate) unsafe fn store_owner(self, owner: PageOwnerId)")
            && atomic_rs.contains("pub(crate) unsafe fn compare_exchange_owner(\n        self,\n        expected: &mut PageThreadId,")
            && strict_check_rs.contains("ALLOWED_XTHREAD_ID = {")
            && strict_check_rs.contains("\"src/types/page.rs\"")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"xthread-id-bitop\"")
            && strict_check_rs.contains("message=\"direct xthread_id bit operation outside typed helpers\""),
        "xthread_id bit operations should be isolated to PageThreadId/PageFlags/AtomicPageThreadId and enforced by the strict checker"
    );
    let freelist_rs = audit_sources.source("src/types/freelist.rs");
    assert!(
        freelist_rs.contains("pub(crate) struct ThreadFreeWord(pub(crate) usize);")
            && freelist_rs.contains("pub(crate) const OWNED_BIT: usize = 1;")
            && freelist_rs.contains("let ptr_val = self.0 & !Self::OWNED_BIT;")
            && freelist_rs.contains("use super::constants::mi_block_t;")
            && freelist_rs.contains("pub(crate) fn head_raw(self) -> *mut mi_block_t")
            && freelist_rs.contains("pub(crate) const fn is_owned(self) -> bool")
            && freelist_rs.contains("(self.0 & Self::OWNED_BIT) != 0")
            && freelist_rs.contains(
                "pub(crate) fn from_parts(head: Option<BlockPtr>, owned: ThreadFreeOwner) -> Self"
            )
            && freelist_rs
                .contains("crate::internal::mi_assert_internal!((val & Self::OWNED_BIT) == 0);")
            && freelist_rs.contains("Self(ptr_val | bit)")
            && atomic_rs
                .contains("pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);")
            && atomic_rs.contains("pub(crate) fn load_relaxed(self) -> ThreadFreeWord")
            && atomic_rs.contains("pub(crate) fn push_remote_acq_rel(")
            && atomic_rs.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool"
            )
            && atomic_rs.contains("pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList")
            && atomic_rs.contains("pub(crate) unsafe fn claim_acq_rel(self) -> bool")
            && strict_check_rs.contains("ALLOWED_XTHREAD_FREE = {")
            && strict_check_rs.contains("\"src/types/freelist.rs\"")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"xthread-free-bitop\"")
            && strict_check_rs
                .contains("message=\"direct xthread_free bit operation outside typed helpers\""),
        "xthread_free bit operations should be isolated to ThreadFreeWord/AtomicThreadFree and enforced by the strict checker"
    );
    let ptr_core_rs = audit_sources.source("src/types/ptr.rs");
    let core_types_layout_tests_rs = audit_sources.source("src/types/layout_tests.rs");
    let freelist_wrappers_rs = audit_sources.source("src/types/freelist_wrappers.rs");
    let internal_rs = audit_sources.source("src/internal/mod.rs");
    let allowlist_rs = audit_sources.strict_type_allowlist;
    assert!(
        ptr_core_rs.contains("#[repr(transparent)]\n        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n        pub(crate) struct $name(NonNull<$raw_type>);")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) fn new(ptr: *mut $raw_type) -> Option<Self>")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) fn as_ptr(self) -> *mut $raw_type")
            && ptr_core_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\npub(crate) struct PageConstPtr(NonNull<mi_page_t>);")
            && page_states_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct LocalPage(PagePtr);")
            && page_states_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct OwnedAbandonedPage(PagePtr);")
            && page_queues_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct NormalPageQueue(PageQueuePtr);")
            && page_queues_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct FullPageQueue(PageQueuePtr);")
            && validation_extra_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct FreeBlock(BlockPtr);")
            && freelist_wrappers_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug)]\npub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);")
            && freelist_wrappers_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug)]\npub(crate) struct LocalFreeList(pub(crate) Option<BlockPtr>);")
            && core_types_layout_tests_rs.contains("fn pointer_wrappers_match_nonnull_layout()")
            && core_types_layout_tests_rs.contains("fn optional_pointer_wrappers_are_pointer_sized()")
            && core_types_layout_tests_rs.contains("fn witness_wrappers_match_pointer_layout()")
            && core_types_layout_tests_rs
                .contains("fn packed_and_atomic_wrappers_match_raw_layout()")
            && !unsafe_migration_allowlist.contains("src/types/ptr.rs")
            && ptr_core_rs
                .matches("SAFETY[AUDIT:typed-pointer-accessors]")
                .count()
                >= 150,
        "hot strict-type wrappers should stay transparent, copyable where they carry copyable fast-path witnesses, inlineable, and covered by layout/niche tests"
    );
    assert!(
        freelist_wrappers_rs.contains("pub(crate) struct EncodedPtr<T> {")
            && freelist_wrappers_rs.contains("word: EncodedWord,")
            && freelist_wrappers_rs.contains(
                "pub(crate) unsafe fn read_word_from_raw(block: *const mi_block_t) -> Self"
            )
            && freelist_wrappers_rs
                .contains("word: EncodedWord::new(unsafe { (*block.cast_mut()).next })")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn write_word_to_raw(self, block: BlockPtr)")
            && freelist_wrappers_rs
                .contains("unsafe { (*block.as_ptr()).next = self.word.get() };")
            && freelist_wrappers_rs
                .contains("unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_wrappers_rs.contains("Self(unsafe { (*block.cast_mut()).next })")
            && freelist_wrappers_rs.contains("unsafe fn write_to(self, block: BlockPtr)")
            && freelist_wrappers_rs.contains("unsafe { (*block.as_ptr()).next = self.0 };")
            && freelist_wrappers_rs.contains("pub(crate) enum BlockLink {")
            && freelist_wrappers_rs.contains("FreeList(EncodedPtr<crate::types::mi_block_t>),")
            && freelist_wrappers_rs.contains("Guarded(GuardedBlockTag),")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_wrappers_rs
                .contains("Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn write_to(self, block: BlockPtr)")
            && freelist_wrappers_rs
                .contains("Self::FreeList(link) => unsafe { link.write_word_to_raw(block) }")
            && freelist_wrappers_rs
                .contains("Self::Guarded(tag) => unsafe { tag.write_to(block) }")
            && freelist_wrappers_rs
                .contains("pub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);")
            && freelist_wrappers_rs
                .contains("pub(crate) struct LocalFreeList(pub(crate) Option<BlockPtr>);")
            && freelist_wrappers_rs
                .contains("let next_raw = unsafe { mi_block_next(raw_page, head_raw) };")
            && freelist_wrappers_rs
                .contains("unsafe { mi_block_set_next(raw_page, block_raw, prev_raw) };")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn encoded_link(self) -> EncodedPtr<mi_block_t>")
            && ptr_core_rs.contains("unsafe { EncodedPtr::read_word_from_raw(self.as_ptr()) }")
            && ptr_core_rs.contains("pub(crate) unsafe fn write_link(self, link: BlockLink)")
            && ptr_core_rs.contains("unsafe { link.write_to(self) };")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn next(self, page: PagePtr) -> Option<BlockPtr>")
            && ptr_core_rs.contains(
                "pub(crate) unsafe fn set_next(self, page: PagePtr, next: Option<BlockPtr>)"
            )
            && ptr_core_rs.contains("unsafe { self.write_link(BlockLink::FreeList(link)) };")
            && strict_check_rs.contains("ALLOWED_BLOCK_NEXT = {")
            && strict_check_rs.contains("\"src/types/freelist_wrappers.rs\"")
            && strict_check_rs.contains("kind=\"block-next-access\"")
            && strict_check_rs
                .contains("message=\"direct mi_block_s.next access outside free-list wrappers\"")
            && !allowlist_rs
                .contains("raw-pointer-parameter|src/types/freelist_wrappers.rs|*|mi_block_t|")
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|read_word_from_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|from_unencoded_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|encode_block_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|read_guarded_from_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|read_from_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|is_guarded_raw|mi_block_t|"
            )
            && allowlist_rs.contains(
                "raw-pointer-parameter|src/types/freelist_wrappers.rs|from_raw|mi_block_t|"
            ),
        "mi_block_s.next reads/writes should stay behind EncodedPtr, BlockLink, BlockPtr, or free-list wrappers and strict checker coverage"
    );
    assert!(
        atomic_rs.contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && atomic_rs.contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && atomic_rs.contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && atomic_rs.contains("let atomic = unsafe { AtomicUsize::from_ptr(self.0) };")
            && atomic_rs.contains("let atomic = unsafe { AtomicI64::from_ptr(self.0) };")
            && atomic_rs.contains("let atomic = unsafe { AtomicPtr::from_ptr(self.0) };")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder)")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder)")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && atomic_rs.contains("SAFETY[AUDIT:typed-atomic-domain]")
            && atomic_rs
                .matches("SAFETY[AUDIT:typed-atomic-domain]")
                .count()
                >= 50
            && strict_check_rs.contains("ALLOWED_ATOMIC_CAST = {")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"atomic-cast\"")
            && strict_check_rs
                .contains("message=\"pointer-to-atomic cast outside atomic field wrappers\"")
            && strict_check_rs.contains("ATOMIC_CAST_RE = re.compile")
            && !strict_check_rs.contains("\"src/internal/atomic.rs\",\n}")
            && strict_check_rs.contains(
                "\"src/internal/bitmap/bfield.rs\": {\"src/bitmap.rs\", \"src/internal/bitmap.rs\"}"
            )
            && !strict_check_rs
                .contains("\"src/allocator/page/bins.rs\": {\"src/allocator/page.rs\"}")
            && !strict_check_rs
                .contains("\"src/allocator/page/counts.rs\": {\"src/allocator/page.rs\"}")
            && !strict_check_rs
                .contains("\"src/allocator/page/flags.rs\": {\"src/allocator/page.rs\"}")
            && !strict_check_rs
                .contains("\"src/allocator/page/validation.rs\": {\"src/allocator/page.rs\"}")
            && strict_check_rs.contains("ALLOWLIST = ROOT / \"strict-type-allowlist.txt\""),
        "pointer-to-atomic casts for allocator metadata should be centralized in typed atomic field wrappers and guarded by the strict checker"
    );
    assert!(
        strict_check_rs.contains("REMOVE_ENTRY_CAP = 0")
            && !allowlist_rs.contains("bare-domain-usize|src/allocator/aligned.rs|*|usize|")
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|aligned_layout_or_report|usize|"
            )
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|mi_theap_malloc_zero_aligned_at|usize|"
            )
            && allowlist_rs.contains(
                "bare-domain-usize|src/allocator/aligned.rs|mi_theap_recalloc_aligned|usize|"
            ),
        "aligned strict-type allowlist coverage should stay symbol-level with no removable broad entries"
    );
    assert!(
        internal_atomic_rs.contains("pub enum mi_memory_order_e {")
            && internal_atomic_rs.contains("pub type mi_memory_order = mi_memory_order_e;")
            && internal_atomic_rs.contains("fn load_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn store_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn rmw_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn cas_failure_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e)")
            && atomic_rs.contains("pub(crate) type MemoryOrder = Ordering;")
            && atomic_rs
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount")
            && ptr_core_rs.contains(
                "pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder)"
            )
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount"),
        "C-compatible mi_memory_order_e should stay in the atomic facade while internal wrappers use MemoryOrder/Ordering"
    );
    assert!(
        ptr_core_rs.contains("pub(crate) fn ptr_exposed_addr<T>(ptr: *const T) -> usize")
            && ptr_core_rs.contains("ptr as usize")
            && ptr_core_rs.contains("ptr.addr()")
            && ptr_core_rs.contains("pub(crate) fn ptr_addr_diff<T, U>(")
            && ptr_core_rs.contains("ptr_exposed_addr(hi).wrapping_sub(ptr_exposed_addr(lo))")
            && ptr_core_rs.contains("pub(crate) fn ptr_align_up<T>(")
            && ptr_core_rs
                .contains(".map_addr(|addr| crate::internal::_mi_align_up(addr, alignment))")
            && ptr_core_rs.contains(
                "crate::internal::_mi_align_up(ptr_exposed_addr(ptr), alignment) as *mut c_void"
            )
            && ptr_core_rs.contains("pub(crate) fn ptr_align_down<T>(")
            && ptr_core_rs
                .contains(".map_addr(|addr| crate::internal::_mi_align_down(addr, alignment))")
            && ptr_core_rs.contains(
                "crate::internal::_mi_align_down(ptr_exposed_addr(ptr), alignment) as *mut c_void"
            )
            && freelist_rs.contains("pub(crate) struct ThreadFreeWord(pub(crate) usize);")
            && freelist_rs.contains("use super::constants::mi_block_t;")
            && freelist_rs.contains("use super::ptr::{BlockPtr, ptr_exposed_addr};")
            && freelist_rs.contains("pub(crate) fn head_raw(self) -> *mut mi_block_t")
            && freelist_rs.contains("crate::provenance::resolve_address(ptr_val)")
            && freelist_rs.contains("ptr_val as *mut mi_block_t")
            && freelist_rs.contains("Some(unsafe { BlockPtr::new_unchecked(ptr) })")
            && freelist_rs.contains("let val = ptr_exposed_addr(ptr.as_ptr());")
            && freelist_rs.contains("crate::provenance::remember_address(val, ptr.as_ptr());")
            && freelist_wrappers_rs.contains("use super::ptr::{BlockPtr, ptr_exposed_addr};")
            && freelist_wrappers_rs.contains("pub(crate) struct EncodedWord(usize);")
            && freelist_wrappers_rs.contains("let word = ptr_exposed_addr(block);")
            && freelist_wrappers_rs.contains("crate::provenance::remember_address(word, block);")
            && freelist_wrappers_rs.contains("word as *mut mi_block_t")
            && internal_rs.contains("pub(crate) fn mi_ptr_decode(")
            && internal_rs.contains(
                "let decoded_addr = mi_rotr(x.get().wrapping_sub(keys[0]), keys[0]) ^ keys[1];"
            )
            && internal_rs.contains("crate::provenance::resolve_encoded(x.get(), null, keys)")
            && internal_rs.contains("let p = decoded_addr as *mut c_void;")
            && internal_rs.contains("pub(crate) fn mi_ptr_encode(")
            && internal_rs.contains("crate::core_types::ptr_exposed_addr(null)")
            && internal_rs.contains("crate::core_types::ptr_exposed_addr(p)")
            && internal_rs
                .contains("crate::provenance::remember_encoded(word.get(), null, keys, p);")
            && strict_check_rs.contains("PTR_USIZE_RE = re.compile")
            && strict_check_rs.contains("ALLOWED_PTR_USIZE = {")
            && strict_check_rs.contains("\"src/types/ptr.rs\"")
            && strict_check_rs.contains("kind=\"pointer-usize-roundtrip\"")
            && strict_check_rs.contains("domain=\"provenance\"")
            && strict_check_rs.contains(
                "message=\"pointer/integer round trip outside typed provenance helpers\""
            )
            && !allowlist_rs.contains(
                "pointer-usize-roundtrip|src/types/freelist_wrappers.rs|*|encoded-pointer|"
            )
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/internal/mod.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/internal/mod.rs|mi_page_slice_offset_of|packed-pointer|"
            )
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/aligned.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/aligned.rs|mi_theap_malloc_zero_aligned_at_overalloc|alignment/provenance|"
            )
            && allowlist_rs
                .contains("pointer-usize-roundtrip|src/allocator/alloc.rs|*|padding/provenance|")
            && allowlist_rs
                .contains("pointer-usize-roundtrip|src/types/freelist.rs|*|packed-thread-free|")
            && allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/free.rs|*|validation|")
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/platform/os.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/platform/os.rs|mi_os_page_align_areax|os-memory|"
            )
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/platform/os.rs|_mi_os_free_ex_typed|os-memory|"
            )
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/platform/os.rs|mi_os_alloc_aligned_memid|os-memory|"
            ),
        "pointer-to-integer round trips should stay behind typed packed-pointer/provenance helpers or explicit strict-type allowlist boundaries"
    );
    assert!(
        strict_check_rs.contains("METADATA_REF_RE = re.compile")
            && strict_check_rs.contains("kind=\"metadata-reference\"")
            && strict_check_rs.contains("domain=\"metadata\"")
            && strict_check_rs
                .contains("message=\"broad raw metadata reference outside typed field helpers\"")
            && allowlist_rs.contains("metadata-reference|src/allocator/free.rs|*|mi_page_t|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/stats.rs|*|")
            && allowlist_rs
                .contains("metadata-reference|src/allocator/stats.rs|mi_heap_stats_get|mi_heap_t|")
            && allowlist_rs.contains("metadata-reference|src/types/atomic.rs|*|atomic-field|")
            && ptr_core_rs.contains("let raw = self.as_ptr();")
            && ptr_core_rs.contains("ptr::read(ptr::addr_of!((*raw).block_size))")
            && ptr_core_rs
                .contains("ptr::write(ptr::addr_of_mut!((*raw).block_size), block_size.get())")
            && ptr_core_rs
                .contains("AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).free), head)")
            && ptr_core_rs
                .contains("LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).local_free), head)")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_id_cell(self) -> AtomicPageThreadId")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_free_cell(self) -> AtomicThreadFree")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_cell(self) -> AtomicPtrField<mi_heap_t>")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count_cell(self) -> AtomicUsizeField")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count_cell(self) -> AtomicUsizeField"),
        "aliasable allocator metadata should be accessed through pointer wrapper field helpers and guarded against broad Rust references"
    );
    assert!(
        strict_check_rs.contains("class ItemContext:")
            && strict_check_rs
                .contains("def item_contexts_by_line(text: str) -> dict[int, ItemContext]:")
            && strict_check_rs.contains("def run_self_tests() -> None:")
            && strict_check_rs.contains("symbol=context.symbols")
            && strict_check_rs.contains("f\"{message} in {context.primary_symbol}\"")
            && strict_check_rs.contains("assert contexts[3].symbols == (\"free_function\",)")
            && strict_check_rs
                .contains("assert contexts[8].symbols == (\"AtomicField::load\", \"load\")")
            && strict_check_rs
                .contains("assert contexts[13].symbols == (\"AtomicField::MASK\", \"MASK\")")
            && strict_check_rs.contains("assert \"imported\" in contexts[18].symbols")
            && strict_check_rs.contains("assert contexts[23].symbols == (\"TABLE\",)")
            && strict_check_rs.contains("\"pointer-usize-roundtrip\"")
            && strict_check_rs.contains("\"AtomicField::load\"")
            && strict_check_rs.contains("strict-type self-test failed"),
        "strict-type line-pattern checks should use item-aware symbols and self-test symbol-level line-pattern allowlist entries"
    );
    for platform_path in [
        "src/platform/windows.rs",
        "src/platform/unix.rs",
        "src/platform/wasi.rs",
        "src/platform/os.rs",
        "src/allocator/aligned.rs",
        "src/internal/mod.rs",
        "src/allocator/stats.rs",
        "src/allocator/thread_heap.rs",
    ] {
        assert!(
            !allowlist_rs.lines().any(|line| {
                let mut fields = line.split('|');
                let _kind = fields.next();
                let path = fields.next();
                let symbol = fields.next();
                path == Some(platform_path) && symbol == Some("*")
            }),
            "{platform_path} should not retain path-wide strict-type allowlist entries"
        );
    }
    for required in [
        "raw-pointer-parameter|src/platform/windows.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/platform/unix.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/platform/wasi.rs|_mi_prim_thread_associate_default_theap|mi_theap_t|",
        "raw-pointer-parameter|src/platform/os.rs|_mi_os_free_ex|mi_subproc_t|",
        "metadata-reference|src/platform/unix.rs|_mi_prim_getenv|platform|",
        "metadata-reference|src/platform/windows.rs|McGenControlCallbackV2|platform|",
        "pointer-usize-roundtrip|src/platform/os.rs|mi_os_alloc_aligned_memid|os-memory|",
        "pointer-usize-roundtrip|src/platform/unix.rs|unix_mmap_prim_aligned|platform|",
        "pointer-usize-roundtrip|src/platform/wasi.rs|mi_prim_mem_grow|platform|",
        "heap-allocating-type|src/platform/windows.rs|mi_getenv_value|String|",
        "bare-domain-usize|src/platform/windows.rs|win_virtual_alloc|usize|",
        "bare-domain-usize|src/platform/unix.rs|unix_mmap_prim_aligned|usize|",
        "bare-domain-usize|src/platform/wasi.rs|mi_wasi_try_alloc|usize|",
        "bare-domain-usize|src/platform/os.rs|mi_os_alloc_aligned_ex|usize|",
        "raw-pointer-parameter|src/allocator/aligned.rs|mi_padding_shrink_call|mi_block_t|",
        "raw-pointer-parameter|src/allocator/aligned.rs|mi_theap_malloc_zero_aligned_at|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/aligned.rs|mi_heap_realloc_aligned|mi_heap_t|",
        "pointer-usize-roundtrip|src/allocator/aligned.rs|mi_theap_realloc_zero_aligned_at_req|alignment/provenance|",
        "raw-pointer-parameter|src/internal/mod.rs|_mi_malloc_generic|mi_theap_t|",
        "raw-pointer-parameter|src/internal/mod.rs|mi_page_block_size|mi_page_t|",
        "raw-pointer-parameter|src/internal/mod.rs|mi_block_nextx|mi_block_t|",
        "pointer-usize-roundtrip|src/internal/mod.rs|mi_page_slice_offset_of|packed-pointer|",
        "raw-pointer-parameter|src/allocator/stats.rs|mi_stats_merge_theap_to_heap|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/stats.rs|mi_heap_stats_get_json|mi_heap_t|",
        "metadata-reference|src/allocator/stats.rs|__mi_stat_increase_mt|mi_heap_t|",
        "bare-domain-usize|src/allocator/stats.rs|mi_stats_get_bin_size_from_raw|usize|",
        "raw-pointer-parameter|src/allocator/thread_heap.rs|_mi_heap_area_init|mi_page_t|",
        "raw-pointer-parameter|src/allocator/thread_heap.rs|_mi_theap_create|mi_heap_t|",
        "raw-pointer-parameter|src/allocator/thread_heap.rs|mi_theap_collect_ex|mi_theap_t|",
        "pointer-usize-roundtrip|src/allocator/thread_heap.rs|mi_theap_area_visit_blocks_typed|area-visitor|",
    ] {
        assert!(
            allowlist_rs.contains(required),
            "strict-type allowlist should retain symbol-level entry {required}"
        );
    }
    let callbacks_rs = audit_sources.source("src/types/callbacks.rs");
    let types_rs = audit_sources.source("src/types/constants.rs");
    let abi_rs = audit_sources.source("src/ffi/abi.rs");
    let options_rs = audit_sources.source("src/allocator/options.rs");
    let deferred_rs = audit_sources.source("src/allocator/page/deferred.rs");
    let arena_heap_pages_rs = audit_sources.source("src/allocator/arena/heap_pages.rs");
    let init_rs = audit_sources.source("src/allocator/init.rs");
    let threadlocal_rs = audit_sources.source("src/allocator/threadlocal.rs");
    assert!(
        callbacks_rs.contains("pub(crate) struct CallbackArg(*mut c_void);")
            && callbacks_rs.contains("callback_arg_wrapper!(DeferredFreeCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(OutputCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(ErrorCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(HeapVisitCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(BlockVisitCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(ThreadLocalClaimCallbackArg);")
            && callbacks_rs.contains("SAFETY[AUDIT:callback-wrappers]")
            && callbacks_rs.contains(
                "optional_callback_wrapper!(DeferredFreeCallback, mi_deferred_free_fun);"
            )
            && callbacks_rs.contains("optional_callback_wrapper!(OutputCallback, mi_output_fun);")
            && callbacks_rs.contains("optional_callback_wrapper!(ErrorCallback, mi_error_fun);")
            && callbacks_rs
                .contains("optional_callback_wrapper!(HeapVisitCallback, mi_heap_visit_fun);")
            && callbacks_rs
                .contains("optional_callback_wrapper!(BlockVisitCallback, mi_block_visit_fun);")
            && callbacks_rs.contains("pub(crate) struct ThreadLocalClaimCallback(mi_claim_fun_t);")
            && core_mod.contains("DeferredFreeCallback")
            && core_mod.contains("OutputCallback")
            && core_mod.contains("ErrorCallback")
            && core_mod.contains("HeapVisitCallback")
            && core_mod.contains("BlockVisitCallback")
            && core_mod.contains("ThreadLocalClaimCallback")
            && core_mod.contains("DeferredFreeCallbackArg")
            && core_mod.contains("OutputCallbackArg")
            && core_mod.contains("ErrorCallbackArg")
            && core_mod.contains("HeapVisitCallbackArg")
            && core_mod.contains("BlockVisitCallbackArg")
            && core_mod.contains("ThreadLocalClaimCallbackArg")
            && types_rs.contains("pub type mi_output_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_error_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_deferred_free_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_heap_visit_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_block_visit_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_commit_fun_t = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_claim_fun_t =\n    unsafe extern \"C\" fn(")
            && abi_rs.contains("out: Option<mi_output_fun>, arg: *mut c_void")
            && abi_rs.contains("fun: Option<mi_error_fun>, arg: *mut c_void")
            && abi_rs.contains("deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void")
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && !types_rs.contains("CallbackArg")
            && !abi_rs.contains("CallbackArg")
            && deferred_rs.contains(
                "let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);"
            )
            && deferred_rs
                .contains("let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);")
            && options_rs.contains(
                "fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback"
            )
            && options_rs.contains("let arg = OutputCallbackArg::from_raw(arg);")
            && options_rs.contains(
                "let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));"
            )
            && arena_heap_pages_rs.contains("visitor: BlockVisitCallback,")
            && arena_heap_pages_rs.contains("arg: BlockVisitCallbackArg,")
            && arena_heap_pages_rs.contains("BlockVisitCallback::new(Some(visitor))")
            && arena_heap_pages_rs.contains("BlockVisitCallbackArg::from_raw(arg)")
            && init_rs.contains("let visitor = HeapVisitCallback::new(visitor);")
            && init_rs.contains("let arg = HeapVisitCallbackArg::from_raw(arg);")
            && threadlocal_rs.contains("ThreadLocalClaimCallback::new(mi_thread_local_claim_fun)"),
        "callback function and context pointers should be typed inside allocator internals while raw callback typedefs stay at C ABI boundaries"
    );
    assert!(
        callbacks_rs
            .matches("SAFETY[AUDIT:callback-wrappers]")
            .count()
            >= 8,
        "callback wrapper unsafe operations should retain operation-level audit markers"
    );
    let padding_rs = audit_sources.source("src/types/padding.rs");
    assert!(
        padding_rs.contains("pub(crate) struct PaddingCanary(u32);")
            && padding_rs.contains("pub(crate) struct PaddingDelta(u32);")
            && !padding_rs.contains("pub(crate) struct PaddingCanary(pub(crate) u32);")
            && !padding_rs.contains("pub(crate) struct PaddingDelta(pub(crate) u32);")
            && padding_rs.contains("pub(crate) const fn get(self) -> u32"),
        "padding scalar witnesses should keep backing fields private behind accessors"
    );
    assert!(
        padding_rs.contains(
            "pub(crate) struct Padding {\n    canary: PaddingCanary,\n    delta: PaddingDelta,\n}"
        ) && padding_rs.contains(
            "pub(crate) const fn new(canary: PaddingCanary, delta: PaddingDelta) -> Self"
        ) && padding_rs.contains("pub(crate) const fn canary(self) -> PaddingCanary")
            && padding_rs.contains("pub(crate) const fn delta(self) -> PaddingDelta"),
        "Padding should keep aggregate fields private behind constructor/accessors"
    );
    for witness in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            validation_extra_rs.contains(&format!("pub(crate) struct {witness}(BlockPtr);")),
            "{witness} should keep its block pointer field private"
        );
    }
    assert!(
        validation_extra_rs.contains("pub(crate) unsafe fn new_unchecked(block: BlockPtr) -> Self")
            && validation_extra_rs
                .contains("pub(in crate::types) unsafe fn new_unchecked(block: BlockPtr) -> Self")
            && validation_extra_rs.contains("pub(crate) fn block(self) -> BlockPtr"),
        "allocated/free block witnesses should expose constructors only at the needed boundary"
    );
    for witness in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            !validation_extra_rs.contains(&format!(
                "pub(crate) struct {witness}(pub(crate) BlockPtr);"
            )),
            "{witness} should not expose tuple construction outside validation_extra"
        );
    }

    for (rel, source) in audit_sources.rust_sources {
        if rel.ends_with("/tests.rs")
            || rel == "src/types/atomic/concurrency_models.rs"
            || rel == "src/types/layout_tests.rs"
        {
            continue;
        }

        if rel != "src/types/memid.rs" && rel != "src/types/constants.rs" {
            for raw_union_arm in ["mem.os", "mem.arena", "mem.meta"] {
                assert!(
                    !source.contains(raw_union_arm),
                    "{rel} should inspect memid union arms through MemId::view, not {raw_union_arm}"
                );
            }
        }

        if rel != "src/types/atomic.rs" {
            for atomic_cast in [
                "AtomicUsize::from_ptr",
                "cast::<AtomicUsize>",
                "cast::<core::sync::atomic::AtomicUsize>",
            ] {
                assert!(
                    !source.contains(atomic_cast),
                    "{rel} should route pointer-to-atomic casts through core_types::Atomic*Field"
                );
            }
        }

        if rel.starts_with("src/types/") {
            for deref_impl in ["impl core::ops::Deref", "impl Deref", "Deref for"] {
                assert!(
                    !source.contains(deref_impl),
                    "{rel} should keep allocator metadata wrappers raw-pointer based, without Deref"
                );
            }
        }

        if rel != "src/types/freelist_wrappers.rs" {
            for block_next_access in [
                "(*block).next",
                "(*block.cast_mut()).next",
                "(*block.as_ptr()).next",
            ] {
                assert!(
                    !source.contains(block_next_access),
                    "{rel} should read/write mi_block_s.next through BlockLink, EncodedPtr, or free-list wrappers"
                );
            }
        }

        if rel != "src/types/padding.rs" && rel != "src/types/constants.rs" {
            for raw_padding_access in [
                "mi_padding_t",
                "(*padding).canary",
                "(*padding).delta",
                "cast::<mi_padding_t>",
            ] {
                assert!(
                    !source.contains(raw_padding_access),
                    "{rel} should route padding metadata access through core_types::PaddingPtr"
                );
            }
        }
    }

    let types_spec = audit_sources.types_spec;
    let atomic = audit_sources.source("src/types/atomic.rs");
    let padding_rs = audit_sources.source("src/types/padding.rs");
    let validation_types = audit_sources.source("src/types/validation.rs");
    let alloc_rs = audit_sources.source("src/allocator/alloc.rs");
    let free_rs = audit_sources.source("src/allocator/free.rs");
    assert!(
        padding_rs.contains(
            "pub(crate) fn from_page_block_keys(page: PagePtr, block: BlockPtr, keys: PageKeys) -> Self"
        )
            && padding_rs.contains("page.as_ptr().cast()")
            && padding_rs.contains("block.as_ptr().cast()")
            && !padding_rs.contains("page: *const c_void")
            && !padding_rs.contains("block: *const c_void")
            && padding_rs.contains(
                "pub(crate) fn for_usable(delta: usize, usable_size: UsableSize) -> Option<Self>"
            )
            && padding_rs.contains("u32::try_from(delta).ok().map(Self)")
            && alloc_rs.contains("PaddingCanary::from_page_block_keys")
            && alloc_rs.contains("PagePtr::new_unchecked(page)")
            && alloc_rs.contains("PaddingDelta::for_usable(delta, usable_size)")
            && alloc_rs.contains("crate::core_types::PaddingDelta::new_unchecked(delta as u32)")
            && free_rs.contains("PaddingCanary::from_page_block_keys")
            && free_rs.contains("let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());")
            && free_rs.contains("let block_ptr = crate::core_types::BlockPtr::new_unchecked(block.cast_mut());")
            && free_rs.contains("PaddingDelta::for_usable("),
        "padding metadata should construct canaries from typed PagePtr/BlockPtr plus PageKeys and deltas with the usable-size bound"
    );
    assert!(
        atomic.contains("pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);")
            && !atomic.contains("pub(crate) struct AtomicThreadFree(pub(crate) *mut usize);"),
        "AtomicThreadFree should be backed by AtomicUsizeField, matching TYPES_SPEC.md"
    );
    assert!(
        types_spec.contains("The thread-free atomic cell is `AtomicThreadFree(AtomicUsizeField)`")
            && types_spec
                .contains("Earlier\nplans used the placeholder name `ThreadFreeCell(*mut usize)`")
            && !types_spec.contains("pub(crate) struct ThreadFreeCell(*mut usize);")
            && types_spec.contains("pub(crate) fn push_remote_acq_rel(")
            && types_spec.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool;"
            ),
        "TYPES_SPEC.md should document AtomicThreadFree as the reconciled thread-free cell"
    );
    assert!(
        types_spec.contains(
            "`AbandonedCountCell` is the staged arena-domain name for the same storage now operated on by `AtomicAbandonedCount(*const AtomicUsize)`"
        ) && types_spec.contains(
            "`ArenaPageCell` is the staged arena-domain name for the same storage now operated on by `AtomicArenaPage(*const AtomicPtr<c_void>)`"
        ) && types_spec.contains(
            "`ArenaPageTablePtr` remains the non-atomic page-table pointer witness"
        ),
        "TYPES_SPEC.md should reconcile staged arena page-table cell names with live atomic wrappers"
    );

    let page_rs = audit_sources.source("src/allocator/page.rs");
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_local(\n    page_wrapper: crate::core_types::LocalPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        ) && free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,"
        ) && page_rs.contains(
            "pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,"
        ),
        "typed free/reclaim entry points should reject raw page ownership states"
    );
    assert!(
        page_rs.contains(
            "pub(crate) unsafe fn try_reclaim_abandoned(\n    page_wrapper: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
            )
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_page_reclaim_raw")
            && page_rs.contains("_mi_theap_page_reclaim(wrapper, theap_wrapper);")
            && page_states_rs.contains("pub(crate) fn reclaim(self, _target: ThreadHeapLocal) -> ActivePage")
            && free_rs.contains("unsafe fn mi_abandoned_page_try_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,")
            && free_rs.contains("_mi_theap_page_reclaim(\n            page,\n            crate::core_types::ThreadHeapLocal::new_unchecked")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: *mut mi_page_t")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    theap: *mut mi_theap_t")
            && !free_rs.contains("unsafe fn mi_abandoned_page_try_reclaim(\n    page: *mut mi_page_t"),
        "reclaim should accept only OwnedAbandonedPage and ThreadHeapLocal below the raw shim"
    );
    assert!(
        validation_types.contains("impl BlockForFree {\n    fn new(block: BlockPtr, was_guarded: WasGuarded) -> Self")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("let block = BlockForFree::new(located.block(), located.was_guarded());")
            && validation_types.contains("Some(ValidatedFree { user, page, block })")
            && free_rs.contains(
                "pub(crate) unsafe fn mi_free_block_local(\n    page_wrapper: crate::core_types::LocalPage,\n    block_wrapper: crate::core_types::BlockForFree,"
            )
            && free_rs.contains("mi_free_block_local(\n                    local_page,\n                    validated.block(),")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new"),
        "mi_free_block_local should accept only LocalPage plus validation-created BlockForFree"
    );
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        )
            && free_rs.contains("mi_free_block_mt(foreign_page, validated.block());")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_rs.contains(
                "unsafe fn mi_free_try_collect_mt_typed(\n    page: crate::core_types::OwnedAbandonedPage,\n    remote_free: crate::core_types::RemoteFreeList,"
            )
            && free_rs.contains(
                "pub(crate) unsafe extern \"C\" fn mi_free_try_collect_mt(\n    page: *mut mi_page_t,\n    mt_free: *mut mi_block_t,"
            )
            && free_rs.contains("crate::core_types::OwnedAbandonedPage::new_unchecked")
            && !free_rs.contains("pub(crate) unsafe fn mi_free_block_mt(\n    page: *mut mi_page_t")
            && !free_rs.contains("unsafe fn mi_free_try_collect_mt_typed(\n    page: *mut mi_page_t"),
        "remote free should enter through ForeignPage or abandoned-page witnesses before touching remote-free internals"
    );
    assert!(
        page_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
        ) && page_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
        ) && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)"),
        "page queue transitions should require typed queue/page witnesses"
    );
    assert!(
        page_rs.contains(
            "unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::FullPageQueue,\n    append: crate::core_types::FullPageQueue,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains("crate::core_types::FullPage::new_unchecked(page_wrapper.page())")
            && page_rs.contains(
                "crate::core_types::QueuedPage::<crate::core_types::FullQueue>::new(full, queue)"
            )
            && page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,\n    to: crate::core_types::NormalPageQueue,"
            )
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: FullPageQueue,"
            )
            && page_queues_rs.contains("crate::page::mi_page_is_in_full(page.page().as_ptr())")
            && page_queues_rs.contains("queue: PageQueue::full(queue)")
            && !page_rs.contains("unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::FullPage"
        ),
        "full-queue operations should accept only FullPage witnesses and FullPageQueue handles"
    );
    assert!(
        page_queues_rs.contains(
            "pub(crate) unsafe fn new(\n        page: crate::core_types::ActivePage,\n        queue: NormalPageQueue,"
        )
            && page_queues_rs.contains("let queue_ptr = queue.ptr().as_ptr();")
            && page_queues_rs.contains("let page_ptr = page.page().as_ptr();")
            && page_queues_rs.contains("&& !crate::page::mi_page_is_in_full(page_ptr)")
            && page_queues_rs.contains(
                ".is_some_and(|block_size| page.page().block_size().get() == block_size.get())"
            )
            && page_queues_rs.contains(
                "crate::page::mi_page_is_huge(page_ptr)\n                        && crate::page::mi_page_queue_is_huge(queue_ptr)"
            )
            && page_queues_rs.contains("queue: PageQueue::normal(queue)")
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_move_to_front_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_append_normal_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    append: crate::core_types::NormalPageQueue,"
            )
            && !page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: NormalPageQueue,"
            )
            && !page_rs.contains("unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::PagePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::PagePtr"),
        "normal queue operations should accept only active non-full pages matching NormalPageQueue or the huge-page sentinel queue"
    );
}
