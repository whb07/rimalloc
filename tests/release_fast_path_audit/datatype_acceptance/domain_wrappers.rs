use super::DatatypeAcceptanceSources;

pub(super) fn assert_domain_wrapper_acceptance(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let core_mod = audit_sources.source("src/types/mod.rs");
    let core_layout_tests = audit_sources.source("src/types/layout_tests.rs");
    let heap_rs = audit_sources.source("src/types/heap.rs");
    let heap_api_rs = audit_sources.source("src/allocator/heap.rs");
    let ptr_rs = audit_sources.source("src/types/ptr.rs");
    let scalar_rs = audit_sources.source("src/types/scalar.rs");
    let unsafe_migration_allowlist = audit_sources.unsafe_migration_allowlist;
    let page_states_rs = audit_sources.source("src/types/page_states.rs");
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
}
