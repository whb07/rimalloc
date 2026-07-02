use super::DatatypeAcceptanceSources;

pub(super) fn assert_strict_type_and_provenance_acceptance(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let unsafe_migration_allowlist = audit_sources.unsafe_migration_allowlist;
    let page_states_rs = audit_sources.source("src/types/page_states.rs");
    let page_queues_rs = audit_sources.source("src/types/page_queues.rs");
    let validation_extra_rs = audit_sources.source("src/types/validation_extra.rs");

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
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/alloc.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/alloc.rs|_mi_page_malloc_zero_typed|padding/provenance|"
            )
            && allowlist_rs
                .contains("pointer-usize-roundtrip|src/types/freelist.rs|*|packed-thread-free|")
            && !allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/free.rs|*|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/free.rs|mi_validate_pointer|validation|"
            )
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
            && !allowlist_rs.contains("metadata-reference|src/bitmap.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/internal/bitmap.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/free.rs|*|")
            && !allowlist_rs.contains("metadata-reference|src/allocator/options.rs|*|")
            && allowlist_rs
                .contains("metadata-reference|src/internal/bitmap.rs|mi_bitmap_setN|bitmap|")
            && allowlist_rs.contains(
                "metadata-reference|src/allocator/free.rs|mi_page_decode_padding|mi_page_t|"
            )
            && allowlist_rs.contains(
                "metadata-reference|src/allocator/options.rs|with_options_table|options|"
            )
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
            && strict_check_rs.contains("assert contexts[8].symbols == (\"generic_function\",)")
            && strict_check_rs
                .contains("assert contexts[13].symbols == (\"AtomicField::load\", \"load\")")
            && strict_check_rs
                .contains("assert contexts[18].symbols == (\"AtomicField::MASK\", \"MASK\")")
            && strict_check_rs.contains("assert \"imported\" in contexts[23].symbols")
            && strict_check_rs.contains("assert contexts[28].symbols == (\"TABLE\",)")
            && strict_check_rs.contains("\"pointer-usize-roundtrip\"")
            && strict_check_rs.contains("\"generic_function\"")
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
        "src/allocator/alloc.rs",
        "src/allocator/free.rs",
        "src/allocator/init.rs",
        "src/allocator/page_map.rs",
        "src/allocator/options.rs",
        "src/bitmap.rs",
        "src/internal/bitmap.rs",
        "src/types/arena.rs",
        "src/types/ptr.rs",
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
        "raw-pointer-parameter|src/allocator/alloc.rs|mi_theap_malloc|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/alloc.rs|mi_heap_malloc|mi_heap_t|",
        "pointer-usize-roundtrip|src/allocator/alloc.rs|_mi_page_malloc_zero_typed|padding/provenance|",
        "bare-domain-usize|src/allocator/alloc.rs|malloc_layout_or_report|usize|",
        "raw-pointer-parameter|src/allocator/free.rs|_mi_free_generic|mi_page_t|",
        "metadata-reference|src/allocator/free.rs|mi_page_decode_padding|mi_page_t|",
        "pointer-usize-roundtrip|src/allocator/free.rs|mi_validate_pointer|validation|",
        "raw-pointer-parameter|src/allocator/init.rs|_mi_thread_done|mi_theap_t|",
        "raw-pointer-parameter|src/allocator/init.rs|_mi_subproc_to_id|mi_subproc_t|",
        "raw-pointer-parameter|src/allocator/init.rs|_mi_is_heap_main|mi_heap_t|",
        "raw-pointer-parameter|src/allocator/page_map.rs|_mi_page_map_register|mi_page_t|",
        "raw-pointer-parameter|src/allocator/page_map.rs|_mi_page_map_unsafe_destroy|mi_subproc_t|",
        "raw-pointer-parameter|src/internal/bitmap.rs|_mi_bitmap_forall_set|mi_arena_t|",
        "metadata-reference|src/internal/bitmap.rs|mi_bitmap_setN|bitmap|",
        "bare-domain-usize|src/internal/bitmap.rs|mi_bitmap_setN|usize|",
        "raw-pointer-parameter|src/types/ptr.rs|new|mi_page_t|",
        "bare-domain-usize|src/types/ptr.rs|checked_byte_add|usize|",
        "metadata-reference|src/allocator/options.rs|with_options_table|options|",
        "bare-domain-usize|src/allocator/options.rs|option_value_load|usize|",
        "bare-domain-usize|src/types/arena.rs|new|usize|",
        "pointer-usize-roundtrip|src/types/arena.rs|SliceRange::from_raw|slice-index|",
    ] {
        assert!(
            allowlist_rs.contains(required),
            "strict-type allowlist should retain symbol-level entry {required}"
        );
    }
}
